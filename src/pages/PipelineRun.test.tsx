import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("../api", () => ({
  onStageEvent: vi.fn(),
  approveCheckpoint: vi.fn(),
  requestChanges: vi.fn(),
  rejectCheckpoint: vi.fn(),
}));

import { onStageEvent, approveCheckpoint, requestChanges, rejectCheckpoint } from "../api";
import PipelineRun from "./PipelineRun";
import type { RunRecord, StageEventPayload } from "../types";

const runningRun: RunRecord = {
  runId: "run1",
  templateId: "t1",
  targetDir: "/tmp/proj",
  status: "awaiting-checkpoint",
  currentStageIndex: 0,
  stages: [
    { id: "s1", status: "awaiting-checkpoint", sessionId: "sess1", log: [] },
    { id: "s2", status: "pending", sessionId: null, log: [] },
  ],
};

beforeEach(() => {
  vi.mocked(onStageEvent).mockReset();
  vi.mocked(onStageEvent).mockResolvedValue(() => {});
  vi.mocked(approveCheckpoint).mockReset();
  vi.mocked(requestChanges).mockReset();
  vi.mocked(rejectCheckpoint).mockReset();
});

describe("PipelineRun", () => {
  it("shows the checkpoint panel when the run is awaiting checkpoint", () => {
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    expect(screen.getByRole("button", { name: "승인" })).toBeInTheDocument();
  });

  it("appends incoming stage events to the live log", async () => {
    let capturedHandler: (payload: StageEventPayload) => void = () => {};
    vi.mocked(onStageEvent).mockImplementation((handler) => {
      capturedHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    capturedHandler({ runId: "run1", stageId: "s1", event: { kind: "assistantText", text: "작업 중입니다" } });
    expect(await screen.findByText("작업 중입니다")).toBeInTheDocument();
  });

  it("calls approveCheckpoint and updates run state on approve click", async () => {
    const updated: RunRecord = { ...runningRun, status: "completed", currentStageIndex: 1 };
    vi.mocked(approveCheckpoint).mockResolvedValue(updated);
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "승인" }));
    await waitFor(() => expect(approveCheckpoint).toHaveBeenCalledWith("run1"));
    expect(await screen.findByText(/completed/)).toBeInTheDocument();
  });

  it("submits feedback via requestChanges", async () => {
    const updated: RunRecord = { ...runningRun };
    vi.mocked(requestChanges).mockResolvedValue(updated);
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("수정 요청 내용"), { target: { value: "더 자세히 써줘" } });
    fireEvent.click(screen.getByRole("button", { name: "수정 요청 보내기" }));
    await waitFor(() => expect(requestChanges).toHaveBeenCalledWith("run1", "더 자세히 써줘"));
  });

  it("calls rejectCheckpoint and onFinished when rejected", async () => {
    const onFinished = vi.fn();
    const cancelled: RunRecord = { ...runningRun, status: "cancelled" };
    vi.mocked(rejectCheckpoint).mockResolvedValue(cancelled);
    render(<PipelineRun initialRun={runningRun} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "거부" }));
    await waitFor(() => expect(rejectCheckpoint).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });
});
