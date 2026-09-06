import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { RunRecord, StageEventPayload, Template } from "./types";

export function listTemplates(): Promise<Template[]> {
  return invoke("list_templates");
}

export function loadTemplate(id: string): Promise<Template> {
  return invoke("load_template", { id });
}

export function saveTemplate(template: Template): Promise<void> {
  return invoke("save_template", { template });
}

export function deleteTemplate(id: string): Promise<void> {
  return invoke("delete_template", { id });
}

export function checkCli(): Promise<string> {
  return invoke("check_cli");
}

export function startPipelineRun(templateId: string, targetDir: string): Promise<RunRecord> {
  return invoke("start_pipeline_run", { templateId, targetDir });
}

export function approveCheckpoint(runId: string): Promise<RunRecord> {
  return invoke("approve_checkpoint", { runId });
}

export function requestChanges(runId: string, feedback: string): Promise<RunRecord> {
  return invoke("request_changes", { runId, feedback });
}

export function rejectCheckpoint(runId: string): Promise<RunRecord> {
  return invoke("reject_checkpoint", { runId });
}

export function onStageEvent(handler: (payload: StageEventPayload) => void): Promise<UnlistenFn> {
  return listen<StageEventPayload>("pipeline://stage-event", (event) => handler(event.payload));
}
