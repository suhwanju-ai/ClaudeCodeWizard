use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::template::is_valid_id;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Running,
    AwaitingCheckpoint,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StageStatus {
    Pending,
    Running,
    AwaitingCheckpoint,
    Approved,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageRun {
    pub id: String,
    pub status: StageStatus,
    pub session_id: Option<String>,
    pub log: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    pub template_id: String,
    pub target_dir: String,
    pub status: RunStatus,
    pub current_stage_index: usize,
    pub stages: Vec<StageRun>,
}

impl RunRecord {
    pub fn new(run_id: String, template_id: String, target_dir: String, stage_ids: &[String]) -> Self {
        Self {
            run_id,
            template_id,
            target_dir,
            status: RunStatus::Running,
            current_stage_index: 0,
            stages: stage_ids
                .iter()
                .map(|id| StageRun {
                    id: id.clone(),
                    status: StageStatus::Pending,
                    session_id: None,
                    log: Vec::new(),
                })
                .collect(),
        }
    }

    pub fn current_stage_mut(&mut self) -> &mut StageRun {
        &mut self.stages[self.current_stage_index]
    }

    pub fn mark_current_awaiting_checkpoint(&mut self, session_id: String) {
        self.current_stage_mut().status = StageStatus::AwaitingCheckpoint;
        self.current_stage_mut().session_id = Some(session_id);
        self.status = RunStatus::AwaitingCheckpoint;
    }

    pub fn mark_current_failed(&mut self) {
        self.current_stage_mut().status = StageStatus::Failed;
        self.status = RunStatus::Failed;
    }

    /// Marks the current stage approved and advances to the next stage.
    /// Returns `true` if this was the last stage (run is now complete).
    pub fn approve_current(&mut self) -> bool {
        self.current_stage_mut().status = StageStatus::Approved;
        if self.current_stage_index + 1 < self.stages.len() {
            self.current_stage_index += 1;
            self.status = RunStatus::Running;
            false
        } else {
            self.status = RunStatus::Completed;
            true
        }
    }

    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunRecordStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("run '{0}' not found")]
    NotFound(String),
    #[error("invalid id '{0}': ids must be non-empty, contain only letters, digits, '.', '_', or '-', and not be '.' or '..'")]
    InvalidId(String),
}

pub struct RunRecordStore {
    dir: PathBuf,
}

impl RunRecordStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, run_id: &str) -> PathBuf {
        self.dir.join(format!("{run_id}.json"))
    }

    pub fn save(&self, record: &RunRecord) -> Result<(), RunRecordStoreError> {
        if !is_valid_id(&record.run_id) {
            return Err(RunRecordStoreError::InvalidId(record.run_id.clone()));
        }
        fs::create_dir_all(&self.dir)?;
        let content = serde_json::to_string_pretty(record)?;
        fs::write(self.path_for(&record.run_id), content)?;
        Ok(())
    }

    pub fn load(&self, run_id: &str) -> Result<RunRecord, RunRecordStoreError> {
        if !is_valid_id(run_id) {
            return Err(RunRecordStoreError::InvalidId(run_id.to_string()));
        }
        let path = self.path_for(run_id);
        if !path.exists() {
            return Err(RunRecordStoreError::NotFound(run_id.to_string()));
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(stage_ids: &[&str]) -> RunRecord {
        let ids: Vec<String> = stage_ids.iter().map(|s| s.to_string()).collect();
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &ids)
    }

    #[test]
    fn new_starts_at_stage_zero_with_pending_stages() {
        let r = record(&["a", "b"]);
        assert_eq!(r.current_stage_index, 0);
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages.len(), 2);
        assert!(r.stages.iter().all(|s| s.status == StageStatus::Pending));
    }

    #[test]
    fn mark_current_awaiting_checkpoint_updates_stage_and_run() {
        let mut r = record(&["a", "b"]);
        r.mark_current_awaiting_checkpoint("sess1".to_string());
        assert_eq!(r.status, RunStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].session_id, Some("sess1".to_string()));
    }

    #[test]
    fn mark_current_failed_updates_stage_and_run() {
        let mut r = record(&["a"]);
        r.mark_current_failed();
        assert_eq!(r.status, RunStatus::Failed);
        assert_eq!(r.stages[0].status, StageStatus::Failed);
    }

    #[test]
    fn approve_current_advances_to_next_stage() {
        let mut r = record(&["a", "b"]);
        let completed = r.approve_current();
        assert!(!completed);
        assert_eq!(r.current_stage_index, 1);
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
    }

    #[test]
    fn approve_current_on_last_stage_completes_run() {
        let mut r = record(&["a"]);
        let completed = r.approve_current();
        assert!(completed);
        assert_eq!(r.status, RunStatus::Completed);
    }

    #[test]
    fn cancel_sets_cancelled_status() {
        let mut r = record(&["a"]);
        r.cancel();
        assert_eq!(r.status, RunStatus::Cancelled);
    }

    #[test]
    fn store_save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let r = record(&["a"]);
        store.save(&r).unwrap();
        assert_eq!(store.load("run1").unwrap(), r);
    }

    #[test]
    fn store_load_missing_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        assert!(matches!(store.load("missing"), Err(RunRecordStoreError::NotFound(_))));
    }

    #[test]
    fn store_load_rejects_id_with_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        assert!(matches!(store.load("../../../etc/passwd"), Err(RunRecordStoreError::InvalidId(_))));
    }

    #[test]
    fn store_load_rejects_id_with_slash() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        assert!(matches!(store.load("sub/dir"), Err(RunRecordStoreError::InvalidId(_))));
    }

    #[test]
    fn store_save_rejects_id_with_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let mut r = record(&["a"]);
        r.run_id = "../../../etc/passwd".to_string();
        assert!(matches!(store.save(&r), Err(RunRecordStoreError::InvalidId(_))));
    }
}
