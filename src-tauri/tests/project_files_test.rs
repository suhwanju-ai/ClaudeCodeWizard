//! Integration tests for the read-only directory listing (IMP-024/025/027/032).
//! orchestrator_test.rs is deliberately untouched — PRD G-5.

use std::path::{Path, PathBuf};

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::{Orchestrator, OrchestratorError};
use claude_pipeline_wizard_lib::engine::project_files::ProjectFilesError;
use claude_pipeline_wizard_lib::engine::run_record::{RunRecord, RunRecordStore};
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

struct Harness {
    orchestrator: Orchestrator,
    _template_dir: tempfile::TempDir,
    run_dir: tempfile::TempDir,
    target_dir: tempfile::TempDir,
    _dump_dir: tempfile::TempDir,
}

impl Harness {
    fn target(&self) -> PathBuf {
        self.target_dir.path().to_path_buf()
    }

    fn persisted(&self, run_id: &str) -> RunRecord {
        RunRecordStore::new(self.run_dir.path()).load(run_id).unwrap()
    }
}

fn setup_with(template: Template) -> Harness {
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
            stage_timeout: std::time::Duration::from_secs(30),
            extra_env: vec![(
                "MOCK_CLAUDE_DUMP_DIR".to_string(),
                dump_dir.path().to_string_lossy().to_string(),
            )],
        },
    );
    Harness {
        orchestrator,
        _template_dir: template_dir,
        run_dir,
        target_dir,
        _dump_dir: dump_dir,
    }
}

fn one_stage_template(fixture: &str) -> Template {
    Template {
        id: "files-one-stage".to_string(),
        name: "Files One Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![stage("stage1", fixture, true)],
    }
}

/// T-G20 — PRD G-20. The frontend can only tell these two errors apart by their
/// prefix, so the prefix is a contract and this test is what stops it being edited away.
#[test]
fn error_prefixes_that_the_frontend_matches_on_are_pinned() {
    assert!(ProjectFilesError::OutsideTargetDir("x".into())
        .to_string()
        .starts_with("PATH_OUTSIDE_TARGET_DIR:"));
    assert!(ProjectFilesError::NotFound("x".into())
        .to_string()
        .starts_with("PATH_NOT_FOUND:"));
}

/// T-G1 — PRD G-1. The same inputs U-G1a/U-G1c pin at the unit level, replayed through
/// the real command entry point, so no wiring can bypass the check.
#[tokio::test]
async fn containment_holds_through_the_orchestrator_entry_point() {
    let harness = setup_with(one_stage_template("orchestrator_stage1.jsonl"));
    harness
        .orchestrator
        .start_run("files-one-stage", harness.target(), "run-containment".to_string())
        .unwrap();

    for input in ["../secret.txt", "sub/../../secret.txt", "..", "C:\\Windows\\win.ini", "/etc/passwd", "\\\\server\\share"] {
        let result = harness.orchestrator.list_project_dir("run-containment", input).await;
        match result {
            Err(OrchestratorError::ProjectFiles(ProjectFilesError::OutsideTargetDir(_))) => {}
            other => panic!("expected OutsideTargetDir for {input:?}, got {other:?}"),
        }
    }

    // And the happy path still works, so the test is not passing by refusing everything.
    let listing = harness.orchestrator.list_project_dir("run-containment", "").await.unwrap();
    assert_eq!(listing.path, "");
    assert!(listing.entries.iter().any(|e| e.name == ".claude-pipeline-wizard"));
}

/// T-G6 — PRD G-6. Listing changes nothing: the persisted record is byte-identical
/// before and after.
#[tokio::test]
async fn listing_leaves_the_run_record_byte_identical() {
    let harness = setup_with(one_stage_template("orchestrator_stage1.jsonl"));
    harness
        .orchestrator
        .start_run("files-one-stage", harness.target(), "run-noop".to_string())
        .unwrap();

    let before = serde_json::to_string(&harness.persisted("run-noop")).unwrap();
    harness.orchestrator.list_project_dir("run-noop", "").await.unwrap();
    let after = serde_json::to_string(&harness.persisted("run-noop")).unwrap();

    assert_eq!(before, after);
}

/// T-G13 — the observable form of "it takes no run lock": the listing returns while a
/// stage is still executing, instead of waiting for it. No wall-clock threshold is
/// asserted, only that the listing succeeded while the run was Running.
#[tokio::test]
async fn listing_does_not_block_on_a_running_stage() {
    let harness = setup_with(one_stage_template("project_files_slow_stage.jsonl"));
    harness
        .orchestrator
        .start_run("files-one-stage", harness.target(), "run-slow".to_string())
        .unwrap();

    let stage_future = harness.orchestrator.start_stage("run-slow", 0, None, |_, _| {});
    let listing_future = async {
        // Long enough for start_stage to have persisted Running and spawned the mock,
        // far shorter than the fixture's 3s sleep.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let status_during = harness.persisted("run-slow").status;
        let listing = harness.orchestrator.list_project_dir("run-slow", "").await;
        (status_during, listing)
    };

    let (stage_result, (status_during, listing)) = tokio::join!(stage_future, listing_future);

    stage_result.unwrap();
    assert!(listing.is_ok(), "listing must not wait for the stage: {listing:?}");
    assert_eq!(
        status_during,
        claude_pipeline_wizard_lib::engine::run_record::RunStatus::Running
    );
}
