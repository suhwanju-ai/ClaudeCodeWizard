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

  const refresh = () => {
    listTemplates().then(setTemplates);
  };

  useEffect(() => {
    refresh();
    checkCli().then(setCliStatus);
  }, []);

  const handleDelete = async (id: string) => {
    await deleteTemplate(id);
    refresh();
  };

  return (
    <div>
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
