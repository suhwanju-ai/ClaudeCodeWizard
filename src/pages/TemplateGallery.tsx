import { useEffect, useState } from "react";
import { checkCli, deleteTemplate, listTemplates } from "../api";
import type { Template } from "../types";

interface Props {
  onRun: (template: Template) => void;
  onEdit: (template: Template) => void;
  onNew: () => void;
}

export default function TemplateGallery({ onRun, onEdit, onNew }: Props) {
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
      {error && <p role="alert">{error}</p>}
      {cliStatus === "not-found" && <p>Claude Code CLI가 설치되어 있지 않습니다. 설치 후 다시 시도하세요.</p>}
      <button onClick={onNew}>새 템플릿</button>
      <ul>
        {templates.map((template) => (
          <li key={template.id}>
            <h3>{template.name}</h3>
            <p>{template.description}</p>
            <button onClick={() => onRun(template)}>불러와서 실행</button>
            <button onClick={() => onEdit(template)}>편집</button>
            <button onClick={() => handleDelete(template.id)}>삭제</button>
          </li>
        ))}
      </ul>
    </div>
  );
}
