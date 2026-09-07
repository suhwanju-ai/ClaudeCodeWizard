# Claude Pipeline Wizard — Per-Stage Run Editing & Project-Local Manifest (Design Addendum)

Date: 2026-09-07
Status: Approved (decisions confirmed with user)
Supersedes/extends: `docs/superpowers/specs/2026-09-06-claude-pipeline-wizard-design.md`

## Motivation

The v1 app (see the original design doc and `README.md`'s "Known v1 limitations")
already has:

- Template JSON save/load (`TemplateStore`, `TemplateEditor.tsx`, `TemplateGallery.tsx`).
- A run-state JSON file (`RunRecord`), but it is written **only** to the app's
  own data directory (`app_data_dir/runs/<runId>.json`), never into the
  project (target) folder a run is running against.
- A checkpoint that pauses **after** a `checkpoint: true` stage finishes
  (approve / request changes / reject). Stages with `checkpoint: false`
  auto-run back-to-back with **no pause at all**, before or after. The only
  existing "edit" points are (a) editing the whole template once before
  starting a run at all, and (b) editing-and-retrying a stage that already
  **failed**.

This addendum adds a **pre-stage edit gate**: every stage now pauses
immediately before it runs so the user can review/edit it, and the run's
live state plus its resolved (possibly edited) stage definitions are also
written into the target project folder, not just the app's data directory.

## Decisions

1. **Pre-stage edit gate scope: all stages.** Every stage — checkpointed or
   not — pauses in a new `awaiting-stage-start` state before it executes.
   The existing "non-checkpoint stages auto-advance with zero pause" fast
   path is removed. `checkpoint: true` continues to mean "also pause **after**
   this stage for approve/request-changes/reject" — that post-execution
   semantic is unchanged.
2. **Project-directory JSON: run state + resolved template snapshot.**
   `<targetDir>/.claude-pipeline-wizard/run.json` (the full `RunRecord`) and
   `<targetDir>/.claude-pipeline-wizard/pipeline.json` (a `Template`-shaped
   snapshot: the run's `resolvedStages`, i.e. what's actually executing,
   including any per-run edits) are written on every state transition, in
   addition to — not instead of — the existing `app_data_dir/runs/` copy.
3. **Runtime edits are run-scoped only.** Editing a stage at its pre-start
   gate updates that run's `resolvedStages` (and therefore `pipeline.json`)
   but never writes back to the saved gallery template. To change the
   template permanently, the user still uses the Template Editor.

## Data Model Changes

`RunStatus` gains `awaiting-stage-start` (run is paused before the current
stage runs). `StageStatus` gains `awaiting-start` (the specific stage
currently paused pre-execution) — `pending` is kept for stages not yet
reached. `RunRecord` gains `resolvedStages: Stage[]`, seeded from the
template's stages at `start_run` and mutated in place whenever a pre-start
edit is applied.

```json
{
  "runId": "...",
  "templateId": "web-app-dev",
  "targetDir": "C:/projects/my-app",
  "status": "awaiting-stage-start",
  "currentStageIndex": 1,
  "stages": [
    { "id": "requirements", "status": "approved", "sessionId": "sess_abc", "log": [] },
    { "id": "design", "status": "awaiting-start", "sessionId": null, "log": [] }
  ],
  "resolvedStages": [ /* Stage[], same shape as Template.stages, per-run edits applied */ ]
}
```

## Execution Flow (replaces the "Execution & Checkpoint Flow" section of the
original design doc)

1. User loads a template and confirms/creates a target folder → `start_run`
   creates a `RunRecord` with `resolvedStages` = a clone of the template's
   stages, `status: awaiting-stage-start`, stage 0 `awaiting-start`. It is
   persisted to both `app_data_dir/runs/<runId>.json` and
   `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json`. **Nothing
   executes yet.**
2. The pipeline-run screen shows the paused stage's fields (prompt,
   permission mode, allowed tools, checkpoint toggle — pre-filled from
   `resolvedStages[currentStageIndex]`) as an editable form. The user edits
   as desired (or leaves it as-is) and clicks **이 단계 실행**.
3. `start_stage(runId, stageOverride?)`: if an override is present (id must
   match the current stage), it replaces `resolvedStages[currentStageIndex]`
   and is persisted immediately; then the stage's `claude` process is
   spawned exactly as before (resuming from the previous stage's session id
   when not the first stage), streaming events to the frontend.
4. On process exit:
   - Non-zero exit → stage `failed`, run `failed` (existing failure UX,
     including "edit template & start a new run", is unchanged).
   - Zero exit, `checkpoint: false` → stage `approved`, advance to the next
     stage as `awaiting-start` (or `completed` if it was the last stage).
     Same pause point as step 2, for the next stage.
   - Zero exit, `checkpoint: true` → stage `awaiting-checkpoint` (existing
     approve / request-changes / reject panel, unchanged) — **approving**
     now only advances to the next stage's `awaiting-stage-start` pause; it
     no longer immediately executes that next stage.
5. Approving the final stage's checkpoint (or a non-checkpoint final stage
   finishing) sets run status to `completed`.

`request_changes` (the existing post-checkpoint feedback loop) now reads
the stage to re-run from `resolvedStages` instead of the raw saved
template, so it inherits any pre-start edits (permission mode, allowed
tools, checkpoint) made for this run — only the prompt is overridden with
the feedback text, and that override is **not** written back into
`resolvedStages` (consistent with decision 3: a one-off, not a template
edit).

## Non-Goals

- No cross-restart resume UI is added by this addendum — the project-local
  manifest makes a paused/failed run easier to inspect and hand-edit, but
  reattaching to it from a restarted app is still the existing v1
  limitation.
- The pre-stage edit panel does not support adding/removing/reordering
  stages or renaming stage ids — only the current stage's own fields
  (name, prompt, permission mode, allowed tools, checkpoint) are editable
  there. Structural template changes remain Template Editor-only.

## Testing Strategy

Same layers as the original design doc: Rust unit tests for the
`RunRecord` state machine and the new project-manifest writer, a Rust
integration test (using the existing `mock_claude` fixture binary) driving
`start_run` → `start_stage` → `approve_checkpoint` → `start_stage` end to
end, and Vitest/RTL tests for the new pre-stage edit panel and the
refactored shared stage-fields form.
