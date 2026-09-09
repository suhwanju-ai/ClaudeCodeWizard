import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("./api", async () => {
  const actual = await vi.importActual<typeof import("./api")>("./api");
  return {
    listTemplates: vi.fn(),
    deleteTemplate: vi.fn(),
    checkCli: vi.fn(),
    saveTemplate: vi.fn(),
    startPipelineRun: vi.fn(),
    onStageEvent: vi.fn(),
    approveCheckpoint: vi.fn(),
    requestChanges: vi.fn(),
    rejectCheckpoint: vi.fn(),
    startStage: vi.fn(),
    cancelRun: vi.fn(),
    getRun: vi.fn(),
    // The real predicate and prefix — App never calls them, but PipelineRun does.
    isStaleStageIndexError: actual.isStaleStageIndexError,
    STALE_STAGE_INDEX_PREFIX: actual.STALE_STAGE_INDEX_PREFIX,
    listProjectDir: vi.fn(),
    isPathNotFoundError: actual.isPathNotFoundError,
    isPathOutsideTargetDirError: actual.isPathOutsideTargetDirError,
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import {
  listTemplates,
  checkCli,
  saveTemplate,
  startPipelineRun,
  onStageEvent,
  startStage,
  listProjectDir,
} from "./api";
import { open } from "@tauri-apps/plugin-dialog";
import App from "./App";
import type { RunRecord, Template } from "./types";

const sample: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "desc",
  stages: [
    {
      id: "s1",
      name: "요구사항",
      prompt: "PRD 작성",
      permissionMode: "acceptEdits",
      allowedTools: [],
      checkpoint: true,
    },
  ],
};

/** What the backend answers with once start_pipeline_run resolves. */
const backendRun: RunRecord = {
  runId: "run1",
  templateId: "t1",
  targetDir: "/tmp/new-project",
  status: "awaiting-stage-start",
  currentStageIndex: 0,
  stages: [{ id: "s1", status: "awaiting-start", sessionId: null, log: [] }],
  // Renamed on purpose, so a test can watch the optimistic record be replaced by the
  // authoritative one rather than just matching itself.
  resolvedStages: [{ ...sample.stages[0], name: "백엔드가 확정한 요구사항" }],
};

beforeEach(() => {
  vi.mocked(listTemplates).mockReset().mockResolvedValue([sample]);
  vi.mocked(checkCli).mockReset().mockResolvedValue("available:mock-claude 0.0.1");
  vi.mocked(saveTemplate).mockReset().mockResolvedValue(undefined);
  vi.mocked(onStageEvent).mockReset().mockResolvedValue(() => {});
  vi.mocked(startStage).mockReset();
  vi.mocked(open).mockReset();
  vi.mocked(startPipelineRun).mockReset();
});

describe("App routing", () => {
  it("starts on the template gallery", async () => {
    render(<App />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  // IMP-012 (decision D2): running starts at the gallery card, not in the editor.
  it("picks a target folder and starts a run when 실행 is clicked on a gallery card", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    let resolveStart: (value: RunRecord) => void = () => {};
    vi.mocked(startPipelineRun).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveStart = resolve;
        })
    );

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    await waitFor(() => expect(open).toHaveBeenCalledWith({ directory: true }));
    await waitFor(() =>
      expect(startPipelineRun).toHaveBeenCalledWith("t1", "/tmp/new-project", expect.any(String))
    );

    // The run view mounts on the optimistic record before startPipelineRun resolves, so
    // the stage-event listener is live from the very first stage — and that record must
    // already describe the gate, not a running stage (PRD A-1).
    expect(await screen.findByText("상태: awaiting-stage-start")).toBeInTheDocument();

    // ...and is then replaced by the authoritative record the backend created.
    resolveStart(backendRun);
    expect(await screen.findByText("백엔드가 확정한 요구사항: awaiting-start")).toBeInTheDocument();
  });

  // PRD C-1: no write to the saved template anywhere on the path into a run.
  it("never saves the template on the way to a run", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(startPipelineRun).mockResolvedValue(backendRun);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    await waitFor(() => expect(startPipelineRun).toHaveBeenCalled());
    expect(saveTemplate).not.toHaveBeenCalled();
  });

  // The optimistic record must carry resolvedStages, or the gate panel has nothing to
  // render and the user stares at an empty screen until the backend answers.
  it("renders the first stage's gate panel from the optimistic record", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(startPipelineRun).mockImplementation(() => new Promise(() => {}));

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    expect(await screen.findByLabelText("이 단계 프롬프트")).toHaveValue("PRD 작성");
    expect(screen.getByLabelText("단계 ID")).toHaveValue("s1");
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "이 run 취소" })).toBeInTheDocument();
  });

  it("stays on the gallery when the folder dialog is dismissed", async () => {
    vi.mocked(open).mockResolvedValue(null);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    await waitFor(() => expect(open).toHaveBeenCalled());
    expect(startPipelineRun).not.toHaveBeenCalled();
    expect(screen.getByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  it("shows an error and returns to the gallery when startPipelineRun rejects", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(startPipelineRun).mockRejectedValue(new Error("target dir already contains a pipeline run"));

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("target dir already contains a pipeline run");
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  it("opens a pure editor from 편집 — no run button, no folder dialog", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "편집" }));

    expect(await screen.findByRole("button", { name: "저장" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "실행" })).not.toBeInTheDocument();
    expect(open).not.toHaveBeenCalled();
  });

  // F-G-app — IMP-034: the optimistic record is not confirmed, so the file tab must not
  // query it; once startPipelineRun answers, it may.
  it("marks the run unconfirmed until startPipelineRun answers", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    vi.mocked(checkCli).mockResolvedValue("available:1.0.0");
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(listProjectDir).mockResolvedValue({ path: "", entries: [], truncated: false });
    let resolveStart: (r: RunRecord) => void = () => {};
    vi.mocked(startPipelineRun).mockReturnValue(
      new Promise<RunRecord>((resolve) => {
        resolveStart = resolve;
      })
    );

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    // The optimistic window: the file tab shows the notice and calls nothing.
    fireEvent.click(await screen.findByRole("tab", { name: "파일" }));
    expect(
      await screen.findByText("실행을 준비하는 중입니다 — 폴더를 확인한 뒤 목록을 불러옵니다.")
    ).toBeInTheDocument();
    expect(listProjectDir).not.toHaveBeenCalled();

    resolveStart(backendRun);
    await waitFor(() => expect(listProjectDir).toHaveBeenCalledWith(backendRun.runId, ""));
  });
});
