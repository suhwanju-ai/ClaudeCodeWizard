import { useEffect, useState } from "react";
import { isPathNotFoundError, isPathOutsideTargetDirError, listProjectDir } from "../api";
import type { DirEntry, DirListing, RunRecord } from "../types";

interface FileBrowserProps {
  run: RunRecord;
  /** False during App.tsx's optimistic pendingRun window (IMP-034). */
  confirmed: boolean;
  /** Event-derived paths from the live stage log — not a filesystem reading (IMP-029). */
  changedFiles: string[];
}

/**
 * IMP-029. `changedFiles` holds whatever string the CLI put in a Write/Edit tool call,
 * which may be absolute or relative and may use either separator, so matching is a
 * suffix comparison on normalized paths.
 *
 * Known limit: a same-named path outside targetDir would produce a false positive. The
 * highlight is a secondary marker and does not change the truth of the list (which is
 * measured from disk), so the imprecision is accepted — exact matching would need a
 * guarantee that CLI events carry absolute paths, and there is none.
 */
export function isChangedFile(changedFiles: string[], relPath: string): boolean {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/^\.\//, "");
  const target = norm(relPath);
  return changedFiles.some((f) => {
    const n = norm(f);
    return n === target || n.endsWith("/" + target);
  });
}

function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut === -1 ? "" : path.slice(0, cut);
}

function formatSize(size: number | null): string {
  if (size === null) return "";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function formatModified(modifiedMs: number | null): string {
  if (modifiedMs === null) return "";
  const d = new Date(modifiedMs);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function kindLabel(kind: DirEntry["kind"]): string {
  if (kind === "directory") return "dir";
  if (kind === "symlink") return "link";
  if (kind === "file") return "file";
  return "etc";
}

export default function FileBrowser({ run, confirmed, changedFiles }: FileBrowserProps) {
  const [path, setPath] = useState("");
  const [listing, setListing] = useState<DirListing | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    // IMP-034: before the backend has answered, targetDir has been through neither
    // validate_target_dir nor create_dir_all, so listing it would show an unexplained
    // empty list or a raw error.
    if (!confirmed) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    listProjectDir(run.runId, path)
      .then((result) => {
        if (cancelled) return;
        setListing(result);
      })
      .catch((e) => {
        if (cancelled) return;
        if (isPathNotFoundError(e)) {
          // claude deleted the folder while we were looking at it. An error banner is
          // the wrong response; going back up and reloading is the right one.
          setNotice("이 폴더는 더 이상 존재하지 않습니다. 상위 폴더로 돌아갔습니다.");
          setPath((current) => (current === "" ? "" : parentOf(current)));
        } else if (isPathOutsideTargetDirError(e)) {
          // A boundary violation the user cannot fix. Do not echo the raw path back.
          setNotice("이 run의 폴더 밖은 볼 수 없습니다.");
          setPath("");
        } else {
          setError(String(e));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [run.runId, path, confirmed]);

  const enter = (name: string) => {
    setNotice(null);
    setPath(path === "" ? name : `${path}/${name}`);
  };

  // '..' is never sent to the command — the backend rejects it outright (TRD 9.3-(2)
  // step 1), so the truncation happens here.
  const goUp = () => {
    setNotice(null);
    setPath(parentOf(path));
  };

  const crumbs = path === "" ? [] : path.split("/");

  if (!confirmed) {
    return (
      <div className="card">
        <p className="help-text">실행을 준비하는 중입니다 — 폴더를 확인한 뒤 목록을 불러옵니다.</p>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="section-label">이 폴더의 파일 (파일시스템 실측)</div>
      <p className="help-text" style={{ marginBottom: 12 }}>
        이 폴더의 실제 내용입니다 (파일시스템 실측). 위 로그와 달리 지금 디스크에 있는 것을 그대로 보여줍니다.
      </p>

      <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 6, marginBottom: 12 }}>
        <span className="badge mono">프로젝트 루트</span>
        {crumbs.map((crumb, i) => (
          <span key={`${crumb}-${i}`} className="badge mono">
            {crumb}
          </span>
        ))}
        <span style={{ flex: 1 }} />
        <button className="btn btn-outline" onClick={goUp} disabled={path === ""}>
          상위로
        </button>
      </div>

      {notice && <p className="help-text">{notice}</p>}
      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}
      {loading && <p className="help-text">불러오는 중…</p>}

      {!loading && listing && listing.entries.length === 0 && (
        <p className="help-text">이 폴더는 비어 있습니다.</p>
      )}

      {!loading &&
        listing &&
        listing.entries.map((entry) => {
          const relPath = listing.path === "" ? entry.name : `${listing.path}/${entry.name}`;
          return (
            <div
              key={entry.name}
              style={{ display: "flex", alignItems: "center", gap: 10, padding: "4px 0" }}
            >
              <span className="badge mono">{kindLabel(entry.kind)}</span>
              {entry.kind === "directory" ? (
                <button className="btn btn-outline" onClick={() => enter(entry.name)}>
                  {entry.name}
                </button>
              ) : (
                <span className="mono">{entry.name}</span>
              )}
              {isChangedFile(changedFiles, relPath) && <span className="badge">변경됨</span>}
              <span style={{ flex: 1 }} />
              <span className="mono help-text">{formatSize(entry.size)}</span>
              <span className="mono help-text">{formatModified(entry.modifiedMs)}</span>
            </div>
          );
        })}
    </div>
  );
}
