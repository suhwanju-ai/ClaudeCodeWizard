import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

vi.mock("../api", () => ({
  listTemplates: vi.fn(),
  deleteTemplate: vi.fn(),
  checkCli: vi.fn(),
}));

import { listTemplates, deleteTemplate, checkCli } from "../api";
import TemplateGallery from "./TemplateGallery";
import type { Template } from "../types";

const sample: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "요구사항부터 배포까지",
  stages: [],
};

beforeEach(() => {
  vi.mocked(listTemplates).mockReset();
  vi.mocked(deleteTemplate).mockReset();
  vi.mocked(checkCli).mockReset();
  vi.mocked(checkCli).mockResolvedValue("available:mock-claude 0.0.1");
  vi.spyOn(window, "confirm").mockReturnValue(true);
});

describe("TemplateGallery", () => {
  it("renders templates returned by listTemplates", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
    expect(screen.getByText("요구사항부터 배포까지")).toBeInTheDocument();
  });

  it("shows a warning banner when the CLI is not found", async () => {
    vi.mocked(listTemplates).mockResolvedValue([]);
    vi.mocked(checkCli).mockResolvedValue("not-found");
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    expect(await screen.findByText(/Claude Code CLI/)).toBeInTheDocument();
  });

  it("calls deleteTemplate and refetches when delete is clicked after confirmation", async () => {
    vi.mocked(listTemplates).mockResolvedValueOnce([sample]).mockResolvedValueOnce([]);
    vi.mocked(deleteTemplate).mockResolvedValue(undefined);
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    const deleteButton = await screen.findByRole("button", { name: "삭제" });
    fireEvent.click(deleteButton);
    expect(window.confirm).toHaveBeenCalled();
    await waitFor(() => expect(deleteTemplate).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(listTemplates).toHaveBeenCalledTimes(2));
  });

  it("does not delete when the confirmation is declined", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    vi.mocked(window.confirm).mockReturnValue(false);
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    const deleteButton = await screen.findByRole("button", { name: "삭제" });
    fireEvent.click(deleteButton);
    expect(deleteTemplate).not.toHaveBeenCalled();
  });

  it("shows an error when listTemplates rejects", async () => {
    vi.mocked(listTemplates).mockRejectedValue(new Error("claude CLI not found"));
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("claude CLI not found");
  });

  // IMP-012 (decision D2): the gallery is the run entry point again.
  it("runs a template straight from its gallery card", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    const onRun = vi.fn();
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={onRun} />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));
    expect(onRun).toHaveBeenCalledWith(sample);
  });

  it("offers 실행 alongside 편집 and 삭제 on every card", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    await screen.findByText("웹 프로그램 개발");
    expect(screen.getByRole("button", { name: "실행" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "편집" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "삭제" })).toBeInTheDocument();
  });
});
