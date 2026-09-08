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

fn resume_arg(call: &[String]) -> Option<&str> {
    call.iter().position(|a| a == "--resume").and_then(|i| call.get(i + 1)).map(String::as_str)
}

// T-A2
#[tokio::test]
async fn non_checkpoint_stage_completion_returns_to_gate() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let record = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    // PRD A-2: no auto-advance. The run parks at the *next* stage's gate.
    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 1);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::AwaitingStart);
    assert_eq!(h.call_count(), 1, "exactly one stage ran");
}

// T-A4
#[tokio::test]
async fn full_pipeline_requires_n_start_stage_calls() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    let record = h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(record.stages[1].status, StageStatus::Approved);
    // PRD A-2: N stages needed exactly N explicit start_stage calls.
    assert_eq!(h.call_count(), 2);
}

// T-A5 — session chaining survives the removal of drive()
#[tokio::test]
async fn start_stage_resumes_previous_stage_session() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    let calls = h.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(resume_arg(&calls[0]), None, "the first stage starts a fresh session");
    assert_eq!(
        resume_arg(&calls[1]),
        Some("sess-orch-1"),
        "the second stage resumes stage 1's session (TRD 3.3-(4a) rule (a))"
    );
    assert_eq!(h.persisted("run1").stages[0].session_id, Some("sess-orch-1".to_string()));
}

// T-A5, checkpoint variant — the same chaining must hold across an approval (T7)
#[tokio::test]
async fn start_stage_after_approve_checkpoint_resumes_the_checkpointed_session() {
    let h = setup(); // stage1 checkpoints
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();
    h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    let calls = h.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(resume_arg(&calls[1]), Some("sess-orch-1"));
}

// T-E1
#[tokio::test]
async fn start_stage_on_non_gate_status_returns_not_awaiting_stage_start() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap(); // now AwaitingCheckpoint

    let result = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::NotAwaitingStageStart(ref id)) if id == "run1"), "got {result:?}");
    assert_eq!(h.call_count(), 1);
}

// T-F1
#[tokio::test]
async fn start_stage_rejects_an_invalid_override() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let mut blank = stage("stage1", "orchestrator_stage1.jsonl", true);
    blank.prompt = "   ".to_string();
    let result = h.orchestrator.start_stage("run1", 0, Some(blank), |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::StageValidation(_))), "got {result:?}");
    assert_eq!(h.call_count(), 0, "a rejected override must not spawn anything");
    assert_eq!(h.persisted("run1").status, RunStatus::AwaitingStageStart, "a rejection is not a transition");
}

#[tokio::test]
async fn start_stage_rejects_an_override_for_a_different_stage() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let wrong = stage("stage2", "orchestrator_stage2.jsonl", false);
    let result = h.orchestrator.start_stage("run1", 0, Some(wrong), |_, _| {}).await;

    assert!(
        matches!(result, Err(OrchestratorError::StageIdMismatch { ref expected, ref got }) if expected == "stage1" && got == "stage2"),
        "got {result:?}"
    );
    assert_eq!(h.call_count(), 0);
}

#[tokio::test]
async fn start_stage_applies_a_valid_override_to_resolved_stages_only() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let mut edited = stage("stage1", "orchestrator_stage1.jsonl", false);
    edited.name = "Renamed by the user".to_string();
    edited.allowed_tools = vec!["Read".to_string(), "Grep".to_string()];
    let record = h.orchestrator.start_stage("run1", 0, Some(edited), |_, _| {}).await.unwrap();

    assert_eq!(record.resolved_stages[0].name, "Renamed by the user");
    assert_eq!(record.resolved_stages[1].name, "Stage stage2", "only the current stage is replaced");
    let calls = h.calls();
    assert!(
        calls[0].windows(2).any(|w| w[0] == "--allowedTools" && w[1] == "Read,Grep"),
        "the override's allowed tools were actually used: {:?}",
        calls[0]
    );
}

