import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { startPipelineRun } from "./api";
import TemplateGallery from "./pages/TemplateGallery";
import TemplateEditor from "./pages/TemplateEditor";
import PipelineRun from "./pages/PipelineRun";
import type { RunRecord, Template } from "./types";

type View =
  | { name: "gallery" }
  | { name: "editor"; template: Template | null }
  | { name: "run"; run: RunRecord };

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
        status: "running",
        currentStageIndex: 0,
        stages: template.stages.map((s) => ({ id: s.id, status: "pending", sessionId: null, log: [] })),
      };
      setView({ name: "run", run: pendingRun });
      const run = await startPipelineRun(template.id, targetDir, runId);
      setView({ name: "run", run });
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
        onCancel={() => setView({ name: "gallery" })}
      />
    );
  } else if (view.name === "run") {
    content = <PipelineRun initialRun={view.run} onFinished={() => setView({ name: "gallery" })} />;
  } else {
    content = (
      <TemplateGallery
        onRun={handleRun}
        onEdit={(template) => setView({ name: "editor", template })}
        onNew={() => setView({ name: "editor", template: null })}
      />
    );
  }

  return (
    <>
      {error && <p role="alert">{error}</p>}
      {content}
    </>
  );
}
