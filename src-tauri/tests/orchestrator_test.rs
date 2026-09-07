use std::fs;
use std::path::Path;

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::{Orchestrator, OrchestratorError};
use claude_pipeline_wizard_lib::engine::run_record::{RunRecordStore, RunStatus, StageStatus};
use claude_pipeline_wizard_lib::template::store::TemplateStore;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage, Template};

fn fixture_prompt(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    format!("FIXTURE:{}", path.to_string_lossy())
}

fn three_stage_template() -> Template {
    Template {
        id: "three-stage".to_string(),
        name: "Three Stage".to_string(),
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
            Stage {
                id: "stage3".to_string(),
                name: "Stage 3".to_string(),
                prompt: fixture_prompt("orchestrator_stage3.jsonl"),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            },
        ],
    }
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
        ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(), ..Default::default() },
    );
    (orchestrator, template_dir, run_dir, target_dir)
}

#[test]
fn start_run_creates_record_awaiting_first_stage_without_executing() {
    let (orchestrator, _t, _r, target_dir) = setup();
    let record = orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string())
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[0].session_id, None);

    let manifest_dir = target_dir.path().join(".claude-pipeline-wizard");
    assert!(manifest_dir.join("run.json").exists());
    assert!(manifest_dir.join("pipeline.json").exists());
}

#[tokio::test]
async fn start_stage_without_override_runs_current_stage_and_pauses_at_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();

    let mut events = Vec::new();
    let record = orchestrator
        .start_stage("run1", None, |stage_id, event| events.push((stage_id.to_string(), event)))
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingCheckpoint);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1".to_string()));
    assert!(events.iter().any(|(id, _)| id == "stage1"));
    assert_eq!(record.resolved_stages[0].prompt, fixture_prompt("orchestrator_stage1.jsonl"));
}

#[tokio::test]
async fn start_stage_with_override_replaces_resolved_stage_prompt_before_running() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();

    let mut edited = orchestrator.template_store.load("two-stage").unwrap().stages[0].clone();
    edited.prompt = fixture_prompt("orchestrator_stage1_edited.jsonl");

    let record = orchestrator.start_stage("run1", Some(edited), |_, _| {}).await.unwrap();

    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-edited".to_string()));
    assert_eq!(record.resolved_stages[0].prompt, fixture_prompt("orchestrator_stage1_edited.jsonl"));

    // Run-scoped only: the saved gallery template is untouched.
    let saved_template = orchestrator.template_store.load("two-stage").unwrap();
    assert_eq!(saved_template.stages[0].prompt, fixture_prompt("orchestrator_stage1.jsonl"));

    let pipeline_json_path = target_dir.path().join(".claude-pipeline-wizard/pipeline.json");
    let snapshot: Template = serde_json::from_str(&fs::read_to_string(pipeline_json_path).unwrap()).unwrap();
    assert_eq!(snapshot.stages[0].prompt, fixture_prompt("orchestrator_stage1_edited.jsonl"));
}

#[tokio::test]
async fn start_stage_rejects_override_with_mismatched_id() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();

    let mut wrong = orchestrator.template_store.load("two-stage").unwrap().stages[0].clone();
    wrong.id = "not-stage1".to_string();

    let result = orchestrator.start_stage("run1", Some(wrong), |_, _| {}).await;
    assert!(matches!(result, Err(OrchestratorError::StageIdMismatch(id)) if id == "not-stage1"));
}

#[tokio::test]
async fn approve_checkpoint_advances_to_awaiting_start_without_running_next_stage() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let record = orchestrator.approve_checkpoint("run1").unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 1);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[1].session_id, None);
}

#[tokio::test]
async fn full_two_stage_run_completes_after_starting_each_stage_explicitly() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();
    orchestrator.approve_checkpoint("run1").unwrap();

    let mut events = Vec::new();
    let record = orchestrator
        .start_stage("run1", None, |stage_id, event| events.push((stage_id.to_string(), event)))
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::Approved);
    assert!(events.iter().any(|(id, _)| id == "stage2"));
}

#[tokio::test]
async fn request_changes_reruns_current_stage_using_resolved_prompt_override() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let record = orchestrator
        .request_changes("run1", &fixture_prompt("orchestrator_feedback.jsonl"), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-revised".to_string()));
    // The stage's canonical (resolved) prompt is unchanged by feedback.
    assert_eq!(record.resolved_stages[0].prompt, fixture_prompt("orchestrator_stage1.jsonl"));
}

#[tokio::test]
async fn reject_checkpoint_cancels_run() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let record = orchestrator.reject_checkpoint("run1").unwrap();
    assert_eq!(record.status, RunStatus::Cancelled);
}

