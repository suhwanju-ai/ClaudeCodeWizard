use std::path::{Path, PathBuf};

use crate::template::{
    store::StoreError as TemplateStoreError, store::TemplateStore, validate_stage, Stage, Template,
    TemplateValidationError,
};

use super::executor::{run_stage, ExecutorConfig};
use super::project_manifest::write_project_manifest;
use super::run_record::{RunRecord, RunRecordStore, RunRecordStoreError, RunStatus, StageStatus};
use super::stream_json::StageEvent;

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    TemplateStore(#[from] TemplateStoreError),
    #[error(transparent)]
    RunRecordStore(#[from] RunRecordStoreError),
    #[error(transparent)]
    StageValidation(#[from] TemplateValidationError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("run '{0}' is not awaiting a checkpoint")]
    NotAwaitingCheckpoint(String),
    #[error("run '{0}' is not awaiting a stage start")]
    NotAwaitingStageStart(String),
    #[error("stage override id '{0}' does not match the current stage")]
    StageIdMismatch(String),
}

pub struct Orchestrator {
    pub template_store: TemplateStore,
    pub run_store: RunRecordStore,
    pub executor_config: ExecutorConfig,
}

impl Orchestrator {
    pub fn new(template_store: TemplateStore, run_store: RunRecordStore, executor_config: ExecutorConfig) -> Self {
        Self { template_store, run_store, executor_config }
    }

    /// Persists `record` to the authoritative app-data-dir store, then makes a
    /// best-effort attempt to mirror it into the target project's own
    /// `.claude-pipeline-wizard/` folder for inspection.
    ///
    /// The project-local manifest write is deliberately non-fatal: `run_store`
    /// is the source of truth for the state machine, and every caller here
    /// gates entry on `record.status` as loaded from `run_store`. If the
    /// project-local write failed silently but `save()` still returned `Err`,
    /// the in-memory/`run_store` state would already have advanced (e.g. to
    /// `Running`) past what any entry gate accepts, permanently stranding the
    /// run with no in-app recovery. So a manifest-write failure is logged and
    /// swallowed here; only a `run_store.save` failure is fatal.
    fn save(&self, template: &Template, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.save_state_only(record)?;
        if let Err(e) = write_project_manifest(
            Path::new(&record.target_dir),
            &template.id,
            &template.name,
            &template.description,
            record,
        ) {
            eprintln!("failed to write project manifest for run '{}': {e}", record.run_id);
        }
        Ok(())
    }

    /// Persists only to the authoritative `run_store`, skipping the
    /// project-local manifest mirror. Used for the interim "stage began
    /// executing" checkpoint inside `start_stage`: writing both manifest
    /// files on top of `run_store` at both the start and end of every stage
    /// run isn't worth the extra I/O, since the manifest catches up with
    /// the accurate final state at the closing `save()` call regardless.
    fn save_state_only(&self, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.run_store.save(record)?;
        Ok(())
    }

    /// Runs `stage` (resuming from `resume_session_id` if given), streaming
    /// events to `on_event`, and appends the collected raw event log onto
    /// `record.stages[stage_index]`. Returns `run_stage`'s raw result and the
    /// last-seen session id — callers decide how a non-zero exit or timeout
    /// maps onto record state, since that differs between `start_stage`
    /// (checkpoint-or-auto-advance) and `request_changes` (always re-pauses
    /// at the checkpoint).
    async fn run_and_record_stage<F: FnMut(&str, StageEvent)>(
        &self,
        record: &mut RunRecord,
        stage_index: usize,
        stage: &Stage,
        target_dir: &Path,
        resume_session_id: Option<&str>,
        mut on_event: F,
    ) -> (std::io::Result<i32>, Option<String>) {
        let stage_id = stage.id.clone();
        let mut latest_session_id = resume_session_id.map(|s| s.to_string());
        let mut collected_log = Vec::new();

        let result = run_stage(&self.executor_config, stage, target_dir, resume_session_id, |event| {
            if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                latest_session_id = Some(session_id.clone());
            }
            collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
            on_event(&stage_id, event);
        })
        .await;

        record.stages[stage_index].log.extend(collected_log);
        (result, latest_session_id)
    }

    pub fn start_run(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
    ) -> Result<RunRecord, OrchestratorError> {
        let template = self.template_store.load(template_id)?;
        std::fs::create_dir_all(&target_dir)?;
        let record = RunRecord::new(
            run_id,
            template.id.clone(),
            target_dir.to_string_lossy().to_string(),
            &template.stages,
        );
        self.save(&template, &record)?;
        Ok(record)
    }

    pub async fn start_stage<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        stage_override: Option<Stage>,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingStageStart {
            return Err(OrchestratorError::NotAwaitingStageStart(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;

        if let Some(stage) = stage_override {
            if stage.id != record.current_resolved_stage().id {
                return Err(OrchestratorError::StageIdMismatch(stage.id));
            }
            validate_stage(&stage)?;
            record.apply_stage_override(stage);
        }

        record.begin_current_stage();
        // Interim checkpoint only — see save_state_only's doc comment for why
        // the project manifest isn't also rewritten here.
        self.save_state_only(&record)?;

        let stage_index = record.current_stage_index;
        let stage = record.current_resolved_stage().clone();
        let target_dir = PathBuf::from(record.target_dir.clone());
        let resume_session_id = if stage_index == 0 {
            None
        } else {
            record.stages[stage_index - 1].session_id.clone()
        };

        let (result, latest_session_id) = self
            .run_and_record_stage(&mut record, stage_index, &stage, &target_dir, resume_session_id.as_deref(), &mut on_event)
            .await;

        // `_` also catches a timed-out/IO-failed `run_stage` (an `Err`), not just a
        // non-zero exit code — mirrors `request_changes`'s existing timeout handling
        // below, so a hung stage is marked `Failed` and persisted instead of the
        // error propagating raw and leaving the record stuck at `Running`.
        match result {
            Ok(0) => {
                let session_id = latest_session_id.unwrap_or_default();
                if stage.checkpoint {
                    record.mark_current_awaiting_checkpoint(session_id);
                } else {
                    record.current_stage_mut().session_id = Some(session_id);
                    record.approve_current();
                }
            }
            _ => record.mark_current_failed(),
        }

        self.save(&template, &record)?;
        Ok(record)
    }

    pub fn approve_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        record.approve_current();
        self.save(&template, &record)?;
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
        let mut feedback_stage = record.resolved_stages[stage_index].clone();
        feedback_stage.prompt = feedback.to_string();

        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        record.status = RunStatus::Running;
        record.current_stage_mut().status = StageStatus::Running;
        // Interim checkpoint only — see save_state_only's doc comment for why
        // the project manifest isn't also rewritten here.
        self.save_state_only(&record)?;

        let (result, latest_session_id) = self
            .run_and_record_stage(&mut record, stage_index, &feedback_stage, &target_dir, resume_session_id.as_deref(), &mut on_event)
            .await;

        match result {
            Ok(0) => record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default()),
            _ => record.mark_current_failed(),
        }
        self.save(&template, &record)?;
        Ok(record)
    }

    pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        record.cancel();
        self.save(&template, &record)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::PermissionMode;

    fn sample_template() -> Template {
        Template {
            id: "t1".to_string(),
            name: "T".to_string(),
            description: "d".to_string(),
            stages: vec![Stage {
                id: "s1".to_string(),
                name: "S1".to_string(),
                prompt: "do it".to_string(),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            }],
        }
    }

    #[test]
    fn save_state_only_skips_the_project_manifest_write() {
        let template_dir = tempfile::tempdir().unwrap();
        let run_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let template_store = TemplateStore::new(template_dir.path());
        let template = sample_template();
        template_store.save(&template).unwrap();
        let orchestrator =
            Orchestrator::new(template_store, RunRecordStore::new(run_dir.path()), ExecutorConfig::default());

        let record = RunRecord::new(
            "run1".to_string(),
            template.id.clone(),
            target_dir.path().to_string_lossy().to_string(),
            &template.stages,
        );

        let manifest_dir = target_dir.path().join(".claude-pipeline-wizard");

        orchestrator.save_state_only(&record).unwrap();
        assert!(orchestrator.run_store.load("run1").is_ok(), "run_store must still be authoritative");
        assert!(!manifest_dir.exists(), "save_state_only must not touch the project-local manifest");

        orchestrator.save(&template, &record).unwrap();
        assert!(manifest_dir.join("run.json").exists(), "the full save() must still write the manifest");
    }
}
