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

  const handleRun = async (template: Template) => {
    const targetDir = await open({ directory: true });
    if (!targetDir || Array.isArray(targetDir)) return;
    const run = await startPipelineRun(template.id, targetDir);
    setView({ name: "run", run });
  };

  if (view.name === "editor") {
    return (
      <TemplateEditor
        initial={view.template}
        onSaved={() => setView({ name: "gallery" })}
        onCancel={() => setView({ name: "gallery" })}
      />
    );
  }

  if (view.name === "run") {
    return <PipelineRun initialRun={view.run} onFinished={() => setView({ name: "gallery" })} />;
  }

  return (
    <TemplateGallery
      onRun={handleRun}
      onEdit={(template) => setView({ name: "editor", template })}
      onNew={() => setView({ name: "editor", template: null })}
    />
  );
}
