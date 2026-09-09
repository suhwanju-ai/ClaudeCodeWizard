import { useEffect, useState } from "react";
import {
  approveCheckpoint,
  cancelRun,
  getRun,
  isStaleStageIndexError,
  onStageEvent,
  rejectCheckpoint,
  requestChanges,
  startStage,
} from "../api";
import StageFields from "../components/StageFields";
import type { RunRecord, Stage, StageEventPayload, StageStatus, Template } from "../types";

interface Props {
  initialRun: RunRecord;
  template: Template;
  onFinished: () => void;
  onEditTemplate: (template: Template) => void;
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
  if (status === "awaiting-start") return "stage-timeline__dot--awaiting";
  if (status === "failed") return "stage-timeline__dot--failed";
  return "";
}

function statusBadgeClass(status: RunRecord["status"]): string {
  if (status === "awaiting-checkpoint") return "badge badge-warning";
  if (status === "awaiting-stage-start") return "badge badge-warning";
  if (status === "completed") return "badge badge-success";
  if (status === "failed") return "badge badge-danger";
  if (status === "cancelled") return "badge badge-muted";
  return "badge";
}

export default function PipelineRun({ initialRun, template, onFinished, onEditTemplate }: Props) {
  const [run, setRun] = useState<RunRecord>(initialRun);
  // The run's own resolved names win over the template's: a pre-start edit can rename a
  // stage for this run only.
  const stageNameById = new Map<string, string>([
    ...template.stages.map((s) => [s.id, s.name] as [string, string]),
    ...run.resolvedStages.map((s) => [s.id, s.name] as [string, string]),
  ]);
  const [log, setLog] = useState<LogLine[]>([]);
  const [changedFiles, setChangedFiles] = useState<string[]>([]);
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stageDraft, setStageDraft] = useState<Stage | null>(null);

  useEffect(() => {
    setRun(initialRun);
  }, [initialRun]);

  useEffect(() => {
    setChangedFiles([]);
  }, [run.currentStageIndex]);

  // PRD E-3: the draft follows the gate. When the index advances, the previous stage's
  // draft must not linger on screen.
  useEffect(() => {
    if (run.status === "awaiting-stage-start") {
      setStageDraft(run.resolvedStages[run.currentStageIndex] ?? null);
    } else {
      setStageDraft(null);
    }
  }, [run.status, run.currentStageIndex, run.resolvedStages]);

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

  const handleStartStage = async () => {
    if (!stageDraft) return;
    setError(null);
    setBusy(true);
    try {
      // The second argument is the generation guard: the very index this panel rendered
      // from, so the backend runs the stage the user was actually looking at.
      const updated = await startStage(run.runId, run.currentStageIndex, stageDraft);
      setRun(updated);
      if (updated.status === "completed" || updated.status === "cancelled" || updated.status === "failed") {
        onFinished();
      }
    } catch (e) {
      if (isStaleStageIndexError(e)) {
        // By definition this means our copy of the run is stale, not that anything went
        // wrong. Showing an error would strand the user on a stage that no longer exists;
        // refetch instead and let the effect above rebuild the draft (TRD 3.10).
        try {
          setRun(await getRun(run.runId));
        } catch (refreshError) {
          setError(String(refreshError));
        }
      } else {
        setError(String(e));
      }
    } finally {
      setBusy(false);
    }
  };

  const handleCancelRun = async () => {
    setError(null);
    setBusy(true);
    try {
      const updated = await cancelRun(run.runId);
      setRun(updated);
      onFinished();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  // IMP-013 (decision D3): this leaves the run's own edit scope and changes the saved
  // template for every future run, so it asks first (PRD C-5).
  const handleEditTemplate = () => {
    const confirmed = window.confirm(
      "이 편집은 저장된 원본 템플릿을 바꾸며, 진행 중인 이 run에는 반영되지 않습니다. 계속할까요?"
    );
    if (confirmed) onEditTemplate(template);
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

        {run.status === "awaiting-stage-start" && stageDraft && (
          <div className="card">
            <div className="section-label">다음 단계 — 실행 전 확인/수정</div>
            <p className="help-text" style={{ marginBottom: 12 }}>
              여기서 고친 내용은 이 run에만 적용되며 저장된 템플릿은 바뀌지 않습니다.
            </p>

            <StageFields
              stage={stageDraft}
              onChange={(patch) => setStageDraft((s) => (s ? { ...s, ...patch } : s))}
              idPrefix="run-stage"
              promptLabel="이 단계 프롬프트"
              lockId
            />

            <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 16 }}>
              <button className="btn btn-success" onClick={handleStartStage} disabled={busy}>
                이 단계 실행
              </button>
              <span style={{ flex: 1 }} />
              <button className="btn btn-danger-outline" onClick={handleCancelRun} disabled={busy}>
                이 run 취소
              </button>
            </div>
          </div>
        )}

        {run.status === "failed" && (
          <div className="card">
            <div className="section-label">이 run은 실패로 종료되었습니다</div>
            <p className="help-text">
              실패한 단계는 이 run에서 다시 시작할 수 없습니다. 프롬프트를 고쳐 다시 시도하려면 새 run을 시작하세요.
            </p>
          </div>
        )}

        <div style={{ marginTop: 18, display: "flex", gap: 8 }}>
          <button className="btn btn-outline" onClick={handleEditTemplate}>
            원본 템플릿 편집 (모든 향후 실행에 적용)
          </button>
          <button className="btn btn-outline" onClick={onFinished}>
            갤러리로 돌아가기
          </button>
        </div>
      </div>
    </div>
  );
}
