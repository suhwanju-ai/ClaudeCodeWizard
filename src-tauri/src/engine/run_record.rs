use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::template::{is_valid_id, Stage};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    AwaitingStageStart,
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
    AwaitingStart,
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
    pub resolved_stages: Vec<Stage>,
}

impl RunRecord {
    pub fn new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self {
        let stage_runs = stages
            .iter()
            .enumerate()
            .map(|(i, s)| StageRun {
                id: s.id.clone(),
                status: if i == 0 { StageStatus::AwaitingStart } else { StageStatus::Pending },
                session_id: None,
                log: Vec::new(),
            })
            .collect();
        Self {
            run_id,
            template_id,
            target_dir,
            status: RunStatus::AwaitingStageStart,
            current_stage_index: 0,
            stages: stage_runs,
            resolved_stages: stages.to_vec(),
        }
    }

    pub fn current_stage_mut(&mut self) -> &mut StageRun {
        &mut self.stages[self.current_stage_index]
    }

    pub fn current_resolved_stage(&self) -> &Stage {
        &self.resolved_stages[self.current_stage_index]
    }

    pub fn apply_stage_override(&mut self, stage: Stage) {
        self.resolved_stages[self.current_stage_index] = stage;
    }

    pub fn begin_current_stage(&mut self) {
        self.current_stage_mut().status = StageStatus::Running;
        self.status = RunStatus::Running;
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

    /// Marks the current stage approved and advances to the next stage,
    /// which becomes `AwaitingStart` (paused for the pre-stage edit gate).
    /// Returns `true` if this was the last stage (run is now complete).
    pub fn approve_current(&mut self) -> bool {
        self.current_stage_mut().status = StageStatus::Approved;
        if self.current_stage_index + 1 < self.stages.len() {
            self.current_stage_index += 1;
            self.current_stage_mut().status = StageStatus::AwaitingStart;
            self.status = RunStatus::AwaitingStageStart;
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
    #[error("run '{0}' is corrupted: current_stage_index is out of range or stages/resolved_stages lengths differ")]
    Corrupted(String),
}

fn validate_invariants(record: &RunRecord) -> Result<(), RunRecordStoreError> {
    if record.current_stage_index >= record.stages.len() || record.stages.len() != record.resolved_stages.len() {
        return Err(RunRecordStoreError::Corrupted(record.run_id.clone()));
    }
    Ok(())
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
        validate_invariants(record)?;
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
        let record: RunRecord = serde_json::from_str(&content)?;
        validate_invariants(&record)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::PermissionMode;

    fn stage(id: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: id.to_string(),
            prompt: "do it".to_string(),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec![],
            checkpoint: true,
        }
    }

    fn record(ids: &[&str]) -> RunRecord {
        let stages: Vec<Stage> = ids.iter().map(|id| stage(id)).collect();
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &stages)
    }

    #[test]
    fn new_starts_at_stage_zero_awaiting_start() {
        let r = record(&["a", "b"]);
        assert_eq!(r.current_stage_index, 0);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingStart);
        assert_eq!(r.stages[1].status, StageStatus::Pending);
        assert_eq!(r.resolved_stages.len(), 2);
        assert_eq!(r.resolved_stages[0].id, "a");
    }

    #[test]
    fn begin_current_stage_marks_running() {
        let mut r = record(&["a"]);
        r.begin_current_stage();
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages[0].status, StageStatus::Running);
    }

    #[test]
    fn apply_stage_override_replaces_resolved_stage() {
        let mut r = record(&["a", "b"]);
        let mut edited = r.resolved_stages[0].clone();
        edited.prompt = "edited prompt".to_string();
        r.apply_stage_override(edited);
        assert_eq!(r.resolved_stages[0].prompt, "edited prompt");
        assert_eq!(r.resolved_stages[1].prompt, "do it");
    }

    #[test]
    fn mark_current_awaiting_checkpoint_updates_stage_and_run() {
        let mut r = record(&["a", "b"]);
        r.begin_current_stage();
        r.mark_current_awaiting_checkpoint("sess1".to_string());
        assert_eq!(r.status, RunStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].session_id, Some("sess1".to_string()));
    }

    #[test]
    fn mark_current_failed_updates_stage_and_run() {
        let mut r = record(&["a"]);
        r.begin_current_stage();
        r.mark_current_failed();
        assert_eq!(r.status, RunStatus::Failed);
        assert_eq!(r.stages[0].status, StageStatus::Failed);
    }

    #[test]
    fn approve_current_advances_to_awaiting_start() {
        let mut r = record(&["a", "b"]);
        let completed = r.approve_current();
        assert!(!completed);
        assert_eq!(r.current_stage_index, 1);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
        assert_eq!(r.stages[1].status, StageStatus::AwaitingStart);
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

    #[test]
    fn save_rejects_current_stage_index_out_of_range() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let mut r = record(&["a"]);
        r.current_stage_index = 5;
        assert!(matches!(store.save(&r), Err(RunRecordStoreError::Corrupted(_))));
    }

    #[test]
    fn save_rejects_resolved_stages_length_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let mut r = record(&["a", "b"]);
        r.resolved_stages.pop();
        assert!(matches!(store.save(&r), Err(RunRecordStoreError::Corrupted(_))));
    }

    #[test]
    fn load_rejects_a_corrupted_record_found_on_disk() {
        // A record can only become corrupted by something outside save()'s own
        // validation -- e.g. hand-editing or disk corruption -- so this test
        // writes the bad JSON directly rather than going through `save`.
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let mut r = record(&["a"]);
        r.current_stage_index = 5;
        fs::write(dir.path().join("run1.json"), serde_json::to_string(&r).unwrap()).unwrap();
        assert!(matches!(store.load("run1"), Err(RunRecordStoreError::Corrupted(_))));
    }
}
