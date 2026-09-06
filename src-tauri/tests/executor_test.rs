use std::path::{Path, PathBuf};

use claude_pipeline_wizard_lib::engine::executor::{run_stage, ExecutorConfig};
use claude_pipeline_wizard_lib::engine::stream_json::StageEvent;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage};

fn stage(prompt: &str) -> Stage {
    Stage {
        id: "s1".to_string(),
        name: "Stage 1".to_string(),
        prompt: prompt.to_string(),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: vec![],
        checkpoint: true,
    }
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[tokio::test]
async fn streams_parsed_events_and_returns_success_exit_code() {
    let config = ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string() };
    let fixture = fixture_path("executor_success.jsonl");
    let prompt = format!("FIXTURE:{}", fixture.to_string_lossy());
    let target_dir = std::env::temp_dir();

    let mut events = Vec::new();
    let exit_code = run_stage(&config, &stage(&prompt), &target_dir, None, |e| events.push(e))
        .await
        .unwrap();

    assert_eq!(exit_code, 0);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], StageEvent::Init { session_id: "sess-exec-1".to_string() });
    assert!(matches!(events[1], StageEvent::AssistantText { .. }));
    assert_eq!(
        events[2],
        StageEvent::Result { session_id: "sess-exec-1".to_string(), success: true, result: Some("done".to_string()) }
    );
}

#[tokio::test]
async fn returns_non_zero_exit_code_on_failure() {
    let config = ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string() };
    let fixture = fixture_path("executor_failure.jsonl");
    let prompt = format!("FIXTURE:{}", fixture.to_string_lossy());
    let target_dir = std::env::temp_dir();

    let mut events = Vec::new();
    let exit_code = run_stage(&config, &stage(&prompt), &target_dir, None, |e| events.push(e))
        .await
        .unwrap();

    assert_eq!(exit_code, 1);
    assert_eq!(events.len(), 1);
}
