import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");

import { invoke as invokeMock } from "@tauri-apps/api/core";
import { listen as listenMock } from "@tauri-apps/api/event";
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
} from "./api";
import type { Template } from "./types";

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

  it("startPipelineRun invokes start_pipeline_run with templateId and targetDir", async () => {
    invokeMock.mockResolvedValue({});
    await startPipelineRun("t1", "/tmp/proj");
    expect(invokeMock).toHaveBeenCalledWith("start_pipeline_run", { templateId: "t1", targetDir: "/tmp/proj" });
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
    listenMock.mockImplementation((_event, cb) => {
      cb({ payload: { runId: "run1", stageId: "s1", event: { kind: "init", sessionId: "sess1" } } });
      return Promise.resolve(() => {});
    });
    await onStageEvent(handler);
    expect(handler).toHaveBeenCalledWith({ runId: "run1", stageId: "s1", event: { kind: "init", sessionId: "sess1" } });
  });
});
