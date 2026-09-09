import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  listTemplates,
  loadTemplate,
  saveTemplate,
  deleteTemplate,
  checkCli,
  startPipelineRun,
  approveCheckpoint,
  requestChanges,
  rejectCheckpoint,
  onStageEvent,
  startStage,
  cancelRun,
  getRun,
  isStaleStageIndexError,
  listProjectDir,
  isPathOutsideTargetDirError,
  isPathNotFoundError,
} from "./api";
import type { Stage, Template } from "./types";

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
});

describe("api wrapper", () => {
  it("listTemplates invokes list_templates", async () => {
    invokeMock.mockResolvedValue([]);
    await listTemplates();
    expect(invokeMock).toHaveBeenCalledWith("list_templates");
  });

  it("loadTemplate invokes load_template with id", async () => {
    invokeMock.mockResolvedValue({});
    await loadTemplate("t1");
    expect(invokeMock).toHaveBeenCalledWith("load_template", { id: "t1" });
  });

  it("saveTemplate invokes save_template with template", async () => {
    invokeMock.mockResolvedValue(undefined);
    const template: Template = { id: "t1", name: "T", description: "d", stages: [] };
    await saveTemplate(template);
    expect(invokeMock).toHaveBeenCalledWith("save_template", { template });
  });

  it("deleteTemplate invokes delete_template with id", async () => {
    invokeMock.mockResolvedValue(undefined);
    await deleteTemplate("t1");
    expect(invokeMock).toHaveBeenCalledWith("delete_template", { id: "t1" });
  });

  it("checkCli invokes check_cli", async () => {
    invokeMock.mockResolvedValue("not-found");
    await checkCli();
    expect(invokeMock).toHaveBeenCalledWith("check_cli");
  });

  it("startPipelineRun invokes start_pipeline_run with templateId, targetDir, and runId", async () => {
    invokeMock.mockResolvedValue({});
    await startPipelineRun("t1", "/tmp/proj", "run1");
    expect(invokeMock).toHaveBeenCalledWith("start_pipeline_run", {
      templateId: "t1",
      targetDir: "/tmp/proj",
      runId: "run1",
    });
  });

  it("approveCheckpoint invokes approve_checkpoint with runId", async () => {
    invokeMock.mockResolvedValue({});
    await approveCheckpoint("run1");
    expect(invokeMock).toHaveBeenCalledWith("approve_checkpoint", { runId: "run1" });
  });

  it("requestChanges invokes request_changes with runId and feedback", async () => {
    invokeMock.mockResolvedValue({});
    await requestChanges("run1", "fix this");
    expect(invokeMock).toHaveBeenCalledWith("request_changes", { runId: "run1", feedback: "fix this" });
  });

  it("rejectCheckpoint invokes reject_checkpoint with runId", async () => {
    invokeMock.mockResolvedValue({});
    await rejectCheckpoint("run1");
    expect(invokeMock).toHaveBeenCalledWith("reject_checkpoint", { runId: "run1" });
  });

  it("onStageEvent listens on pipeline://stage-event and forwards the payload", async () => {
    const handler = vi.fn();
    listenMock.mockImplementation((_event: string, cb: (event: { payload: unknown; id: number; event: string }) => void) => {
      cb({
        payload: { runId: "run1", stageId: "s1", event: { kind: "init", sessionId: "sess1" } },
        id: 1,
        event: "pipeline://stage-event"
      });
      return Promise.resolve(() => {});
    });
    await onStageEvent(handler);
    expect(handler).toHaveBeenCalledWith({ runId: "run1", stageId: "s1", event: { kind: "init", sessionId: "sess1" } });
  });

  it("startStage invokes start_stage with runId, expectedStageIndex, and the override", async () => {
    invokeMock.mockResolvedValue({});
    const stage: Stage = {
      id: "s1",
      name: "요구사항",
      prompt: "PRD 작성",
      permissionMode: "acceptEdits",
      allowedTools: ["Read"],
      checkpoint: true,
    };
    await startStage("run1", 2, stage);
    expect(invokeMock).toHaveBeenCalledWith("start_stage", {
      runId: "run1",
      expectedStageIndex: 2,
      stageOverride: stage,
    });
  });

  it("startStage sends null rather than undefined when no override is given", async () => {
    invokeMock.mockResolvedValue({});
    await startStage("run1", 0);
    expect(invokeMock).toHaveBeenCalledWith("start_stage", {
      runId: "run1",
      expectedStageIndex: 0,
      stageOverride: null,
    });
  });

  it("cancelRun invokes cancel_run with runId", async () => {
    invokeMock.mockResolvedValue({});
    await cancelRun("run1");
    expect(invokeMock).toHaveBeenCalledWith("cancel_run", { runId: "run1" });
  });

  it("getRun invokes get_run with runId", async () => {
    invokeMock.mockResolvedValue({});
    await getRun("run1");
    expect(invokeMock).toHaveBeenCalledWith("get_run", { runId: "run1" });
  });

  it("isStaleStageIndexError recognizes the backend prefix in every shape it can arrive in", () => {
    // Tauri rejects with the raw string; a caller may also wrap it in an Error.
    expect(isStaleStageIndexError("STALE_STAGE_INDEX: run is at stage 1, caller expected 0")).toBe(true);
    expect(isStaleStageIndexError(new Error("STALE_STAGE_INDEX: run is at stage 1, caller expected 0"))).toBe(true);
    expect(isStaleStageIndexError("run 'run1' is not awaiting a stage start")).toBe(false);
    expect(isStaleStageIndexError(null)).toBe(false);
    expect(isStaleStageIndexError(undefined)).toBe(false);
  });

  // F-G-api — PRD G-19/G-20.
  it("listProjectDir invokes list_project_dir with camelCase args", async () => {
    invokeMock.mockResolvedValue({ path: "", entries: [], truncated: false });
    const result = await listProjectDir("run1", "src/engine");
    expect(invokeMock).toHaveBeenCalledWith("list_project_dir", { runId: "run1", subPath: "src/engine" });
    expect(result).toEqual({ path: "", entries: [], truncated: false });
  });

  it("recognizes the two project-file error prefixes and nothing else", () => {
    expect(isPathOutsideTargetDirError("PATH_OUTSIDE_TARGET_DIR: ../secret.txt")).toBe(true);
    expect(isPathOutsideTargetDirError(new Error("PATH_OUTSIDE_TARGET_DIR: x"))).toBe(true);
    expect(isPathOutsideTargetDirError("PATH_NOT_FOUND: gone")).toBe(false);
    expect(isPathOutsideTargetDirError(null)).toBe(false);

    expect(isPathNotFoundError("PATH_NOT_FOUND: gone")).toBe(true);
    expect(isPathNotFoundError(new Error("PATH_NOT_FOUND: gone"))).toBe(true);
    expect(isPathNotFoundError("io error: nope")).toBe(false);
    expect(isPathNotFoundError(undefined)).toBe(false);
  });
});
