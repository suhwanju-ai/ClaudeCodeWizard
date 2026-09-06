# Claude Pipeline Wizard — Design

Date: 2026-09-06
Status: Approved (pending user review of this document)

## Overview

A Tauri desktop application that stores reusable **pipeline templates** (e.g.
"웹 프로그램 개발", "데스크톱 프로그램 개발") describing a multi-stage sequence
of Claude Code invocations. Loading a template and pointing it at a new
target project folder runs the full pipeline end-to-end, pausing at
per-stage checkpoints for user approval before continuing.

The app depends on the Claude Code CLI being installed and configured on the
user's machine (existing `~/.claude/skills` and `~/.claude/agents` are
reused by pipeline stages via prompt instructions) rather than embedding the
Claude Agent SDK directly.

## Goals

- Store, create, edit, duplicate, import/export pipeline templates.
- Run a template against a newly created target folder, executing each
  stage as a headless Claude Code CLI invocation.
- Preserve conversation context across stages within one run (a stage can
  see what a prior stage did/created).
- Pause after each stage (configurable) for human approval, revision
  feedback, or rejection before proceeding.
- Survive app restarts mid-run by persisting run state to disk.

## Non-Goals

- Running pipelines against arbitrary existing codebases (v1 targets new
  folders only).
- Parallel/branching stage execution (v1 is strictly sequential).
- Bundling or embedding the Claude Agent SDK / a Node.js runtime — the app
  shells out to the `claude` CLI directly.
- Multi-user/team template sharing infrastructure beyond flat JSON
  import/export.

## Architecture

**Stack**: Tauri v2 (Rust core) + React/TypeScript frontend.

**Rust core responsibilities**:
- **Template store** — CRUD over JSON files in the app data directory
  (`templates/*.json`). No database; templates are just files.
- **Execution engine** — for each stage, spawns a `claude` child process:
  `claude --print --output-format stream-json --permission-mode <mode> -p "<stage prompt>"`
  (with `--resume <session_id>` from stage 2 onward). Parses the
  newline-delimited JSON stream from stdout into structured events.
- **Event bridge** — forwards parsed events to the frontend in real time via
  Tauri's `emit` (`pipeline://stage-event`, `pipeline://checkpoint`).
- **Checkpoint state machine** — pauses the run after a stage's process
  exits (if `checkpoint: true`), waits for an approve / request-changes /
  reject decision from the frontend before proceeding.
- **Run persistence** — writes/updates a run record JSON file after every
  state transition so an interrupted run can be resumed after an app
  restart.
- **Target project management** — creates the destination folder the user
  selects and passes it as the working directory for every stage's `claude`
  invocation.

**Frontend screens**:
1. **Template gallery** — list templates; actions: load & run, edit,
   duplicate, delete, new, import/export JSON.
2. **Pipeline run view** — stage timeline, live log for the running/most
   recent stage, checkpoint approval panel (approve / request changes /
   reject) with a summary of files changed.
3. **Template editor** — form-based stage list (drag to reorder, add/remove
   stages); each stage edits name, prompt, permission mode, allowed tools,
   and checkpoint toggle. Raw-JSON view/edit toggle for advanced users.

## Data Model

### Template

```json
{
  "id": "web-app-dev",
  "name": "웹 프로그램 개발",
  "description": "요구사항→설계→구현→테스트→배포 전체 파이프라인",
  "stages": [
    {
      "id": "requirements",
      "name": "요구사항 정의",
      "prompt": "PRD.md를 작성하라. project-architect 스킬을 사용하라.",
      "permissionMode": "acceptEdits",
      "allowedTools": ["Read", "Write", "Glob", "Grep", "WebSearch"],
      "checkpoint": true
    }
  ]
}
```

