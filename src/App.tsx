import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { checkCli, startPipelineRun } from "./api";
import { useTheme } from "./useTheme";
import TemplateGallery from "./pages/TemplateGallery";
import TemplateEditor from "./pages/TemplateEditor";
import PipelineRun from "./pages/PipelineRun";
import type { RunRecord, Template } from "./types";

type View =
  | { name: "gallery" }
  | { name: "editor"; template: Template | null }
  | { name: "run"; run: RunRecord; template: Template };

function CliStatus() {
  const [status, setStatus] = useState<string>("checking");

  useEffect(() => {
    checkCli()
      .then(setStatus)
      .catch(() => setStatus("not-found"));
  }, []);

  const ok = status.startsWith("available");
  const version = ok ? status.slice("available:".length) : null;

  return (
    <div>
      <div className="app-sidebar__footer-label">CLI 상태</div>
      <div className="cli-status">
        <span className={`cli-status__dot ${ok ? "cli-status__dot--ok" : "cli-status__dot--warn"}`} />
        <span>{ok ? version : "찾을 수 없음"}</span>
      </div>
      <p className="cli-status__hint">앱 시작 시 claude --version 으로 확인</p>
    </div>
  );
}

function ThemeToggle() {
  const [theme, toggle] = useTheme();
  return (
    <div className="theme-toggle">
      <button
        className={`theme-toggle__option ${theme === "dark" ? "theme-toggle__option--active" : ""}`}
        onClick={() => theme !== "dark" && toggle()}
      >
        다크
      </button>
      <button
        className={`theme-toggle__option ${theme === "light" ? "theme-toggle__option--active" : ""}`}
        onClick={() => theme !== "light" && toggle()}
      >
        라이트
      </button>
    </div>
  );
}

export default function App() {
  const [view, setView] = useState<View>({ name: "gallery" });
  const [error, setError] = useState<string | null>(null);

  const handleRun = async (template: Template) => {
    setError(null);
    try {
      const targetDir = await open({ directory: true });
      if (!targetDir || Array.isArray(targetDir)) return;
      const runId = crypto.randomUUID();
      const pendingRun: RunRecord = {
        runId,
        templateId: template.id,
        targetDir,
        status: "awaiting-stage-start",
        currentStageIndex: 0,
        stages: template.stages.map((s, i) => ({
          id: s.id,
          status: i === 0 ? "awaiting-start" : "pending",
          sessionId: null,
          log: [],
        })),
        resolvedStages: template.stages,
      };
      setView({ name: "run", run: pendingRun, template });
      const run = await startPipelineRun(template.id, targetDir, runId);
      setView({ name: "run", run, template });
    } catch (e) {
      setError(String(e));
      setView({ name: "gallery" });
    }
  };

  let content;
  if (view.name === "editor") {
    content = (
      <TemplateEditor
        initial={view.template}
        onSaved={() => setView({ name: "gallery" })}
        onRun={handleRun}
        onCancel={() => setView({ name: "gallery" })}
      />
    );
  } else if (view.name === "run") {
    content = (
      <PipelineRun
        initialRun={view.run}
        template={view.template}
        onFinished={() => setView({ name: "gallery" })}
        onEditTemplate={(template) => setView({ name: "editor", template })}
      />
    );
  } else {
    content = (
      <TemplateGallery
        onEdit={(template) => setView({ name: "editor", template })}
        onNew={() => setView({ name: "editor", template: null })}
      />
    );
  }

  return (
    <div className="app-shell">
      <aside className="app-sidebar">
        <div className="app-sidebar__label">화면</div>
        <nav className="app-sidebar__nav">
          <button
            className={`app-sidebar__item ${view.name === "gallery" ? "app-sidebar__item--active" : ""}`}
            onClick={() => setView({ name: "gallery" })}
          >
            <span className="app-sidebar__item-num">01</span>
            <span>템플릿 갤러리</span>
          </button>
          <div className={`app-sidebar__item ${view.name === "run" ? "app-sidebar__item--active" : ""}`}>
            <span className="app-sidebar__item-num">02</span>
            <span>파이프라인 실행</span>
          </div>
          <div className={`app-sidebar__item ${view.name === "editor" ? "app-sidebar__item--active" : ""}`}>
            <span className="app-sidebar__item-num">03</span>
            <span>템플릿 편집</span>
          </div>
        </nav>

        <div className="app-sidebar__spacer" />

        {error && (
          <p className="alert" role="alert">
            {error}
          </p>
        )}

        <div className="app-sidebar__footer">
          <CliStatus />
          <ThemeToggle />
        </div>
      </aside>
      <main className="app-main">{content}</main>
    </div>
  );
}
