use std::path::{Path, PathBuf};

use crate::template::{
    store::StoreError as TemplateStoreError, store::TemplateStore, validate_stage, Stage, Template,
    TemplateValidationError,
};

use super::executor::{run_stage, ExecutorConfig};
use super::project_manifest::{write_project_manifest, ProjectManifestError};
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
    #[error(transparent)]
    ProjectManifest(#[from] ProjectManifestError),
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

    fn save(&self, template: &Template, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.run_store.save(record)?;
        write_project_manifest(
            Path::new(&record.target_dir),
            &template.id,
            &template.name,
            &template.description,
            record,
        )?;
        Ok(())
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
        self.save(&template, &record)?;

        let stage_index = record.current_stage_index;
        let stage = record.current_resolved_stage().clone();
        let stage_id = stage.id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());
        let resume_session_id = if stage_index == 0 {
            None
        } else {
            record.stages[stage_index - 1].session_id.clone()
        };

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let result = run_stage(&self.executor_config, &stage, &target_dir, resume_session_id.as_deref(), |event| {
            if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                latest_session_id = Some(session_id.clone());
            }
            collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
            on_event(&stage_id, event);
        })
        .await;

        record.stages[stage_index].log.extend(collected_log);

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
        let stage_id = feedback_stage.id.clone();

        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        record.status = RunStatus::Running;
        record.current_stage_mut().status = StageStatus::Running;
        self.save(&template, &record)?;

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
