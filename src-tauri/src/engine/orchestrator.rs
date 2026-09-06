use std::path::{Path, PathBuf};

use crate::template::{store::StoreError as TemplateStoreError, store::TemplateStore, Template};

use super::executor::{run_stage, ExecutorConfig};
use super::run_record::{RunRecord, RunRecordStore, RunRecordStoreError, RunStatus, StageStatus};
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

    pub async fn start_run<F: FnMut(&str, StageEvent)>(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let template = self.template_store.load(template_id)?;
        let stage_ids: Vec<String> = template.stages.iter().map(|s| s.id.clone()).collect();
        let mut record = RunRecord::new(run_id, template.id.clone(), target_dir.to_string_lossy().to_string(), &stage_ids);
        std::fs::create_dir_all(&target_dir)?;
        // v1 targets newly created folders only; a non-empty target_dir warning would need
        // frontend UI beyond this fix wave's scope (see final-review.md #15), so it's not enforced here.
        self.drive(&template, &mut record, &target_dir, None, &mut on_event).await?;
        self.run_store.save(&record)?;
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
            self.drive(&template, &mut record, &target_dir, resume_session_id, &mut on_event).await?;
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

        let exit_code = run_stage(&self.executor_config, &feedback_stage, &target_dir, resume_session_id.as_deref(), |event| {
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
        } else {
            record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default());
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
