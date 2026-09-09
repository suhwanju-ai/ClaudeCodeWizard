import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    listProjectDir: vi.fn(),
    // The real predicates — the point is that the component uses them correctly.
    isPathNotFoundError: actual.isPathNotFoundError,
    isPathOutsideTargetDirError: actual.isPathOutsideTargetDirError,
  };
});

import { listProjectDir } from "../api";
import FileBrowser, { isChangedFile } from "./FileBrowser";
import type { DirListing, RunRecord } from "../types";

const run: RunRecord = {
  runId: "run1",
  templateId: "t1",
  targetDir: "/tmp/proj",
  status: "awaiting-checkpoint",
  currentStageIndex: 0,
  stages: [],
  resolvedStages: [],
};

const rootListing: DirListing = {
  path: "",
  entries: [
    { name: "src", kind: "directory", size: null, modifiedMs: null },
    { name: "main.rs", kind: "file", size: 2100, modifiedMs: 1757400000000 },
  ],
};

const srcListing: DirListing = {
  path: "src",
  entries: [{ name: "engine", kind: "directory", size: null, modifiedMs: null }],
};

const engineListing: DirListing = { path: "src/engine", entries: [] };

beforeEach(() => {
  vi.mocked(listProjectDir).mockReset();
});

// F-G17b — TRD 9.7-(2). changedFiles comes from CLI events and may be absolute or
// relative, with either separator, so the matching rule is a pure function with tests.
describe("isChangedFile", () => {
  it("matches an exact relative path", () => {
    expect(isChangedFile(["src/main.rs"], "src/main.rs")).toBe(true);
  });
  it("matches an absolute path by suffix", () => {
    expect(isChangedFile(["/tmp/proj/src/main.rs"], "src/main.rs")).toBe(true);
  });
  it("matches across separator styles", () => {
    expect(isChangedFile(["C:\\tmp\\proj\\src\\main.rs"], "src/main.rs")).toBe(true);
  });
  it("strips a leading ./", () => {
    expect(isChangedFile(["./src/main.rs"], "src/main.rs")).toBe(true);
  });
  it("does not match a different file whose name merely ends similarly", () => {
    expect(isChangedFile(["/tmp/proj/other/xmain.rs"], "main.rs")).toBe(false);
  });
  it("returns false for an empty list", () => {
    expect(isChangedFile([], "src/main.rs")).toBe(false);
  });
});