// T-B1 (second half)
#[tokio::test]
async fn manifest_is_rewritten_on_each_transition() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();
    let after_start_run = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    let after_stage_one = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();

    assert_ne!(after_start_run, after_stage_one, "the manifest must change on every transition (PRD B-1)");
    assert!(after_stage_one.contains("\"currentStageIndex\": 1"));
    assert!(after_stage_one.contains("\"awaiting-start\""));
}

// T-D4a — mutex + status guard
#[tokio::test]
async fn concurrent_start_stage_spawns_only_one_process() {
    // The two calls race. The winner writes status=Running to disk inside the lock and
    // releases it *before* spawning; the loser then reads Running and trips guard (i).
    // The generation guard is never reached here — see T-D4b for that.
    let h = std::sync::Arc::new(setup_with(no_checkpoint_two_stage_template()));
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let a = { let h = h.clone(); tokio::spawn(async move { h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.map(|_| ()) }) };
    let b = { let h = h.clone(); tokio::spawn(async move { h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.map(|_| ()) }) };
    let (ra, rb) = (a.await.unwrap(), b.await.unwrap());

    assert_eq!(h.call_count(), 1, "the mutex must prevent a second claude process (PRD D-4)");
    let failures: Vec<_> = [ra, rb].into_iter().filter_map(Result::err).collect();
    assert_eq!(failures.len(), 1, "exactly one call is rejected");
    assert!(
        matches!(failures[0], OrchestratorError::NotAwaitingStageStart(ref id) if id == "run1"),
        "got {:?}",
        failures[0]
    );
}

// T-D4b — generation guard, deterministic and sequential
#[tokio::test]
async fn stale_stage_index_is_rejected_after_gate_advance() {
    // A no-checkpoint template with a following stage is the *only* shape that returns
    // the run to AwaitingStageStart at a different index, which is the one path where
    // the status guard passes and only the generation guard can reject.
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let after = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    assert_eq!(after.status, RunStatus::AwaitingStageStart);
    assert_eq!(after.current_stage_index, 1);

    // A duplicate click / late retry arrives carrying the stale index 0.
    let result = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await;

    assert!(
        matches!(result, Err(OrchestratorError::StaleStageIndex { expected: 0, actual: 1 })),
        "got {result:?}"
    );
    assert_eq!(h.call_count(), 1, "stage 2 must not have been spawned");
    let persisted = h.persisted("run1");
    assert_eq!(persisted.status, RunStatus::AwaitingStageStart, "a rejection is not a transition");
    assert_eq!(persisted.current_stage_index, 1);
}

// T-A6 replacement for the deleted drive-timeout test (08e9144 precedent)
#[tokio::test]
async fn start_stage_marks_run_failed_instead_of_stuck_running_on_timeout() {
    let h = setup_with_timeout(
        Template {
            id: "hang".to_string(),
            name: "Hang".to_string(),
            description: "desc".to_string(),
            stages: vec![stage("stage1", "executor_hang.jsonl", false)],
        },
        std::time::Duration::from_millis(100),
    );
    h.orchestrator.start_run("hang", h.target(), "run1".to_string()).unwrap();

    // run_current_stage returns Err; start_stage must absorb it, not propagate with `?`.
    let record = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::Failed);
    assert_eq!(record.stages[0].status, StageStatus::Failed);
    let persisted = h.persisted("run1");
    assert_eq!(persisted.status, RunStatus::Failed, "the on-disk record must not be stuck at running");
    assert_eq!(persisted.stages[0].status, StageStatus::Failed);
}

// GAP-3 decision D1: Failed is terminal.
#[tokio::test]
async fn failed_run_rejects_start_stage() {
    let h = setup_with_timeout(
        Template {
            id: "hang2".to_string(),
            name: "Hang2".to_string(),
            description: "desc".to_string(),
            stages: vec![stage("stage1", "executor_hang.jsonl", false), stage("stage2", "orchestrator_stage2.jsonl", false)],
        },
        std::time::Duration::from_millis(100),
    );
    h.orchestrator.start_run("hang2", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    let result = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::NotAwaitingStageStart(_))), "got {result:?}");
}
