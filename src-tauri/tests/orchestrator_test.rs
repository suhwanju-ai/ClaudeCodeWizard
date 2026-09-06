use std::path::Path;

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::Orchestrator;
use claude_pipeline_wizard_lib::engine::run_record::{RunRecordStore, RunStatus, StageStatus};
use claude_pipeline_wizard_lib::engine::stream_json::StageEvent;
use claude_pipeline_wizard_lib::template::store::TemplateStore;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage, Template};

fn fixture_prompt(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    format!("FIXTURE:{}", path.to_string_lossy())
}

fn two_stage_template() -> Template {
    Template {
        id: "two-stage".to_string(),
        name: "Two Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![
            Stage {
                id: "stage1".to_string(),
                name: "Stage 1".to_string(),
                prompt: fixture_prompt("orchestrator_stage1.jsonl"),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            },
            Stage {
                id: "stage2".to_string(),
                name: "Stage 2".to_string(),
                prompt: fixture_prompt("orchestrator_stage2.jsonl"),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: false,
            },
        ],
    }
}

fn setup() -> (Orchestrator, tempfile::TempDir, tempfile::TempDir, tempfile::TempDir) {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&two_stage_template()).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string() },
    );
    (orchestrator, template_dir, run_dir, target_dir)
}

#[tokio::test]
async fn start_run_stops_at_first_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    let record = orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingCheckpoint);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1".to_string()));
}

#[tokio::test]
async fn approve_checkpoint_advances_and_then_completes() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    let mut events = Vec::new();
    let record = orchestrator
        .approve_checkpoint("run1", |stage_id, event| events.push((stage_id.to_string(), event)))
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::Approved);
    assert!(events.iter().any(|(id, _)| id == "stage2"));
}

#[tokio::test]
async fn request_changes_reruns_current_stage_and_stays_awaiting_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    let mut template = orchestrator.template_store.load("two-stage").unwrap();
    template.stages[0].prompt = fixture_prompt("orchestrator_feedback.jsonl");
    orchestrator.template_store.save(&template).unwrap();

    let record = orchestrator
        .request_changes("run1", fixture_prompt("orchestrator_feedback.jsonl").as_str(), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-revised".to_string()));
}

#[tokio::test]
async fn reject_checkpoint_cancels_run() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    let record = orchestrator.reject_checkpoint("run1").unwrap();
    assert_eq!(record.status, RunStatus::Cancelled);
}
