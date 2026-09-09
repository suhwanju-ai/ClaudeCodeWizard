import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("../api", () => ({ saveTemplate: vi.fn() }));

import { saveTemplate } from "../api";
import TemplateEditor from "./TemplateEditor";
import type { Template } from "../types";

beforeEach(() => {
  vi.mocked(saveTemplate).mockReset();
  vi.mocked(saveTemplate).mockResolvedValue(undefined);
});

const existing: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "desc",
  stages: [
    {
      id: "s1",
      name: "요구사항",
      prompt: "PRD.md 작성",
      permissionMode: "acceptEdits",
      allowedTools: ["Read"],
      checkpoint: true,
    },
  ],
};

describe("TemplateEditor", () => {
  it("disables save when a stage prompt is empty", () => {
    render(<TemplateEditor initial={existing} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("단계 1 프롬프트"), { target: { value: "" } });
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();
  });

  it("disables save when there are no stages", () => {
    render(<TemplateEditor initial={null} onSaved={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();
  });

  it("saves the edited template and calls onSaved", async () => {
    const onSaved = vi.fn();
    render(<TemplateEditor initial={existing} onSaved={onSaved} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("템플릿 이름"), { target: { value: "웹 프로그램 개발 v2" } });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() =>
      expect(saveTemplate).toHaveBeenCalledWith(expect.objectContaining({ name: "웹 프로그램 개발 v2" }))
    );
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
  });

  it("adds a new stage when '단계 추가' is clicked", () => {
    render(<TemplateEditor initial={null} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "단계 추가" }));
    expect(screen.getByLabelText("단계 1 프롬프트")).toBeInTheDocument();
  });

  it("shows an error when saveTemplate rejects", async () => {
    vi.mocked(saveTemplate).mockRejectedValue(new Error("disk full"));
    render(<TemplateEditor initial={existing} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
  });

  it("does not call onSaved when saveTemplate rejects", async () => {
    vi.mocked(saveTemplate).mockRejectedValue(new Error("disk full"));
    const onSaved = vi.fn();
    render(<TemplateEditor initial={existing} onSaved={onSaved} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(onSaved).not.toHaveBeenCalled();
  });

  // IMP-012 (decision D2) / PRD C-1: the editor is not a run entry point any more, so
  // saving can never happen as a side effect of running.
  it("offers no 실행 button", () => {
    render(<TemplateEditor initial={existing} onSaved={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "실행" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "저장" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "취소" })).toBeInTheDocument();
  });
});
