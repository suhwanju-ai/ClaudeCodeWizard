import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("./api", () => ({
  listTemplates: vi.fn(),
  deleteTemplate: vi.fn(),
  checkCli: vi.fn(),
  saveTemplate: vi.fn(),
  startPipelineRun: vi.fn(),
  onStageEvent: vi.fn(),
  approveCheckpoint: vi.fn(),
  requestChanges: vi.fn(),
  rejectCheckpoint: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import { listTemplates, checkCli, startPipelineRun, onStageEvent } from "./api";
import { open } from "@tauri-apps/plugin-dialog";
import App from "./App";
import type { RunRecord, Template } from "./types";

const sample: Template = { id: "t1", name: "웹 프로그램 개발", description: "desc", stages: [] };

beforeEach(() => {
  vi.mocked(listTemplates).mockReset().mockResolvedValue([sample]);
  vi.mocked(checkCli).mockReset().mockResolvedValue("available:mock-claude 0.0.1");
  vi.mocked(onStageEvent).mockReset().mockResolvedValue(() => {});
  vi.mocked(open).mockReset();
  vi.mocked(startPipelineRun).mockReset();
});

describe("App routing", () => {
  it("starts on the template gallery", async () => {
    render(<App />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  it("picks a target folder and starts a run when '불러와서 실행' is clicked", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    const runRecord: RunRecord = {
      runId: "run1",
      templateId: "t1",
      targetDir: "/tmp/new-project",
      status: "awaiting-checkpoint",
      currentStageIndex: 0,
      stages: [{ id: "s1", status: "awaiting-checkpoint", sessionId: "sess1", log: [] }],
    };
    vi.mocked(startPipelineRun).mockResolvedValue(runRecord);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "불러와서 실행" }));

    await waitFor(() => expect(open).toHaveBeenCalledWith({ directory: true }));
    await waitFor(() => expect(startPipelineRun).toHaveBeenCalledWith("t1", "/tmp/new-project"));
    expect(await screen.findByText("상태: awaiting-checkpoint")).toBeInTheDocument();
  });
});
