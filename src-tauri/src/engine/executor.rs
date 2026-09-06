use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::template::{PermissionMode, Stage};

use super::stream_json::{parse_line, StageEvent};

pub struct ExecutorConfig {
    pub claude_binary: String,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self { claude_binary: "claude".to_string() }
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

    cmd.arg("-p").arg(&stage.prompt);
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

    let _ = stderr_drain.await;
    let status = child.wait().await?;

    if let Some(e) = stdout_error {
        return Err(e);
    }

    Ok(status.code().unwrap_or(-1))
}
