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
const ID_PATTERN = /^(?!\.\.?$)[A-Za-z0-9._-]+$/;

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
  if (!ID_PATTERN.test(template.id)) return "템플릿 ID는 영문/숫자/./_/- 만 사용할 수 있습니다.";
  const ids = new Set<string>();
  for (const stage of template.stages) {
    if (stage.prompt.trim() === "") return `단계 '${stage.name}'의 프롬프트가 비어 있습니다.`;
    if (!ID_PATTERN.test(stage.id)) return `단계 '${stage.name}'의 ID는 영문/숫자/./_/- 만 사용할 수 있습니다.`;
    if (ids.has(stage.id)) return `단계 id '${stage.id}'가 중복되었습니다.`;
    ids.add(stage.id);
  }
  return null;
}

export default function TemplateEditor({ initial, onSaved, onCancel }: Props) {
  const [template, setTemplate] = useState<Template>(
    initial ?? { id: `template-${Date.now()}`, name: "", description: "", stages: [] }
  );
  const [selectedIndex, setSelectedIndex] = useState<number>(template.stages.length > 0 ? 0 : -1);
  const [showJson, setShowJson] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const error = validate(template);
  const selectedStage = selectedIndex >= 0 ? template.stages[selectedIndex] : null;

  const updateStage = (index: number, patch: Partial<Stage>) => {
    setTemplate((t) => ({
      ...t,
      stages: t.stages.map((s, i) => (i === index ? { ...s, ...patch } : s)),
    }));
  };

  const addStage = () => {
    const newIndex = template.stages.length;
    setTemplate((t) => ({ ...t, stages: [...t.stages, emptyStage(t.stages.length)] }));
    setSelectedIndex(newIndex);
  };

  const removeStage = (index: number) => {
    setTemplate((t) => ({ ...t, stages: t.stages.filter((_, i) => i !== index) }));
    setSelectedIndex((prev) => {
      const remaining = template.stages.length - 1;
      if (remaining <= 0) return -1;
      if (index < prev) return prev - 1;
      if (index === prev) return Math.min(prev, remaining - 1);
      return prev;
    });
  };

  const moveStage = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    setTemplate((t) => {
      if (target < 0 || target >= t.stages.length) return t;
      const stages = [...t.stages];
      [stages[index], stages[target]] = [stages[target], stages[index]];
      return { ...t, stages };
    });
    setSelectedIndex((prev) => {
      if (target < 0 || target >= template.stages.length) return prev;
      if (prev === index) return target;
      if (prev === target) return index;
      return prev;
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
      {saveError && (
        <p className="alert" role="alert">
          {saveError}
        </p>
      )}

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 16 }}>
        <div style={{ flex: 1, marginRight: 20, display: "flex", flexDirection: "column", gap: 8 }}>
          <label className="field">
            <span className="section-label">템플릿 이름</span>
            <input
              className="input"
              style={{ fontSize: "16px", fontWeight: 600 }}
              value={template.name}
              onChange={(e) => setTemplate((t) => ({ ...t, name: e.target.value }))}
            />
          </label>
          <label className="field">
            <span className="section-label">설명</span>
            <input
              className="input"
              value={template.description}
              onChange={(e) => setTemplate((t) => ({ ...t, description: e.target.value }))}
            />
          </label>
        </div>
        <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
          <button className="btn btn-outline" onClick={() => setShowJson((s) => !s)}>
            {"{ } JSON"}
          </button>
          <button className="btn btn-outline" onClick={onCancel}>
            취소
          </button>
          <button className="btn btn-primary" onClick={handleSave} disabled={!!error}>
            저장
          </button>
        </div>
      </div>

      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}

      {showJson && (
        <div className="card" style={{ marginBottom: 16 }}>
          <pre className="mono" style={{ margin: 0, whiteSpace: "pre-wrap", fontSize: "11.5px", color: "var(--text-secondary)" }}>
            {JSON.stringify(template, null, 2)}
          </pre>
        </div>
      )}

      <div style={{ display: "flex", gap: 20, alignItems: "flex-start" }}>
        <div style={{ width: 220, flexShrink: 0 }}>
          <div className="section-label">단계 목록</div>
          <div className="stage-list">
            {template.stages.map((stage, index) => (
              <button
                key={stage.id}
                className={`stage-list__row ${index === selectedIndex ? "stage-list__row--selected" : ""}`}
                onClick={() => setSelectedIndex(index)}
              >
                <span className="stage-list__num">{String(index + 1).padStart(2, "0")}</span>
                <span>{stage.name}</span>
                <span className={`stage-list__dot ${stage.checkpoint ? "" : "stage-list__dot--off"}`} />
              </button>
            ))}
          </div>
          <button className="btn btn-outline" style={{ marginTop: 8, width: "100%" }} onClick={addStage}>
            단계 추가
          </button>
        </div>

        <div style={{ flex: 1, minWidth: 0 }}>
          {selectedStage ? (
            <div className="card">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 14 }}>
                <div className="section-label" style={{ margin: 0 }}>
                  단계 편집 · {selectedStage.name}
                </div>
                <button className="btn btn-ghost" onClick={() => removeStage(selectedIndex)}>
                  단계 삭제
                </button>
              </div>

              <div style={{ display: "flex", gap: 12, marginBottom: 12 }}>
                <label className="field" style={{ flex: 1 }}>
                  <span>단계 이름</span>
                  <input
                    id={`stage-name-${selectedIndex}`}
                    className="input"
                    value={selectedStage.name}
                    onChange={(e) => updateStage(selectedIndex, { name: e.target.value })}
                  />
                </label>
                <label className="field" style={{ flex: 1 }}>
                  <span>단계 ID</span>
                  <input
                    id={`stage-id-${selectedIndex}`}
                    className="input mono"
                    value={selectedStage.id}
                    onChange={(e) => updateStage(selectedIndex, { id: e.target.value })}
                  />
                </label>
              </div>

              <div className="field" style={{ marginBottom: 12 }}>
                <label htmlFor={`stage-prompt-${selectedIndex}`}>단계 {selectedIndex + 1} 프롬프트</label>
                <textarea
                  id={`stage-prompt-${selectedIndex}`}
                  className="textarea"
                  rows={4}
                  value={selectedStage.prompt}
                  onChange={(e) => updateStage(selectedIndex, { prompt: e.target.value })}
                />
                <span className="help-text">스킬/에이전트는 프롬프트에서 이름으로 지정합니다.</span>
              </div>

              <div style={{ marginBottom: 12 }}>
                <div className="section-label">권한 모드</div>
                <div className="segmented">
                  {PERMISSION_MODES.map((mode) => (
                    <button
                      key={mode}
                      className={`segmented__option ${
                        selectedStage.permissionMode === mode ? "segmented__option--selected" : ""
                      }`}
                      onClick={() => updateStage(selectedIndex, { permissionMode: mode })}
                    >
                      {mode}
                    </button>
                  ))}
                </div>
                <p className="help-text" style={{ marginTop: 6 }}>
                  헤드리스 실행이므로 단계 중간에는 권한 질문에 답할 수 없습니다.
                </p>
              </div>

              <div style={{ marginBottom: 16 }}>
                <div className="section-label">허용 도구</div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                  {KNOWN_TOOLS.map((tool) => {
                    const selected = selectedStage.allowedTools.includes(tool);
                    return (
                      <button
                        key={tool}
                        className={`chip ${selected ? "chip--selected" : ""}`}
                        onClick={() => {
                          const allowedTools = selected
                            ? selectedStage.allowedTools.filter((t) => t !== tool)
                            : [...selectedStage.allowedTools, tool];
                          updateStage(selectedIndex, { allowedTools });
                        }}
                      >
                        {tool}
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="switch-row">
                <button
                  className={`switch ${selectedStage.checkpoint ? "switch--on" : ""}`}
                  onClick={() => updateStage(selectedIndex, { checkpoint: !selectedStage.checkpoint })}
                  aria-label="체크포인트에서 일시 정지"
                >
                  <span className="switch__knob" />
                </button>
                <div>
                  <p className="switch-row__title">체크포인트에서 일시 정지</p>
                  <p className="switch-row__desc">이 단계가 끝나면 승인 전까지 다음 단계로 넘어가지 않습니다.</p>
                </div>
              </div>

              <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
                <button
                  className="btn btn-outline"
                  onClick={() => moveStage(selectedIndex, -1)}
                  disabled={selectedIndex === 0}
                >
                  위로
                </button>
                <button
                  className="btn btn-outline"
                  onClick={() => moveStage(selectedIndex, 1)}
                  disabled={selectedIndex === template.stages.length - 1}
                >
                  아래로
                </button>
              </div>
            </div>
          ) : (
            <div className="card">
              <p className="help-text">단계를 추가하면 여기에서 편집할 수 있습니다.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
