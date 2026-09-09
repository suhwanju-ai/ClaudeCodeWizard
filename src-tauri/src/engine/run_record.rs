use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::template::{is_valid_id, Stage};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Running,
    /// Parked at a pre-stage edit gate: nothing is executing and the user may
    /// review/edit the current stage before starting it.
    AwaitingStageStart,
    AwaitingCheckpoint,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StageStatus {
    /// Not reached yet. Meaning unchanged.
    Pending,
    /// Reached, sitting at its pre-start gate.
    AwaitingStart,
    Running,
    AwaitingCheckpoint,
    Approved,
    Failed,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StageOverrideError {
    #[error("stage override id '{got}' does not match resolved stage '{expected}'")]
    StageIdMismatch { expected: String, got: String },
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
    /// The run's own copy of the pipeline. Pre-start edits mutate this and never the
    /// saved template. Required — there is deliberately no `#[serde(default)]`; legacy
    /// records are quarantined at startup instead (IMP-018 decision B).
    pub resolved_stages: Vec<Stage>,
}

impl RunRecord {
    pub fn new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self {
        Self {
            run_id,
            template_id,
            target_dir,
            status: RunStatus::AwaitingStageStart,
            current_stage_index: 0,
            stages: stages
                .iter()
                .enumerate()
                .map(|(i, stage)| StageRun {
                    id: stage.id.clone(),
                    status: if i == 0 { StageStatus::AwaitingStart } else { StageStatus::Pending },
                    session_id: None,
                    log: Vec::new(),
                })
                .collect(),
            resolved_stages: stages.to_vec(),
        }
    }

    pub fn current_stage_mut(&mut self) -> &mut StageRun {
        &mut self.stages[self.current_stage_index]
    }

    /// The stage definition this run will actually execute next.
    pub fn current_resolved_stage(&self) -> &Stage {
        &self.resolved_stages[self.current_stage_index]
    }

    /// Replaces the current stage's definition with a user-edited one. The id must match:
    /// changing it would desynchronize `StageRun.id` from `resolved_stages`.
    pub fn apply_stage_override(&mut self, stage: Stage) -> Result<(), StageOverrideError> {
        let expected = &self.resolved_stages[self.current_stage_index].id;
        if &stage.id != expected {
            return Err(StageOverrideError::StageIdMismatch {
                expected: expected.clone(),
                got: stage.id,
            });
        }
        self.resolved_stages[self.current_stage_index] = stage;
        Ok(())
    }

    /// Leaves the gate: the current stage and the run both go to `Running`.
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

    /// Records the session a finished non-checkpoint stage used, then returns to the
    /// next stage's gate. Returns `true` if that was the last stage.
    pub fn advance_to_gate(&mut self, session_id: String) -> bool {
        self.current_stage_mut().session_id = Some(session_id);
        self.approve_current()
    }

    /// Marks the current stage approved and advances to the next stage's *gate*
    /// (not to `Running` — PRD A-3). Returns `true` if the run is now complete.
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
    use crate::template::{PermissionMode, Stage};

    fn stage(id: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: format!("Stage {id}"),
            prompt: format!("prompt for {id}"),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec!["Read".to_string()],
            checkpoint: true,
        }
    }

    fn record(stage_ids: &[&str]) -> RunRecord {
        let stages: Vec<Stage> = stage_ids.iter().map(|id| stage(id)).collect();
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &stages)
    }

    // U-1
    #[test]
    fn new_starts_awaiting_stage_start_with_stage0_awaiting() {
        let r = record(&["a", "b"]);
        assert_eq!(r.current_stage_index, 0);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.stages.len(), 2);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingStart);
        assert_eq!(r.stages[1].status, StageStatus::Pending);
    }

    // U-2
    #[test]
    fn new_stores_resolved_stages_copy() {
        let r = record(&["a", "b"]);
        assert_eq!(r.resolved_stages.len(), 2);
        assert_eq!(r.resolved_stages[0], stage("a"));
        assert_eq!(r.resolved_stages[1], stage("b"));
    }

    // U-3
    #[test]
    fn approve_current_advances_to_awaiting_start_gate() {
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
        assert!(r.approve_current());
        assert_eq!(r.status, RunStatus::Completed);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
    }

    // U-4
    #[test]
    fn apply_stage_override_replaces_only_current_index() {
        let mut r = record(&["a", "b"]);
        let mut edited = stage("a");
        edited.prompt = "edited prompt".to_string();
        edited.checkpoint = false;

        r.apply_stage_override(edited.clone()).unwrap();

        assert_eq!(r.resolved_stages[0], edited);
        assert_eq!(r.resolved_stages[1], stage("b"));
    }

    // U-5
    #[test]
    fn apply_stage_override_rejects_id_mismatch() {
        let mut r = record(&["a", "b"]);
        let result = r.apply_stage_override(stage("b"));
        assert_eq!(
            result,
            Err(StageOverrideError::StageIdMismatch { expected: "a".to_string(), got: "b".to_string() })
        );
        assert_eq!(r.resolved_stages[0], stage("a"));
    }

    // U-6
    #[test]
    fn begin_current_stage_sets_running_on_stage_and_run() {
        let mut r = record(&["a", "b"]);
        r.begin_current_stage();
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages[0].status, StageStatus::Running);
        assert_eq!(r.stages[1].status, StageStatus::Pending);
    }

    #[test]
    fn advance_to_gate_records_session_and_returns_to_the_gate() {
        let mut r = record(&["a", "b"]);
        r.begin_current_stage();
        let completed = r.advance_to_gate("sess-a".to_string());
        assert!(!completed);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
        assert_eq!(r.stages[0].session_id, Some("sess-a".to_string()));
        assert_eq!(r.stages[1].status, StageStatus::AwaitingStart);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.current_stage_index, 1);
    }

    #[test]
    fn advance_to_gate_on_last_stage_completes_run() {
        let mut r = record(&["a"]);
        assert!(r.advance_to_gate("sess-a".to_string()));
        assert_eq!(r.status, RunStatus::Completed);
        assert_eq!(r.stages[0].session_id, Some("sess-a".to_string()));
    }

    #[test]
    fn current_resolved_stage_follows_the_index() {
        let mut r = record(&["a", "b"]);
        assert_eq!(r.current_resolved_stage().id, "a");
        r.approve_current();
        assert_eq!(r.current_resolved_stage().id, "b");
    }

    // U-7
    #[test]
    fn serializes_new_statuses_as_kebab_case() {
        assert_eq!(serde_json::to_string(&RunStatus::AwaitingStageStart).unwrap(), "\"awaiting-stage-start\"");
        assert_eq!(serde_json::to_string(&StageStatus::AwaitingStart).unwrap(), "\"awaiting-start\"");
    }

    #[test]
    fn serializes_resolved_stages_under_a_camel_case_key() {
        let json = serde_json::to_value(record(&["a"])).unwrap();
        assert!(json.get("resolvedStages").is_some(), "expected a resolvedStages key, got {json}");
        assert_eq!(json["resolvedStages"].as_array().unwrap().len(), 1);
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
