use std::path::{Path, PathBuf};

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::{Orchestrator, OrchestratorError};
use claude_pipeline_wizard_lib::engine::project_manifest::{manifest_dir, manifest_run_path};
use claude_pipeline_wizard_lib::engine::run_record::{RunRecord, RunRecordStore, RunStatus, StageStatus};
use claude_pipeline_wizard_lib::template::store::TemplateStore;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage, Template};

fn fixture_prompt(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    format!("FIXTURE:{}", path.to_string_lossy())
}

fn stage(id: &str, fixture: &str, checkpoint: bool) -> Stage {
    Stage {
        id: id.to_string(),
        name: format!("Stage {id}"),
        prompt: fixture_prompt(fixture),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: vec![],
        checkpoint,
    }
}

/// stage1 checkpoints, stage2 does not. Drives the checkpoint-path tests.
fn two_stage_template() -> Template {
    Template {
        id: "two-stage".to_string(),
        name: "Two Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![
            stage("stage1", "orchestrator_stage1.jsonl", true),
            stage("stage2", "orchestrator_stage2.jsonl", false),
        ],
    }
}

/// Neither stage checkpoints. This is the fixture the gate tests need: only a
/// non-checkpoint stage *followed by another stage* produces the "back at
/// AwaitingStageStart, but now at index 1" state that T-D4b depends on (TRD 5.2).
#[allow(dead_code)]
fn no_checkpoint_two_stage_template() -> Template {
    Template {
        id: "no-cp-two-stage".to_string(),
        name: "No Checkpoint Two Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![
            stage("stage1", "orchestrator_stage1.jsonl", false),
            stage("stage2", "orchestrator_stage2.jsonl", false),
        ],
    }
}

struct Harness {
    orchestrator: Orchestrator,
    _template_dir: tempfile::TempDir,
    run_dir: tempfile::TempDir,
    target_dir: tempfile::TempDir,
    dump_dir: tempfile::TempDir,
}

impl Harness {
    fn target(&self) -> PathBuf {
        self.target_dir.path().to_path_buf()
    }

    /// Every argv the mock claude binary saw, oldest call first.
    fn calls(&self) -> Vec<Vec<String>> {
        let mut files: Vec<PathBuf> = match std::fs::read_dir(self.dump_dir.path()) {
            Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
            Err(_) => return Vec::new(),
        };
        files.sort();
        files
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap().lines().map(str::to_string).collect())
            .collect()
    }

    fn call_count(&self) -> usize {
        self.calls().len()
    }

    fn persisted(&self, run_id: &str) -> RunRecord {
        RunRecordStore::new(self.run_dir.path()).load(run_id).unwrap()
    }
}

#[allow(dead_code)]
fn setup_with_timeout(template: Template, stage_timeout: std::time::Duration) -> Harness {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let dump_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&template).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig {
            claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
            stage_timeout,
            extra_env: vec![(
                "MOCK_CLAUDE_DUMP_DIR".to_string(),
                dump_dir.path().to_string_lossy().to_string(),
            )],
        },
    );
    Harness { orchestrator, _template_dir: template_dir, run_dir, target_dir, dump_dir }
}

#[allow(dead_code)]
fn setup_with(template: Template) -> Harness {
    setup_with_timeout(template, std::time::Duration::from_secs(30))
}

fn setup() -> Harness {
    setup_with(two_stage_template())
}

// T-A1
#[tokio::test]
async fn start_run_spawns_nothing_and_returns_awaiting_stage_start() {
    let h = setup();
    let record = h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[1].status, StageStatus::Pending);
    assert_eq!(record.resolved_stages.len(), 2);
    assert_eq!(h.call_count(), 0, "start_run must not spawn a claude process (PRD A-1)");
}

// T-B1 (first half) + T-B2
#[tokio::test]
async fn start_run_writes_both_the_manifest_and_the_app_data_record() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    // B-1: the project folder carries the manifest.
    assert!(manifest_run_path(h.target_dir.path()).exists());
    assert!(manifest_dir(h.target_dir.path()).join("pipeline.json").exists());

    // B-2: the app_data copy is still written; the manifest is additional, not a replacement.
    assert_eq!(h.persisted("run1").status, RunStatus::AwaitingStageStart);
}

// B-3 (IMP-017, decision D4)
#[tokio::test]
async fn start_run_refuses_a_target_dir_that_already_holds_a_run() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let result = h.orchestrator.start_run("two-stage", h.target(), "run2".to_string());

    assert!(matches!(result, Err(OrchestratorError::TargetDirInUse(_))), "got {result:?}");
    // The first run's snapshot is untouched — nothing was silently overwritten.
    let run_json = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();
    assert!(run_json.contains("\"runId\": \"run1\""), "first run's manifest survived: {run_json}");
}

// B-4 (IMP-019, decision D6)
#[tokio::test]
async fn start_run_rejects_a_relative_target_dir_without_writing_anything() {
    let h = setup();
    let result = h.orchestrator.start_run("two-stage", PathBuf::from("relative/dir"), "run1".to_string());

    assert!(matches!(result, Err(OrchestratorError::TargetDir(_))), "got {result:?}");
    assert!(!Path::new("relative/dir").exists(), "a rejected target dir must not be created");
    assert!(RunRecordStore::new(h.run_dir.path()).load("run1").is_err());
}

// D-3 (IMP-015, decision D5)
#[tokio::test]
async fn start_run_surfaces_a_manifest_write_failure() {
    let h = setup();
    // Occupy the manifest directory slot with a plain file so create_dir_all fails.
    std::fs::write(manifest_dir(h.target_dir.path()), "not a directory").unwrap();

    let result = h.orchestrator.start_run("two-stage", h.target(), "run1".to_string());

    assert!(matches!(result, Err(OrchestratorError::ProjectManifest(_))), "got {result:?}");
}

// M1
#[tokio::test]
async fn stale_stage_index_error_carries_the_frontend_prefix() {
    // commands.rs collapses every error to a string, so this prefix is the frontend's
    // only handle on this error (src/api.ts isStaleStageIndexError).
    let e = OrchestratorError::StaleStageIndex { expected: 0, actual: 1 };
    assert!(e.to_string().starts_with("STALE_STAGE_INDEX:"), "got {e}");
}

#[tokio::test]
async fn approve_checkpoint_marks_run_failed_instead_of_stuck_running_on_drive_timeout() {
    let h = setup_with_timeout(
        Template {
            id: "hang-two-stage".to_string(),
            name: "Hang Two Stage".to_string(),
            description: "desc".to_string(),
            stages: vec![
                stage("stage1", "orchestrator_stage1.jsonl", true),
                stage("stage2", "executor_hang.jsonl", true),
            ],
        },
        std::time::Duration::from_millis(100),
    );
    h.orchestrator.start_run("hang-two-stage", h.target(), "run1".to_string()).unwrap();

    let result = h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.stages[1].status, StageStatus::Failed);
    assert_eq!(h.persisted("run1").status, RunStatus::Failed);
}

#[tokio::test]
async fn request_changes_marks_run_failed_instead_of_stuck_running_on_run_stage_timeout() {
    let h = setup_with_timeout(two_stage_template(), std::time::Duration::from_millis(100));

    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let hang_feedback = fixture_prompt("executor_hang.jsonl");
    let result = h.orchestrator.request_changes("run1", &hang_feedback, |_, _| {}).await.unwrap();

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.stages[0].status, StageStatus::Failed);

    assert_eq!(h.persisted("run1").status, RunStatus::Failed);
    assert_eq!(h.persisted("run1").stages[0].status, StageStatus::Failed);
}
