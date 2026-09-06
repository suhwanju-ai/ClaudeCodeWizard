import { useState } from "react";
import { saveTemplate } from "../api";
import type { PermissionMode, Stage, Template } from "../types";

interface Props {
  initial: Template | null;
  onSaved: (template: Template) => void;
  onCancel: () => void;
}

const PERMISSION_MODES: PermissionMode[] = ["acceptEdits", "bypassPermissions", "default"];
const KNOWN_TOOLS = ["Read", "Write", "Edit", "Glob", "Grep", "Bash", "WebSearch", "WebFetch"];

function emptyStage(index: number): Stage {
  return {
    id: `stage-${index}-${crypto.randomUUID()}`,
    name: `단계 ${index + 1}`,
    prompt: "",
    permissionMode: "acceptEdits",
    allowedTools: [],
    checkpoint: true,
  };
}

function validate(template: Template): string | null {
  if (template.stages.length === 0) return "단계가 최소 1개 필요합니다.";
  const ids = new Set<string>();
  for (const stage of template.stages) {
    if (stage.prompt.trim() === "") return `단계 '${stage.name}'의 프롬프트가 비어 있습니다.`;
    if (ids.has(stage.id)) return `단계 id '${stage.id}'가 중복되었습니다.`;
    ids.add(stage.id);
  }
  return null;
}

export default function TemplateEditor({ initial, onSaved, onCancel }: Props) {
  const [template, setTemplate] = useState<Template>(
    initial ?? { id: `template-${Date.now()}`, name: "", description: "", stages: [] }
  );
  const [saveError, setSaveError] = useState<string | null>(null);

  const error = validate(template);

  const updateStage = (index: number, patch: Partial<Stage>) => {
    setTemplate((t) => ({
      ...t,
      stages: t.stages.map((s, i) => (i === index ? { ...s, ...patch } : s)),
    }));
  };

  const addStage = () => {
    setTemplate((t) => ({ ...t, stages: [...t.stages, emptyStage(t.stages.length)] }));
  };

  const removeStage = (index: number) => {
    setTemplate((t) => ({ ...t, stages: t.stages.filter((_, i) => i !== index) }));
  };

  const moveStage = (index: number, direction: -1 | 1) => {
    setTemplate((t) => {
      const target = index + direction;
      if (target < 0 || target >= t.stages.length) return t;
      const stages = [...t.stages];
      [stages[index], stages[target]] = [stages[target], stages[index]];
      return { ...t, stages };
    });
  };

  const handleSave = async () => {
    if (error) return;
    setSaveError(null);
    try {
      await saveTemplate(template);
      onSaved(template);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  return (
    <div>
      {saveError && <p role="alert">{saveError}</p>}
      <label>
        템플릿 이름
        <input value={template.name} onChange={(e) => setTemplate((t) => ({ ...t, name: e.target.value }))} />
      </label>
      <label>
        설명
        <input
          value={template.description}
          onChange={(e) => setTemplate((t) => ({ ...t, description: e.target.value }))}
        />
      </label>

      {template.stages.map((stage, index) => (
        <fieldset key={stage.id}>
          <legend>{stage.name}</legend>
          <label htmlFor={`stage-name-${index}`}>단계 {index + 1} 이름</label>
          <input
            id={`stage-name-${index}`}
            value={stage.name}
            onChange={(e) => updateStage(index, { name: e.target.value })}
          />
          <label htmlFor={`stage-prompt-${index}`}>단계 {index + 1} 프롬프트</label>
          <textarea
            id={`stage-prompt-${index}`}
            aria-label={`단계 ${index + 1} 프롬프트`}
            value={stage.prompt}
            onChange={(e) => updateStage(index, { prompt: e.target.value })}
          />
          <label htmlFor={`stage-mode-${index}`}>권한 모드</label>
          <select
            id={`stage-mode-${index}`}
            value={stage.permissionMode}
            onChange={(e) => updateStage(index, { permissionMode: e.target.value as PermissionMode })}
          >
            {PERMISSION_MODES.map((mode) => (
              <option key={mode} value={mode}>
                {mode}
              </option>
            ))}
          </select>
          {KNOWN_TOOLS.map((tool) => (
            <label key={tool}>
              <input
                type="checkbox"
                checked={stage.allowedTools.includes(tool)}
                onChange={(e) => {
                  const allowedTools = e.target.checked
                    ? [...stage.allowedTools, tool]
                    : stage.allowedTools.filter((t) => t !== tool);
                  updateStage(index, { allowedTools });
                }}
              />
              {tool}
            </label>
          ))}
          <label>
            체크포인트
            <input
              type="checkbox"
              checked={stage.checkpoint}
              onChange={(e) => updateStage(index, { checkpoint: e.target.checked })}
            />
          </label>
          <button onClick={() => moveStage(index, -1)}>위로</button>
          <button onClick={() => moveStage(index, 1)}>아래로</button>
          <button onClick={() => removeStage(index)}>단계 삭제</button>
        </fieldset>
      ))}

      <button onClick={addStage}>단계 추가</button>
      {error && <p role="alert">{error}</p>}
      <button onClick={handleSave} disabled={!!error}>
        저장
      </button>
      <button onClick={onCancel}>취소</button>
    </div>
  );
}
