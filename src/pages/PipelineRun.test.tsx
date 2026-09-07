import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

vi.mock("../api", () => ({
  onStageEvent: vi.fn(),
  approveCheckpoint: vi.fn(),
  requestChanges: vi.fn(),
  rejectCheckpoint: vi.fn(),
  startStage: vi.fn(),
}));

import { onStageEvent, approveCheckpoint, requestChanges, rejectCheckpoint, startStage } from "../api";
import PipelineRun from "./PipelineRun";
import type { RunRecord, StageEventPayload, Template } from "../types";

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
  resolvedStages: [
    { id: "s1", name: "요구사항 정리", prompt: "p1", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
    { id: "s2", name: "구현", prompt: "p2", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
  ],
};

const pendingRun: RunRecord = {
  ...runningRun,
  status: "awaiting-stage-start",
  stages: [
    { id: "s1", status: "awaiting-start", sessionId: null, log: [] },
    { id: "s2", status: "pending", sessionId: null, log: [] },
  ],
};

const sampleTemplate: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "desc",
  stages: [
    { id: "s1", name: "요구사항 정리", prompt: "p1", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
    { id: "s2", name: "구현", prompt: "p2", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
  ],
};

beforeEach(() => {
  vi.mocked(onStageEvent).mockReset();
  vi.mocked(onStageEvent).mockResolvedValue(() => {});
  vi.mocked(approveCheckpoint).mockReset();
  vi.mocked(requestChanges).mockReset();
  vi.mocked(rejectCheckpoint).mockReset();
  vi.mocked(startStage).mockReset();
});

describe("PipelineRun", () => {
  it("shows the checkpoint panel when the run is awaiting checkpoint", () => {
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByRole("button", { name: "승인" })).toBeInTheDocument();
  });

  it("appends incoming stage events to the live log", async () => {
    let capturedHandler: (payload: StageEventPayload) => void = () => {};
    vi.mocked(onStageEvent).mockImplementation((handler) => {
      capturedHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    act(() => {
      capturedHandler({ runId: "run1", stageId: "s1", event: { kind: "assistantText", text: "작업 중입니다" } });
    });
    expect(await screen.findByText("작업 중입니다")).toBeInTheDocument();
  });

  it("shows the exit code and stderr text when a stage fails with a processError event", async () => {
    let capturedHandler: (payload: StageEventPayload) => void = () => {};
    vi.mocked(onStageEvent).mockImplementation((handler) => {
      capturedHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    act(() => {
      capturedHandler({
        runId: "run1",
        stageId: "s1",
        event: { kind: "processError", exitCode: 1, stderr: "Error: unknown slash command '/does-not-exist'" },
      });
    });
    expect(await screen.findByText(/종료 코드 1/)).toBeInTheDocument();
    expect(screen.getByText(/unknown slash command/)).toBeInTheDocument();
  });

  it("calls approveCheckpoint and updates run state on approve click", async () => {
    const updated: RunRecord = { ...runningRun, status: "completed", currentStageIndex: 1 };
    vi.mocked(approveCheckpoint).mockResolvedValue(updated);
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "승인" }));
    await waitFor(() => expect(approveCheckpoint).toHaveBeenCalledWith("run1"));
    expect(await screen.findByText(/completed/)).toBeInTheDocument();
  });

  it("submits feedback via requestChanges", async () => {
    const updated: RunRecord = { ...runningRun };
    vi.mocked(requestChanges).mockResolvedValue(updated);
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("수정 요청 내용"), { target: { value: "더 자세히 써줘" } });
    fireEvent.click(screen.getByRole("button", { name: "수정 요청 보내기" }));
    await waitFor(() => expect(requestChanges).toHaveBeenCalledWith("run1", "더 자세히 써줘"));
  });

  it("calls rejectCheckpoint and onFinished when rejected", async () => {
    const onFinished = vi.fn();
    const cancelled: RunRecord = { ...runningRun, status: "cancelled" };
    vi.mocked(rejectCheckpoint).mockResolvedValue(cancelled);
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={onFinished} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "거부" }));
    await waitFor(() => expect(rejectCheckpoint).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });

  it("shows an error when approveCheckpoint rejects", async () => {
    vi.mocked(approveCheckpoint).mockRejectedValue(new Error("claude CLI not found"));
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "승인" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("claude CLI not found");
  });

  it("disables checkpoint action buttons while a request is in flight", async () => {
    let resolveApprove: (value: RunRecord) => void = () => {};
    vi.mocked(approveCheckpoint).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveApprove = resolve;
        })
    );
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "승인" }));

    expect(screen.getByRole("button", { name: "승인" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "거부" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "수정 요청 보내기" })).toBeDisabled();

    resolveApprove({ ...runningRun, status: "completed" });
    await waitFor(() => expect(screen.queryByRole("button", { name: "승인" })).not.toBeInTheDocument());
  });

  it("always renders a button back to the gallery, even on a dead-end failed run", () => {
    const failedRun: RunRecord = { ...runningRun, status: "failed" };
    const onFinished = vi.fn();
    render(<PipelineRun initialRun={failedRun} template={sampleTemplate} onFinished={onFinished} onEditTemplate={vi.fn()} />);
    const backButton = screen.getByRole("button", { name: "갤러리로 돌아가기" });
    expect(backButton).toBeInTheDocument();
    fireEvent.click(backButton);
    expect(onFinished).toHaveBeenCalled();
  });

  it("shows each stage's configured name instead of its raw id", () => {
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByText("요구사항 정리: awaiting-checkpoint")).toBeInTheDocument();
    expect(screen.getByText("구현: pending")).toBeInTheDocument();
    expect(screen.queryByText(/^s1:/)).not.toBeInTheDocument();
  });

  it("lets the user jump to the template editor from a failed run", () => {
    const failedRun: RunRecord = { ...runningRun, status: "failed" };
    const onEditTemplate = vi.fn();
    render(<PipelineRun initialRun={failedRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={onEditTemplate} />);
    fireEvent.click(screen.getByRole("button", { name: "템플릿 편집" }));
    expect(onEditTemplate).toHaveBeenCalledWith(sampleTemplate);
  });

  it("shows the pre-stage edit panel prefilled with the current stage's prompt when awaiting stage start", () => {
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByLabelText("프롬프트")).toHaveValue("p1");
  });

  it("sends the edited stage via startStage when running the stage", async () => {
    vi.mocked(startStage).mockResolvedValue({ ...pendingRun, status: "awaiting-checkpoint" });
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("프롬프트"), { target: { value: "수정된 프롬프트" } });
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    await waitFor(() =>
      expect(startStage).toHaveBeenCalledWith("run1", expect.objectContaining({ id: "s1", prompt: "수정된 프롬프트" }))
    );
  });

  it("runs the stage unedited when the user clicks run without changing anything", async () => {
    vi.mocked(startStage).mockResolvedValue({ ...pendingRun, status: "awaiting-checkpoint" });
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    await waitFor(() =>
      expect(startStage).toHaveBeenCalledWith("run1", expect.objectContaining({ id: "s1", prompt: "p1" }))
    );
  });

  it("shows an error when startStage rejects", async () => {
    vi.mocked(startStage).mockRejectedValue(new Error("claude CLI not found"));
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("claude CLI not found");
  });

  it("does not show the checkpoint approval panel while awaiting stage start", () => {
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "승인" })).not.toBeInTheDocument();
  });

  it("optimistically shows a running status and hides the pre-stage panel while the stage starts", async () => {
    let resolveStart: (value: RunRecord) => void = () => {};
    vi.mocked(startStage).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveStart = resolve;
        })
    );
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    await waitFor(() => expect(screen.getByText(/상태: running/)).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument();

    resolveStart({ ...pendingRun, status: "awaiting-checkpoint" });
    await waitFor(() => expect(screen.getByText(/상태: awaiting-checkpoint/)).toBeInTheDocument());
  });

  it("restores the pre-stage panel when startStage fails, instead of leaving a phantom running state", async () => {
    vi.mocked(startStage).mockRejectedValue(new Error("claude CLI not found"));
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("claude CLI not found");
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByText(/상태: awaiting-stage-start/)).toBeInTheDocument();
  });

  it("calls onFinished when starting the stage lands on a terminal status", async () => {
    const onFinished = vi.fn();
    vi.mocked(startStage).mockResolvedValue({ ...pendingRun, status: "completed" });
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={onFinished} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });

  it("hides the pre-stage panel instead of crashing when resolvedStages doesn't cover the current stage", () => {
    const brokenRun: RunRecord = { ...pendingRun, resolvedStages: [] };
    render(<PipelineRun initialRun={brokenRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument();
  });
});