- `prompt`: free-text stage instruction. To steer a stage toward a specific
  existing skill/agent, the prompt names it explicitly (e.g. "project-architect
  스킬을 사용하라") — this is sufficient because the user's Claude Code
  environment already mandates skill invocation when applicable.
- `permissionMode`: stages run headless (`--print`), so interactive tool
  permission prompts cannot be answered by a human mid-stage. Each stage
  picks `acceptEdits`, `bypassPermissions`, or `default`. Human oversight
  happens at the checkpoint between stages, not mid-stage.
- `allowedTools`: per-stage tool whitelist.

Validation on save: at least one stage, every stage has a non-empty
`prompt`, stage/template `id`s are unique.

### Run record (`runs/<runId>.json`, updated on every transition)

```json
{
  "runId": "...",
  "templateId": "web-app-dev",
  "targetDir": "C:/projects/my-app",
  "status": "awaiting-checkpoint",
  "currentStageIndex": 1,
  "stages": [
    { "id": "requirements", "status": "approved", "sessionId": "sess_abc", "log": [] },
    { "id": "design", "status": "awaiting-checkpoint", "sessionId": "sess_def", "log": [] }
  ]
}
```

`status` ∈ `running | awaiting-checkpoint | completed | failed | cancelled`.
Stage `status` ∈ `pending | running | awaiting-checkpoint | approved | failed`.

## Cross-Stage Context Continuity

Each stage is a separate OS process, so conversation history does not
survive by default. The engine preserves it explicitly:

1. Stage 1 runs without `--resume`; its `session_id` is captured from the
   stream-json output (init/result events carry it).
2. Stage 2+ runs with `--resume <previous session_id>`, so the model sees
   the full prior conversation (files created, decisions made) automatically.
3. The current session id is persisted in the run record so a resumed app
   session can continue a paused/interrupted run from the correct point.

## Execution & Checkpoint Flow

1. User loads a template and confirms/creates a target folder → run record
   created with `status: running`, `currentStageIndex: 0`.
2. Engine spawns the `claude` process for the current stage. Rust reads
   stdout line-by-line, parses each JSON event, and emits
   `pipeline://stage-event` to the frontend for live log rendering.
3. On process exit, the final `result` event (incl. `session_id`) is
   recorded.
   - `checkpoint: false` → auto-advance to the next stage.
   - `checkpoint: true` → run status becomes `awaiting-checkpoint`; a
     `pipeline://checkpoint` event carries a summary of files changed.
4. Checkpoint actions:
   - **Approve** → advance to the next stage, resuming the session.
   - **Request changes** (free-text feedback) → re-invoke the same stage's
     session (`claude --resume <session_id> -p "<feedback>"`), then
     re-present the checkpoint.
   - **Reject** → run status becomes `cancelled`; pipeline stops.
5. Approving the final stage's checkpoint sets run status to `completed`.

## Error Handling

- On app startup, verify the `claude` CLI is available (`claude --version`);
  if missing, show install guidance instead of letting runs fail silently.
- A stage process that exits non-zero or times out is marked `failed`; the
  frontend shows the error log with **Retry** (re-run, resuming the same
  session), **Edit prompt & retry**, or **Cancel run**.
- Unparseable stream-json lines are logged as warnings without aborting the
  run.
- If the app crashes or is closed mid-run, the on-disk run record (with the
  last known `session_id`) lets the user resume the run from where it left
  off on next launch.

## Testing Strategy

- **Rust unit tests**: stream-json line parser (sample lines → event
  structs), template schema validation, run state-transition logic
  (approve/reject/retry).
- **Integration tests**: a mock `claude` script (emitting canned
  stream-json) swapped in via `PATH`, exercising the full
  spawn → parse → checkpoint → resume cycle without real API calls.
- **Frontend tests**: Vitest + React Testing Library for the checkpoint
  approval panel and template editor form validation.
- **Manual smoke test**: run a real one-stage template against a temp
  folder with the actual `claude` CLI before calling v1 done.

## Open Items for Implementation Planning

- Exact Tauri IPC command surface (commands vs. events) — left to the
  implementation plan.
- Seed templates to ship by default: "웹 프로그램 개발",
  "데스크톱 프로그램 개발(Tauri)" — content to be authored during
  implementation.
