use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use crate::template::{
    store::StoreError as TemplateStoreError, store::TemplateStore, validate_stage,
    validate_template, Stage, TemplateValidationError,
};

use super::executor::{run_stage, ExecutorConfig};
use super::project_files::{self, DirListing, ProjectFilesError};
use super::project_manifest::{manifest_run_path, validate_target_dir, write_project_manifest, TargetDirError};
use super::run_record::{
    RunRecord, RunRecordStore, RunRecordStoreError, RunStatus, StageOverrideError, StageStatus,
};
use super::stream_json::StageEvent;

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    TemplateStore(#[from] TemplateStoreError),
    #[error(transparent)]
    RunRecordStore(#[from] RunRecordStoreError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("run '{0}' is not awaiting a checkpoint")]
    NotAwaitingCheckpoint(String),
    #[error("run '{0}' is not awaiting a stage start")]
    NotAwaitingStageStart(String),
    #[error("stage override id '{got}' does not match resolved stage '{expected}'")]
    StageIdMismatch { expected: String, got: String },
    // The literal prefix is the frontend's only way to tell this error apart, because
    // commands.rs collapses every error to a string (M1). src/api.ts matches on it in
    // isStaleStageIndexError(); a test pins it so it cannot be edited away.
    #[error("STALE_STAGE_INDEX: run is at stage {actual}, caller expected {expected}")]
    StaleStageIndex { expected: usize, actual: usize },
    #[error(transparent)]
    StageValidation(#[from] TemplateValidationError),
    #[error("failed to write project manifest: {0}")]
    ProjectManifest(String),
    #[error(transparent)]
    TargetDir(#[from] TargetDirError),
    #[error("target dir already contains a pipeline run: {0}")]
    TargetDirInUse(String),
    #[error("run '{0}' cannot be cancelled in its current state")]
    NotCancellable(String),
    #[error(transparent)]
    ProjectFiles(#[from] ProjectFilesError),
}

impl From<StageOverrideError> for OrchestratorError {
    fn from(e: StageOverrideError) -> Self {
        match e {
            StageOverrideError::StageIdMismatch { expected, got } => {
                OrchestratorError::StageIdMismatch { expected, got }
            }
        }
    }
}

pub struct Orchestrator {
    pub template_store: TemplateStore,
    pub run_store: RunRecordStore,
    pub executor_config: ExecutorConfig,
    /// One async mutex per run, serializing the load-check-save window of every
    /// state-changing entry point (IMP-016). The outer std mutex guards only the map.
    run_locks: StdMutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Orchestrator {
    pub fn new(template_store: TemplateStore, run_store: RunRecordStore, executor_config: ExecutorConfig) -> Self {
        Self { template_store, run_store, executor_config, run_locks: StdMutex::new(HashMap::new()) }
    }

    #[allow(dead_code)]
    fn lock_for(&self, run_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.run_locks.lock().expect("run_locks mutex poisoned");
        locks
            .entry(run_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// The single persistence entry point. Writing the app_data record first keeps
    /// PRD B-2 true (the manifest is additional, never a replacement); writing the
    /// manifest on every transition keeps PRD B-1 true.
    fn save(&self, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.run_store.save(record)?;
        let target = PathBuf::from(&record.target_dir);
        write_project_manifest(&target, record)
            .map_err(|e| OrchestratorError::ProjectManifest(e.to_string()))
    }

    /// IMP-015 (decision D5): if the manifest write fails after the app_data record has
    /// already advanced, put `snapshot` back on disk so the run is left in a state that
    /// still accepts actions (PRD D-3) instead of stranded mid-transition. A failing
    /// rollback is logged and the original error is returned.
    #[allow(dead_code)]
    fn save_with_rollback(&self, record: &RunRecord, snapshot: &RunRecord) -> Result<(), OrchestratorError> {
        match self.save(record) {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Err(rollback_err) = self.run_store.save(snapshot) {
                    eprintln!(
                        "run '{}': manifest write failed and the rollback save also failed: {rollback_err}",
                        record.run_id
                    );
                }
                Err(e)
            }
        }
    }

    /// Creates the run record and its manifest. Spawns nothing — the first claude
    /// process appears only when the user calls `start_stage` (PRD A-1).
    pub fn start_run(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
    ) -> Result<RunRecord, OrchestratorError> {
        let template = self.template_store.load(template_id)?;
        validate_template(&template)?;
        validate_target_dir(&target_dir)?;
        // IMP-017 (decision D4): the manifest path is fixed per folder, so a second run
        // here would overwrite the first run's snapshot. Refuse instead (PRD B-3).
        if manifest_run_path(&target_dir).exists() {
            return Err(OrchestratorError::TargetDirInUse(target_dir.to_string_lossy().to_string()));
        }
        let record = RunRecord::new(
            run_id,
            template.id.clone(),
            target_dir.to_string_lossy().to_string(),
            &template.stages,
        );
        self.save(&record)?;
        Ok(record)
    }

    /// TRD 3.3-(4a) rule (a). A stage continues the previous stage's Claude session; the
    /// first stage starts fresh. At any gate `stages[current_stage_index - 1].session_id`
    /// is already populated — `advance_to_gate` fills it for non-checkpoint stages and
    /// `mark_current_awaiting_checkpoint` fills it for checkpointed ones — so this reads
    /// the session the previous stage really used. The value is passed through verbatim,
    /// including the empty string the old `unwrap_or_default()` could produce; that is
    /// existing behaviour and normalizing it is out of scope.
    fn resume_session_id_for(record: &RunRecord) -> Option<String> {
        let idx = record.current_stage_index;
        if idx == 0 {
            None
        } else {
            record.stages[idx - 1].session_id.clone()
        }
    }

    /// Executes exactly one stage — the one `record.current_resolved_stage()` names — and
    /// returns `(exit_code, latest_session_id)`. It makes no state decisions and contains
    /// no loop: `start_stage` and `request_changes` own every transition (M2).
    async fn run_current_stage<F: FnMut(&str, StageEvent)>(
        &self,
        record: &mut RunRecord,
        resume_session_id: Option<String>,
        on_event: &mut F,
    ) -> Result<(i32, Option<String>), OrchestratorError> {
        let stage_index = record.current_stage_index;
        // Cloned because `record` is mutably borrowed for the log append below.
        let stage = record.current_resolved_stage().clone();
        let stage_id = stage.id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let exit_code = run_stage(
            &self.executor_config,
            &stage,
            &target_dir,
            resume_session_id.as_deref(),
            |event| {
                if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                    latest_session_id = Some(session_id.clone());
                }
                collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
                on_event(&stage_id, event);
            },
        )
        .await?;

        record.stages[stage_index].log.extend(collected_log);
        Ok((exit_code, latest_session_id))
    }

    /// The only path in the app that spawns a claude process (PRD A-4).
    ///
    /// `expected_stage_index` is the index the caller believes the run is at. It is a
    /// required argument, not an optional nicety: `stage_override` is `Option`, so the
    /// id check inside `apply_stage_override` cannot be relied on to catch a stale call
    /// (TRD 3.8).
    ///
    /// Lock scope is steps 1-5 only. The child process (step 6) and the post-run
    /// transition (steps 7-8) run outside it, so a multi-minute stage never blocks this
    /// run's own `cancel_run` (TRD 3.3-(3), 3.8).
    pub async fn start_stage<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        expected_stage_index: usize,
        stage_override: Option<Stage>,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let lock = self.lock_for(run_id);
        let mut record = {
            let _guard = lock.lock().await;

            // 1. load
            let mut record = self.run_store.load(run_id)?;

            // 2. guard (i) — status. Also what rejects the loser of a concurrent race,
            //    because the winner has already persisted Running by the time the lock
            //    is handed over (T-D4a).
            if record.status != RunStatus::AwaitingStageStart {
                return Err(OrchestratorError::NotAwaitingStageStart(run_id.to_string()));
            }

            // 2b. guard (ii) — generation. Rejects a call that was composed against an
            //     earlier gate and arrived after the run moved on (T-D4b). A rejection
            //     is not a transition: nothing on disk changes.
            if record.current_stage_index != expected_stage_index {
                return Err(OrchestratorError::StaleStageIndex {
                    expected: expected_stage_index,
                    actual: record.current_stage_index,
                });
            }

            // 3. apply the user's pre-start edits to this run only (IMP-006).
            if let Some(override_stage) = stage_override {
                validate_stage(&override_stage)?;
                record.apply_stage_override(override_stage)?;
            }

            // 4-5. persist Running before spawning (0235a40 precedent), rolling back if
            //      the manifest write fails (IMP-015).
            let snapshot = record.clone();
            record.begin_current_stage();
            self.save_with_rollback(&record, &snapshot)?;

            record
        }; // 5. lock released here — before the spawn.

        // 6. run the stage, outside the lock.
        let resume = Self::resume_session_id_for(&record);
        let outcome = self.run_current_stage(&mut record, resume, &mut on_event).await;

        // 7. absorb the outcome. An Err is never propagated with `?` — that is exactly
        //    how 08e9144 stranded a run at status=running.
        match outcome {
            Ok((0, session_id)) => {
                let session_id = session_id.unwrap_or_default();
                if record.current_resolved_stage().checkpoint {
                    record.mark_current_awaiting_checkpoint(session_id); // T3
                } else {
                    record.advance_to_gate(session_id); // T4 / T5
                }
            }
            _ => record.mark_current_failed(), // T6
        }

        // 8. persist the result.
        self.save(&record)?;
        Ok(record)
    }

    /// Approving a checkpoint no longer starts the next stage. It advances to the next
    /// stage's gate and returns (PRD A-3), so this method is now purely synchronous
    /// work. `on_event` is kept in the signature — and never called — so the Tauri
    /// command and its callers do not change shape.
    pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        _on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let lock = self.lock_for(run_id);
        let _guard = lock.lock().await;

        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        // No template_store.load here any more: the run's own resolved_stages are the
        // source of truth, which is half of what closes the C-4 leak (IMP-007).
        let snapshot = record.clone();
        record.approve_current();
        self.save_with_rollback(&record, &snapshot)?;
        Ok(record)
    }

    /// Re-runs the current stage with the user's feedback as its prompt.
    ///
    /// The stage definition comes from `resolved_stages`, so pre-start edits to
    /// permission mode / allowed tools / checkpoint are inherited (PRD C-3) and a
    /// mid-run template edit cannot leak in (PRD C-4). The prompt override is applied to
    /// a local clone only and is deliberately *not* written back to `resolved_stages` —
    /// feedback is one-shot (design decision, and IMP-020 decision D7 keeps it unlogged).
    pub async fn request_changes<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        feedback: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let lock = self.lock_for(run_id);
        let mut record = {
            let _guard = lock.lock().await;

            let mut record = self.run_store.load(run_id)?;
            if record.status != RunStatus::AwaitingCheckpoint {
                return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
            }
            // Persist Running before spawning (0235a40 precedent).
            let snapshot = record.clone();
            record.status = RunStatus::Running;
            record.current_stage_mut().status = StageStatus::Running;
            self.save_with_rollback(&record, &snapshot)?;
            record
        }; // lock released before the spawn, same scope rule as start_stage.

        // TRD 3.3-(4a) rule (b): a re-run continues its OWN stage's session.
        let resume_session_id = record.stages[record.current_stage_index].session_id.clone();

        // Swap in the feedback prompt on a throwaway clone of the resolved stage.
        let stage_index = record.current_stage_index;
        let mut feedback_stage = record.current_resolved_stage().clone();
        feedback_stage.prompt = feedback.to_string();
        let stage_id = feedback_stage.id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let result = run_stage(
            &self.executor_config,
            &feedback_stage,
            &target_dir,
            resume_session_id.as_deref(),
            |event| {
                if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                    latest_session_id = Some(session_id.clone());
                }
                collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
                on_event(&stage_id, event);
            },
        )
        .await;

        record.stages[stage_index].log.extend(collected_log);

        match result {
            Ok(0) => record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default()),
            _ => record.mark_current_failed(),
        }
        self.save(&record)?;
        Ok(record)
    }

    /// IMP-014 / PRD D-1, D-2. A run parked at a pre-stage gate previously had exactly
    /// one available action — "run it" — which is the shape of the 08e9144 stuck-state
    /// bug. This widens the escape hatch to both waiting states.
    ///
    /// `Failed` is deliberately absent: decision D1 keeps it terminal, so a failed run is
    /// already in a final state and does not need cancelling.
    ///
    /// Takes no lock: it is synchronous, and a `tokio::sync::Mutex` cannot be acquired
    /// from a non-async function. Its load-check-save window is short and its only
    /// interleaving risk is with a running stage, which by design releases the lock
    /// before spawning anyway — so a lock here would not add protection.
    pub fn cancel_run(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if !matches!(record.status, RunStatus::AwaitingCheckpoint | RunStatus::AwaitingStageStart) {
            return Err(OrchestratorError::NotCancellable(run_id.to_string()));
        }
        record.cancel();
        self.save(&record)?;
        Ok(record)
    }

    pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        record.cancel();
        self.save(&record)?;
        Ok(record)
    }

    /// Read-only, exactly like `get_run` (commands.rs:118-123): it loads the record,
    /// reads the filesystem, and returns. No `lock_for`, no `save`, no transition
    /// (IMP-027).
    pub async fn list_project_dir(
        &self,
        run_id: &str,
        sub_path: &str,
    ) -> Result<DirListing, OrchestratorError> {
        let record = self.run_store.load(run_id)?;
        project_files::list_dir(Path::new(&record.target_dir), sub_path).await.map_err(Into::into)
    }
}
