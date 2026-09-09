export type PermissionMode = "acceptEdits" | "bypassPermissions" | "default";

export interface Stage {
  id: string;
  name: string;
  prompt: string;
  permissionMode: PermissionMode;
  allowedTools: string[];
  checkpoint: boolean;
}

export interface Template {
  id: string;
  name: string;
  description: string;
  stages: Stage[];
}

export type StageStatus =
  | "pending"
  | "awaiting-start"
  | "running"
  | "awaiting-checkpoint"
  | "approved"
  | "failed";

export type RunStatus =
  | "running"
  | "awaiting-stage-start"
  | "awaiting-checkpoint"
  | "completed"
  | "failed"
  | "cancelled";

export interface StageRun {
  id: string;
  status: StageStatus;
  sessionId: string | null;
  log: unknown[];
}

export interface RunRecord {
  runId: string;
  templateId: string;
  targetDir: string;
  status: RunStatus;
  currentStageIndex: number;
  stages: StageRun[];
  /** The run's own copy of the pipeline. Pre-start edits land here, never in the template. */
  resolvedStages: Stage[];
}

export type StageEvent =
  | { kind: "init"; sessionId: string }
  | { kind: "assistantText"; text: string }
  | { kind: "toolUse"; name: string; input: unknown }
  | { kind: "toolResult"; content: unknown }
  | { kind: "result"; sessionId: string; success: boolean; result: string | null }
  | { kind: "processError"; exitCode: number | null; stderr: string }
  | { kind: "unknown"; raw: unknown };

export interface StageEventPayload {
  runId: string;
  stageId: string;
  event: StageEvent;
}

/**
 * Mirrors of `src-tauri/src/engine/project_files.rs`. Hand-written, like every other
 * type in this file: a commit that changes those Rust structs changes these in the same
 * commit (TRD 9.9-(3)). A Rust test pins the serialized key names.
 */
export type EntryKind = "file" | "directory" | "symlink" | "other";

export interface DirEntry {
  name: string;
  kind: EntryKind;
  size: number | null;
  modifiedMs: number | null;
}

export interface DirListing {
  path: string;
  entries: DirEntry[];
}
