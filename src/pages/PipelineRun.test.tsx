import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    onStageEvent: vi.fn(),
    approveCheckpoint: vi.fn(),
    requestChanges: vi.fn(),
    rejectCheckpoint: vi.fn(),
    startStage: vi.fn(),
    cancelRun: vi.fn(),
    getRun: vi.fn(),
    // The real predicate — the point of F-E6 is that the component uses it correctly.
    isStaleStageIndexError: actual.isStaleStageIndexError,
    STALE_STAGE_INDEX_PREFIX: actual.STALE_STAGE_INDEX_PREFIX,
  };
});

import { onStageEvent, approveCheckpoint, requestChanges, rejectCheckpoint, startStage, cancelRun, getRun } from "../api";
import PipelineRun from "./PipelineRun";
import type { RunRecord, Stage, StageEventPayload, Template } from "../types";

const stages: Stage[] = [
  { id: "s1", name: "요구사항 정리", prompt: "p1", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
  { id: "s2", name: "구현", prompt: "p2", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
];

const sampleTemplate: Template = { id: "t1", name: "웹 프로그램 개발", description: "desc", stages };

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
  resolvedStages: stages,
};

const gateRun: RunRecord = {
  ...runningRun,
  status: "awaiting-stage-start",
  currentStageIndex: 0,
  stages: [
    { id: "s1", status: "awaiting-start", sessionId: null, log: [] },
    { id: "s2", status: "pending", sessionId: null, log: [] },
  ],
};

beforeEach(() => {
  vi.mocked(onStageEvent).mockReset().mockResolvedValue(() => {});
  vi.mocked(approveCheckpoint).mockReset();
  vi.mocked(requestChanges).mockReset();
  vi.mocked(rejectCheckpoint).mockReset();
  vi.mocked(startStage).mockReset();
  vi.mocked(cancelRun).mockReset();
  vi.mocked(getRun).mockReset();
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

  // F-D1 / PRD D-2
  it("offers both 실행 and 취소 at the pre-stage gate, never just one action", () => {
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "이 run 취소" })).toBeInTheDocument();
  });

  // F-E4a / PRD E-4
  it("pre-fills the gate panel from resolvedStages with the id shown but disabled", () => {
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("p1");
    expect(screen.getByLabelText("단계 이름")).toHaveValue("요구사항 정리");
    const idInput = screen.getByLabelText("단계 ID");
    expect(idInput).toHaveValue("s1");
    expect(idInput).toBeDisabled();
  });

  it("sends the edited draft and the rendered stage index to startStage", async () => {
    vi.mocked(startStage).mockResolvedValue({ ...gateRun, status: "awaiting-checkpoint" });
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("이 단계 프롬프트"), { target: { value: "이 과제에 맞게 수정한 프롬프트" } });
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    await waitFor(() =>
      expect(startStage).toHaveBeenCalledWith(
        "run1",
        0,
        expect.objectContaining({ id: "s1", prompt: "이 과제에 맞게 수정한 프롬프트" })
      )
    );
  });

  // F-E3 / PRD E-3
  it("replaces the draft with the next stage's values when the index advances", async () => {
    const advanced: RunRecord = {
      ...gateRun,
      currentStageIndex: 1,
      stages: [
        { id: "s1", status: "approved", sessionId: "sess1", log: [] },
        { id: "s2", status: "awaiting-start", sessionId: null, log: [] },
      ],
    };
    vi.mocked(startStage).mockResolvedValue(advanced);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("이 단계 프롬프트"), { target: { value: "이전 단계 초안" } });
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    // The stale draft must not survive the advance.
    await waitFor(() => expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("p2"));
    expect(screen.getByLabelText("단계 ID")).toHaveValue("s2");
  });

  // F-D4 — dec3360 double-click guard
  it("disables the gate buttons while startStage is in flight", async () => {
    let resolveStart: (value: RunRecord) => void = () => {};
    vi.mocked(startStage).mockImplementation(() => new Promise((resolve) => { resolveStart = resolve; }));
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "이 run 취소" })).toBeDisabled();

    resolveStart({ ...gateRun, status: "awaiting-checkpoint" });
    await waitFor(() => expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument());
  });

  // F-E6 / TRD 3.10 — a stale index refreshes the screen instead of showing an error
  it("silently refetches the run when startStage rejects with a stale stage index", async () => {
    vi.mocked(startStage).mockRejectedValue("STALE_STAGE_INDEX: run is at stage 1, caller expected 0");
    const fresh: RunRecord = {
      ...gateRun,
      currentStageIndex: 1,
      stages: [
        { id: "s1", status: "approved", sessionId: "sess1", log: [] },
        { id: "s2", status: "awaiting-start", sessionId: null, log: [] },
      ],
    };
    vi.mocked(getRun).mockResolvedValue(fresh);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    await waitFor(() => expect(getRun).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("p2"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows an error for any startStage failure that is not a stale stage index", async () => {
    vi.mocked(startStage).mockRejectedValue("run 'run1' is not awaiting a stage start");
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("not awaiting a stage start");
    expect(getRun).not.toHaveBeenCalled();
  });

  // T-D1's frontend half / PRD D-1
  it("cancels the run and returns to the gallery", async () => {
    const onFinished = vi.fn();
    vi.mocked(cancelRun).mockResolvedValue({ ...gateRun, status: "cancelled" });
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={onFinished} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 run 취소" }));

    await waitFor(() => expect(cancelRun).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });

  it("hides the gate panel when the run is not at a gate", () => {
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "승인" })).toBeInTheDocument();
  });

  // GAP-3 decision D1 / PRD D-3 — Failed is terminal and must say so
  it("states that a failed run is final and offers no retry", () => {
    const failedRun: RunRecord = { ...runningRun, status: "failed" };
    render(<PipelineRun initialRun={failedRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByText(/이 run은 실패로 종료되었습니다/)).toBeInTheDocument();
    expect(screen.getByText(/새 run을 시작하세요/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument();
  });

  // PRD C-5 (IMP-013 decision D3) — the two edit paths must be distinguishable
  it("warns before sending the user to the original template editor", () => {
    const onEditTemplate = vi.fn();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={onEditTemplate} />);

    fireEvent.click(screen.getByRole("button", { name: "원본 템플릿 편집 (모든 향후 실행에 적용)" }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining("저장된 원본 템플릿"));
    expect(onEditTemplate).toHaveBeenCalledWith(sampleTemplate);
    confirmSpy.mockRestore();
  });

  it("does not navigate to the original template editor when the warning is declined", () => {
    const onEditTemplate = vi.fn();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={onEditTemplate} />);

    fireEvent.click(screen.getByRole("button", { name: "원본 템플릿 편집 (모든 향후 실행에 적용)" }));

    expect(onEditTemplate).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it("prefers the run's resolved stage names over the template's", () => {
    const renamed: RunRecord = {
      ...gateRun,
      resolvedStages: [{ ...stages[0], name: "이 run에서만 바꾼 이름" }, stages[1]],
    };
    render(<PipelineRun initialRun={renamed} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByText("이 run에서만 바꾼 이름: awaiting-start")).toBeInTheDocument();
  });

  // F-G12a — PRD G-12/G-16. The strip reuses the existing .segmented visual language.
  it("renders a tab strip with 실행 selected by default", () => {
    const { container } = render(
      <PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />
    );
    const strip = container.querySelector(".segmented");
    expect(strip).toBeInTheDocument();
    const runTab = screen.getByRole("tab", { name: "실행" });
    const filesTab = screen.getByRole("tab", { name: "파일" });
    expect(runTab).toHaveClass("segmented__option--selected");
    expect(filesTab).not.toHaveClass("segmented__option--selected");
    expect(runTab).toHaveAttribute("aria-selected", "true");
  });

  // F-G12b — PRD G-7/G-12. The representative regression assertion: the existing blocks
  // are queried exactly as the 23 pre-tab tests query them (TRD 9.11-(3)).
  it("keeps every existing run-screen block visible on the default tab", () => {
    render(
      <PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />
    );
    expect(screen.getByText("상태: awaiting-checkpoint")).toBeInTheDocument();
    expect(screen.getByText("아직 로그가 없습니다.")).toBeInTheDocument();
    expect(screen.getByText("체크포인트 — 계속 진행할까요?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "승인" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "수정 요청 보내기" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "거부" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "원본 템플릿 편집 (모든 향후 실행에 적용)" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "갤러리로 돌아가기" })).toBeInTheDocument();
  });

  // F-G12c — PRD G-12: the 240px left column lives outside the tabs.
  it("keeps the left column visible on the 파일 tab", () => {
    render(
      <PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />
    );
    fireEvent.click(screen.getByRole("tab", { name: "파일" }));
    expect(screen.getByText("웹 프로그램 개발")).toBeInTheDocument();
    expect(screen.getByText("/tmp/proj")).toBeInTheDocument();
    expect(screen.getByText("요구사항 정리: awaiting-checkpoint")).toBeInTheDocument();
    // And the run-tab content is gone.
    expect(screen.queryByRole("button", { name: "승인" })).not.toBeInTheDocument();
  });

  // F-G23 — TRD 9.1-(2)(c). The error alert lives in the 실행 tab, so a failure raised
  // while the user is on 파일 would otherwise be an invisible regression.
  it("returns to the 실행 tab when an action fails while the 파일 tab is open", async () => {
    // The rejection is held open on purpose: the user has to be sitting on the 파일 tab
    // at the moment it lands, which is the whole scenario.
    let rejectApprove: (reason: unknown) => void = () => {};
    vi.mocked(approveCheckpoint).mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectApprove = reject;
      })
    );
    render(
      <PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />
    );
    fireEvent.click(screen.getByRole("button", { name: "승인" }));
    fireEvent.click(screen.getByRole("tab", { name: "파일" }));
    expect(screen.getByRole("tab", { name: "파일" })).toHaveClass("segmented__option--selected");

    await act(async () => {
      rejectApprove("boom");
    });

    expect(screen.getByRole("alert")).toHaveTextContent("boom");
    expect(screen.getByRole("tab", { name: "실행" })).toHaveClass("segmented__option--selected");
  });
});