describe("FileBrowser", () => {
  // F-G-nav — TRD 9.13: '..' is never sent to the command; the frontend truncates.
  it("navigates into a folder and back up without ever sending '..'", async () => {
    vi.mocked(listProjectDir)
      .mockResolvedValueOnce(rootListing)
      .mockResolvedValueOnce(srcListing)
      .mockResolvedValueOnce(engineListing)
      .mockResolvedValueOnce(srcListing);

    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    await waitFor(() => expect(listProjectDir).toHaveBeenCalledWith("run1", ""));

    fireEvent.click(await screen.findByRole("button", { name: "src" }));
    await waitFor(() => expect(listProjectDir).toHaveBeenCalledWith("run1", "src"));

    fireEvent.click(await screen.findByRole("button", { name: "engine" }));
    await waitFor(() => expect(listProjectDir).toHaveBeenCalledWith("run1", "src/engine"));

    fireEvent.click(screen.getByRole("button", { name: "상위로" }));
    await waitFor(() => expect(listProjectDir).toHaveBeenLastCalledWith("run1", "src"));

    for (const call of vi.mocked(listProjectDir).mock.calls) {
      expect(call[1]).not.toContain("..");
    }
  });

  it("disables 상위로 at the root", async () => {
    vi.mocked(listProjectDir).mockResolvedValue(rootListing);
    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    expect(await screen.findByRole("button", { name: "상위로" })).toBeDisabled();
  });

  // F-G-err — PRD G-20 / TRD 9.10-(1). Each prefix gets its own recovery.
  it("falls back to the parent and offers a retry when the folder disappeared", async () => {
    vi.mocked(listProjectDir)
      .mockResolvedValueOnce(rootListing)
      .mockResolvedValueOnce(srcListing)
      .mockRejectedValueOnce("PATH_NOT_FOUND: src/engine")
      .mockResolvedValueOnce(srcListing);

    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    fireEvent.click(await screen.findByRole("button", { name: "src" }));
    fireEvent.click(await screen.findByRole("button", { name: "engine" }));

    expect(await screen.findByText("이 폴더는 더 이상 존재하지 않습니다. 상위 폴더로 돌아갔습니다.")).toBeInTheDocument();
    await waitFor(() => expect(listProjectDir).toHaveBeenLastCalledWith("run1", "src"));
  });

  it("falls back to the root with a boundary notice when the path was outside targetDir", async () => {
    vi.mocked(listProjectDir)
      .mockResolvedValueOnce(rootListing)
      .mockRejectedValueOnce("PATH_OUTSIDE_TARGET_DIR: ../secret.txt")
      .mockResolvedValueOnce(rootListing);

    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    fireEvent.click(await screen.findByRole("button", { name: "src" }));

    expect(await screen.findByText("이 run의 폴더 밖은 볼 수 없습니다.")).toBeInTheDocument();
    // The raw path is not echoed back to the user.
    expect(screen.queryByText(/secret\.txt/)).not.toBeInTheDocument();
    await waitFor(() => expect(listProjectDir).toHaveBeenLastCalledWith("run1", ""));
  });

  it("shows any other listing failure in an alert", async () => {
    vi.mocked(listProjectDir).mockRejectedValue("io error: disk on fire");
    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("io error: disk on fire");
  });

  it("clears the old listing on a generic error instead of showing stale entries under the new breadcrumb", async () => {
    vi.mocked(listProjectDir)
      .mockResolvedValueOnce(rootListing)
      .mockRejectedValueOnce("io error: permission denied");

    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    await screen.findByRole("button", { name: "src" });
    fireEvent.click(screen.getByRole("button", { name: "src" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("io error: permission denied");
    expect(screen.queryByRole("button", { name: "src" })).not.toBeInTheDocument();
    expect(screen.queryByText("main.rs")).not.toBeInTheDocument();
  });

  it("marks entries that the run log says were changed", async () => {
    vi.mocked(listProjectDir).mockResolvedValue(rootListing);
    render(<FileBrowser run={run} confirmed changedFiles={["/tmp/proj/main.rs"]} />);
    expect(await screen.findByText("변경됨")).toBeInTheDocument();
  });

  it("says the folder is empty rather than showing nothing", async () => {
    vi.mocked(listProjectDir).mockResolvedValue(engineListing);
    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    expect(await screen.findByText("이 폴더는 비어 있습니다.")).toBeInTheDocument();
  });

  it("re-fetches the current path when 새로고침 is clicked", async () => {
    vi.mocked(listProjectDir).mockResolvedValue(rootListing);
    render(<FileBrowser run={run} confirmed changedFiles={[]} />);
    await waitFor(() => expect(listProjectDir).toHaveBeenCalledWith("run1", ""));

    fireEvent.click(await screen.findByRole("button", { name: "새로고침" }));
    await waitFor(() => expect(listProjectDir).toHaveBeenCalledTimes(2));
    expect(vi.mocked(listProjectDir).mock.calls[1]).toEqual(["run1", ""]);
  });

  // F-G14 (component half) — IMP-034. Nothing is queried before the record is confirmed.
  it("does not call the command before the run is confirmed", () => {
    render(<FileBrowser run={run} confirmed={false} changedFiles={[]} />);
    expect(listProjectDir).not.toHaveBeenCalled();
    expect(
      screen.getByText("실행을 준비하는 중입니다 — 폴더를 확인한 뒤 목록을 불러옵니다.")
    ).toBeInTheDocument();
  });
});
