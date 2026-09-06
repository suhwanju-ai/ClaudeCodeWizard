import { useEffect, useState } from "react";
import { approveCheckpoint, onStageEvent, rejectCheckpoint, requestChanges } from "../api";
import type { RunRecord, StageEventPayload, StageStatus } from "../types";

interface Props {
  initialRun: RunRecord;
  onFinished: () => void;
  templateName?: string;
}

interface LogLine {
  kind: string;
  text: string;
}

function formatToolInput(input: unknown): string {
  if (input && typeof input === "object" && !Array.isArray(input)) {
    const entries = Object.entries(input as Record<string, unknown>)
      .map(([k, v]) => `${k}: ${typeof v === "string" ? `"${v}"` : JSON.stringify(v)}`)
      .join(", ");
    return `{ ${entries} }`;
  }
  return JSON.stringify(input);
}

function describeEvent(payload: StageEventPayload): LogLine | null {
  const e = payload.event;
  switch (e.kind) {
    case "init":
      return { kind: "init", text: `세션 시작: ${e.sessionId}` };
    case "assistantText":
      return { kind: "assistant", text: e.text };
    case "toolUse":
      return { kind: "tool_use", text: `${e.name} ${formatToolInput(e.input)}` };
    case "toolResult":
      return { kind: "tool_result", text: typeof e.content === "string" ? e.content : JSON.stringify(e.content) };
    case "result":
      return { kind: "result", text: e.result ?? "(결과 없음)" };
    default:
      return null;
  }
}

function logLabelClass(kind: string): string {
  if (kind === "assistant") return "log-line__label--assistant";
  if (kind === "tool_use" || kind === "tool_result") return "log-line__label--tool";
  if (kind === "result") return "log-line__label--result";
  return "";
}

function stageDotClass(status: StageStatus): string {
  if (status === "approved") return "stage-timeline__dot--approved";
  if (status === "awaiting-checkpoint") return "stage-timeline__dot--awaiting";
  if (status === "failed") return "stage-timeline__dot--failed";
  return "";
}

function statusBadgeClass(status: RunRecord["status"]): string {
  if (status === "awaiting-checkpoint") return "badge badge-warning";
  if (status === "completed") return "badge badge-success";
  if (status === "failed") return "badge badge-danger";
  if (status === "cancelled") return "badge badge-muted";
  return "badge";
}

export default function PipelineRun({ initialRun, onFinished, templateName }: Props) {
  const [run, setRun] = useState<RunRecord>(initialRun);
  const [log, setLog] = useState<LogLine[]>([]);
  const [changedFiles, setChangedFiles] = useState<string[]>([]);
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setRun(initialRun);
  }, [initialRun]);

  useEffect(() => {
    setChangedFiles([]);
  }, [run.currentStageIndex]);

  useEffect(() => {
    const unlistenPromise = onStageEvent((payload) => {
      if (payload.runId !== run.runId) return;
      const described = describeEvent(payload);
      if (described) setLog((prev) => [...prev, described]);

      if (payload.event.kind === "toolUse" && (payload.event.name === "Write" || payload.event.name === "Edit")) {
        const input = payload.event.input as Record<string, unknown> | undefined;
        const path = input && typeof input.path === "string" ? input.path : null;
        if (path) setChangedFiles((prev) => (prev.includes(path) ? prev : [...prev, path]));
      }
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [run.runId]);

  const handleApprove = async () => {
    setError(null);
    setBusy(true);
    try {
      const updated = await approveCheckpoint(run.runId);
      setRun(updated);
      if (updated.status === "completed" || updated.status === "cancelled" || updated.status === "failed") {
        onFinished();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleRequestChanges = async () => {
    setError(null);
    setBusy(true);
    try {
      const updated = await requestChanges(run.runId, feedback);
      setRun(updated);
      setFeedback("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleReject = async () => {
    setError(null);
    setBusy(true);
    try {
      const updated = await rejectCheckpoint(run.runId);
      setRun(updated);
      onFinished();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ display: "flex", gap: 28, alignItems: "flex-start" }}>
      <div style={{ width: 240, flexShrink: 0 }}>
        <h1 className="page-title">{templateName ?? run.templateId}</h1>
        <p className="page-subtitle mono" style={{ marginBottom: 18, wordBreak: "break-all" }}>
          {run.targetDir}
        </p>
        <ol className="stage-timeline">
          {run.stages.map((stage) => (
            <li key={stage.id} className="stage-timeline__item">
              <span className={`stage-timeline__dot ${stageDotClass(stage.status)}`} />
              <div className="stage-timeline__name">
                {stage.id}: {stage.status}
              </div>
              {stage.sessionId && <div className="stage-timeline__meta">{stage.sessionId}</div>}
            </li>
          ))}
        </ol>
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        {error && (
          <p className="alert" role="alert">
            {error}
          </p>
        )}

        <div style={{ marginBottom: 14 }}>
          <span className={statusBadgeClass(run.status)}>상태: {run.status}</span>
        </div>

        <div className="card" style={{ marginBottom: 16, minHeight: 120 }}>
          {log.length === 0 && <p className="help-text">아직 로그가 없습니다.</p>}
          {log.map((line, i) => (
            <div key={i} className="log-line">
              <span className={`log-line__label ${logLabelClass(line.kind)}`}>{line.kind}</span>
              <span className="log-line__body">{line.text}</span>
            </div>
          ))}
        </div>

        {run.status === "awaiting-checkpoint" && (
          <div className="card">
            <div className="section-label">체크포인트 — 계속 진행할까요?</div>

            {changedFiles.length > 0 && (
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8, margin: "10px 0" }}>
                {changedFiles.map((path) => (
                  <span key={path} className="badge mono">
                    {path}
                  </span>
                ))}
              </div>
            )}

            <div className="field" style={{ margin: "12px 0" }}>
              <label htmlFor="feedback">수정 요청 내용</label>
              <textarea
                id="feedback"
                className="textarea"
                rows={3}
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
              />
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <button className="btn btn-success" onClick={handleApprove} disabled={busy}>
                승인
              </button>
              <button className="btn btn-outline" onClick={handleRequestChanges} disabled={busy}>
                수정 요청 보내기
              </button>
              <span style={{ flex: 1 }} />
              <button className="btn btn-danger-outline" onClick={handleReject} disabled={busy}>
                거부
              </button>
            </div>
          </div>
        )}

        <div style={{ marginTop: 18 }}>
          <button className="btn btn-outline" onClick={onFinished}>
            갤러리로 돌아가기
          </button>
        </div>
      </div>
    </div>
  );
}
