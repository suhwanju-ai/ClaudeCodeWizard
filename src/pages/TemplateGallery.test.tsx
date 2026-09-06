import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

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
});

describe("TemplateGallery", () => {
  it("renders templates returned by listTemplates", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    render(<TemplateGallery onRun={vi.fn()} onEdit={vi.fn()} onNew={vi.fn()} />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
    expect(screen.getByText("요구사항부터 배포까지")).toBeInTheDocument();
  });

  it("shows a warning banner when the CLI is not found", async () => {
    vi.mocked(listTemplates).mockResolvedValue([]);
    vi.mocked(checkCli).mockResolvedValue("not-found");
    render(<TemplateGallery onRun={vi.fn()} onEdit={vi.fn()} onNew={vi.fn()} />);
    expect(await screen.findByText(/Claude Code CLI/)).toBeInTheDocument();
  });

  it("calls deleteTemplate and refetches when delete is clicked", async () => {
    vi.mocked(listTemplates).mockResolvedValueOnce([sample]).mockResolvedValueOnce([]);
    vi.mocked(deleteTemplate).mockResolvedValue(undefined);
    render(<TemplateGallery onRun={vi.fn()} onEdit={vi.fn()} onNew={vi.fn()} />);
    const deleteButton = await screen.findByRole("button", { name: "삭제" });
    deleteButton.click();
    await waitFor(() => expect(deleteTemplate).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(listTemplates).toHaveBeenCalledTimes(2));
  });
});
