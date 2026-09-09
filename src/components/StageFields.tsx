import type { PermissionMode, Stage } from "../types";

export const PERMISSION_MODES: PermissionMode[] = ["acceptEdits", "bypassPermissions", "default"];
export const KNOWN_TOOLS = ["Read", "Write", "Edit", "Glob", "Grep", "Bash", "WebSearch", "WebFetch"];

interface Props {
  stage: Stage;
  onChange: (patch: Partial<Stage>) => void;
  /** Prefixes every input id, so the editor and the run screen can render this at once. */
  idPrefix: string;
  /** e.g. "단계 3 프롬프트" in the editor, "이 단계 프롬프트" on the run screen. */
  promptLabel: string;
  /**
   * Disables the stage-id input without hiding it (PRD E-4). Editing a stage id during a
   * run would desynchronize StageRun.id from resolvedStages and the backend would reject
   * the override with StageIdMismatch.
   */
  lockId: boolean;
}

export default function StageFields({ stage, onChange, idPrefix, promptLabel, lockId }: Props) {
  return (
    <>
      <div style={{ display: "flex", gap: 12, marginBottom: 12 }}>
        <label className="field" style={{ flex: 1 }}>
          <span>단계 이름</span>
          <input
            id={`${idPrefix}-name`}
            className="input"
            value={stage.name}
            onChange={(e) => onChange({ name: e.target.value })}
          />
        </label>
        <div className="field" style={{ flex: 1 }}>
          <label htmlFor={`${idPrefix}-id`}>단계 ID</label>
          <input
            id={`${idPrefix}-id`}
            className="input mono"
            value={stage.id}
            disabled={lockId}
            onChange={(e) => onChange({ id: e.target.value })}
          />
          {lockId && <span className="help-text">실행 중에는 단계 ID를 바꿀 수 없습니다.</span>}
        </div>
      </div>

      <div className="field" style={{ marginBottom: 12 }}>
        <label htmlFor={`${idPrefix}-prompt`}>{promptLabel}</label>
        <textarea
          id={`${idPrefix}-prompt`}
          className="textarea"
          rows={4}
          value={stage.prompt}
          onChange={(e) => onChange({ prompt: e.target.value })}
        />
        <span className="help-text">스킬/에이전트는 프롬프트에서 이름으로 지정합니다.</span>
      </div>

      <div style={{ marginBottom: 12 }}>
        <div className="section-label">권한 모드</div>
        <div className="segmented">
          {PERMISSION_MODES.map((mode) => (
            <button
              key={mode}
              className={`segmented__option ${stage.permissionMode === mode ? "segmented__option--selected" : ""}`}
              onClick={() => onChange({ permissionMode: mode })}
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
            const selected = stage.allowedTools.includes(tool);
            return (
              <button
                key={tool}
                className={`chip ${selected ? "chip--selected" : ""}`}
                onClick={() =>
                  onChange({
                    allowedTools: selected
                      ? stage.allowedTools.filter((t) => t !== tool)
                      : [...stage.allowedTools, tool],
                  })
                }
              >
                {tool}
              </button>
            );
          })}
        </div>
      </div>

      <div className="switch-row">
        <button
          className={`switch ${stage.checkpoint ? "switch--on" : ""}`}
          onClick={() => onChange({ checkpoint: !stage.checkpoint })}
          aria-label="체크포인트에서 일시 정지"
        >
          <span className="switch__knob" />
        </button>
        <div>
          <p className="switch-row__title">체크포인트에서 일시 정지</p>
          <p className="switch-row__desc">이 단계가 끝나면 승인 전까지 다음 단계로 넘어가지 않습니다.</p>
        </div>
      </div>
    </>
  );
}
