use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use crate::template::{
    store::StoreError as TemplateStoreError, store::TemplateStore, validate_template, Template,
    TemplateValidationError,
};

use super::executor::{run_stage, ExecutorConfig};
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

    /// Runs stages starting at `record.current_stage_index`, auto-advancing
    /// through any stage with `checkpoint == false`, and stops at the first
    /// checkpointed stage, a failure, or the end of the pipeline.
    async fn drive<F: FnMut(&str, StageEvent)>(
        &self,
        template: &Template,
        record: &mut RunRecord,
        target_dir: &Path,
        mut resume_session_id: Option<String>,
        on_event: &mut F,
    ) -> Result<(), OrchestratorError> {
        loop {
            let stage_index = record.current_stage_index;
            let stage = &template.stages[stage_index];
            let stage_id = stage.id.clone();
            let mut latest_session_id = resume_session_id.clone();
            let mut collected_log = Vec::new();

            let exit_code = run_stage(&self.executor_config, stage, target_dir, resume_session_id.as_deref(), |event| {
                if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                    latest_session_id = Some(session_id.clone());
                }
                collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
                on_event(&stage_id, event);
            })
            .await?;

            record.stages[stage_index].log.extend(collected_log);

            if exit_code != 0 {
                record.mark_current_failed();
                return Ok(());
            }

            let session_id = latest_session_id.unwrap_or_default();
            if stage.checkpoint {
                record.mark_current_awaiting_checkpoint(session_id);
                return Ok(());
            }

            record.current_stage_mut().status = StageStatus::Approved;
            record.current_stage_mut().session_id = Some(session_id.clone());
            if record.current_stage_index + 1 >= record.stages.len() {
                record.status = RunStatus::Completed;
                return Ok(());
            }
            record.current_stage_index += 1;
            record.status = RunStatus::Running;
            resume_session_id = Some(session_id);
        }
    }

    pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());
        let completed = record.approve_current();
        if !completed {
            record.current_stage_mut().status = StageStatus::Running;
            self.run_store.save(&record)?;
            if self.drive(&template, &mut record, &target_dir, resume_session_id, &mut on_event).await.is_err() {
                record.mark_current_failed();
            }
        }
        self.run_store.save(&record)?;
        Ok(record)
    }

    pub async fn request_changes<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        feedback: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        let stage_index = record.current_stage_index;
        let mut feedback_stage = template.stages[stage_index].clone();
        feedback_stage.prompt = feedback.to_string();
        let stage_id = feedback_stage.id.clone();

        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        record.status = RunStatus::Running;
        record.current_stage_mut().status = StageStatus::Running;
        self.run_store.save(&record)?;

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let result = run_stage(&self.executor_config, &feedback_stage, &target_dir, resume_session_id.as_deref(), |event| {
            if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                latest_session_id = Some(session_id.clone());
            }
            collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
            on_event(&stage_id, event);
        })
        .await;

        record.stages[stage_index].log.extend(collected_log);

        match result {
            Ok(0) => record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default()),
            _ => record.mark_current_failed(),
        }
        self.run_store.save(&record)?;
        Ok(record)
    }

    pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        record.cancel();
        self.run_store.save(&record)?;
        Ok(record)
    }
}
