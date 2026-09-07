import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import StageFields from "./StageFields";
import type { Stage } from "../types";

const stage: Stage = {
  id: "s1",
  name: "요구사항",
  prompt: "PRD 작성",
  permissionMode: "acceptEdits",
  allowedTools: ["Read"],
  checkpoint: true,
};

describe("StageFields", () => {
  it("renders the prompt with the given label and current value", () => {
    render(<StageFields stage={stage} onChange={vi.fn()} idPrefix="stage-0" promptLabel="단계 1 프롬프트" />);
    expect(screen.getByLabelText("단계 1 프롬프트")).toHaveValue("PRD 작성");
  });

  it("calls onChange with the edited prompt", () => {
    const onChange = vi.fn();
    render(<StageFields stage={stage} onChange={onChange} idPrefix="stage-0" promptLabel="프롬프트" />);
    fireEvent.change(screen.getByLabelText("프롬프트"), { target: { value: "새 프롬프트" } });
    expect(onChange).toHaveBeenCalledWith({ prompt: "새 프롬프트" });
  });

  it("toggles an allowed tool chip", () => {
    const onChange = vi.fn();
    render(<StageFields stage={stage} onChange={onChange} idPrefix="stage-0" promptLabel="프롬프트" />);
    fireEvent.click(screen.getByRole("button", { name: "Write" }));
    expect(onChange).toHaveBeenCalledWith({ allowedTools: ["Read", "Write"] });
  });

  it("disables the id input when lockId is set", () => {
    render(<StageFields stage={stage} onChange={vi.fn()} idPrefix="stage-0" promptLabel="프롬프트" lockId />);
    expect(screen.getByLabelText("단계 ID")).toBeDisabled();
  });

  it("leaves the id input enabled by default", () => {
    render(<StageFields stage={stage} onChange={vi.fn()} idPrefix="stage-0" promptLabel="프롬프트" />);
    expect(screen.getByLabelText("단계 ID")).not.toBeDisabled();
  });
});
