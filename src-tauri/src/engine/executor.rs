use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
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
    let mut reader = BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await? {
        match parse_line(&line) {
            Ok(event) => on_event(event),
            Err(warning) => on_event(StageEvent::Unknown {
                raw: serde_json::json!({ "parseWarning": warning.0 }),
            }),
        }
    }

    let status = child.wait().await?;
    Ok(status.code().unwrap_or(-1))
}
