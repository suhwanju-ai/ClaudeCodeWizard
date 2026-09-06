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
        let _ = stderr_drain.await;
        let _ = child.wait().await;
        return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "stage timed out"));
    }

    let _ = stderr_drain.await;
    let status = child.wait().await?;

    if let Some(e) = stdout_error {
        return Err(e);
    }

    Ok(status.code().unwrap_or(-1))
}
