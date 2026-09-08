use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::cli_check::{check_claude_cli, CliCheckResult};
use crate::engine::orchestrator::Orchestrator;
use crate::engine::run_record::RunRecord;
use crate::engine::stream_json::StageEvent;
use crate::template::Template;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StageEventPayload {
    run_id: String,
    stage_id: String,
    event: StageEvent,
}

fn emit_stage_event(app: &AppHandle, run_id: &str, stage_id: &str, event: StageEvent) {
    let _ = app.emit(
        "pipeline://stage-event",
        StageEventPayload { run_id: run_id.to_string(), stage_id: stage_id.to_string(), event },
    );
}

#[tauri::command]
pub fn list_templates(orchestrator: State<Orchestrator>) -> Result<Vec<Template>, String> {
    orchestrator.template_store.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_template(orchestrator: State<Orchestrator>, id: String) -> Result<Template, String> {
    orchestrator.template_store.load(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_template(orchestrator: State<Orchestrator>, template: Template) -> Result<(), String> {
    orchestrator.template_store.save(&template).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_template(orchestrator: State<Orchestrator>, id: String) -> Result<(), String> {
    orchestrator.template_store.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_cli(orchestrator: State<Orchestrator>) -> String {
    match check_claude_cli(&orchestrator.executor_config.claude_binary) {
        CliCheckResult::Available(version) => format!("available:{version}"),
        CliCheckResult::NotFound => "not-found".to_string(),
    }
}

#[tauri::command]
pub fn start_pipeline_run(
    orchestrator: State<Orchestrator>,
    template_id: String,
    target_dir: String,
    run_id: String,
) -> Result<RunRecord, String> {
    orchestrator.start_run(&template_id, target_dir.into(), run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn approve_checkpoint(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
) -> Result<RunRecord, String> {
    orchestrator
        .approve_checkpoint(&run_id, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn request_changes(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
    feedback: String,
) -> Result<RunRecord, String> {
    orchestrator
        .request_changes(&run_id, &feedback, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reject_checkpoint(orchestrator: State<Orchestrator>, run_id: String) -> Result<RunRecord, String> {
    orchestrator.reject_checkpoint(&run_id).map_err(|e| e.to_string())
}
