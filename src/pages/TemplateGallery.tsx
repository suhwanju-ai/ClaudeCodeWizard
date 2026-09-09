import { useEffect, useState } from "react";
import { checkCli, deleteTemplate, listTemplates } from "../api";
import type { Template } from "../types";

interface Props {
  onEdit: (template: Template) => void;
  onNew: () => void;
  /**
   * IMP-012 (decision D2). Running starts here again, from the saved template — the
   * editor no longer runs anything, so pressing 실행 can never overwrite what it runs.
   * Per-run edits happen at the run screen's pre-stage gate instead.
   */
  onRun: (template: Template) => void;
}

export default function TemplateGallery({ onEdit, onNew, onRun }: Props) {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [cliStatus, setCliStatus] = useState<string>("available");
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    listTemplates()
      .then(setTemplates)
      .catch((e) => setError(String(e)));
  };

  useEffect(() => {
    refresh();
    checkCli()
      .then(setCliStatus)
      .catch((e) => setError(String(e)));
  }, []);

  const handleDelete = async (id: string) => {
    if (!window.confirm("이 템플릿을 삭제하시겠습니까?")) return;
    setError(null);
    try {
      await deleteTemplate(id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div>
      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}
      {cliStatus === "not-found" && (
        <p className="alert">Claude Code CLI가 설치되어 있지 않습니다. 설치 후 다시 시도하세요.</p>
      )}

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 22 }}>
        <div>
          <h1 className="page-title">템플릿 갤러리</h1>
          <p className="page-subtitle">
            템플릿을 실행하면 각 단계가 실행 직전에 멈춥니다 — 그 자리에서 확인·수정한 뒤 실행하세요.
          </p>
        </div>
        <button className="btn btn-primary" onClick={onNew}>
          새 템플릿
        </button>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))", gap: 16 }}>
        {templates.map((template) => (
          <div key={template.id} className="card">
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
              <div>
                <h3 style={{ margin: "0 0 2px", fontSize: "15.5px", fontWeight: 600 }}>{template.name}</h3>
                <div className="mono" style={{ fontSize: "11px", color: "var(--text-faint)" }}>
                  {template.id}
                </div>
              </div>
              <span className="badge">{template.stages.length} 단계</span>
            </div>

            <p style={{ fontSize: "12.5px", color: "var(--text-tertiary)", margin: "10px 0 12px" }}>
              {template.description}
            </p>

            <div style={{ display: "flex", flexWrap: "wrap", gap: "4px 16px", marginBottom: 14 }}>
              {template.stages.map((stage) => (
                <span key={stage.id} style={{ fontSize: "11.5px", color: "var(--text-secondary)" }}>
                  <span style={{ color: stage.checkpoint ? "var(--yellow)" : "var(--text-faint)" }}>●</span>{" "}
                  {stage.name}
                </span>
              ))}
            </div>

            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                borderTop: "1px solid var(--border)",
                paddingTop: 12,
              }}
            >
              <button className="btn btn-success" onClick={() => onRun(template)}>
                실행
              </button>
              <button className="btn btn-success-subtle" onClick={() => onEdit(template)}>
                편집
              </button>
              <span style={{ flex: 1 }} />
              <button className="btn btn-ghost" onClick={() => handleDelete(template.id)}>
                삭제
              </button>
            </div>
          </div>
        ))}
      </div>

      <div className="notice" style={{ marginTop: 20 }}>
        <span className="mono" style={{ color: "var(--text-muted)" }}>
          runs/
        </span>{" "}
        v1에서는 중단된 실행 기록을 앱에서 다시 열 수 없습니다. 기록은 디스크에 남으며 복구는 JSON 파일을 직접
        확인해야 합니다.
      </div>
    </div>
  );
}
