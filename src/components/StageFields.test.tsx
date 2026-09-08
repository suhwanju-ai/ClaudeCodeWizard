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

function renderFields(overrides: Partial<React.ComponentProps<typeof StageFields>> = {}) {
  const onChange = vi.fn();
  render(
    <StageFields
      stage={stage}
      onChange={onChange}
      idPrefix="test-stage"
      promptLabel="이 단계 프롬프트"
      lockId={false}
      {...overrides}
    />
  );
  return onChange;
}

describe("StageFields", () => {
  it("renders every stage field", () => {
    renderFields();
    expect(screen.getByLabelText("단계 이름")).toHaveValue("요구사항");
    expect(screen.getByLabelText("단계 ID")).toHaveValue("s1");
    expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("PRD 작성");
    expect(screen.getByRole("button", { name: "체크포인트에서 일시 정지" })).toBeInTheDocument();
  });

  it("uses idPrefix for input ids so two instances can coexist", () => {
    renderFields();
    expect(screen.getByLabelText("단계 이름")).toHaveAttribute("id", "test-stage-name");
    expect(screen.getByLabelText("단계 ID")).toHaveAttribute("id", "test-stage-id");
    expect(screen.getByLabelText("이 단계 프롬프트")).toHaveAttribute("id", "test-stage-prompt");
  });

  it("emits a patch when the prompt changes", () => {
    const onChange = renderFields();
    fireEvent.change(screen.getByLabelText("이 단계 프롬프트"), { target: { value: "새 프롬프트" } });
    expect(onChange).toHaveBeenCalledWith({ prompt: "새 프롬프트" });
  });

  it("emits a patch when a permission mode is picked", () => {
    const onChange = renderFields();
    fireEvent.click(screen.getByRole("button", { name: "bypassPermissions" }));
    expect(onChange).toHaveBeenCalledWith({ permissionMode: "bypassPermissions" });
  });

  it("toggles an allowed tool on and off", () => {
    const onChange = renderFields();
    fireEvent.click(screen.getByRole("button", { name: "Grep" }));
    expect(onChange).toHaveBeenCalledWith({ allowedTools: ["Read", "Grep"] });

    onChange.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Read" }));
    expect(onChange).toHaveBeenCalledWith({ allowedTools: [] });
  });

  it("emits a patch when the checkpoint switch is flipped", () => {
    const onChange = renderFields();
    fireEvent.click(screen.getByRole("button", { name: "체크포인트에서 일시 정지" }));
    expect(onChange).toHaveBeenCalledWith({ checkpoint: false });
  });

  // PRD E-4: shown, but not editable.
  it("shows the stage id but disables it when lockId is set", () => {
    renderFields({ lockId: true });
    const idInput = screen.getByLabelText("단계 ID");
    expect(idInput).toBeInTheDocument();
    expect(idInput).toBeDisabled();
  });

  it("leaves the stage id editable when lockId is not set", () => {
    const onChange = renderFields({ lockId: false });
    const idInput = screen.getByLabelText("단계 ID");
    expect(idInput).toBeEnabled();
    fireEvent.change(idInput, { target: { value: "s1-renamed" } });
    expect(onChange).toHaveBeenCalledWith({ id: "s1-renamed" });
  });
});
