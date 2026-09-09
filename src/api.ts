import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { RunRecord, Stage, StageEventPayload, Template } from "./types";

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

export function startPipelineRun(templateId: string, targetDir: string, runId: string): Promise<RunRecord> {
  return invoke("start_pipeline_run", { templateId, targetDir, runId });
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

/**
 * Starts the stage the run is currently parked on. `expectedStageIndex` is required,
 * not optional: it is the generation guard the backend uses to reject a click that was
 * composed against an earlier gate. Pass the same `run.currentStageIndex` the panel
 * rendered from.
 */
export function startStage(
  runId: string,
  expectedStageIndex: number,
  stageOverride?: Stage
): Promise<RunRecord> {
  return invoke("start_stage", { runId, expectedStageIndex, stageOverride: stageOverride ?? null });
}

export function cancelRun(runId: string): Promise<RunRecord> {
  return invoke("cancel_run", { runId });
}

/** Read-only refetch, used to resynchronize after a stale-stage-index rejection. */
export function getRun(runId: string): Promise<RunRecord> {
  return invoke("get_run", { runId });
}

/**
 * Tauri commands collapse every backend error to a string, so this prefix — set on the
 * Rust side in OrchestratorError::StaleStageIndex — is the only way to tell this error
 * apart from any other. Keep the two in sync; a Rust test pins the prefix.
 */
export const STALE_STAGE_INDEX_PREFIX = "STALE_STAGE_INDEX:";

export function isStaleStageIndexError(error: unknown): boolean {
  if (error === null || error === undefined) return false;
  const text = typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
  return text.includes(STALE_STAGE_INDEX_PREFIX);
}