#[tokio::test]
async fn reject_checkpoint_rejects_a_run_not_awaiting_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    orchestrator.reject_checkpoint("run1").unwrap();

    let result = orchestrator.reject_checkpoint("run1");
    assert!(matches!(result, Err(OrchestratorError::NotAwaitingCheckpoint(id)) if id == "run1"));
}

#[tokio::test]
async fn start_stage_when_not_awaiting_stage_start_errors() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap(); // now AwaitingCheckpoint

    let result = orchestrator.start_stage("run1", None, |_, _| {}).await;
    assert!(matches!(result, Err(OrchestratorError::NotAwaitingStageStart(id)) if id == "run1"));
}

fn hanging_two_stage_template() -> Template {
    Template {
        id: "hang-two-stage".to_string(),
        name: "Hang Two Stage".to_string(),
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
                prompt: fixture_prompt("executor_hang.jsonl"),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            },
        ],
    }
}

#[tokio::test]
async fn start_stage_marks_run_failed_instead_of_stuck_running_on_timeout() {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&hanging_two_stage_template()).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig {
            claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
            stage_timeout: std::time::Duration::from_millis(100),
            ..Default::default()
        },
    );

    orchestrator.start_run("hang-two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();
    orchestrator.approve_checkpoint("run1").unwrap();

    let result = orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.stages[1].status, StageStatus::Failed);

    // The on-disk record must reflect the failure too, not be stuck at "running".
    let persisted = RunRecordStore::new(run_dir.path()).load("run1").unwrap();
    assert_eq!(persisted.status, RunStatus::Failed);
    assert_eq!(persisted.stages[1].status, StageStatus::Failed);
}

#[tokio::test]
async fn request_changes_marks_run_failed_instead_of_stuck_running_on_run_stage_timeout() {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&two_stage_template()).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig {
            claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
            stage_timeout: std::time::Duration::from_millis(100),
            ..Default::default()
        },
    );

    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let hang_feedback = fixture_prompt("executor_hang.jsonl");
    let result = orchestrator.request_changes("run1", &hang_feedback, |_, _| {}).await.unwrap();

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.stages[0].status, StageStatus::Failed);

    let persisted = RunRecordStore::new(run_dir.path()).load("run1").unwrap();
    assert_eq!(persisted.status, RunStatus::Failed);
    assert_eq!(persisted.stages[0].status, StageStatus::Failed);
}

#[test]
fn manifest_write_failure_does_not_fail_the_run_or_leave_it_stuck() {
    let (orchestrator, _t, run_dir, target_dir) = setup();

    // Block project-manifest writes: pre-create the manifest path as a plain
    // *file*, so `fs::create_dir_all` inside `write_project_manifest` fails
    // because a non-directory already occupies that exact path. `target_dir`
    // itself is left as a normal, writable directory, so `run_store.save`
    // (which writes into the unrelated app-data `run_dir`) is unaffected.
    std::fs::create_dir_all(target_dir.path()).unwrap();
    std::fs::write(target_dir.path().join(".claude-pipeline-wizard"), b"blocked").unwrap();

    let record = orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string())
        .expect("start_run must succeed even when the project-local manifest write fails");

    assert_eq!(record.status, RunStatus::AwaitingStageStart);

    // The authoritative app-data-dir record must reflect the same state,
    // proving `run_store.save` actually succeeded and the run isn't stuck.
    let persisted = RunRecordStore::new(run_dir.path()).load("run1").unwrap();
    assert_eq!(persisted.status, RunStatus::AwaitingStageStart);
    assert_eq!(persisted.current_stage_index, 0);
}

#[tokio::test]
async fn middle_non_checkpoint_stage_stops_and_does_not_auto_advance_into_next_stage() {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&three_stage_template()).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(), ..Default::default() },
    );

    orchestrator.start_run("three-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    // stage1 (checkpoint: true) runs and pauses at its checkpoint.
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();
    // Approving advances to stage2 (checkpoint: false), paused for pre-stage edit.
    orchestrator.approve_checkpoint("run1").unwrap();

    // Running stage2 (checkpoint: false) must NOT auto-run stage3 as a side effect --
    // it should stop and wait for its own explicit `start_stage` call, just like a
    // checkpointed stage would.
    let record = orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 2);
    assert_eq!(record.stages[1].status, StageStatus::Approved);
    // Stage 3 is only paused for its pre-stage edit gate -- it was not executed.
    assert_eq!(record.stages[2].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[2].session_id, None);
}
