use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::template::{PermissionMode, Stage};

use super::stream_json::{parse_line, StageEvent};

/// A stage that runs longer than this without producing output is assumed hung and killed.
pub const STAGE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub struct ExecutorConfig {
    pub claude_binary: String,
    pub stage_timeout: Duration,
    /// Extra environment variables set only on the spawned child process (used by tests).
    pub extra_env: Vec<(String, String)>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self { claude_binary: "claude".to_string(), stage_timeout: STAGE_TIMEOUT, extra_env: Vec::new() }
    }
}

fn permission_mode_arg(mode: &PermissionMode) -> &'static str {
    match mode {
        PermissionMode::AcceptEdits => "acceptEdits",
        PermissionMode::BypassPermissions => "bypassPermissions",
        PermissionMode::Default => "default",
    }
}

pub async fn run_stage<F: FnMut(StageEvent)>(
    config: &ExecutorConfig,
    stage: &Stage,
    target_dir: &Path,
    resume_session_id: Option<&str>,
    mut on_event: F,
) -> Result<i32, std::io::Error> {
    let mut cmd = Command::new(&config.claude_binary);
    cmd.current_dir(target_dir)
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        // claude CLI refuses --print with --output-format=stream-json unless --verbose
        // is also set ("Error: When using --print, --output-format=stream-json requires
        // --verbose"). It doesn't change stdout's shape, since parse_line() already
        // ignores any event `type` it doesn't recognize.
        .arg("--verbose")
        .arg("--permission-mode")
        .arg(permission_mode_arg(&stage.permission_mode));

    if let Some(session_id) = resume_session_id {
        cmd.arg("--resume").arg(session_id);
    }

    if !stage.allowed_tools.is_empty() {
        cmd.arg("--allowedTools").arg(stage.allowed_tools.join(","));
    }

    cmd.arg("-p").arg(&stage.prompt);
    cmd.envs(config.extra_env.iter().cloned());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let stderr_drain = tokio::spawn(async move {
        let mut sink = Vec::new();
        let _ = BufReader::new(stderr).read_to_end(&mut sink).await;
        sink
    });

    let mut reader = BufReader::new(stdout).lines();
    let mut stdout_error = None;

    let read_loop = async {
        loop {
            match reader.next_line().await {
                Ok(Some(line)) => match parse_line(&line) {
                    Ok(event) => on_event(event),
                    Err(warning) => on_event(StageEvent::Unknown {
                        raw: serde_json::json!({ "parseWarning": warning.0 }),
                    }),
                },
                Ok(None) => break,
                Err(e) => {
                    stdout_error = Some(e);
                    break;
                }
            }
        }
    };

    if tokio::time::timeout(config.stage_timeout, read_loop).await.is_err() {
        let _ = child.kill().await;
        let stderr_bytes = stderr_drain.await.unwrap_or_default();
        let _ = child.wait().await;
        let minutes = config.stage_timeout.as_secs() / 60;
        let mut message = format!("{minutes}분 동안 응답이 없어 프로세스를 강제 종료했습니다.");
        let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
        if !stderr_text.is_empty() {
            message.push('\n');
            message.push_str(&stderr_text);
        }
        on_event(StageEvent::ProcessError { exit_code: None, stderr: message });
        return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "stage timed out"));
    }

    let stderr_bytes = stderr_drain.await.unwrap_or_default();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
    let status = child.wait().await?;

    if let Some(e) = stdout_error {
        let mut message = format!("claude 프로세스의 표준 출력을 읽는 중 오류가 발생했습니다: {e}");
        if !stderr_text.is_empty() {
            message.push('\n');
            message.push_str(&stderr_text);
        }
        on_event(StageEvent::ProcessError { exit_code: status.code(), stderr: message });
        return Err(e);
    }

    let exit_code = status.code().unwrap_or(-1);
    if exit_code != 0 {
        let message = if stderr_text.is_empty() {
            "claude 프로세스가 표준 오류 메시지 없이 실패했습니다.".to_string()
        } else {
            stderr_text
        };
        on_event(StageEvent::ProcessError { exit_code: status.code(), stderr: message });
    }

    Ok(exit_code)
}
