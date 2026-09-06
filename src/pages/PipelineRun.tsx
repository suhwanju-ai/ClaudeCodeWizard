import { useEffect, useState } from "react";
import { approveCheckpoint, onStageEvent, rejectCheckpoint, requestChanges } from "../api";
import type { RunRecord, StageEventPayload } from "../types";

interface Props {
  initialRun: RunRecord;
  onFinished: () => void;
}

function eventText(payload: StageEventPayload): string {
  switch (payload.event.kind) {
    case "assistantText":
      return payload.event.text;
    case "toolUse":
      return `도구 사용: ${payload.event.name}`;
    case "result":
      return payload.event.result ?? "(결과 없음)";
    case "init":
      return `세션 시작: ${payload.event.sessionId}`;
    default:
      return "";
  }
}

export default function PipelineRun({ initialRun, onFinished }: Props) {
  const [run, setRun] = useState<RunRecord>(initialRun);
  const [log, setLog] = useState<string[]>([]);
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setRun(initialRun);
  }, [initialRun]);

  useEffect(() => {
    const unlistenPromise = onStageEvent((payload) => {
      if (payload.runId !== run.runId) return;
      const text = eventText(payload);
      if (text) setLog((prev) => [...prev, text]);
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
    <div>
      {error && <p role="alert">{error}</p>}
      <p>상태: {run.status}</p>
      <ol>
        {run.stages.map((stage) => (
          <li key={stage.id}>
            {stage.id}: {stage.status}
          </li>
        ))}
      </ol>
      <div>
        {log.map((line, i) => (
          <p key={i}>{line}</p>
        ))}
      </div>
      {run.status === "awaiting-checkpoint" && (
        <div>
          <button onClick={handleApprove} disabled={busy}>승인</button>
          <button onClick={handleReject} disabled={busy}>거부</button>
          <label htmlFor="feedback">수정 요청 내용</label>
          <textarea id="feedback" value={feedback} onChange={(e) => setFeedback(e.target.value)} />
          <button onClick={handleRequestChanges} disabled={busy}>수정 요청 보내기</button>
        </div>
      )}
      <div>
        <button onClick={onFinished}>갤러리로 돌아가기</button>
      </div>
    </div>
  );
}
