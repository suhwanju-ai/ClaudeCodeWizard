import { useEffect, useRef, useState } from "react";
import { approveCheckpoint, onStageEvent, rejectCheckpoint, requestChanges, startStage } from "../api";
import type { RunRecord, Stage, StageEventPayload, StageStatus, Template } from "../types";
import StageFields from "../components/StageFields";

interface Props {
  initialRun: RunRecord;
  template: Template;
  onFinished: () => void;
  onEditTemplate: (template: Template) => void;
}

interface LogLine {
  id: number;
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

function describeEvent(payload: StageEventPayload): Omit<LogLine, "id"> | null {
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
    case "processError": {
      const code = e.exitCode === null ? "알 수 없음" : String(e.exitCode);
      return { kind: "error", text: `종료 코드 ${code}\n${e.stderr}` };
    }
    default:
      return null;
  }
}

function logLabelClass(kind: string): string {
  if (kind === "assistant") return "log-line__label--assistant";
  if (kind === "tool_use" || kind === "tool_result") return "log-line__label--tool";
  if (kind === "result") return "log-line__label--result";
  if (kind === "error") return "log-line__label--error";
  return "";
}

function stageDotClass(status: StageStatus): string {
  if (status === "approved") return "stage-timeline__dot--approved";
  if (status === "awaiting-checkpoint") return "stage-timeline__dot--awaiting";
  // "awaiting-start" is also a paused-for-user-action state (pre-stage edit
  // gate), so it shares styling with "awaiting-checkpoint" rather than
  // looking like an untouched "pending" stage.
  if (status === "awaiting-start") return "stage-timeline__dot--awaiting";
  if (status === "failed") return "stage-timeline__dot--failed";
  return "";
}

function statusBadgeClass(status: RunRecord["status"]): string {
  if (status === "awaiting-checkpoint") return "badge badge-warning";
  // "awaiting-stage-start" is also a paused-for-user-action state, so it
  // shares styling with "awaiting-checkpoint" rather than the generic default.
  if (status === "awaiting-stage-start") return "badge badge-warning";
  if (status === "completed") return "badge badge-success";
  if (status === "failed") return "badge badge-danger";
  if (status === "cancelled") return "badge badge-muted";
  return "badge";
}

interface PreStagePanelProps {
  stage: Stage;
  onChange: (patch: Partial<Stage>) => void;
  onStart: () => void;
  busy: boolean;
}

function PreStagePanel({ stage, onChange, onStart, busy }: PreStagePanelProps) {
  return (
    <div className="card">
      <div className="section-label">다음 단계 — 실행 전 확인/수정</div>
      <StageFields stage={stage} onChange={onChange} idPrefix="pending-stage" promptLabel="프롬프트" lockId />
      <div style={{ display: "flex", marginTop: 12 }}>
        <button className="btn btn-success" onClick={onStart} disabled={busy}>
          이 단계 실행
        </button>
      </div>
    </div>
  );
}

interface CheckpointPanelProps {
  changedFiles: string[];
  feedback: string;
  onFeedbackChange: (value: string) => void;
  onApprove: () => void;
  onRequestChanges: () => void;
  onReject: () => void;
  busy: boolean;
}

function CheckpointPanel({
  changedFiles,
  feedback,
  onFeedbackChange,
  onApprove,
  onRequestChanges,
  onReject,
  busy,
}: CheckpointPanelProps) {
  return (
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
          onChange={(e) => onFeedbackChange(e.target.value)}
        />
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <button className="btn btn-success" onClick={onApprove} disabled={busy}>
          승인
        </button>
        <button className="btn btn-outline" onClick={onRequestChanges} disabled={busy}>
          수정 요청 보내기
        </button>
        <span style={{ flex: 1 }} />
        <button className="btn btn-danger-outline" onClick={onReject} disabled={busy}>
          거부
        </button>
      </div>
    </div>
  );
}

