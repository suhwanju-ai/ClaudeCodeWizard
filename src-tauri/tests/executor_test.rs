use std::fs;
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
    let config = ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(), ..Default::default() };
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
    let config = ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(), ..Default::default() };
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

#[tokio::test]
async fn kills_hung_child_and_returns_error_on_timeout() {
    let config = ExecutorConfig {
        claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
        stage_timeout: std::time::Duration::from_millis(100),
        ..Default::default()
    };
    let fixture = fixture_path("executor_hang.jsonl");
    let prompt = format!("FIXTURE:{}", fixture.to_string_lossy());
    let target_dir = std::env::temp_dir();

    let start = std::time::Instant::now();
    let result = run_stage(&config, &stage(&prompt), &target_dir, None, |_| {}).await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    assert!(elapsed < std::time::Duration::from_secs(5), "expected the hung child to be killed quickly, took {elapsed:?}");
}

#[tokio::test]
async fn forwards_allowed_tools_as_a_comma_joined_flag() {
    let dump_dir = tempfile::tempdir().unwrap();
    let dump_path = dump_dir.path().join("args.txt");
    let config = ExecutorConfig {
        claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
        extra_env: vec![("MOCK_CLAUDE_DUMP_ARGS".to_string(), dump_path.to_string_lossy().to_string())],
        ..Default::default()
    };
    let mut s = stage("");
    s.allowed_tools = vec!["Read".to_string(), "Bash".to_string()];
    let target_dir = std::env::temp_dir();

    run_stage(&config, &s, &target_dir, None, |_| {}).await.unwrap();

    let dumped = fs::read_to_string(&dump_path).unwrap();
    let args: Vec<&str> = dumped.lines().collect();
    let idx = args.iter().position(|a| *a == "--allowedTools").expect("--allowedTools flag not forwarded");
    assert_eq!(args[idx + 1], "Read,Bash");
}

#[tokio::test]
async fn omits_allowed_tools_flag_when_empty() {
    let dump_dir = tempfile::tempdir().unwrap();
    let dump_path = dump_dir.path().join("args.txt");
    let config = ExecutorConfig {
        claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
        extra_env: vec![("MOCK_CLAUDE_DUMP_ARGS".to_string(), dump_path.to_string_lossy().to_string())],
        ..Default::default()
    };
    let target_dir = std::env::temp_dir();

    run_stage(&config, &stage(""), &target_dir, None, |_| {}).await.unwrap();

    let dumped = fs::read_to_string(&dump_path).unwrap();
    assert!(!dumped.lines().any(|a| a == "--allowedTools"));
}