export default function PipelineRun({ initialRun, template, onFinished, onEditTemplate }: Props) {
  const [run, setRun] = useState<RunRecord>(initialRun);
  const stageNameById = new Map(template.stages.map((s) => [s.id, s.name]));
  const [log, setLog] = useState<LogLine[]>([]);
  const logIdCounter = useRef(0);
  const [changedFiles, setChangedFiles] = useState<string[]>([]);
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Relies on the backend invariant that a run in "awaiting-stage-start" always
  // has a valid current stage at run.resolvedStages[run.currentStageIndex] --
  // guarded as optional below so a violated invariant hides the panel instead
  // of crashing the render.
  const [stageDraft, setStageDraft] = useState<Stage | undefined>(run.resolvedStages[run.currentStageIndex]);

  useEffect(() => {
    setRun(initialRun);
  }, [initialRun]);

  useEffect(() => {
    setChangedFiles([]);
  }, [run.currentStageIndex]);

  useEffect(() => {
    if (run.status === "awaiting-stage-start") {
      setStageDraft(run.resolvedStages[run.currentStageIndex]);
    }
  }, [run.status, run.currentStageIndex, run.resolvedStages]);

  useEffect(() => {
    const unlistenPromise = onStageEvent((payload) => {
      if (payload.runId !== run.runId) return;
      const described = describeEvent(payload);
      if (described) setLog((prev) => [...prev, { id: logIdCounter.current++, ...described }]);

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

  const handleStartStage = async () => {
    if (!stageDraft) return;
    setError(null);
    setBusy(true);
    const preCallRun = run;
    // Optimistically reflect that the stage is now running, so the badge and
    // pre-stage panel (gated on run.status === "awaiting-stage-start") stop
    // showing the stale "paused, ready to edit" state while it executes.
    setRun((prev) => ({ ...prev, status: "running" }));
    try {
      const updated = await startStage(run.runId, stageDraft);
      setRun(updated);
      if (updated.status === "completed" || updated.status === "cancelled" || updated.status === "failed") {
        onFinished();
      }
    } catch (e) {
      setError(String(e));
      // Restore the pre-call state so the pre-stage panel comes back instead
      // of leaving the UI stuck showing a phantom "running" state.
      setRun(preCallRun);
    } finally {
      setBusy(false);
    }
  };

  const handleApprove = async () => {
    setError(null);
    setBusy(true);
    try {
      const updated = await approveCheckpoint(run.runId);
      setRun(updated);
      // `approve_checkpoint` only ever produces `AwaitingStageStart` or
      // `Completed` -- it no longer executes anything, so it can't fail or
      // need cancelling.
      if (updated.status === "completed") {
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
        <h1 className="page-title">{template.name}</h1>
        <p className="page-subtitle mono" style={{ marginBottom: 18, wordBreak: "break-all" }}>
          {run.targetDir}
        </p>
        <ol className="stage-timeline">
          {run.stages.map((stage) => (
            <li key={stage.id} className="stage-timeline__item">
              <span className={`stage-timeline__dot ${stageDotClass(stage.status)}`} />
              <div className="stage-timeline__name">
                {stageNameById.get(stage.id) ?? stage.id}: {stage.status}
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
          {log.map((line) => (
            <div key={line.id} className="log-line">
              <span className={`log-line__label ${logLabelClass(line.kind)}`}>{line.kind}</span>
              <span className="log-line__body">{line.text}</span>
            </div>
          ))}
        </div>

        {run.status === "awaiting-stage-start" && stageDraft && (
          <PreStagePanel
            stage={stageDraft}
            onChange={(patch) => setStageDraft((prev) => (prev ? { ...prev, ...patch } : prev))}
            onStart={handleStartStage}
            busy={busy}
          />
        )}

        {run.status === "awaiting-checkpoint" && (
          <CheckpointPanel
            changedFiles={changedFiles}
            feedback={feedback}
            onFeedbackChange={setFeedback}
            onApprove={handleApprove}
            onRequestChanges={handleRequestChanges}
            onReject={handleReject}
            busy={busy}
          />
        )}

        <div style={{ marginTop: 18, display: "flex", gap: 8 }}>
          <button className="btn btn-outline" onClick={() => onEditTemplate(template)}>
            템플릿 편집
          </button>
          <button className="btn btn-outline" onClick={onFinished}>
            갤러리로 돌아가기
          </button>
        </div>
      </div>
    </div>
  );
}
