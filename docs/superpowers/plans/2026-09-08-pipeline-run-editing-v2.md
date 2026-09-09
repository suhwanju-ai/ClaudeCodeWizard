# Pre-Stage Edit Gate & Project-Local Manifest Implementation Plan (v2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every pipeline stage stop at an editable `awaiting-stage-start` gate before it spawns a Claude process, and record the stage definitions actually used into `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json`, without ever writing back to the saved gallery template.

**Architecture:** `Orchestrator::drive()`'s auto-advance loop is removed. `start_run` becomes a synchronous record-creating command that spawns nothing; a new `start_stage(run_id, expected_stage_index, stage_override?)` becomes the single process-spawn path, guarded by a per-run async mutex (concurrency) plus an `expected_stage_index` generation check (stale/duplicate calls). `RunRecord` gains `resolved_stages: Vec<Stage>` — the run's private copy of the pipeline — so pre-start edits, `request_changes` re-runs and the project-local manifest all read from the run instead of re-loading the template from disk. On the frontend, a shared `StageFields` component backs both the Template Editor and a new "다음 단계 — 실행 전 확인/수정" panel on the run screen.

**Tech Stack:** Rust (edition 2021) + Tauri v2 + tokio + serde/serde_json + thiserror; React 19 + TypeScript + Vite; Vitest + @testing-library/react (frontend), `cargo test` with a `mock_claude` test binary (backend).

**Spec:** `TRD.md` (및 `PRD.md`) — both at the repository root, both Approved 2026-09-08. TRD §3 carries the file-and-function level change spec; PRD §5 carries the success criteria (A-1…F-4) that each task below cites.

---

## Decisions Made From TRD §6

> **These are provisional decisions made by the plan author so the plan is executable. They are NOT final — every one of them is subject to user re-confirmation.** TRD §6 deliberately left eight policy questions open and offered options A/B/C for each. Where a decision is overturned, the affected tasks are named in the "Affects" column so the rework surface is visible up front.
>
> Selection principle applied throughout: **pick the option with the smallest implementation surface that still satisfies the PRD success criterion it blocks, and that does not fork into further sub-decisions.**

| # | TRD § | Question | **Chosen** | One-line rationale | Affects |
|---|---|---|---|---|---|
| D1 | §6.1 | GAP-3 — does `Failed` have outgoing edges? | **A — `Failed` is terminal; retry means starting a new run** | Keeps the §3.2 transition table exactly as drawn (zero new states, zero new commands, no change to `cancel_run`'s guard or to `PipelineRun`'s `onFinished` condition). | Tasks 9, 14 |
| D2 | §6.2 | IMP-012 — how to break `persistTemplate`'s save-then-run coupling? | **B — move the run entry point back to the gallery; remove the editor's 실행 button** | Option A forks into three further sub-decisions about `start_run`'s signature (TRD §6.2-A "주의"); option C invalidates PRD C-1. B is frontend-only, touches no backend signature, and the pre-stage gate now performs the job commit `a246384` had assigned to the editor. | Task 15 |
| D3 | §6.3 | IMP-013 — what to do with the run screen's '템플릿 편집' button? | **A — keep the navigation, relabel it and add a confirm dialog** | ~15 lines; preserves commit `f3be3bf`'s "edit a failed run without losing context" flow, which matters *more* under D1 (`Failed` terminal); option B needs a whole new backend command plus a rule set. | Task 14 |
| D4 | §6.4 | IMP-017 — same `targetDir` re-run? | **A — block unconditionally when `.claude-pipeline-wizard/run.json` already exists** | Makes "silently lost" (PRD B-3) impossible by construction with one `exists()` check plus one error variant; the TRD's terminal-status relaxation is itself another sub-decision, and option C deviates from the approved design's fixed manifest path. | Tasks 6, 7 |
| D5 | §6.5 | IMP-015 / GAP-4 — manifest write failure policy? | **A — fatal, with on-disk state rollback to the pre-transition snapshot** | ~10 lines (`RunRecord` is already `Clone`), leaves the run at `AwaitingStageStart` and therefore retryable/cancellable, which satisfies PRD D-3 unambiguously; option B forfeits the PRD B-1 guarantee and needs a new event channel + banner. | Task 7 |
| D6 | §6.6 | IMP-019 — `targetDir` validation policy? | **A — require an absolute path; `create_dir_all` first, then `canonicalize` for checking only; store and display the caller's original path** | Satisfies PRD B-4 and matches the `is_valid_id` whitelist precedent; the ordering and the Windows `\\?\` display problem the TRD flags are both resolved in the conservative direction (existing create-new-folder behaviour preserved, UI path stays clean). | Task 6 |
| D7 | §6.7 | IMP-020 — record `request_changes` feedback prompts? | **C — do not record; document the decision explicitly** | Zero code; PRD B-5 explicitly admits this branch; avoids stacking a second `RunRecord` schema change onto the migration and avoids worsening GAP-5 (sensitive prompts left in the project folder). | Task 16 |
| D8 | §6.8 | IMP-018 / GAP-6 — legacy `run.json` migration? | **B — quarantine legacy records into `<app_data_dir>/runs-legacy-v1/` at startup** | New code never has to know the legacy shape, no data is deleted, GAP-6 (in-flight runs) is absorbed by the same move, and the app has no past-run list UI so real user loss is ~0; option A is admitted by the TRD to be insufficient alone, option C is over-engineered for 2 record types — and its extensibility payoff evaporates under D7=C. | Task 3 |

**Consequence of D8 that unblocks the TRD:** `resolved_stages` stays a **required** field with **no `#[serde(default)]`**. TRD §1.2-2 ("§3.1 must not merge before the §6.8 decision") is therefore satisfied by Task 3 landing before Task 4.

## Minor Items Carried Over From TRD Review

These four were raised when the TRD was approved and are not (or not fully) reflected in TRD.md itself. They are decided here and folded into the tasks named.

| # | Item | Decision | Task |
|---|---|---|---|
| M1 | `StaleStageIndex` is unidentifiable from the frontend, because `commands.rs` stringifies every error with `.map_err(\|e\| e.to_string())` | Make the error message itself machine-readable: `thiserror`'s `#[error]` string for `StaleStageIndex` **begins with the literal prefix `STALE_STAGE_INDEX:`**, and `src/api.ts` exports `isStaleStageIndexError(error: unknown): boolean` as the single place the frontend tests that prefix. A Rust unit test pins the prefix so it cannot be edited away. | Tasks 7, 12, 14 |
| M2 | TRD §3.3-(4) gives `run_current_stage` the return type `Result<i32, _>`, but tests T3/T4 also need the stage's `session_id` | Widen it to **`Result<(i32, Option<String>), OrchestratorError>`** — `(exit_code, latest_session_id)`. The caller owns all state-transition decisions, as TRD §3.3-(4) requires. | Task 8 |
| M3 | `get_run` / `getRun` / test F-E6 are missing from TRD §3.15 and 부록 A (a documentation omission in the TRD, not an implementation omission — they *are* specified in §3.11 and §5.2) | Included in full: the `get_run` Tauri command (Task 11), the `getRun` api.ts wrapper (Task 12), the F-E6 frontend test (Task 14), and the `documents/` rows for both (Task 16). | Tasks 11, 12, 14, 16 |
| M4 | TRD §3.4's note leaves `pipeline.json`'s `name`/`description` sourcing open ("리뷰어가 이견이 있으면 조정 대상") | Keep the TRD's own draft: `name` reuses `record.template_id`, `description` is the empty string. `RunRecord` gains no new field for this. | Task 6 |

---

## Global Constraints

Every task's requirements implicitly include this section.

- **Rust edition 2021**, crate `claude-pipeline-wizard`, lib target `claude_pipeline_wizard_lib`. Integration tests import through the lib name.
- **serde casing is load-bearing and must not change:** `RunStatus`/`StageStatus` use `#[serde(rename_all = "kebab-case")]`; `StageRun`/`RunRecord`/`Stage`/`Template`/`PermissionMode` use `#[serde(rename_all = "camelCase")]`.
- **`resolved_stages` is a required field. Do NOT add `#[serde(default)]`** (per decision D8).
- **New serialized values, exactly these spellings:** `RunStatus::AwaitingStageStart` → `"awaiting-stage-start"`; `StageStatus::AwaitingStart` → `"awaiting-start"`. Frontend mirrors in `src/types.ts` must use the identical strings.
- **Manifest directory name:** `.claude-pipeline-wizard` (constant `MANIFEST_DIR_NAME`). Files inside: `run.json`, `pipeline.json`. This path is fixed per `targetDir` — do not introduce a per-run subdirectory (decision D4).
- **`app_data_dir/runs/<runId>.json` keeps being written.** The manifest is *additional*, never a replacement (PRD B-2).
- **Error identification prefix:** `STALE_STAGE_INDEX:` — literal, at the start of that error's `Display` output (decision M1).
- **Legacy quarantine directory name:** `runs-legacy-v1`, a sibling of `runs/` under `app_data_dir` (decision D8).
- **`is_valid_id` charset is unchanged:** non-empty, not `.`, not `..`, only ASCII alphanumerics plus `.`, `_`, `-`.
- **UI copy is Korean.** New button labels used by tests, verbatim: `이 단계 실행`, `이 run 취소`, `원본 템플릿 편집 (모든 향후 실행에 적용)`, `실행` (gallery card).
- **Lock scope is fixed:** the per-run mutex is held for load → guards → override → `begin_current_stage` → `save` only. It is **never** held across the child-process await or across the post-run transition/save (TRD §3.3-(3), §3.8, §5.2 all depend on this single scope).
- **`src/types.ts` is a hand-maintained 1:1 mirror of the Rust IPC types.** There is no compile-time link. Any Rust model change in this plan has a paired `types.ts` change.
- **Test commands** (runnable from the repository root):
  - Backend: `cargo test --manifest-path src-tauri/Cargo.toml`
  - Single backend test: `cargo test --manifest-path src-tauri/Cargo.toml <test_name> -- --exact --nocapture`
  - Frontend: `npm test`
  - Single frontend file: `npx vitest run src/pages/PipelineRun.test.tsx`
  - Frontend type-check: `npx tsc --noEmit`
- **Commit after every task.** Conventional-commit prefixes (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`).

---

## File Structure

**Created**

| Path | Responsibility |
|---|---|
| `src-tauri/src/engine/project_manifest.rs` | Manifest path helpers, `targetDir` validation, and the `run.json` + `pipeline.json` writer. |
| `src-tauri/src/engine/legacy_migration.rs` | One-shot startup quarantine of pre-`resolvedStages` run records. |
| `src/components/StageFields.tsx` | The single stage-editing form shared by the Template Editor and the run screen; owns `PERMISSION_MODES` and `KNOWN_TOOLS`. |
| `src/components/StageFields.test.tsx` | Unit tests for the shared form (`lockId` behaviour, `idPrefix` isolation). |

**Modified**

| Path | Change |
|---|---|
| `src-tauri/src/engine/run_record.rs` | Two new enum values, `resolved_stages` field, `RunRecord::new` signature, four new/changed transition methods, `StageOverrideError`. |
| `src-tauri/src/engine/orchestrator.rs` | `drive()` deleted; `start_run` de-executed; `start_stage`/`run_current_stage`/`cancel_run`/`save`/`save_with_rollback`/`lock_for`/`resume_session_id_for` added; `approve_checkpoint`/`request_changes` rewritten. |
| `src-tauri/src/engine/mod.rs` | Two `pub mod` lines. |
| `src-tauri/src/template/mod.rs` | `validate_stage` extracted; `validate_template` delegates to it. |
| `src-tauri/src/commands.rs` | `start_pipeline_run` de-asynced; `start_stage`, `cancel_run`, `get_run` added. |
| `src-tauri/src/lib.rs` | Legacy migration in `setup`; three commands registered. |
| `src-tauri/src/bin/mock_claude.rs` | Per-invocation argv dump directory. |
| `src-tauri/Cargo.toml` | tokio `sync` feature. |
| `src-tauri/tests/orchestrator_test.rs` | Rewritten wholesale against the new state machine. |
| `src/types.ts`, `src/api.ts` | New states, `resolvedStages`, `startStage`/`cancelRun`/`getRun`/`isStaleStageIndexError`. |
| `src/pages/PipelineRun.tsx` | The `awaiting-stage-start` panel, cancel, stale-index recovery, failed-run notice, relabelled template-edit button. |
| `src/pages/TemplateEditor.tsx` | Uses `StageFields`; loses the 실행 button and the `onRun` prop. |
| `src/pages/TemplateGallery.tsx` | Gains the per-card 실행 button and an `onRun` prop. |
| `src/App.tsx` | Optimistic record matches the new state machine; run wiring moves to the gallery. |
| `src/App.test.tsx`, `src/api.test.ts`, `src/pages/*.test.tsx` | Updated per task. |
| `README.md`, `documents/*.md`, `documents/docs-portal.html` | Documentation sweep. |

---

## Task 1: Baseline commits (IMP-021)

TRD §3.14 / §4.2 step 0 / PRD F-3. Every line-number citation in PRD.md and TRD.md is anchored to the working tree as of 2026-09-08. Committing the two untracked source documents **first**, on their own, is what makes those citations reproducible. This task writes no code.

**Files:**
- Commit (untracked): `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md`, `docs/superpowers/plans/2026-09-07-pipeline-run-editing.md`
- Commit (untracked): `PRD.md`, `TRD.md`, `docs/superpowers/plans/2026-09-08-pipeline-run-editing-v2.md`
- Commit (modified): `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`
- Test: none (repository-state task)

**Interfaces:**
- Consumes: nothing.
- Produces: a git history in which `HEAD~2` or earlier contains the two superseded design documents, so later tasks can `git diff` against a stable baseline.

- [ ] **Step 1: Confirm the untracked set is what this task expects**

Run:
```bash
git status --short
```
Expected: `?? docs/superpowers/plans/2026-09-07-pipeline-run-editing.md`, `?? docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md`, `?? PRD.md`, `?? TRD.md`, plus ` M package.json`, ` M src-tauri/Cargo.lock`, ` M src-tauri/Cargo.toml`, ` M src-tauri/tauri.conf.json`.

Two other untracked artifacts exist — `Claude Pipeline Wizard.html`, `error.png`, `_workspace/` — **do not commit them**; they are scratch output, not baseline.

- [ ] **Step 2: Commit the two superseded design documents alone**

```bash
git add docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md docs/superpowers/plans/2026-09-07-pipeline-run-editing.md
git commit -m "docs: commit the approved 2026-09-07 run-editing design and plan as the baseline

PRD.md and TRD.md cite line numbers in these two files. They must be in
version control before implementation starts (IMP-021 / PRD F-3)."
```

- [ ] **Step 3: Commit the installer bump-version artifacts separately**

These four files are the residue of an installer version bump and are unrelated to this feature. TRD §3.14-2 requires them in their own commit.

```bash
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "chore: commit installer bump-version artifacts

Separated from the design-baseline commit per IMP-021."
```

- [ ] **Step 4: Commit the specs this plan implements**

```bash
git add PRD.md TRD.md docs/superpowers/plans/2026-09-08-pipeline-run-editing-v2.md
git commit -m "docs: add approved PRD, TRD, and the v2 implementation plan"
```

- [ ] **Step 5: Verify the working tree is clean of tracked changes**

Run:
```bash
git status --short
```
Expected: only `?? "Claude Pipeline Wizard.html"`, `?? _workspace/`, `?? error.png` remain. No ` M` lines.

---

## Task 2: Extract `validate_stage` (IMP-005)

TRD §3.12 / PRD F-1. `start_stage` must validate a *single* overridden stage, but today the only validation entry point is `validate_template`, which also enforces template-level rules. Split the per-stage half out **without changing `validate_template`'s observable behaviour** — the ten existing tests in `template/mod.rs` must pass untouched, which is the whole point of F-1.

**Files:**
- Modify: `src-tauri/src/template/mod.rs:53-73` (`validate_template`)
- Test: `src-tauri/src/template/mod.rs` `#[cfg(test)] mod tests` (append; do not edit the existing ten tests)

**Interfaces:**
- Consumes: `Stage`, `TemplateValidationError`, `is_valid_id` — all already in this file.
- Produces: `pub fn validate_stage(stage: &Stage) -> Result<(), TemplateValidationError>` — checks id charset then non-blank prompt, in that order. Task 8's `start_stage` calls it.

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests` block in `src-tauri/src/template/mod.rs`, after `serializes_permission_mode_as_camel_case`:

```rust
    #[test]
    fn validate_stage_accepts_valid_stage() {
        assert_eq!(validate_stage(&stage("s1", "do it")), Ok(()));
    }

    #[test]
    fn validate_stage_rejects_empty_prompt() {
        assert_eq!(
            validate_stage(&stage("s1", "   ")),
            Err(TemplateValidationError::EmptyPrompt("s1".to_string()))
        );
    }

    #[test]
    fn validate_stage_rejects_invalid_id() {
        assert_eq!(
            validate_stage(&stage("../bad", "do it")),
            Err(TemplateValidationError::InvalidId("../bad".to_string()))
        );
    }

    #[test]
    fn validate_stage_checks_id_before_prompt() {
        // validate_template reports InvalidId before EmptyPrompt for the same stage;
        // the extracted helper must preserve that order (PRD F-1).
        assert_eq!(
            validate_stage(&stage("../bad", "")),
            Err(TemplateValidationError::InvalidId("../bad".to_string()))
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml validate_stage`
Expected: FAIL — `cannot find function 'validate_stage' in this scope`.

- [ ] **Step 3: Extract the function and delegate to it**

In `src-tauri/src/template/mod.rs`, replace lines 53-73 (the whole `validate_template` function) with:

```rust
/// Validates a single stage in isolation: id charset first, then a non-blank prompt.
/// Template-level rules (at least one stage, unique stage ids) stay in `validate_template`.
pub fn validate_stage(stage: &Stage) -> Result<(), TemplateValidationError> {
    if !is_valid_id(&stage.id) {
        return Err(TemplateValidationError::InvalidId(stage.id.clone()));
    }
    if stage.prompt.trim().is_empty() {
        return Err(TemplateValidationError::EmptyPrompt(stage.id.clone()));
    }
    Ok(())
}

pub fn validate_template(template: &Template) -> Result<(), TemplateValidationError> {
    if !is_valid_id(&template.id) {
        return Err(TemplateValidationError::InvalidId(template.id.clone()));
    }
    if template.stages.is_empty() {
        return Err(TemplateValidationError::NoStages);
    }
    let mut seen = std::collections::HashSet::new();
    for stage in &template.stages {
        validate_stage(stage)?;
        if !seen.insert(stage.id.clone()) {
            return Err(TemplateValidationError::DuplicateStageId(stage.id.clone()));
        }
    }
    Ok(())
}
```

The per-stage checks stay in the same relative position inside the loop, so a stage that is both invalid-id and duplicate still reports `InvalidId` first, exactly as before.

- [ ] **Step 4: Run the whole template test module**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib template::`
Expected: PASS — 14 tests (the original 10 unmodified, plus the 4 new ones). If any of the original 10 needed editing, the refactor changed observable behaviour and is wrong.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/template/mod.rs
git commit -m "refactor: extract validate_stage from validate_template (IMP-005)

start_stage needs to validate a single overridden stage. validate_template
now delegates per-stage checks to it; its observable behaviour, including
error precedence, is unchanged and its ten existing tests pass unmodified."
```

---

## Task 3: Quarantine legacy run records at startup (IMP-018 / GAP-6, decision D8)

TRD §6.8 option B / PRD D-5. Task 4 makes `resolved_stages` a required field, which would make every pre-existing `<app_data_dir>/runs/*.json` fail `serde_json::from_str`. Decision D8 moves those files aside at startup instead of teaching the new code the old shape. **This task must land before Task 4** — that ordering is what satisfies TRD §1.2-2.

**Files:**
- Create: `src-tauri/src/engine/legacy_migration.rs`
- Modify: `src-tauri/src/engine/mod.rs`
- Modify: `src-tauri/src/lib.rs:15-27` (the `setup` closure)
- Test: `src-tauri/src/engine/legacy_migration.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub const LEGACY_DIR_NAME: &str = "runs-legacy-v1";` and `pub fn quarantine_legacy_runs(runs_dir: &Path) -> Result<usize, std::io::Error>` — returns how many files were moved. Nothing else in the codebase calls it; `lib.rs` invokes it once at startup.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/engine/legacy_migration.rs` containing **only** the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), body).unwrap();
    }

    const MODERN: &str = r#"{"runId":"r1","templateId":"t1","targetDir":"/tmp/x","status":"awaiting-stage-start","currentStageIndex":0,"stages":[],"resolvedStages":[]}"#;
    const LEGACY: &str = r#"{"runId":"r0","templateId":"t1","targetDir":"/tmp/x","status":"running","currentStageIndex":0,"stages":[]}"#;

    #[test]
    fn moves_records_without_resolved_stages() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "r0.json", LEGACY);

        let moved = quarantine_legacy_runs(&runs).unwrap();

        assert_eq!(moved, 1);
        assert!(!runs.join("r0.json").exists());
        assert!(root.path().join(LEGACY_DIR_NAME).join("r0.json").exists());
    }

    #[test]
    fn keeps_records_that_already_have_resolved_stages() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "r1.json", MODERN);

        let moved = quarantine_legacy_runs(&runs).unwrap();

        assert_eq!(moved, 0);
        assert!(runs.join("r1.json").exists());
        assert!(!root.path().join(LEGACY_DIR_NAME).exists());
    }

    #[test]
    fn moves_unparseable_records_too() {
        // PRD D-5: an unexplained deserialization failure must not block the app.
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "broken.json", "{ this is not json");

        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 1);
        assert!(root.path().join(LEGACY_DIR_NAME).join("broken.json").exists());
    }

    #[test]
    fn ignores_non_json_files() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "notes.txt", "hello");

        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 0);
        assert!(runs.join("notes.txt").exists());
    }

    #[test]
    fn is_a_noop_when_the_runs_dir_does_not_exist() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(quarantine_legacy_runs(&root.path().join("runs")).unwrap(), 0);
    }

    #[test]
    fn is_idempotent_across_two_startups() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "r0.json", LEGACY);

        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 1);
        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 0);
    }
}
```

- [ ] **Step 2: Register the module so the tests compile, then run them**

Add to `src-tauri/src/engine/mod.rs` (keep the list alphabetical):

```rust
pub mod executor;
pub mod legacy_migration;
pub mod orchestrator;
pub mod run_record;
pub mod stream_json;
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib legacy_migration`
Expected: FAIL — `cannot find function 'quarantine_legacy_runs'` and `cannot find value 'LEGACY_DIR_NAME'`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/engine/legacy_migration.rs`, above the test module:

```rust
use std::fs;
use std::path::Path;

/// Sibling of `runs/` that pre-`resolvedStages` records are moved into.
pub const LEGACY_DIR_NAME: &str = "runs-legacy-v1";

/// Moves every run record that predates the required `resolvedStages` field out of
/// `runs_dir` and into a sibling `runs-legacy-v1/` directory, returning how many were
/// moved. Records that fail to parse at all are quarantined too, so a corrupt file can
/// never block startup (PRD D-5). Nothing is deleted (IMP-018 decision B / GAP-6).
pub fn quarantine_legacy_runs(runs_dir: &Path) -> Result<usize, std::io::Error> {
    if !runs_dir.exists() {
        return Ok(0);
    }
    let legacy_dir = runs_dir.with_file_name(LEGACY_DIR_NAME);
    let mut moved = 0usize;

    for entry in fs::read_dir(runs_dir)? {
        let path = entry?.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let is_legacy = match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(serde_json::Value::Object(map)) => !map.contains_key("resolvedStages"),
                _ => true,
            },
            Err(_) => true,
        };
        if !is_legacy {
            continue;
        }
        fs::create_dir_all(&legacy_dir)?;
        let file_name = path.file_name().expect("a file path has a file name").to_owned();
        fs::rename(&path, legacy_dir.join(file_name))?;
        moved += 1;
    }
    Ok(moved)
}
```

`with_file_name` on `<app_data_dir>/runs` yields `<app_data_dir>/runs-legacy-v1`, which is the sibling the tests assert.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib legacy_migration`
Expected: PASS — 6 tests.

- [ ] **Step 5: Call it from the Tauri setup hook**

In `src-tauri/src/lib.rs`, replace the `setup` closure body (lines 15-27) with:

```rust
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("failed to resolve app data dir");
            let runs_dir = app_data_dir.join("runs");
            match engine::legacy_migration::quarantine_legacy_runs(&runs_dir) {
                Ok(0) => {}
                Ok(n) => eprintln!(
                    "moved {n} pre-resolvedStages run record(s) to '{}'",
                    engine::legacy_migration::LEGACY_DIR_NAME
                ),
                Err(e) => eprintln!("failed to quarantine legacy run records: {e}"),
            }
            let orchestrator = Orchestrator::new(
                TemplateStore::new(app_data_dir.join("templates")),
                RunRecordStore::new(runs_dir),
                ExecutorConfig::default(),
            );
            if let Err(e) = template::seed::seed_default_templates(&orchestrator.template_store) {
                eprintln!("failed to seed default templates: {e}");
            }
            app.manage(orchestrator);
            Ok(())
        })
```

Note the migration runs **before** `Orchestrator::new`, and a migration failure is logged rather than propagated — a failed quarantine must not prevent the app from starting.

- [ ] **Step 6: Verify the whole backend still compiles and passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — all pre-existing tests plus the 6 new ones.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/engine/legacy_migration.rs src-tauri/src/engine/mod.rs src-tauri/src/lib.rs
git commit -m "feat: quarantine pre-resolvedStages run records at startup (IMP-018)

TRD 6.8 option B. Legacy run.json files are moved to runs-legacy-v1/ rather
than deleted, so the new required resolvedStages field cannot cause an
unexplained deserialization failure (PRD D-5). Unparseable files are moved
too. This lands before the field is added, per PRD 4.4-2."
```

---

## Task 4: Expand the `RunRecord` data model (IMP-002)

TRD §3.1 / PRD A-5. Adds the two gate states, the run's private `resolved_stages` copy, and the four transition methods the new state machine is built from. `RunRecord::new`'s signature changes from `&[String]` to `&[Stage]`, so every existing caller and unit test in this file changes with it.

**Files:**
- Modify: `src-tauri/src/engine/run_record.rs` (lines 9-15, 17-25, 36-45, 47-99, and the whole `mod tests` at 149-247)
- Test: `src-tauri/src/engine/run_record.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::template::Stage` (new dependency of this module; `template` does not reference `engine`, so there is no cycle), `validate_stage` is **not** used here.
- Produces, for Tasks 7/8/9:
  - `RunStatus::AwaitingStageStart`, `StageStatus::AwaitingStart`
  - `pub resolved_stages: Vec<Stage>` on `RunRecord` (serialized as `resolvedStages`)
  - `RunRecord::new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self`
  - `fn current_resolved_stage(&self) -> &Stage`
  - `fn apply_stage_override(&mut self, stage: Stage) -> Result<(), StageOverrideError>`
  - `fn begin_current_stage(&mut self)`
  - `fn advance_to_gate(&mut self, session_id: String) -> bool` (`true` == run completed)
  - `fn approve_current(&mut self) -> bool` (behaviour changed: advances to `AwaitingStageStart`, not `Running`)
  - `pub enum StageOverrideError { StageIdMismatch { expected: String, got: String } }`
  - Unchanged: `current_stage_mut`, `mark_current_awaiting_checkpoint`, `mark_current_failed`, `cancel`, `RunRecordStore`.

- [ ] **Step 1: Write the failing tests**

Replace the entire `#[cfg(test)] mod tests` block in `src-tauri/src/engine/run_record.rs` (lines 149-247) with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    fn stage(id: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: format!("Stage {id}"),
            prompt: format!("prompt for {id}"),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec!["Read".to_string()],
            checkpoint: true,
        }
    }

    fn record(stage_ids: &[&str]) -> RunRecord {
        let stages: Vec<Stage> = stage_ids.iter().map(|id| stage(id)).collect();
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &stages)
    }

    // U-1
    #[test]
    fn new_starts_awaiting_stage_start_with_stage0_awaiting() {
        let r = record(&["a", "b"]);
        assert_eq!(r.current_stage_index, 0);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.stages.len(), 2);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingStart);
        assert_eq!(r.stages[1].status, StageStatus::Pending);
    }

    // U-2
    #[test]
    fn new_stores_resolved_stages_copy() {
        let r = record(&["a", "b"]);
        assert_eq!(r.resolved_stages.len(), 2);
        assert_eq!(r.resolved_stages[0], stage("a"));
        assert_eq!(r.resolved_stages[1], stage("b"));
    }

    // U-3
    #[test]
    fn approve_current_advances_to_awaiting_start_gate() {
        let mut r = record(&["a", "b"]);
        let completed = r.approve_current();
        assert!(!completed);
        assert_eq!(r.current_stage_index, 1);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
        assert_eq!(r.stages[1].status, StageStatus::AwaitingStart);
    }

    #[test]
    fn approve_current_on_last_stage_completes_run() {
        let mut r = record(&["a"]);
        assert!(r.approve_current());
        assert_eq!(r.status, RunStatus::Completed);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
    }

    // U-4
    #[test]
    fn apply_stage_override_replaces_only_current_index() {
        let mut r = record(&["a", "b"]);
        let mut edited = stage("a");
        edited.prompt = "edited prompt".to_string();
        edited.checkpoint = false;

        r.apply_stage_override(edited.clone()).unwrap();

        assert_eq!(r.resolved_stages[0], edited);
        assert_eq!(r.resolved_stages[1], stage("b"));
    }

    // U-5
    #[test]
    fn apply_stage_override_rejects_id_mismatch() {
        let mut r = record(&["a", "b"]);
        let result = r.apply_stage_override(stage("b"));
        assert_eq!(
            result,
            Err(StageOverrideError::StageIdMismatch { expected: "a".to_string(), got: "b".to_string() })
        );
        assert_eq!(r.resolved_stages[0], stage("a"));
    }

    // U-6
    #[test]
    fn begin_current_stage_sets_running_on_stage_and_run() {
        let mut r = record(&["a", "b"]);
        r.begin_current_stage();
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages[0].status, StageStatus::Running);
        assert_eq!(r.stages[1].status, StageStatus::Pending);
    }

    #[test]
    fn advance_to_gate_records_session_and_returns_to_the_gate() {
        let mut r = record(&["a", "b"]);
        r.begin_current_stage();
        let completed = r.advance_to_gate("sess-a".to_string());
        assert!(!completed);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
        assert_eq!(r.stages[0].session_id, Some("sess-a".to_string()));
        assert_eq!(r.stages[1].status, StageStatus::AwaitingStart);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.current_stage_index, 1);
    }

    #[test]
    fn advance_to_gate_on_last_stage_completes_run() {
        let mut r = record(&["a"]);
        assert!(r.advance_to_gate("sess-a".to_string()));
        assert_eq!(r.status, RunStatus::Completed);
        assert_eq!(r.stages[0].session_id, Some("sess-a".to_string()));
    }

    #[test]
    fn current_resolved_stage_follows_the_index() {
        let mut r = record(&["a", "b"]);
        assert_eq!(r.current_resolved_stage().id, "a");
        r.approve_current();
        assert_eq!(r.current_resolved_stage().id, "b");
    }

    // U-7
    #[test]
    fn serializes_new_statuses_as_kebab_case() {
        assert_eq!(serde_json::to_string(&RunStatus::AwaitingStageStart).unwrap(), "\"awaiting-stage-start\"");
        assert_eq!(serde_json::to_string(&StageStatus::AwaitingStart).unwrap(), "\"awaiting-start\"");
    }

    #[test]
    fn serializes_resolved_stages_under_a_camel_case_key() {
        let json = serde_json::to_value(record(&["a"])).unwrap();
        assert!(json.get("resolvedStages").is_some(), "expected a resolvedStages key, got {json}");
        assert_eq!(json["resolvedStages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn mark_current_awaiting_checkpoint_updates_stage_and_run() {
        let mut r = record(&["a", "b"]);
        r.mark_current_awaiting_checkpoint("sess1".to_string());
        assert_eq!(r.status, RunStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].session_id, Some("sess1".to_string()));
    }

    #[test]
    fn mark_current_failed_updates_stage_and_run() {
        let mut r = record(&["a"]);
        r.mark_current_failed();
        assert_eq!(r.status, RunStatus::Failed);
        assert_eq!(r.stages[0].status, StageStatus::Failed);
    }

    #[test]
    fn cancel_sets_cancelled_status() {
        let mut r = record(&["a"]);
        r.cancel();
        assert_eq!(r.status, RunStatus::Cancelled);
    }

    #[test]
    fn store_save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let r = record(&["a"]);
        store.save(&r).unwrap();
        assert_eq!(store.load("run1").unwrap(), r);
    }

    #[test]
    fn store_load_missing_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        assert!(matches!(store.load("missing"), Err(RunRecordStoreError::NotFound(_))));
    }

    #[test]
    fn store_load_rejects_id_with_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        assert!(matches!(store.load("../../../etc/passwd"), Err(RunRecordStoreError::InvalidId(_))));
    }

    #[test]
    fn store_load_rejects_id_with_slash() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        assert!(matches!(store.load("sub/dir"), Err(RunRecordStoreError::InvalidId(_))));
    }

    #[test]
    fn store_save_rejects_id_with_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunRecordStore::new(dir.path());
        let mut r = record(&["a"]);
        r.run_id = "../../../etc/passwd".to_string();
        assert!(matches!(store.save(&r), Err(RunRecordStoreError::InvalidId(_))));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib run_record`
Expected: FAIL to compile — `no variant named 'AwaitingStageStart'`, `no field 'resolved_stages'`, `cannot find type 'StageOverrideError'`, and a signature mismatch on `RunRecord::new`.

- [ ] **Step 3: Write the implementation**

In `src-tauri/src/engine/run_record.rs`, change the import at line 5 to:

```rust
use crate::template::{is_valid_id, Stage};
```

Replace the two enums (lines 9-25) with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Running,
    /// Parked at a pre-stage edit gate: nothing is executing and the user may
    /// review/edit the current stage before starting it.
    AwaitingStageStart,
    AwaitingCheckpoint,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StageStatus {
    /// Not reached yet. Meaning unchanged.
    Pending,
    /// Reached, sitting at its pre-start gate.
    AwaitingStart,
    Running,
    AwaitingCheckpoint,
    Approved,
    Failed,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StageOverrideError {
    #[error("stage override id '{got}' does not match resolved stage '{expected}'")]
    StageIdMismatch { expected: String, got: String },
}
```

Replace the `RunRecord` struct (lines 36-45) with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    pub template_id: String,
    pub target_dir: String,
    pub status: RunStatus,
    pub current_stage_index: usize,
    pub stages: Vec<StageRun>,
    /// The run's own copy of the pipeline. Pre-start edits mutate this and never the
    /// saved template. Required — there is deliberately no `#[serde(default)]`; legacy
    /// records are quarantined at startup instead (IMP-018 decision B).
    pub resolved_stages: Vec<Stage>,
}
```

Replace the whole `impl RunRecord` block (lines 47-99) with:

```rust
impl RunRecord {
    pub fn new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self {
        Self {
            run_id,
            template_id,
            target_dir,
            status: RunStatus::AwaitingStageStart,
            current_stage_index: 0,
            stages: stages
                .iter()
                .enumerate()
                .map(|(i, stage)| StageRun {
                    id: stage.id.clone(),
                    status: if i == 0 { StageStatus::AwaitingStart } else { StageStatus::Pending },
                    session_id: None,
                    log: Vec::new(),
                })
                .collect(),
            resolved_stages: stages.to_vec(),
        }
    }

    pub fn current_stage_mut(&mut self) -> &mut StageRun {
        &mut self.stages[self.current_stage_index]
    }

    /// The stage definition this run will actually execute next.
    pub fn current_resolved_stage(&self) -> &Stage {
        &self.resolved_stages[self.current_stage_index]
    }

    /// Replaces the current stage's definition with a user-edited one. The id must match:
    /// changing it would desynchronize `StageRun.id` from `resolved_stages`.
    pub fn apply_stage_override(&mut self, stage: Stage) -> Result<(), StageOverrideError> {
        let expected = &self.resolved_stages[self.current_stage_index].id;
        if &stage.id != expected {
            return Err(StageOverrideError::StageIdMismatch {
                expected: expected.clone(),
                got: stage.id,
            });
        }
        self.resolved_stages[self.current_stage_index] = stage;
        Ok(())
    }

    /// Leaves the gate: the current stage and the run both go to `Running`.
    pub fn begin_current_stage(&mut self) {
        self.current_stage_mut().status = StageStatus::Running;
        self.status = RunStatus::Running;
    }

    pub fn mark_current_awaiting_checkpoint(&mut self, session_id: String) {
        self.current_stage_mut().status = StageStatus::AwaitingCheckpoint;
        self.current_stage_mut().session_id = Some(session_id);
        self.status = RunStatus::AwaitingCheckpoint;
    }

    pub fn mark_current_failed(&mut self) {
        self.current_stage_mut().status = StageStatus::Failed;
        self.status = RunStatus::Failed;
    }

    /// Records the session a finished non-checkpoint stage used, then returns to the
    /// next stage's gate. Returns `true` if that was the last stage.
    pub fn advance_to_gate(&mut self, session_id: String) -> bool {
        self.current_stage_mut().session_id = Some(session_id);
        self.approve_current()
    }

    /// Marks the current stage approved and advances to the next stage's *gate*
    /// (not to `Running` — PRD A-3). Returns `true` if the run is now complete.
    pub fn approve_current(&mut self) -> bool {
        self.current_stage_mut().status = StageStatus::Approved;
        if self.current_stage_index + 1 < self.stages.len() {
            self.current_stage_index += 1;
            self.current_stage_mut().status = StageStatus::AwaitingStart;
            self.status = RunStatus::AwaitingStageStart;
            false
        } else {
            self.status = RunStatus::Completed;
            true
        }
    }

    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
    }
}
```

- [ ] **Step 4: Run the run_record tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib run_record`
Expected: PASS — 19 tests.

`cargo test` as a whole will **not** compile yet: `orchestrator.rs:41` still calls `RunRecord::new(..., &stage_ids)` with `&[String]`. That is expected and is fixed in Task 7.

- [ ] **Step 5: Make the crate compile again with a minimal call-site fix**

So this task ends on a green build, patch the one call site in `src-tauri/src/engine/orchestrator.rs`. Replace lines 40-41:

```rust
        let stage_ids: Vec<String> = template.stages.iter().map(|s| s.id.clone()).collect();
        let mut record = RunRecord::new(run_id, template.id.clone(), target_dir.to_string_lossy().to_string(), &stage_ids);
```

with:

```rust
        let mut record =
            RunRecord::new(run_id, template.id.clone(), target_dir.to_string_lossy().to_string(), &template.stages);
```

`drive()` and the rest of `orchestrator.rs` are rewritten in Tasks 7-9; this is only enough to keep the tree compiling.

- [ ] **Step 6: Run the full backend suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: the library and `legacy_migration`/`template`/`run_record` tests PASS. `tests/orchestrator_test.rs` will now FAIL — `start_run_stops_at_first_checkpoint` and the other five assert `RunStatus::Running`/`Pending` semantics that no longer hold. That is expected; Tasks 7-9 rewrite that file. Note which of them fail so Task 7 can confirm it addressed each.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/engine/run_record.rs src-tauri/src/engine/orchestrator.rs
git commit -m "feat: add pre-stage gate states and resolvedStages to RunRecord (IMP-002)

Adds RunStatus::AwaitingStageStart and StageStatus::AwaitingStart, gives every
run its own resolved_stages copy of the pipeline, and adds
current_resolved_stage / apply_stage_override / begin_current_stage /
advance_to_gate. approve_current now advances to the next stage's gate rather
than to Running (PRD A-3). tests/orchestrator_test.rs is rewritten in a
following commit."
```

---

## Task 5: Per-invocation argv dumps in `mock_claude` (test infrastructure)

Four later integration tests need something the mock CLI cannot do today. `MOCK_CLAUDE_DUMP_ARGS` writes to one fixed path, so invocation #2 overwrites invocation #1 — which makes it useless for "how many times was claude spawned?" (T-D4a, T-D4b) and for "what `--resume` did the *second* call get?" (T-A5, T-A6). Add a directory mode that writes one file per invocation.

**Files:**
- Modify: `src-tauri/src/bin/mock_claude.rs:8-10`
- Test: `src-tauri/tests/executor_test.rs` (append one test)

**Interfaces:**
- Consumes: nothing.
- Produces: environment variable **`MOCK_CLAUDE_DUMP_DIR`**. When set, every `mock_claude` invocation writes its full argv (newline-separated, same format as `MOCK_CLAUDE_DUMP_ARGS`) to `<dir>/call-<nanos-since-epoch, zero-padded to 20>-<pid>.txt`. Lexicographic filename order is therefore chronological. `MOCK_CLAUDE_DUMP_ARGS` keeps working unchanged — `executor_test.rs:104/122/143` still use it.
  Tasks 7-9 consume this through a harness method they define in `src-tauri/tests/orchestrator_test.rs`: `fn calls(&self) -> Vec<Vec<String>>` — all dumps, oldest first, each split into argv tokens.

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/tests/executor_test.rs`:

```rust
#[tokio::test]
async fn dump_dir_records_one_file_per_invocation() {
    let target = tempfile::tempdir().unwrap();
    let dump_dir = tempfile::tempdir().unwrap();
    let config = ExecutorConfig {
        claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
        stage_timeout: std::time::Duration::from_secs(30),
        extra_env: vec![(
            "MOCK_CLAUDE_DUMP_DIR".to_string(),
            dump_dir.path().to_string_lossy().to_string(),
        )],
    };
    let stage = Stage {
        id: "s1".to_string(),
        name: "S1".to_string(),
        prompt: fixture_prompt("executor_success.jsonl"),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: vec![],
        checkpoint: false,
    };

    run_stage(&config, &stage, target.path(), None, |_| {}).await.unwrap();
    run_stage(&config, &stage, target.path(), Some("sess-1"), |_| {}).await.unwrap();

    let mut files: Vec<_> = std::fs::read_dir(dump_dir.path()).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();

    assert_eq!(files.len(), 2, "expected one dump file per invocation");
    let first = std::fs::read_to_string(&files[0]).unwrap();
    let second = std::fs::read_to_string(&files[1]).unwrap();
    assert!(!first.contains("--resume"), "first call had no resume session: {first}");
    assert!(second.contains("--resume\nsess-1"), "second call should carry --resume sess-1: {second}");
}
```

Open `src-tauri/tests/executor_test.rs` first and match its existing helper names. It already has a `fixture_prompt`-style helper (the same one `orchestrator_test.rs` uses) and imports `Stage`/`PermissionMode`/`run_stage`/`ExecutorConfig`. If its helper is named differently, use that name rather than adding a duplicate.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test executor_test dump_dir_records_one_file_per_invocation`
Expected: FAIL — `assertion failed: expected one dump file per invocation`, left `0`, right `2` (the env var is ignored, so nothing is written).

- [ ] **Step 3: Write the implementation**

In `src-tauri/src/bin/mock_claude.rs`, replace lines 8-10 with:

```rust
    if let Ok(dump_path) = env::var("MOCK_CLAUDE_DUMP_ARGS") {
        let _ = fs::write(dump_path, args.join("\n"));
    }

    // One file per invocation, so a test can count spawns and inspect each call's argv.
    // The zero-padded nanosecond stamp makes lexicographic filename order chronological.
    if let Ok(dump_dir) = env::var("MOCK_CLAUDE_DUMP_DIR") {
        let _ = fs::create_dir_all(&dump_dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = format!("call-{stamp:020}-{}.txt", std::process::id());
        let _ = fs::write(std::path::Path::new(&dump_dir).join(name), args.join("\n"));
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test executor_test`
Expected: PASS — all existing executor tests plus the new one.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/bin/mock_claude.rs src-tauri/tests/executor_test.rs
git commit -m "test: give mock_claude a per-invocation argv dump directory

MOCK_CLAUDE_DUMP_DIR writes one file per spawn so integration tests can count
claude invocations and inspect each call's --resume argument. The existing
single-file MOCK_CLAUDE_DUMP_ARGS mode is unchanged."
```

---

## Task 6: Project-local manifest module and `targetDir` validation (IMP-003, IMP-019)

TRD §3.4 / PRD B-1, B-4. A new engine module owns everything that touches the target folder: where the manifest lives, whether the folder is acceptable to write into (decision D6), and the two-file write itself. It knows nothing about the state machine — it serializes whatever `RunRecord` it is handed.

**Files:**
- Create: `src-tauri/src/engine/project_manifest.rs`
- Modify: `src-tauri/src/engine/mod.rs` (one `pub mod` line)
- Test: `src-tauri/src/engine/project_manifest.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::template::{Stage, Template}`, `super::run_record::RunRecord` (with `resolved_stages` from Task 4).
- Produces, for Task 7:
  - `pub const MANIFEST_DIR_NAME: &str = ".claude-pipeline-wizard";`
  - `pub fn manifest_dir(target_dir: &Path) -> PathBuf`
  - `pub fn manifest_run_path(target_dir: &Path) -> PathBuf` — `<target_dir>/.claude-pipeline-wizard/run.json`; Task 7's `TargetDirInUse` guard tests this for existence (decision D4).
  - `pub fn validate_target_dir(target_dir: &Path) -> Result<(), TargetDirError>` (decision D6)
  - `pub fn write_project_manifest(target_dir: &Path, record: &RunRecord) -> Result<(), ProjectManifestError>`
  - `pub enum ProjectManifestError { Io(std::io::Error), Json(serde_json::Error) }`
  - `pub enum TargetDirError { NotAbsolute(String), NotADirectory(String), Io(std::io::Error) }`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/engine/project_manifest.rs` containing **only** this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::PermissionMode;

    fn stage(id: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: format!("Stage {id}"),
            prompt: format!("prompt for {id}"),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec!["Read".to_string()],
            checkpoint: true,
        }
    }

    fn record(target_dir: &Path) -> RunRecord {
        RunRecord::new(
            "run1".to_string(),
            "tpl1".to_string(),
            target_dir.to_string_lossy().to_string(),
            &[stage("a"), stage("b")],
        )
    }

    #[test]
    fn manifest_paths_are_under_the_fixed_directory_name() {
        let dir = Path::new("/tmp/proj");
        assert_eq!(manifest_dir(dir), dir.join(".claude-pipeline-wizard"));
        assert_eq!(manifest_run_path(dir), manifest_dir(dir).join("run.json"));
    }

    #[test]
    fn writes_both_files_with_the_expected_shape() {
        let target = tempfile::tempdir().unwrap();
        let rec = record(target.path());

        write_project_manifest(target.path(), &rec).unwrap();

        let run_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(manifest_run_path(target.path())).unwrap()).unwrap();
        assert_eq!(run_json["runId"], "run1");
        assert_eq!(run_json["status"], "awaiting-stage-start");
        assert_eq!(run_json["resolvedStages"].as_array().unwrap().len(), 2);

        let pipeline_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir(target.path()).join("pipeline.json")).unwrap(),
        )
        .unwrap();
        // M4: RunRecord has no template name/description, so the id is reused for the
        // name and the description is left empty.
        assert_eq!(pipeline_json["id"], "tpl1");
        assert_eq!(pipeline_json["name"], "tpl1");
        assert_eq!(pipeline_json["description"], "");
        assert_eq!(pipeline_json["stages"].as_array().unwrap().len(), 2);
        assert_eq!(pipeline_json["stages"][0]["id"], "a");
    }

    #[test]
    fn pipeline_json_snapshots_resolved_stages_not_the_original() {
        let target = tempfile::tempdir().unwrap();
        let mut rec = record(target.path());
        let mut edited = stage("a");
        edited.prompt = "edited for this run only".to_string();
        rec.apply_stage_override(edited).unwrap();

        write_project_manifest(target.path(), &rec).unwrap();

        let pipeline_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir(target.path()).join("pipeline.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(pipeline_json["stages"][0]["prompt"], "edited for this run only");
    }

    #[test]
    fn rewriting_replaces_the_previous_content() {
        let target = tempfile::tempdir().unwrap();
        let mut rec = record(target.path());
        write_project_manifest(target.path(), &rec).unwrap();
        let before = std::fs::read_to_string(manifest_run_path(target.path())).unwrap();

        rec.begin_current_stage();
        write_project_manifest(target.path(), &rec).unwrap();
        let after = std::fs::read_to_string(manifest_run_path(target.path())).unwrap();

        assert_ne!(before, after);
        assert!(after.contains("\"running\""));
    }

    #[test]
    fn write_fails_when_the_manifest_dir_slot_is_occupied_by_a_file() {
        let target = tempfile::tempdir().unwrap();
        std::fs::write(manifest_dir(target.path()), "not a directory").unwrap();
        assert!(matches!(
            write_project_manifest(target.path(), &record(target.path())),
            Err(ProjectManifestError::Io(_))
        ));
    }

    #[test]
    fn validate_target_dir_rejects_a_relative_path() {
        assert!(matches!(
            validate_target_dir(Path::new("relative/path")),
            Err(TargetDirError::NotAbsolute(_))
        ));
    }

    #[test]
    fn validate_target_dir_accepts_and_creates_a_missing_absolute_dir() {
        let root = tempfile::tempdir().unwrap();
        let fresh = root.path().join("brand-new-project");
        assert!(!fresh.exists());

        validate_target_dir(&fresh).unwrap();

        assert!(fresh.is_dir(), "an absolute path that does not exist yet must still be creatable");
    }

    #[test]
    fn validate_target_dir_rejects_a_path_that_is_a_file() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("a-file.txt");
        std::fs::write(&file, "x").unwrap();

        assert!(matches!(validate_target_dir(&file), Err(TargetDirError::Io(_))));
    }
}
```

- [ ] **Step 2: Register the module, then run the tests to verify they fail**

Set `src-tauri/src/engine/mod.rs` to:

```rust
pub mod executor;
pub mod legacy_migration;
pub mod orchestrator;
pub mod project_manifest;
pub mod run_record;
pub mod stream_json;
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib project_manifest`
Expected: FAIL — `cannot find function 'manifest_dir'`, `'manifest_run_path'`, `'write_project_manifest'`, `'validate_target_dir'`; `cannot find type 'ProjectManifestError'`, `'TargetDirError'`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/engine/project_manifest.rs`, above the test module:

```rust
use std::path::{Path, PathBuf};

use crate::template::{Stage, Template};

use super::run_record::RunRecord;

/// The directory this app creates inside every target project folder.
pub const MANIFEST_DIR_NAME: &str = ".claude-pipeline-wizard";

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum TargetDirError {
    #[error("target dir must be an absolute path: {0}")]
    NotAbsolute(String),
    #[error("target dir is not a directory: {0}")]
    NotADirectory(String),
    #[error("io error while checking target dir: {0}")]
    Io(#[from] std::io::Error),
}

pub fn manifest_dir(target_dir: &Path) -> PathBuf {
    target_dir.join(MANIFEST_DIR_NAME)
}

pub fn manifest_run_path(target_dir: &Path) -> PathBuf {
    manifest_dir(target_dir).join("run.json")
}

/// IMP-019 (decision D6). Requires an absolute path — a relative one would resolve
/// against the app's working directory, which the user never sees — then creates the
/// folder if needed and canonicalizes it *only* to confirm it is really a directory
/// once symlinks are resolved.
///
/// The canonical form is deliberately discarded: on Windows `canonicalize` returns a
/// `\\?\` UNC path, and `RunRecord.target_dir` is shown verbatim in the UI. The caller
/// keeps its original path for storage and display.
pub fn validate_target_dir(target_dir: &Path) -> Result<(), TargetDirError> {
    if !target_dir.is_absolute() {
        return Err(TargetDirError::NotAbsolute(target_dir.to_string_lossy().to_string()));
    }
    std::fs::create_dir_all(target_dir)?;
    let canonical = std::fs::canonicalize(target_dir)?;
    if !canonical.is_dir() {
        return Err(TargetDirError::NotADirectory(canonical.to_string_lossy().to_string()));
    }
    Ok(())
}

/// Writes the two manifest files. Called on every state transition, so the folder always
/// reflects the run's current state (PRD B-1).
///
/// `run.json` is the whole `RunRecord`, `resolvedStages` included. `pipeline.json` is a
/// `Template`-shaped snapshot of `resolved_stages` — the definitions this run is actually
/// using, not whatever the gallery template says now.
pub fn write_project_manifest(target_dir: &Path, record: &RunRecord) -> Result<(), ProjectManifestError> {
    let dir = manifest_dir(target_dir);
    std::fs::create_dir_all(&dir)?;

    std::fs::write(dir.join("run.json"), serde_json::to_string_pretty(record)?)?;

    // M4: RunRecord carries no template name/description, so the id is reused as the
    // name and the description is left empty.
    let snapshot = Template {
        id: record.template_id.clone(),
        name: record.template_id.clone(),
        description: String::new(),
        stages: record.resolved_stages.clone(),
    };
    std::fs::write(dir.join("pipeline.json"), serde_json::to_string_pretty(&snapshot)?)?;
    Ok(())
}
```

`Stage` is imported only for the test module's helper. If the compiler warns it is unused in non-test builds, move it into the test module as `use crate::template::{PermissionMode, Stage};` rather than silencing the warning.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib project_manifest`
Expected: PASS — 8 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/project_manifest.rs src-tauri/src/engine/mod.rs
git commit -m "feat: add the project-local manifest writer and targetDir validation

IMP-003 + IMP-019. write_project_manifest emits run.json (the whole RunRecord)
and pipeline.json (a Template snapshot of resolved_stages) into
<targetDir>/.claude-pipeline-wizard/. validate_target_dir requires an absolute
path and canonicalizes for checking only, keeping the caller's original path
for storage and display (TRD 6.6 option A)."
```

---

## Task 7: Orchestrator scaffolding — errors, run locks, dual-write save, non-executing `start_run` (IMP-004 part, IMP-015, IMP-016 part, IMP-017)

TRD §3.3-(1)(2), §3.4, §3.8 / PRD A-1, B-1, B-2, B-3, B-4, D-3. This turns `start_run` into a command that spawns nothing, installs the per-run mutex map, and routes every future transition through one `save` that writes both stores. `drive()` removal and `start_stage` are Task 8.

**Files:**
- Modify: `src-tauri/src/engine/orchestrator.rs:1-48` (imports, error enum, struct, `new`, `start_run`)
- Modify: `src-tauri/src/commands.rs:53-67` (minimal compile fix; Task 11 finishes it)
- Modify: `src-tauri/Cargo.toml` (tokio `sync` feature)
- Test: `src-tauri/tests/orchestrator_test.rs` (new shared harness + the `start_run` tests)

**Interfaces:**
- Consumes: Task 4's `RunRecord::new(&[Stage])` and `StageOverrideError`; Task 5's `MOCK_CLAUDE_DUMP_DIR`; Task 6's `manifest_run_path`, `validate_target_dir`, `write_project_manifest`, `TargetDirError`.
- Produces, for Tasks 8/9/11:
  - `OrchestratorError` variants: `TemplateStore`, `RunRecordStore`, `Io`, `NotAwaitingCheckpoint(String)`, `NotAwaitingStageStart(String)`, `StageIdMismatch { expected: String, got: String }`, `StaleStageIndex { expected: usize, actual: usize }` (**Display begins with `STALE_STAGE_INDEX:`** — M1), `StageValidation(TemplateValidationError)`, `ProjectManifest(String)`, `TargetDir(TargetDirError)`, `TargetDirInUse(String)`, `NotCancellable(String)`
  - `impl From<StageOverrideError> for OrchestratorError`
  - `Orchestrator { template_store, run_store, executor_config, run_locks }`; `Orchestrator::new(TemplateStore, RunRecordStore, ExecutorConfig)` keeps its three-argument signature
  - `fn lock_for(&self, run_id: &str) -> Arc<tokio::sync::Mutex<()>>`
  - `fn save(&self, record: &RunRecord) -> Result<(), OrchestratorError>` — run store first, then manifest
  - `fn save_with_rollback(&self, record: &RunRecord, snapshot: &RunRecord) -> Result<(), OrchestratorError>`
  - `pub fn start_run(&self, template_id: &str, target_dir: PathBuf, run_id: String) -> Result<RunRecord, OrchestratorError>` — **synchronous, no `on_event`, spawns nothing**
  - Test harness for Tasks 8/9: `setup()`, `setup_with(Template)`, `setup_with_timeout(Template, Duration)`, `two_stage_template()`, `no_checkpoint_two_stage_template()`, `stage(id, fixture, checkpoint)`, and `Harness { orchestrator, run_dir, target_dir, dump_dir }` with `target()`, `calls()`, `call_count()`, `persisted(run_id)`

- [ ] **Step 1: Enable tokio's `sync` feature**

`tokio::sync::Mutex` sits behind the `sync` feature, which the current list omits. In `src-tauri/Cargo.toml`, change the tokio line to:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "process", "io-util", "fs", "time", "sync"] }
```

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: compiles (this only widens a feature set).

- [ ] **Step 2: Write the failing tests**

Replace everything in `src-tauri/tests/orchestrator_test.rs` from line 1 through the end of `reject_checkpoint_rejects_a_run_not_awaiting_checkpoint` (line 134) with the block below. Leave the two timeout tests (lines 136-226) alone for now — Step 5 patches them just enough to compile, and Tasks 8/9 rewrite them.

```rust
use std::path::{Path, PathBuf};

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::{Orchestrator, OrchestratorError};
use claude_pipeline_wizard_lib::engine::project_manifest::{manifest_dir, manifest_run_path};
use claude_pipeline_wizard_lib::engine::run_record::{RunRecord, RunRecordStore, RunStatus, StageStatus};
use claude_pipeline_wizard_lib::template::store::TemplateStore;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage, Template};

fn fixture_prompt(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    format!("FIXTURE:{}", path.to_string_lossy())
}

fn stage(id: &str, fixture: &str, checkpoint: bool) -> Stage {
    Stage {
        id: id.to_string(),
        name: format!("Stage {id}"),
        prompt: fixture_prompt(fixture),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: vec![],
        checkpoint,
    }
}

/// stage1 checkpoints, stage2 does not. Drives the checkpoint-path tests.
fn two_stage_template() -> Template {
    Template {
        id: "two-stage".to_string(),
        name: "Two Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![
            stage("stage1", "orchestrator_stage1.jsonl", true),
            stage("stage2", "orchestrator_stage2.jsonl", false),
        ],
    }
}

/// Neither stage checkpoints. This is the fixture the gate tests need: only a
/// non-checkpoint stage *followed by another stage* produces the "back at
/// AwaitingStageStart, but now at index 1" state that T-D4b depends on (TRD 5.2).
fn no_checkpoint_two_stage_template() -> Template {
    Template {
        id: "no-cp-two-stage".to_string(),
        name: "No Checkpoint Two Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![
            stage("stage1", "orchestrator_stage1.jsonl", false),
            stage("stage2", "orchestrator_stage2.jsonl", false),
        ],
    }
}

struct Harness {
    orchestrator: Orchestrator,
    _template_dir: tempfile::TempDir,
    run_dir: tempfile::TempDir,
    target_dir: tempfile::TempDir,
    dump_dir: tempfile::TempDir,
}

impl Harness {
    fn target(&self) -> PathBuf {
        self.target_dir.path().to_path_buf()
    }

    /// Every argv the mock claude binary saw, oldest call first.
    fn calls(&self) -> Vec<Vec<String>> {
        let mut files: Vec<PathBuf> = match std::fs::read_dir(self.dump_dir.path()) {
            Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
            Err(_) => return Vec::new(),
        };
        files.sort();
        files
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap().lines().map(str::to_string).collect())
            .collect()
    }

    fn call_count(&self) -> usize {
        self.calls().len()
    }

    fn persisted(&self, run_id: &str) -> RunRecord {
        RunRecordStore::new(self.run_dir.path()).load(run_id).unwrap()
    }
}

fn setup_with_timeout(template: Template, stage_timeout: std::time::Duration) -> Harness {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let dump_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&template).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig {
            claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string(),
            stage_timeout,
            extra_env: vec![(
                "MOCK_CLAUDE_DUMP_DIR".to_string(),
                dump_dir.path().to_string_lossy().to_string(),
            )],
        },
    );
    Harness { orchestrator, _template_dir: template_dir, run_dir, target_dir, dump_dir }
}

fn setup_with(template: Template) -> Harness {
    setup_with_timeout(template, std::time::Duration::from_secs(30))
}

fn setup() -> Harness {
    setup_with(two_stage_template())
}

// T-A1
#[tokio::test]
async fn start_run_spawns_nothing_and_returns_awaiting_stage_start() {
    let h = setup();
    let record = h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[1].status, StageStatus::Pending);
    assert_eq!(record.resolved_stages.len(), 2);
    assert_eq!(h.call_count(), 0, "start_run must not spawn a claude process (PRD A-1)");
}

// T-B1 (first half) + T-B2
#[tokio::test]
async fn start_run_writes_both_the_manifest_and_the_app_data_record() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    // B-1: the project folder carries the manifest.
    assert!(manifest_run_path(h.target_dir.path()).exists());
    assert!(manifest_dir(h.target_dir.path()).join("pipeline.json").exists());

    // B-2: the app_data copy is still written; the manifest is additional, not a replacement.
    assert_eq!(h.persisted("run1").status, RunStatus::AwaitingStageStart);
}

// B-3 (IMP-017, decision D4)
#[tokio::test]
async fn start_run_refuses_a_target_dir_that_already_holds_a_run() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let result = h.orchestrator.start_run("two-stage", h.target(), "run2".to_string());

    assert!(matches!(result, Err(OrchestratorError::TargetDirInUse(_))), "got {result:?}");
    // The first run's snapshot is untouched — nothing was silently overwritten.
    let run_json = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();
    assert!(run_json.contains("\"runId\": \"run1\""), "first run's manifest survived: {run_json}");
}

// B-4 (IMP-019, decision D6)
#[tokio::test]
async fn start_run_rejects_a_relative_target_dir_without_writing_anything() {
    let h = setup();
    let result = h.orchestrator.start_run("two-stage", PathBuf::from("relative/dir"), "run1".to_string());

    assert!(matches!(result, Err(OrchestratorError::TargetDir(_))), "got {result:?}");
    assert!(!Path::new("relative/dir").exists(), "a rejected target dir must not be created");
    assert!(RunRecordStore::new(h.run_dir.path()).load("run1").is_err());
}

// D-3 (IMP-015, decision D5)
#[tokio::test]
async fn start_run_surfaces_a_manifest_write_failure() {
    let h = setup();
    // Occupy the manifest directory slot with a plain file so create_dir_all fails.
    std::fs::write(manifest_dir(h.target_dir.path()), "not a directory").unwrap();

    let result = h.orchestrator.start_run("two-stage", h.target(), "run1".to_string());

    assert!(matches!(result, Err(OrchestratorError::ProjectManifest(_))), "got {result:?}");
}

// M1
#[tokio::test]
async fn stale_stage_index_error_carries_the_frontend_prefix() {
    // commands.rs collapses every error to a string, so this prefix is the frontend's
    // only handle on this error (src/api.ts isStaleStageIndexError).
    let e = OrchestratorError::StaleStageIndex { expected: 0, actual: 1 };
    assert!(e.to_string().starts_with("STALE_STAGE_INDEX:"), "got {e}");
}
```

`no_checkpoint_two_stage_template` and `setup_with` are unused until Task 8; add `#[allow(dead_code)]` above each if the compiler's dead-code warning is escalated to an error in this repo. Otherwise leave them — Task 8 uses both.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test`
Expected: FAIL to compile — `start_run` takes 4 arguments and is `async`; `OrchestratorError` has no `TargetDirInUse` / `TargetDir` / `ProjectManifest` / `StaleStageIndex` variants; `engine::project_manifest` is not importable from the integration test until Task 6 landed (it did).

- [ ] **Step 4: Write the implementation**

In `src-tauri/src/engine/orchestrator.rs`, replace lines 1-48 (imports through the closing brace of `start_run`) with:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use crate::template::{
    store::StoreError as TemplateStoreError, store::TemplateStore, validate_stage, validate_template,
    Stage, TemplateValidationError,
};

use super::executor::{run_stage, ExecutorConfig};
use super::project_manifest::{manifest_run_path, validate_target_dir, write_project_manifest, TargetDirError};
use super::run_record::{
    RunRecord, RunRecordStore, RunRecordStoreError, RunStatus, StageOverrideError, StageStatus,
};
use super::stream_json::StageEvent;

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    TemplateStore(#[from] TemplateStoreError),
    #[error(transparent)]
    RunRecordStore(#[from] RunRecordStoreError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("run '{0}' is not awaiting a checkpoint")]
    NotAwaitingCheckpoint(String),
    #[error("run '{0}' is not awaiting a stage start")]
    NotAwaitingStageStart(String),
    #[error("stage override id '{got}' does not match resolved stage '{expected}'")]
    StageIdMismatch { expected: String, got: String },
    // The literal prefix is the frontend's only way to tell this error apart, because
    // commands.rs collapses every error to a string (M1). src/api.ts matches on it in
    // isStaleStageIndexError(); a test pins it so it cannot be edited away.
    #[error("STALE_STAGE_INDEX: run is at stage {actual}, caller expected {expected}")]
    StaleStageIndex { expected: usize, actual: usize },
    #[error(transparent)]
    StageValidation(#[from] TemplateValidationError),
    #[error("failed to write project manifest: {0}")]
    ProjectManifest(String),
    #[error(transparent)]
    TargetDir(#[from] TargetDirError),
    #[error("target dir already contains a pipeline run: {0}")]
    TargetDirInUse(String),
    #[error("run '{0}' cannot be cancelled in its current state")]
    NotCancellable(String),
}

impl From<StageOverrideError> for OrchestratorError {
    fn from(e: StageOverrideError) -> Self {
        match e {
            StageOverrideError::StageIdMismatch { expected, got } => {
                OrchestratorError::StageIdMismatch { expected, got }
            }
        }
    }
}

pub struct Orchestrator {
    pub template_store: TemplateStore,
    pub run_store: RunRecordStore,
    pub executor_config: ExecutorConfig,
    /// One async mutex per run, serializing the load-check-save window of every
    /// state-changing entry point (IMP-016). The outer std mutex guards only the map.
    run_locks: StdMutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Orchestrator {
    pub fn new(template_store: TemplateStore, run_store: RunRecordStore, executor_config: ExecutorConfig) -> Self {
        Self { template_store, run_store, executor_config, run_locks: StdMutex::new(HashMap::new()) }
    }

    fn lock_for(&self, run_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.run_locks.lock().expect("run_locks mutex poisoned");
        locks
            .entry(run_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// The single persistence entry point. Writing the app_data record first keeps
    /// PRD B-2 true (the manifest is additional, never a replacement); writing the
    /// manifest on every transition keeps PRD B-1 true.
    fn save(&self, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.run_store.save(record)?;
        let target = PathBuf::from(&record.target_dir);
        write_project_manifest(&target, record)
            .map_err(|e| OrchestratorError::ProjectManifest(e.to_string()))
    }

    /// IMP-015 (decision D5): if the manifest write fails after the app_data record has
    /// already advanced, put `snapshot` back on disk so the run is left in a state that
    /// still accepts actions (PRD D-3) instead of stranded mid-transition. A failing
    /// rollback is logged and the original error is returned.
    fn save_with_rollback(&self, record: &RunRecord, snapshot: &RunRecord) -> Result<(), OrchestratorError> {
        match self.save(record) {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Err(rollback_err) = self.run_store.save(snapshot) {
                    eprintln!(
                        "run '{}': manifest write failed and the rollback save also failed: {rollback_err}",
                        record.run_id
                    );
                }
                Err(e)
            }
        }
    }

    /// Creates the run record and its manifest. Spawns nothing — the first claude
    /// process appears only when the user calls `start_stage` (PRD A-1).
    pub fn start_run(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
    ) -> Result<RunRecord, OrchestratorError> {
        let template = self.template_store.load(template_id)?;
        validate_template(&template)?;
        validate_target_dir(&target_dir)?;
        // IMP-017 (decision D4): the manifest path is fixed per folder, so a second run
        // here would overwrite the first run's snapshot. Refuse instead (PRD B-3).
        if manifest_run_path(&target_dir).exists() {
            return Err(OrchestratorError::TargetDirInUse(target_dir.to_string_lossy().to_string()));
        }
        let record = RunRecord::new(
            run_id,
            template.id.clone(),
            target_dir.to_string_lossy().to_string(),
            &template.stages,
        );
        self.save(&record)?;
        Ok(record)
    }
```

Everything after this point in the file (`drive`, `approve_checkpoint`, `request_changes`, `reject_checkpoint`, and the closing `}`) is untouched by this task. `validate_stage`, `Stage`, `save_with_rollback` and `lock_for` are unused until Task 8; if unused-import or dead-code warnings block the build, add `#[allow(dead_code)]` on `lock_for`/`save_with_rollback` and drop `validate_stage`/`Stage` from the import list, then restore them in Task 8. Do not delete the code.

- [ ] **Step 5: Fix the callers the new `start_run` signature breaks**

In `src-tauri/src/commands.rs`, replace `start_pipeline_run` (lines 53-67) with:

```rust
#[tauri::command]
pub fn start_pipeline_run(
    orchestrator: State<Orchestrator>,
    template_id: String,
    target_dir: String,
    run_id: String,
) -> Result<RunRecord, String> {
    orchestrator.start_run(&template_id, target_dir.into(), run_id).map_err(|e| e.to_string())
}
```

`app: AppHandle` is gone because `start_run` emits nothing. `emit_stage_event` stays alive — `approve_checkpoint` and `request_changes` still use it.

In the two surviving timeout tests at the bottom of `src-tauri/tests/orchestrator_test.rs`, replace each construction block with the harness and drop the `on_event` argument. For example, `approve_checkpoint_marks_run_failed_instead_of_stuck_running_on_drive_timeout` becomes:

```rust
#[tokio::test]
async fn approve_checkpoint_marks_run_failed_instead_of_stuck_running_on_drive_timeout() {
    let h = setup_with_timeout(
        Template {
            id: "hang-two-stage".to_string(),
            name: "Hang Two Stage".to_string(),
            description: "desc".to_string(),
            stages: vec![
                stage("stage1", "orchestrator_stage1.jsonl", true),
                stage("stage2", "executor_hang.jsonl", true),
            ],
        },
        std::time::Duration::from_millis(100),
    );
    h.orchestrator.start_run("hang-two-stage", h.target(), "run1".to_string()).unwrap();

    let result = h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.stages[1].status, StageStatus::Failed);
    assert_eq!(h.persisted("run1").status, RunStatus::Failed);
}
```

Apply the same harness conversion to `request_changes_marks_run_failed_instead_of_stuck_running_on_run_stage_timeout`, using `setup_with_timeout(two_stage_template(), Duration::from_millis(100))`. Both will still **fail their assertions** at this point, because `start_run` no longer executes stage 1 and so the run is not at a checkpoint. That is expected; Tasks 8 and 9 replace both tests.

- [ ] **Step 6: Run the new tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test start_run`
Expected: PASS — 5 tests.

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test stale_stage_index_error_carries_the_frontend_prefix`
Expected: PASS.

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: PASS — every library unit test.

The two timeout tests and the four older checkpoint tests fail. Write down which ones; Tasks 8 and 9 must account for each.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/engine/orchestrator.rs src-tauri/src/commands.rs src-tauri/tests/orchestrator_test.rs
git commit -m "feat: de-execute start_run and add the dual-write save path

start_run is now synchronous, creates the record plus both manifest files, and
spawns nothing (PRD A-1/B-1/B-2). It refuses a targetDir that already holds a
run (IMP-017) and a non-absolute targetDir (IMP-019). Orchestrator gains a
per-run tokio mutex map (IMP-016) and save_with_rollback, which restores the
pre-transition snapshot when the manifest write fails (IMP-015). The
StaleStageIndex message carries the STALE_STAGE_INDEX: prefix the frontend
needs. The stage-execution path is rewritten in the next commit."
```

---

## Task 8: `start_stage` — the single spawn path (IMP-001, IMP-004, IMP-016)

TRD §3.2, §3.3-(3)(4)(4a), §3.8 / PRD A-1…A-5, D-4. This is the centre of the change. `drive()`'s `loop` is deleted and replaced by `run_current_stage`, which executes exactly one stage and returns; `start_stage` owns every guard and every transition around it. Two independent defences are installed and separately tested: the per-run mutex plus the status guard (concurrent calls, T-D4a) and the `expected_stage_index` generation guard (stale calls arriving after the gate has moved, T-D4b).

**Lock scope is the load-check-save window only.** The child process is awaited *outside* the lock, and so is the post-run transition. Holding it across a multi-minute stage would block that run's own `cancel_run` and defeat IMP-014's escape hatch.

**Files:**
- Modify: `src-tauri/src/engine/orchestrator.rs` — delete `drive()` (originally lines 50-100), add `resume_session_id_for`, `run_current_stage`, `start_stage`
- Test: `src-tauri/tests/orchestrator_test.rs` (append; also replace the two timeout tests patched in Task 7)

**Interfaces:**
- Consumes: Task 2's `validate_stage`; Task 4's `current_resolved_stage`, `apply_stage_override`, `begin_current_stage`, `advance_to_gate`, `mark_current_awaiting_checkpoint`, `mark_current_failed`; Task 7's `lock_for`, `save`, `save_with_rollback`, and the error variants; Task 7's test harness.
- Produces, for Tasks 9/11:
  - `pub async fn start_stage<F: FnMut(&str, StageEvent)>(&self, run_id: &str, expected_stage_index: usize, stage_override: Option<Stage>, on_event: F) -> Result<RunRecord, OrchestratorError>`
  - `async fn run_current_stage<F: FnMut(&str, StageEvent)>(&self, record: &mut RunRecord, resume_session_id: Option<String>, on_event: &mut F) -> Result<(i32, Option<String>), OrchestratorError>` — **M2: returns `(exit_code, latest_session_id)`**, makes no state decisions
  - `fn resume_session_id_for(record: &RunRecord) -> Option<String>` — associated function, no `self`

- [ ] **Step 1: Write the failing tests**

Append to `src-tauri/tests/orchestrator_test.rs`:

```rust
fn resume_arg(call: &[String]) -> Option<&str> {
    call.iter().position(|a| a == "--resume").and_then(|i| call.get(i + 1)).map(String::as_str)
}

// T-A2
#[tokio::test]
async fn non_checkpoint_stage_completion_returns_to_gate() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let record = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    // PRD A-2: no auto-advance. The run parks at the *next* stage's gate.
    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 1);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::AwaitingStart);
    assert_eq!(h.call_count(), 1, "exactly one stage ran");
}

// T-A4
#[tokio::test]
async fn full_pipeline_requires_n_start_stage_calls() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    let record = h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(record.stages[1].status, StageStatus::Approved);
    // PRD A-2: N stages needed exactly N explicit start_stage calls.
    assert_eq!(h.call_count(), 2);
}

// T-A5 — session chaining survives the removal of drive()
#[tokio::test]
async fn start_stage_resumes_previous_stage_session() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    let calls = h.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(resume_arg(&calls[0]), None, "the first stage starts a fresh session");
    assert_eq!(
        resume_arg(&calls[1]),
        Some("sess-orch-1"),
        "the second stage resumes stage 1's session (TRD 3.3-(4a) rule (a))"
    );
    assert_eq!(h.persisted("run1").stages[0].session_id, Some("sess-orch-1".to_string()));
}

// T-A5, checkpoint variant — the same chaining must hold across an approval (T7)
#[tokio::test]
async fn start_stage_after_approve_checkpoint_resumes_the_checkpointed_session() {
    let h = setup(); // stage1 checkpoints
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();
    h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    let calls = h.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(resume_arg(&calls[1]), Some("sess-orch-1"));
}

// T-E1
#[tokio::test]
async fn start_stage_on_non_gate_status_returns_not_awaiting_stage_start() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap(); // now AwaitingCheckpoint

    let result = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::NotAwaitingStageStart(ref id)) if id == "run1"), "got {result:?}");
    assert_eq!(h.call_count(), 1);
}

// T-F1
#[tokio::test]
async fn start_stage_rejects_an_invalid_override() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let mut blank = stage("stage1", "orchestrator_stage1.jsonl", true);
    blank.prompt = "   ".to_string();
    let result = h.orchestrator.start_stage("run1", 0, Some(blank), |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::StageValidation(_))), "got {result:?}");
    assert_eq!(h.call_count(), 0, "a rejected override must not spawn anything");
    assert_eq!(h.persisted("run1").status, RunStatus::AwaitingStageStart, "a rejection is not a transition");
}

#[tokio::test]
async fn start_stage_rejects_an_override_for_a_different_stage() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let wrong = stage("stage2", "orchestrator_stage2.jsonl", false);
    let result = h.orchestrator.start_stage("run1", 0, Some(wrong), |_, _| {}).await;

    assert!(
        matches!(result, Err(OrchestratorError::StageIdMismatch { ref expected, ref got }) if expected == "stage1" && got == "stage2"),
        "got {result:?}"
    );
    assert_eq!(h.call_count(), 0);
}

#[tokio::test]
async fn start_stage_applies_a_valid_override_to_resolved_stages_only() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let mut edited = stage("stage1", "orchestrator_stage1.jsonl", false);
    edited.name = "Renamed by the user".to_string();
    edited.allowed_tools = vec!["Read".to_string(), "Grep".to_string()];
    let record = h.orchestrator.start_stage("run1", 0, Some(edited), |_, _| {}).await.unwrap();

    assert_eq!(record.resolved_stages[0].name, "Renamed by the user");
    assert_eq!(record.resolved_stages[1].name, "Stage stage2", "only the current stage is replaced");
    let calls = h.calls();
    assert!(
        calls[0].windows(2).any(|w| w[0] == "--allowedTools" && w[1] == "Read,Grep"),
        "the override's allowed tools were actually used: {:?}",
        calls[0]
    );
}

// T-B1 (second half)
#[tokio::test]
async fn manifest_is_rewritten_on_each_transition() {
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();
    let after_start_run = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();

    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    let after_stage_one = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();

    assert_ne!(after_start_run, after_stage_one, "the manifest must change on every transition (PRD B-1)");
    assert!(after_stage_one.contains("\"currentStageIndex\": 1"));
    assert!(after_stage_one.contains("\"awaiting-start\""));
}

// T-D4a — mutex + status guard
#[tokio::test]
async fn concurrent_start_stage_spawns_only_one_process() {
    // The two calls race. The winner writes status=Running to disk inside the lock and
    // releases it *before* spawning; the loser then reads Running and trips guard (i).
    // The generation guard is never reached here — see T-D4b for that.
    let h = std::sync::Arc::new(setup_with(no_checkpoint_two_stage_template()));
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let a = { let h = h.clone(); tokio::spawn(async move { h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.map(|_| ()) }) };
    let b = { let h = h.clone(); tokio::spawn(async move { h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.map(|_| ()) }) };
    let (ra, rb) = (a.await.unwrap(), b.await.unwrap());

    assert_eq!(h.call_count(), 1, "the mutex must prevent a second claude process (PRD D-4)");
    let failures: Vec<_> = [ra, rb].into_iter().filter_map(Result::err).collect();
    assert_eq!(failures.len(), 1, "exactly one call is rejected");
    assert!(
        matches!(failures[0], OrchestratorError::NotAwaitingStageStart(ref id) if id == "run1"),
        "got {:?}",
        failures[0]
    );
}

// T-D4b — generation guard, deterministic and sequential
#[tokio::test]
async fn stale_stage_index_is_rejected_after_gate_advance() {
    // A no-checkpoint template with a following stage is the *only* shape that returns
    // the run to AwaitingStageStart at a different index, which is the one path where
    // the status guard passes and only the generation guard can reject.
    let h = setup_with(no_checkpoint_two_stage_template());
    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let after = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    assert_eq!(after.status, RunStatus::AwaitingStageStart);
    assert_eq!(after.current_stage_index, 1);

    // A duplicate click / late retry arrives carrying the stale index 0.
    let result = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await;

    assert!(
        matches!(result, Err(OrchestratorError::StaleStageIndex { expected: 0, actual: 1 })),
        "got {result:?}"
    );
    assert_eq!(h.call_count(), 1, "stage 2 must not have been spawned");
    let persisted = h.persisted("run1");
    assert_eq!(persisted.status, RunStatus::AwaitingStageStart, "a rejection is not a transition");
    assert_eq!(persisted.current_stage_index, 1);
}

// T-A6 replacement for the deleted drive-timeout test (08e9144 precedent)
#[tokio::test]
async fn start_stage_marks_run_failed_instead_of_stuck_running_on_timeout() {
    let h = setup_with_timeout(
        Template {
            id: "hang".to_string(),
            name: "Hang".to_string(),
            description: "desc".to_string(),
            stages: vec![stage("stage1", "executor_hang.jsonl", false)],
        },
        std::time::Duration::from_millis(100),
    );
    h.orchestrator.start_run("hang", h.target(), "run1".to_string()).unwrap();

    // run_current_stage returns Err; start_stage must absorb it, not propagate with `?`.
    let record = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::Failed);
    assert_eq!(record.stages[0].status, StageStatus::Failed);
    let persisted = h.persisted("run1");
    assert_eq!(persisted.status, RunStatus::Failed, "the on-disk record must not be stuck at running");
    assert_eq!(persisted.stages[0].status, StageStatus::Failed);
}

// GAP-3 decision D1: Failed is terminal.
#[tokio::test]
async fn failed_run_rejects_start_stage() {
    let h = setup_with_timeout(
        Template {
            id: "hang2".to_string(),
            name: "Hang2".to_string(),
            description: "desc".to_string(),
            stages: vec![stage("stage1", "executor_hang.jsonl", false), stage("stage2", "orchestrator_stage2.jsonl", false)],
        },
        std::time::Duration::from_millis(100),
    );
    h.orchestrator.start_run("hang2", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    let result = h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::NotAwaitingStageStart(_))), "got {result:?}");
}
```

Delete `approve_checkpoint_marks_run_failed_instead_of_stuck_running_on_drive_timeout` — `drive()` no longer exists, and `start_stage_marks_run_failed_instead_of_stuck_running_on_timeout` above carries its protection forward. Leave `request_changes_marks_run_failed_instead_of_stuck_running_on_run_stage_timeout` in place; Task 9 rewrites it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test`
Expected: FAIL to compile — `no method named 'start_stage' found for struct 'Orchestrator'`.

- [ ] **Step 3: Delete `drive()` and write the replacement**

In `src-tauri/src/engine/orchestrator.rs`, delete the entire `drive` function (the doc comment plus the `async fn drive...` block that originally occupied lines 50-100) and insert in its place:

```rust
    /// TRD 3.3-(4a) rule (a). A stage continues the previous stage's Claude session; the
    /// first stage starts fresh. At any gate `stages[current_stage_index - 1].session_id`
    /// is already populated — `advance_to_gate` fills it for non-checkpoint stages and
    /// `mark_current_awaiting_checkpoint` fills it for checkpointed ones — so this reads
    /// the session the previous stage really used. The value is passed through verbatim,
    /// including the empty string the old `unwrap_or_default()` could produce; that is
    /// existing behaviour and normalizing it is out of scope.
    fn resume_session_id_for(record: &RunRecord) -> Option<String> {
        let idx = record.current_stage_index;
        if idx == 0 {
            None
        } else {
            record.stages[idx - 1].session_id.clone()
        }
    }

    /// Executes exactly one stage — the one `record.current_resolved_stage()` names — and
    /// returns `(exit_code, latest_session_id)`. It makes no state decisions and contains
    /// no loop: `start_stage` and `request_changes` own every transition (M2).
    async fn run_current_stage<F: FnMut(&str, StageEvent)>(
        &self,
        record: &mut RunRecord,
        resume_session_id: Option<String>,
        on_event: &mut F,
    ) -> Result<(i32, Option<String>), OrchestratorError> {
        let stage_index = record.current_stage_index;
        // Cloned because `record` is mutably borrowed for the log append below.
        let stage = record.current_resolved_stage().clone();
        let stage_id = stage.id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let exit_code = run_stage(
            &self.executor_config,
            &stage,
            &target_dir,
            resume_session_id.as_deref(),
            |event| {
                if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                    latest_session_id = Some(session_id.clone());
                }
                collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
                on_event(&stage_id, event);
            },
        )
        .await?;

        record.stages[stage_index].log.extend(collected_log);
        Ok((exit_code, latest_session_id))
    }

    /// The only path in the app that spawns a claude process (PRD A-4).
    ///
    /// `expected_stage_index` is the index the caller believes the run is at. It is a
    /// required argument, not an optional nicety: `stage_override` is `Option`, so the
    /// id check inside `apply_stage_override` cannot be relied on to catch a stale call
    /// (TRD 3.8).
    ///
    /// Lock scope is steps 1-5 only. The child process (step 6) and the post-run
    /// transition (steps 7-8) run outside it, so a multi-minute stage never blocks this
    /// run's own `cancel_run` (TRD 3.3-(3), 3.8).
    pub async fn start_stage<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        expected_stage_index: usize,
        stage_override: Option<Stage>,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let lock = self.lock_for(run_id);
        let mut record = {
            let _guard = lock.lock().await;

            // 1. load
            let mut record = self.run_store.load(run_id)?;

            // 2. guard (i) — status. Also what rejects the loser of a concurrent race,
            //    because the winner has already persisted Running by the time the lock
            //    is handed over (T-D4a).
            if record.status != RunStatus::AwaitingStageStart {
                return Err(OrchestratorError::NotAwaitingStageStart(run_id.to_string()));
            }

            // 2b. guard (ii) — generation. Rejects a call that was composed against an
            //     earlier gate and arrived after the run moved on (T-D4b). A rejection
            //     is not a transition: nothing on disk changes.
            if record.current_stage_index != expected_stage_index {
                return Err(OrchestratorError::StaleStageIndex {
                    expected: expected_stage_index,
                    actual: record.current_stage_index,
                });
            }

            // 3. apply the user's pre-start edits to this run only (IMP-006).
            if let Some(override_stage) = stage_override {
                validate_stage(&override_stage)?;
                record.apply_stage_override(override_stage)?;
            }

            // 4-5. persist Running before spawning (0235a40 precedent), rolling back if
            //      the manifest write fails (IMP-015).
            let snapshot = record.clone();
            record.begin_current_stage();
            self.save_with_rollback(&record, &snapshot)?;

            record
        }; // 5. lock released here — before the spawn.

        // 6. run the stage, outside the lock.
        let resume = Self::resume_session_id_for(&record);
        let outcome = self.run_current_stage(&mut record, resume, &mut on_event).await;

        // 7. absorb the outcome. An Err is never propagated with `?` — that is exactly
        //    how 08e9144 stranded a run at status=running.
        match outcome {
            Ok((0, session_id)) => {
                let session_id = session_id.unwrap_or_default();
                if record.current_resolved_stage().checkpoint {
                    record.mark_current_awaiting_checkpoint(session_id); // T3
                } else {
                    record.advance_to_gate(session_id); // T4 / T5
                }
            }
            _ => record.mark_current_failed(), // T6
        }

        // 8. persist the result.
        self.save(&record)?;
        Ok(record)
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test start_stage`
Expected: PASS.

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test`
Expected: the new tests plus all `start_run*` tests PASS. `approve_checkpoint_advances_and_then_completes`, `request_changes_reruns_current_stage_and_stays_awaiting_checkpoint`, `reject_checkpoint_cancels_run`, `reject_checkpoint_rejects_a_run_not_awaiting_checkpoint` and the surviving `request_changes` timeout test still fail or were removed in Task 7's edit — Task 9 finishes them.

Note that `start_stage_after_approve_checkpoint_resumes_the_checkpointed_session` will fail until Task 9, because today's `approve_checkpoint` still calls `drive`. If it does not compile because `drive` was deleted, temporarily replace `approve_checkpoint`'s `drive(...)` call with `record.approve_current();` and no execution — Task 9 writes the final version regardless.

- [ ] **Step 5: Confirm the generation guard is load-bearing (negative control, manual, once)**

Temporarily comment out guard (ii) in `start_stage`, then run:
`cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test stale_stage_index_is_rejected_after_gate_advance`
Expected: FAIL — `assertion left == right failed` on `call_count`, left `2`, right `1`. Stage 2 got spawned without the user ever seeing it.

Then run the same check on the concurrency test:
`cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test concurrent_start_stage_spawns_only_one_process`
Expected: still PASS — the status guard covers that path, which is precisely why the two tests are not interchangeable.

Restore guard (ii). Re-run both: both PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/orchestrator.rs src-tauri/tests/orchestrator_test.rs
git commit -m "feat: replace drive() with start_stage, the single spawn path (IMP-001/004/016)

The auto-advance loop is gone. run_current_stage executes exactly one stage and
returns (exit_code, session_id); start_stage owns the guards and transitions.
A non-checkpoint stage now returns to the next stage's gate instead of spawning
(PRD A-2), so an N-stage pipeline needs N explicit start_stage calls.

Two independent defences, separately tested: the per-run mutex plus the status
guard stop a concurrent double-spawn, and expected_stage_index stops a stale
call that arrives after the gate advanced. Session chaining is preserved —
stage N resumes stage N-1's Claude session (TRD 3.3-(4a))."
```

---

## Task 9: De-execute `approve_checkpoint`, retarget `request_changes`, add `cancel_run` (IMP-001, IMP-007, IMP-014)

TRD §3.3-(5), §3.6, §3.7 / PRD A-3, C-3, C-4, D-1, D-2. Three related changes to the remaining entry points. `approve_checkpoint` stops spawning and stops re-loading the template. `request_changes` reads the stage to re-run from `resolved_stages` instead of the saved template, which is what makes pre-start edits survive a revision round (C-3) and stops mid-run template edits leaking in (C-4). `cancel_run` gives the pre-stage gate an exit other than "run it" (D-2).

Per decision D1, `Failed` is terminal: `cancel_run`'s guard admits `AwaitingCheckpoint` and `AwaitingStageStart` **only**.

**Files:**
- Modify: `src-tauri/src/engine/orchestrator.rs` — `approve_checkpoint`, `request_changes`, add `cancel_run`; `reject_checkpoint` unchanged
- Test: `src-tauri/tests/orchestrator_test.rs` (append; rewrite the surviving `request_changes` timeout test)

**Interfaces:**
- Consumes: Task 4's `approve_current`, `current_resolved_stage`, `cancel`; Task 7's `lock_for`, `save`, `save_with_rollback`, `NotCancellable`; Task 8's `run_current_stage`.
- Produces, for Task 11:
  - `pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(&self, run_id: &str, on_event: F) -> Result<RunRecord, OrchestratorError>` — signature unchanged; **spawns nothing now**. `on_event` is retained so `commands.rs` and its callers need no change, and is simply never invoked.
  - `pub async fn request_changes<F: FnMut(&str, StageEvent)>(&self, run_id: &str, feedback: &str, on_event: F) -> Result<RunRecord, OrchestratorError>` — signature unchanged
  - `pub fn cancel_run(&self, run_id: &str) -> Result<RunRecord, OrchestratorError>` — synchronous
  - `pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError>` — unchanged, still returns `NotAwaitingCheckpoint`, which `orchestrator_test.rs` asserts

- [ ] **Step 1: Write the failing tests**

Append to `src-tauri/tests/orchestrator_test.rs`, and delete the old `approve_checkpoint_advances_and_then_completes`, `request_changes_reruns_current_stage_and_stays_awaiting_checkpoint`, and `request_changes_marks_run_failed_instead_of_stuck_running_on_run_stage_timeout` (replaced below). Keep `reject_checkpoint_cancels_run` and `reject_checkpoint_rejects_a_run_not_awaiting_checkpoint`, converting both to the harness (`h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();` followed by `h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();` to reach the checkpoint).

```rust
// T-A3
#[tokio::test]
async fn approve_checkpoint_does_not_spawn_and_returns_to_gate() {
    let h = setup(); // stage1 checkpoints, stage2 does not
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    let record = h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();

    // PRD A-3: approval advances to the next stage's gate, it does not start it.
    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 1);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::AwaitingStart);
    assert_eq!(h.call_count(), 1, "approve_checkpoint must not spawn (PRD A-4)");
}

#[tokio::test]
async fn approve_checkpoint_on_the_last_stage_completes_the_run() {
    let h = setup_with(Template {
        id: "single-cp".to_string(),
        name: "Single".to_string(),
        description: "desc".to_string(),
        stages: vec![stage("stage1", "orchestrator_stage1.jsonl", true)],
    });
    h.orchestrator.start_run("single-cp", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    let record = h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(h.call_count(), 1);
}

#[tokio::test]
async fn approve_checkpoint_rejects_a_run_that_is_not_at_a_checkpoint() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let result = h.orchestrator.approve_checkpoint("run1", |_, _| {}).await;

    assert!(matches!(result, Err(OrchestratorError::NotAwaitingCheckpoint(ref id)) if id == "run1"), "got {result:?}");
}

#[tokio::test]
async fn request_changes_reruns_the_current_stage_and_stays_at_the_checkpoint() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    let record = h
        .orchestrator
        .request_changes("run1", &fixture_prompt("orchestrator_feedback.jsonl"), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-revised".to_string()));
    assert_eq!(h.call_count(), 2);
}

// T-A6 — TRD 3.3-(4a) rule (b): a re-run resumes its OWN stage's session,
// not the previous stage's.
#[tokio::test]
async fn request_changes_resumes_own_stage_session() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    h.orchestrator
        .request_changes("run1", &fixture_prompt("orchestrator_feedback.jsonl"), |_, _| {})
        .await
        .unwrap();

    let calls = h.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(resume_arg(&calls[1]), Some("sess-orch-1"), "stage 1's own session, not stage 0's");
}

// T-C3
#[tokio::test]
async fn request_changes_inherits_pre_start_edits() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    // Pre-start edit: change permission mode and allowed tools for this run only.
    let mut edited = stage("stage1", "orchestrator_stage1.jsonl", true);
    edited.permission_mode = PermissionMode::BypassPermissions;
    edited.allowed_tools = vec!["Read".to_string(), "Bash".to_string()];
    h.orchestrator.start_stage("run1", 0, Some(edited), |_, _| {}).await.unwrap();

    h.orchestrator
        .request_changes("run1", &fixture_prompt("orchestrator_feedback.jsonl"), |_, _| {})
        .await
        .unwrap();

    let calls = h.calls();
    let rerun = &calls[1];
    assert!(
        rerun.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "bypassPermissions"),
        "the re-run kept the edited permission mode: {rerun:?}"
    );
    assert!(
        rerun.windows(2).any(|w| w[0] == "--allowedTools" && w[1] == "Read,Bash"),
        "the re-run kept the edited allowed tools: {rerun:?}"
    );
}

// T-C4
#[tokio::test]
async fn template_edited_mid_run_does_not_leak_into_the_run() {
    let h = setup(); // stage1 checkpoints, stage2 does not
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    // Someone edits the saved template while the run sits at its checkpoint.
    let mut template = h.orchestrator.template_store.load("two-stage").unwrap();
    template.stages[1].prompt = fixture_prompt("orchestrator_feedback.jsonl");
    template.stages[1].allowed_tools = vec!["Bash".to_string()];
    h.orchestrator.template_store.save(&template).unwrap();

    h.orchestrator.approve_checkpoint("run1", |_, _| {}).await.unwrap();
    let record = h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    // The run used its own resolvedStages copy, not the edited template.
    assert_eq!(record.resolved_stages[1].prompt, fixture_prompt("orchestrator_stage2.jsonl"));
    let calls = h.calls();
    assert!(
        !calls[1].iter().any(|a| a == "--allowedTools"),
        "the mid-run template edit must not reach the spawned process: {:?}",
        calls[1]
    );
}

// Replacement for the deleted request_changes timeout test (08e9144 precedent)
#[tokio::test]
async fn request_changes_marks_run_failed_instead_of_stuck_running_on_timeout() {
    let h = setup_with_timeout(two_stage_template(), std::time::Duration::from_millis(100));
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    let record = h
        .orchestrator
        .request_changes("run1", &fixture_prompt("executor_hang.jsonl"), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::Failed);
    assert_eq!(record.stages[0].status, StageStatus::Failed);
    assert_eq!(h.persisted("run1").status, RunStatus::Failed);
}

// T-D1 / PRD D-1, D-2
#[tokio::test]
async fn cancel_run_from_awaiting_stage_start_sets_cancelled() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();

    let record = h.orchestrator.cancel_run("run1").unwrap();

    assert_eq!(record.status, RunStatus::Cancelled);
    assert_eq!(h.persisted("run1").status, RunStatus::Cancelled, "the on-disk record must reflect it too");
    // The manifest is refreshed by the same transition.
    let run_json = std::fs::read_to_string(manifest_run_path(h.target_dir.path())).unwrap();
    assert!(run_json.contains("\"cancelled\""));
}

#[tokio::test]
async fn cancel_run_from_awaiting_checkpoint_sets_cancelled() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();

    assert_eq!(h.orchestrator.cancel_run("run1").unwrap().status, RunStatus::Cancelled);
}

// Decision D1: Failed is terminal, so it is not cancellable either.
#[tokio::test]
async fn cancel_run_rejects_a_terminal_run() {
    let h = setup();
    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.cancel_run("run1").unwrap();

    let result = h.orchestrator.cancel_run("run1");

    assert!(matches!(result, Err(OrchestratorError::NotCancellable(ref id)) if id == "run1"), "got {result:?}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test`
Expected: FAIL to compile — `no method named 'cancel_run'`. Once that is stubbed, `approve_checkpoint_does_not_spawn_and_returns_to_gate` fails with `call_count` left `2`, right `1`.

- [ ] **Step 3: Write the implementation**

In `src-tauri/src/engine/orchestrator.rs`, replace `approve_checkpoint` and `request_changes` entirely, and add `cancel_run` before `reject_checkpoint`:

```rust
    /// Approving a checkpoint no longer starts the next stage. It advances to the next
    /// stage's gate and returns (PRD A-3), so this method is now purely synchronous
    /// work. `on_event` is kept in the signature — and never called — so the Tauri
    /// command and its callers do not change shape.
    pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        _on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let lock = self.lock_for(run_id);
        let _guard = lock.lock().await;

        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        // No template_store.load here any more: the run's own resolved_stages are the
        // source of truth, which is half of what closes the C-4 leak (IMP-007).
        let snapshot = record.clone();
        record.approve_current();
        self.save_with_rollback(&record, &snapshot)?;
        Ok(record)
    }

    /// Re-runs the current stage with the user's feedback as its prompt.
    ///
    /// The stage definition comes from `resolved_stages`, so pre-start edits to
    /// permission mode / allowed tools / checkpoint are inherited (PRD C-3) and a
    /// mid-run template edit cannot leak in (PRD C-4). The prompt override is applied to
    /// a local clone only and is deliberately *not* written back to `resolved_stages` —
    /// feedback is one-shot (design decision, and IMP-020 decision D7 keeps it unlogged).
    pub async fn request_changes<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        feedback: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let lock = self.lock_for(run_id);
        let mut record = {
            let _guard = lock.lock().await;

            let mut record = self.run_store.load(run_id)?;
            if record.status != RunStatus::AwaitingCheckpoint {
                return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
            }
            // Persist Running before spawning (0235a40 precedent).
            let snapshot = record.clone();
            record.status = RunStatus::Running;
            record.current_stage_mut().status = StageStatus::Running;
            self.save_with_rollback(&record, &snapshot)?;
            record
        }; // lock released before the spawn, same scope rule as start_stage.

        // TRD 3.3-(4a) rule (b): a re-run continues its OWN stage's session.
        let resume_session_id = record.stages[record.current_stage_index].session_id.clone();

        // Swap in the feedback prompt on a throwaway clone of the resolved stage.
        let stage_index = record.current_stage_index;
        let mut feedback_stage = record.current_resolved_stage().clone();
        feedback_stage.prompt = feedback.to_string();
        let stage_id = feedback_stage.id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let result = run_stage(
            &self.executor_config,
            &feedback_stage,
            &target_dir,
            resume_session_id.as_deref(),
            |event| {
                if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                    latest_session_id = Some(session_id.clone());
                }
                collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
                on_event(&stage_id, event);
            },
        )
        .await;

        record.stages[stage_index].log.extend(collected_log);

        match result {
            Ok(0) => record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default()),
            _ => record.mark_current_failed(),
        }
        self.save(&record)?;
        Ok(record)
    }

    /// IMP-014 / PRD D-1, D-2. A run parked at a pre-stage gate previously had exactly
    /// one available action — "run it" — which is the shape of the 08e9144 stuck-state
    /// bug. This widens the escape hatch to both waiting states.
    ///
    /// `Failed` is deliberately absent: decision D1 keeps it terminal, so a failed run is
    /// already in a final state and does not need cancelling.
    pub fn cancel_run(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if !matches!(record.status, RunStatus::AwaitingCheckpoint | RunStatus::AwaitingStageStart) {
            return Err(OrchestratorError::NotCancellable(run_id.to_string()));
        }
        record.cancel();
        self.save(&record)?;
        Ok(record)
    }
```

Leave `reject_checkpoint` exactly as it is except for routing its save through the new helper — change its `self.run_store.save(&record)?;` to `self.save(&record)?;`. Its `AwaitingCheckpoint`-only guard and its `NotAwaitingCheckpoint` error variant stay, because `reject_checkpoint_rejects_a_run_not_awaiting_checkpoint` asserts that variant.

`cancel_run` takes no lock: it is synchronous, and a `tokio::sync::Mutex` cannot be acquired from a non-async function. Its load-check-save window is short and its only interleaving risk is with a running stage, which by design releases the lock before spawning anyway — so a lock here would not add protection. Note this in the code comment.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test`
Expected: PASS — every test in the file, including `start_stage_after_approve_checkpoint_resumes_the_checkpointed_session` from Task 8.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — the whole backend suite.

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`
Expected: no new warnings. If `_on_event`'s generic parameter triggers an unused-type-parameter lint, that is fine and expected; do not remove the parameter.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/orchestrator.rs src-tauri/tests/orchestrator_test.rs
git commit -m "feat: de-execute approve_checkpoint, retarget request_changes, add cancel_run

approve_checkpoint advances to the next stage's gate instead of spawning it
(PRD A-3) and no longer reloads the saved template. request_changes reads the
stage to re-run from resolved_stages, so pre-start edits survive a revision
round (C-3) and a mid-run template edit cannot leak in (C-4); it still resumes
its own stage's session. cancel_run accepts both waiting states, so a run
parked at a pre-stage gate is no longer action-locked to 'run it' (IMP-014).

Per decision D1, Failed stays terminal and is not cancellable."
```

---

## Task 10: Pin the template-store isolation invariant (IMP-006)

TRD §3.5 / PRD C-1, C-2. The isolation itself already holds by construction — `apply_stage_override` only touches `record.resolved_stages`, and no orchestrator path calls `template_store.save`. What PRD C-2 actually asks for is a **test that fails if someone ever adds such a path**. That is this task, and it is deliberately its own reviewable unit: the assertion is the deliverable.

**Files:**
- Test: `src-tauri/tests/orchestrator_test.rs` (append)

**Interfaces:**
- Consumes: Task 7's harness, Task 8's `start_stage`, Task 9's `approve_checkpoint`/`request_changes`.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/tests/orchestrator_test.rs`:

```rust
fn template_file_bytes(h: &Harness, template_id: &str) -> Vec<u8> {
    // TemplateStore stores one JSON file per template, named by id.
    let path = h._template_dir.path().join(format!("{template_id}.json"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

// T-C1
#[tokio::test]
async fn full_run_never_writes_the_template_store() {
    let h = setup_with(no_checkpoint_two_stage_template());
    let before = template_file_bytes(&h, "no-cp-two-stage");

    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    assert_eq!(
        before,
        template_file_bytes(&h, "no-cp-two-stage"),
        "a full run must never write the saved template (PRD C-1)"
    );
}

// T-C2
#[tokio::test]
async fn stage_override_does_not_touch_the_saved_template() {
    let h = setup_with(no_checkpoint_two_stage_template());
    let before = template_file_bytes(&h, "no-cp-two-stage");

    h.orchestrator.start_run("no-cp-two-stage", h.target(), "run1".to_string()).unwrap();

    let mut edited = stage("stage1", "orchestrator_stage1.jsonl", false);
    edited.name = "Edited for this run only".to_string();
    edited.permission_mode = PermissionMode::BypassPermissions;
    h.orchestrator.start_stage("run1", 0, Some(edited), |_, _| {}).await.unwrap();
    let record = h.orchestrator.start_stage("run1", 1, None, |_, _| {}).await.unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    // The edit is visible in the run...
    assert_eq!(record.resolved_stages[0].name, "Edited for this run only");
    // ...and byte-for-byte absent from the template (PRD C-2).
    assert_eq!(before, template_file_bytes(&h, "no-cp-two-stage"));
}

#[tokio::test]
async fn request_changes_does_not_touch_the_saved_template() {
    let h = setup(); // stage1 checkpoints
    let before = template_file_bytes(&h, "two-stage");

    h.orchestrator.start_run("two-stage", h.target(), "run1".to_string()).unwrap();
    h.orchestrator.start_stage("run1", 0, None, |_, _| {}).await.unwrap();
    h.orchestrator
        .request_changes("run1", &fixture_prompt("orchestrator_feedback.jsonl"), |_, _| {})
        .await
        .unwrap();

    assert_eq!(before, template_file_bytes(&h, "two-stage"));
}
```

`Harness._template_dir` is currently named with a leading underscore because Task 7 only held it to keep the temp dir alive. Rename it to `template_dir` in the struct, in `setup_with_timeout`, and in this helper — the field is now genuinely read.

- [ ] **Step 2: Run the tests to verify they pass immediately, then prove they can fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test orchestrator_test template`
Expected: PASS on the first run — this invariant already holds, which is the point.

A test that has never been red proves nothing. Temporarily add `let _ = self.template_store.save(&template);` right after the `validate_template(&template)?;` line in `start_run`, then re-run:
Expected: FAIL — `assertion left == right failed` on the byte comparison in `full_run_never_writes_the_template_store` (the re-serialized file differs, or at minimum the mtime-independent byte compare catches any formatting change).

If the bytes happen to be identical because the round-trip is stable, strengthen the negative control instead: make the temporary line mutate the template first (`let mut t = template.clone(); t.description = "leaked".into(); let _ = self.template_store.save(&t);`). Confirm the test goes red, then remove the temporary line and confirm it goes green again.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/orchestrator_test.rs
git commit -m "test: pin the run-scoped edit isolation invariant (IMP-006)

PRD C-1/C-2 ask for an automated test that fails if a write-back path to the
saved template is ever introduced. A full run, an overridden run, and a
request_changes round each assert the template file is byte-for-byte unchanged.
Verified red by temporarily adding a template_store.save call to start_run."
```

---

## Task 11: Tauri commands and registration (IMP-004, IMP-010, IMP-014, M3)

TRD §3.3-(6)(7), §3.11 / PRD A-4, D-1. Exposes the three new backend entry points over IPC. `get_run` is the read-only command the frontend's stale-index recovery needs — it is specified in TRD §3.11 but missing from §3.15 and 부록 A, which is minor item M3.

**Files:**
- Modify: `src-tauri/src/commands.rs` (add `start_stage`, `cancel_run`, `get_run`)
- Modify: `src-tauri/src/lib.rs:28-38` (`generate_handler!`)
- Test: covered by `cargo check` plus the frontend contract tests in Task 12; Tauri commands are not directly unit-testable here

**Interfaces:**
- Consumes: Task 8's `Orchestrator::start_stage`; Task 9's `cancel_run`; `run_store.load`.
- Produces, for Task 12's `api.ts`, with these **exact** IPC names and argument keys (Tauri converts `snake_case` parameters to `camelCase` on the JS side):
  - `start_stage` ← `{ runId, expectedStageIndex, stageOverride }` → `RunRecord`
  - `cancel_run` ← `{ runId }` → `RunRecord`
  - `get_run` ← `{ runId }` → `RunRecord`
  - `start_pipeline_run` ← `{ templateId, targetDir, runId }` → `RunRecord` (unchanged keys; now synchronous and non-spawning)

- [ ] **Step 1: Add the three commands**

In `src-tauri/src/commands.rs`, append after `reject_checkpoint`:

```rust
#[tauri::command]
pub async fn start_stage(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
    expected_stage_index: usize,
    stage_override: Option<crate::template::Stage>,
) -> Result<RunRecord, String> {
    orchestrator
        .start_stage(&run_id, expected_stage_index, stage_override, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_run(orchestrator: State<Orchestrator>, run_id: String) -> Result<RunRecord, String> {
    orchestrator.cancel_run(&run_id).map_err(|e| e.to_string())
}

/// Read-only. The frontend calls this to resynchronize after a STALE_STAGE_INDEX
/// rejection (TRD 3.10). It changes no state, so it takes no run lock.
#[tauri::command]
pub fn get_run(orchestrator: State<Orchestrator>, run_id: String) -> Result<RunRecord, String> {
    orchestrator.run_store.load(&run_id).map_err(|e| e.to_string())
}
```

`pipeline://stage-event` emission has now moved from `start_pipeline_run` (which no longer takes `AppHandle`) to `start_stage`. `emit_stage_event` itself is unchanged.

- [ ] **Step 2: Register them**

In `src-tauri/src/lib.rs`, replace the `invoke_handler` block (lines 28-38) with:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::list_templates,
            commands::load_template,
            commands::save_template,
            commands::delete_template,
            commands::check_cli,
            commands::start_pipeline_run,
            commands::start_stage,
            commands::approve_checkpoint,
            commands::request_changes,
            commands::reject_checkpoint,
            commands::cancel_run,
            commands::get_run,
        ])
```

- [ ] **Step 3: Verify the whole backend compiles and passes**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`
Expected: no errors. A missing `generate_handler!` registration shows up as a runtime "command not found" rather than a compile error, so re-read the list above against the three `#[tauri::command]` functions you just added and confirm all three are present.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — the entire backend suite.

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`
Expected: no new warnings.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: expose start_stage, cancel_run and get_run over IPC

start_stage takes expectedStageIndex and an optional stageOverride and now owns
pipeline://stage-event emission, which start_pipeline_run gave up when it
stopped spawning. get_run is the read-only command the frontend uses to
resynchronize after a STALE_STAGE_INDEX rejection (TRD 3.11 / minor item M3)."
```

---

## Task 12: Frontend types and API wrappers (IMP-010, M1, M3)

TRD §3.11 / PRD E-5. `src/types.ts` is a hand-maintained mirror of the Rust IPC types with no compile-time link, so it drifts silently — this task brings it back in step and adds the four api.ts functions the run screen needs.

**Files:**
- Modify: `src/types.ts:19-36`
- Modify: `src/api.ts` (append)
- Test: `src/api.test.ts` (append; update the import list)

**Interfaces:**
- Consumes: Task 11's IPC names and argument keys.
- Produces, for Tasks 13-15:
  - `type StageStatus = "pending" | "awaiting-start" | "running" | "awaiting-checkpoint" | "approved" | "failed"`
  - `type RunStatus = "running" | "awaiting-stage-start" | "awaiting-checkpoint" | "completed" | "failed" | "cancelled"`
  - `interface RunRecord { ...; resolvedStages: Stage[] }`
  - `startStage(runId: string, expectedStageIndex: number, stageOverride?: Stage): Promise<RunRecord>`
  - `cancelRun(runId: string): Promise<RunRecord>`
  - `getRun(runId: string): Promise<RunRecord>`
  - `isStaleStageIndexError(error: unknown): boolean`
  - `const STALE_STAGE_INDEX_PREFIX = "STALE_STAGE_INDEX:"`

- [ ] **Step 1: Write the failing tests**

In `src/api.test.ts`, extend the import at lines 8-19 to include `startStage`, `cancelRun`, `getRun`, `isStaleStageIndexError`, and add `import type { Stage, Template } from "./types";`. Then append inside the `describe("api wrapper", ...)` block:

```ts
  it("startStage invokes start_stage with runId, expectedStageIndex, and the override", async () => {
    invokeMock.mockResolvedValue({});
    const stage: Stage = {
      id: "s1",
      name: "요구사항",
      prompt: "PRD 작성",
      permissionMode: "acceptEdits",
      allowedTools: ["Read"],
      checkpoint: true,
    };
    await startStage("run1", 2, stage);
    expect(invokeMock).toHaveBeenCalledWith("start_stage", {
      runId: "run1",
      expectedStageIndex: 2,
      stageOverride: stage,
    });
  });

  it("startStage sends null rather than undefined when no override is given", async () => {
    invokeMock.mockResolvedValue({});
    await startStage("run1", 0);
    expect(invokeMock).toHaveBeenCalledWith("start_stage", {
      runId: "run1",
      expectedStageIndex: 0,
      stageOverride: null,
    });
  });

  it("cancelRun invokes cancel_run with runId", async () => {
    invokeMock.mockResolvedValue({});
    await cancelRun("run1");
    expect(invokeMock).toHaveBeenCalledWith("cancel_run", { runId: "run1" });
  });

  it("getRun invokes get_run with runId", async () => {
    invokeMock.mockResolvedValue({});
    await getRun("run1");
    expect(invokeMock).toHaveBeenCalledWith("get_run", { runId: "run1" });
  });

  it("isStaleStageIndexError recognizes the backend prefix in every shape it can arrive in", () => {
    // Tauri rejects with the raw string; a caller may also wrap it in an Error.
    expect(isStaleStageIndexError("STALE_STAGE_INDEX: run is at stage 1, caller expected 0")).toBe(true);
    expect(isStaleStageIndexError(new Error("STALE_STAGE_INDEX: run is at stage 1, caller expected 0"))).toBe(true);
    expect(isStaleStageIndexError("run 'run1' is not awaiting a stage start")).toBe(false);
    expect(isStaleStageIndexError(null)).toBe(false);
    expect(isStaleStageIndexError(undefined)).toBe(false);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/api.test.ts`
Expected: FAIL — `No "startStage" export is defined on the "./api" mock` / TypeScript cannot resolve `startStage`, `cancelRun`, `getRun`, `isStaleStageIndexError`.

- [ ] **Step 3: Update the types**

In `src/types.ts`, replace lines 19-20 and the `RunRecord` interface (lines 29-36) with:

```ts
export type StageStatus =
  | "pending"
  | "awaiting-start"
  | "running"
  | "awaiting-checkpoint"
  | "approved"
  | "failed";

export type RunStatus =
  | "running"
  | "awaiting-stage-start"
  | "awaiting-checkpoint"
  | "completed"
  | "failed"
  | "cancelled";

export interface StageRun {
  id: string;
  status: StageStatus;
  sessionId: string | null;
  log: unknown[];
}

export interface RunRecord {
  runId: string;
  templateId: string;
  targetDir: string;
  status: RunStatus;
  currentStageIndex: number;
  stages: StageRun[];
  /** The run's own copy of the pipeline. Pre-start edits land here, never in the template. */
  resolvedStages: Stage[];
}
```

(The `StageRun` interface at lines 22-27 is unchanged; it is repeated here only because it sits between the two edited regions.)

- [ ] **Step 4: Add the API wrappers**

In `src/api.ts`, append after `rejectCheckpoint`:

```ts
/**
 * Starts the stage the run is currently parked on. `expectedStageIndex` is required,
 * not optional: it is the generation guard the backend uses to reject a click that was
 * composed against an earlier gate. Pass the same `run.currentStageIndex` the panel
 * rendered from.
 */
export function startStage(
  runId: string,
  expectedStageIndex: number,
  stageOverride?: Stage
): Promise<RunRecord> {
  return invoke("start_stage", { runId, expectedStageIndex, stageOverride: stageOverride ?? null });
}

export function cancelRun(runId: string): Promise<RunRecord> {
  return invoke("cancel_run", { runId });
}

/** Read-only refetch, used to resynchronize after a stale-stage-index rejection. */
export function getRun(runId: string): Promise<RunRecord> {
  return invoke("get_run", { runId });
}

/**
 * Tauri commands collapse every backend error to a string, so this prefix — set on the
 * Rust side in OrchestratorError::StaleStageIndex — is the only way to tell this error
 * apart from any other. Keep the two in sync; a Rust test pins the prefix.
 */
export const STALE_STAGE_INDEX_PREFIX = "STALE_STAGE_INDEX:";

export function isStaleStageIndexError(error: unknown): boolean {
  if (error === null || error === undefined) return false;
  const text = typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
  return text.includes(STALE_STAGE_INDEX_PREFIX);
}
```

Extend the type import on line 3 to `import type { RunRecord, Stage, StageEventPayload, Template } from "./types";`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run src/api.test.ts`
Expected: PASS — the 10 original tests plus the 5 new ones.

Run: `npx tsc --noEmit`
Expected: errors in `src/App.tsx` (the optimistic `pendingRun` now lacks `resolvedStages`) and in `src/pages/PipelineRun.test.tsx` / `src/App.test.tsx` (their `RunRecord` fixtures lack it too). Those are fixed in Tasks 14 and 15. No errors in `src/api.ts` or `src/types.ts`.

- [ ] **Step 6: Commit**

```bash
git add src/types.ts src/api.ts src/api.test.ts
git commit -m "feat: mirror the new run states and commands in the frontend types and api

types.ts gains awaiting-start / awaiting-stage-start / resolvedStages. api.ts
gains startStage (with the required expectedStageIndex generation-guard
argument), cancelRun, getRun, and isStaleStageIndexError — the single place
that matches the backend's STALE_STAGE_INDEX: error prefix (minor item M1)."
```

---

## Task 13: Shared `StageFields` component (IMP-008)

TRD §3.9 / PRD E-4. The stage form lives inline inside `TemplateEditor.tsx` today, so the run screen cannot reuse it. Extract it into `src/components/` — a directory that does not exist yet — with an `idPrefix` so two instances can coexist, a `promptLabel` so each caller keeps its own wording, and a `lockId` flag that **disables the stage-id input without hiding it**: changing a stage id mid-run would desynchronize `StageRun.id` from `resolvedStages` and trip the backend's `StageIdMismatch`.

**Files:**
- Create: `src/components/StageFields.tsx`
- Create: `src/components/StageFields.test.tsx`
- Modify: `src/pages/TemplateEditor.tsx` (remove lines 12-13's constants; replace the inline form at lines 206-294)

**Interfaces:**
- Consumes: `Stage`, `PermissionMode` from `src/types.ts`.
- Produces, for Task 14:
  - `export const PERMISSION_MODES: PermissionMode[]`
  - `export const KNOWN_TOOLS: string[]`
  - `export default function StageFields(props: { stage: Stage; onChange: (patch: Partial<Stage>) => void; idPrefix: string; promptLabel: string; lockId: boolean })`
  - Input ids are `${idPrefix}-name`, `${idPrefix}-id`, `${idPrefix}-prompt`.
  - Accessible names the tests query by: `단계 이름`, `단계 ID`, the caller's `promptLabel`, and `체크포인트에서 일시 정지` (the switch's `aria-label`).

- [ ] **Step 1: Write the failing tests**

Create `src/components/StageFields.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import StageFields from "./StageFields";
import type { Stage } from "../types";

const stage: Stage = {
  id: "s1",
  name: "요구사항",
  prompt: "PRD 작성",
  permissionMode: "acceptEdits",
  allowedTools: ["Read"],
  checkpoint: true,
};

function renderFields(overrides: Partial<React.ComponentProps<typeof StageFields>> = {}) {
  const onChange = vi.fn();
  render(
    <StageFields
      stage={stage}
      onChange={onChange}
      idPrefix="test-stage"
      promptLabel="이 단계 프롬프트"
      lockId={false}
      {...overrides}
    />
  );
  return onChange;
}

describe("StageFields", () => {
  it("renders every stage field", () => {
    renderFields();
    expect(screen.getByLabelText("단계 이름")).toHaveValue("요구사항");
    expect(screen.getByLabelText("단계 ID")).toHaveValue("s1");
    expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("PRD 작성");
    expect(screen.getByRole("button", { name: "체크포인트에서 일시 정지" })).toBeInTheDocument();
  });

  it("uses idPrefix for input ids so two instances can coexist", () => {
    renderFields();
    expect(screen.getByLabelText("단계 이름")).toHaveAttribute("id", "test-stage-name");
    expect(screen.getByLabelText("단계 ID")).toHaveAttribute("id", "test-stage-id");
    expect(screen.getByLabelText("이 단계 프롬프트")).toHaveAttribute("id", "test-stage-prompt");
  });

  it("emits a patch when the prompt changes", () => {
    const onChange = renderFields();
    fireEvent.change(screen.getByLabelText("이 단계 프롬프트"), { target: { value: "새 프롬프트" } });
    expect(onChange).toHaveBeenCalledWith({ prompt: "새 프롬프트" });
  });

  it("emits a patch when a permission mode is picked", () => {
    const onChange = renderFields();
    fireEvent.click(screen.getByRole("button", { name: "bypassPermissions" }));
    expect(onChange).toHaveBeenCalledWith({ permissionMode: "bypassPermissions" });
  });

  it("toggles an allowed tool on and off", () => {
    const onChange = renderFields();
    fireEvent.click(screen.getByRole("button", { name: "Grep" }));
    expect(onChange).toHaveBeenCalledWith({ allowedTools: ["Read", "Grep"] });

    onChange.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Read" }));
    expect(onChange).toHaveBeenCalledWith({ allowedTools: [] });
  });

  it("emits a patch when the checkpoint switch is flipped", () => {
    const onChange = renderFields();
    fireEvent.click(screen.getByRole("button", { name: "체크포인트에서 일시 정지" }));
    expect(onChange).toHaveBeenCalledWith({ checkpoint: false });
  });

  // PRD E-4: shown, but not editable.
  it("shows the stage id but disables it when lockId is set", () => {
    renderFields({ lockId: true });
    const idInput = screen.getByLabelText("단계 ID");
    expect(idInput).toBeInTheDocument();
    expect(idInput).toBeDisabled();
  });

  it("leaves the stage id editable when lockId is not set", () => {
    const onChange = renderFields({ lockId: false });
    const idInput = screen.getByLabelText("단계 ID");
    expect(idInput).toBeEnabled();
    fireEvent.change(idInput, { target: { value: "s1-renamed" } });
    expect(onChange).toHaveBeenCalledWith({ id: "s1-renamed" });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/components/StageFields.test.tsx`
Expected: FAIL — `Failed to resolve import "./StageFields"`.

- [ ] **Step 3: Write the component**

Create `src/components/StageFields.tsx`:

```tsx
import type { PermissionMode, Stage } from "../types";

export const PERMISSION_MODES: PermissionMode[] = ["acceptEdits", "bypassPermissions", "default"];
export const KNOWN_TOOLS = ["Read", "Write", "Edit", "Glob", "Grep", "Bash", "WebSearch", "WebFetch"];

interface Props {
  stage: Stage;
  onChange: (patch: Partial<Stage>) => void;
  /** Prefixes every input id, so the editor and the run screen can render this at once. */
  idPrefix: string;
  /** e.g. "단계 3 프롬프트" in the editor, "이 단계 프롬프트" on the run screen. */
  promptLabel: string;
  /**
   * Disables the stage-id input without hiding it (PRD E-4). Editing a stage id during a
   * run would desynchronize StageRun.id from resolvedStages and the backend would reject
   * the override with StageIdMismatch.
   */
  lockId: boolean;
}

export default function StageFields({ stage, onChange, idPrefix, promptLabel, lockId }: Props) {
  return (
    <>
      <div style={{ display: "flex", gap: 12, marginBottom: 12 }}>
        <label className="field" style={{ flex: 1 }}>
          <span>단계 이름</span>
          <input
            id={`${idPrefix}-name`}
            className="input"
            value={stage.name}
            onChange={(e) => onChange({ name: e.target.value })}
          />
        </label>
        <label className="field" style={{ flex: 1 }}>
          <span>단계 ID</span>
          <input
            id={`${idPrefix}-id`}
            className="input mono"
            value={stage.id}
            disabled={lockId}
            onChange={(e) => onChange({ id: e.target.value })}
          />
          {lockId && <span className="help-text">실행 중에는 단계 ID를 바꿀 수 없습니다.</span>}
        </label>
      </div>

      <div className="field" style={{ marginBottom: 12 }}>
        <label htmlFor={`${idPrefix}-prompt`}>{promptLabel}</label>
        <textarea
          id={`${idPrefix}-prompt`}
          className="textarea"
          rows={4}
          value={stage.prompt}
          onChange={(e) => onChange({ prompt: e.target.value })}
        />
        <span className="help-text">스킬/에이전트는 프롬프트에서 이름으로 지정합니다.</span>
      </div>

      <div style={{ marginBottom: 12 }}>
        <div className="section-label">권한 모드</div>
        <div className="segmented">
          {PERMISSION_MODES.map((mode) => (
            <button
              key={mode}
              className={`segmented__option ${stage.permissionMode === mode ? "segmented__option--selected" : ""}`}
              onClick={() => onChange({ permissionMode: mode })}
            >
              {mode}
            </button>
          ))}
        </div>
        <p className="help-text" style={{ marginTop: 6 }}>
          헤드리스 실행이므로 단계 중간에는 권한 질문에 답할 수 없습니다.
        </p>
      </div>

      <div style={{ marginBottom: 16 }}>
        <div className="section-label">허용 도구</div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
          {KNOWN_TOOLS.map((tool) => {
            const selected = stage.allowedTools.includes(tool);
            return (
              <button
                key={tool}
                className={`chip ${selected ? "chip--selected" : ""}`}
                onClick={() =>
                  onChange({
                    allowedTools: selected
                      ? stage.allowedTools.filter((t) => t !== tool)
                      : [...stage.allowedTools, tool],
                  })
                }
              >
                {tool}
              </button>
            );
          })}
        </div>
      </div>

      <div className="switch-row">
        <button
          className={`switch ${stage.checkpoint ? "switch--on" : ""}`}
          onClick={() => onChange({ checkpoint: !stage.checkpoint })}
          aria-label="체크포인트에서 일시 정지"
        >
          <span className="switch__knob" />
        </button>
        <div>
          <p className="switch-row__title">체크포인트에서 일시 정지</p>
          <p className="switch-row__desc">이 단계가 끝나면 승인 전까지 다음 단계로 넘어가지 않습니다.</p>
        </div>
      </div>
    </>
  );
}
```

- [ ] **Step 4: Run the component tests to verify they pass**

Run: `npx vitest run src/components/StageFields.test.tsx`
Expected: PASS — 8 tests.

- [ ] **Step 5: Refactor `TemplateEditor.tsx` to use it**

Delete lines 12-13 (`PERMISSION_MODES`, `KNOWN_TOOLS`) and import them back plus the component. Keep `ID_PATTERN` on line 14 — it is a template-level validation concern and stays here.

Add to the imports at the top:

```tsx
import StageFields, { } from "../components/StageFields";
```

(No named imports are needed — `TemplateEditor` never referenced `PERMISSION_MODES`/`KNOWN_TOOLS` outside the JSX it is about to delete. Use the plain form `import StageFields from "../components/StageFields";`.)

Then replace the JSX from line 206 (`<div style={{ display: "flex", gap: 12, marginBottom: 12 }}>` — the stage name/id row) through line 294 (the closing `</div>` of the `switch-row` block) with a single element:

```tsx
              <StageFields
                stage={selectedStage}
                onChange={(patch) => updateStage(selectedIndex, patch)}
                idPrefix={`stage-${selectedIndex}`}
                promptLabel={`단계 ${selectedIndex + 1} 프롬프트`}
                lockId={false}
              />
```

Keep the 단계 삭제 button (lines 197-204) and the 위로/아래로 buttons (lines 296-311) outside the component — reordering and deleting are template-editing concerns the run screen must not offer.

- [ ] **Step 6: Verify the editor's existing tests still pass**

Run: `npx vitest run src/pages/TemplateEditor.test.tsx`
Expected: PASS, unchanged. The existing selectors query by accessible name (`단계 1 프롬프트`, `템플릿 이름`), not by input id, and `StageFields` reproduces those names exactly — so no selector updates are needed here. (TRD §3.9 anticipated id-based selectors; the file does not actually use any.)

Run: `npx vitest run`
Expected: `TemplateGallery.test.tsx`, `TemplateEditor.test.tsx`, `api.test.ts`, `StageFields.test.tsx` PASS. `App.test.tsx` and `PipelineRun.test.tsx` still fail on the missing `resolvedStages` in their fixtures — Tasks 14 and 15.

- [ ] **Step 7: Commit**

```bash
git add src/components/StageFields.tsx src/components/StageFields.test.tsx src/pages/TemplateEditor.tsx
git commit -m "refactor: extract the shared StageFields form (IMP-008)

The stage form moves out of TemplateEditor into src/components/StageFields.tsx
so the run screen can render the same fields. idPrefix keeps two instances'
input ids apart, promptLabel keeps each caller's wording, and lockId disables
the stage-id input without hiding it (PRD E-4). PERMISSION_MODES and
KNOWN_TOOLS move with it; ID_PATTERN stays behind as template-level validation."
```

---

## Task 14: The run screen's pre-stage panel, cancel, stale recovery and failed notice (IMP-009, IMP-013, IMP-014, GAP-3)

TRD §3.10, §3.7, §6.1-A, §6.3-A / PRD E-3, E-4, D-1, D-2, D-3, C-5. The run screen gains a sibling to the checkpoint card: a "다음 단계 — 실행 전 확인/수정" panel that pre-fills `StageFields` from `resolvedStages[currentStageIndex]` and offers exactly two actions, so the gate is never a dead end. It also gains the stale-index recovery path, a terminal notice for failed runs (decision D1), and the relabelled original-template button (decision D3).

**Files:**
- Modify: `src/pages/PipelineRun.tsx` — imports, `stageDotClass` (57-62), `statusBadgeClass` (64-70), `stageNameById` (74), new state + effect + three handlers, the new panel, the failed notice, the relabelled footer button
- Test: `src/pages/PipelineRun.test.tsx` (fixtures gain `resolvedStages`; new cases)

**Interfaces:**
- Consumes: Task 12's `startStage`, `cancelRun`, `getRun`, `isStaleStageIndexError`, and the widened `RunRecord`; Task 13's `StageFields`.
- Produces: no exported API change. Props stay `{ initialRun, template, onFinished, onEditTemplate }`.

- [ ] **Step 1: Write the failing tests**

Rewrite `src/pages/PipelineRun.test.tsx`'s mock and fixtures, then add the new cases. Replace lines 1-43 with:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    onStageEvent: vi.fn(),
    approveCheckpoint: vi.fn(),
    requestChanges: vi.fn(),
    rejectCheckpoint: vi.fn(),
    startStage: vi.fn(),
    cancelRun: vi.fn(),
    getRun: vi.fn(),
    // The real predicate — the point of F-E6 is that the component uses it correctly.
    isStaleStageIndexError: actual.isStaleStageIndexError,
    STALE_STAGE_INDEX_PREFIX: actual.STALE_STAGE_INDEX_PREFIX,
  };
});

import { onStageEvent, approveCheckpoint, requestChanges, rejectCheckpoint, startStage, cancelRun, getRun } from "../api";
import PipelineRun from "./PipelineRun";
import type { RunRecord, Stage, StageEventPayload, Template } from "../types";

const stages: Stage[] = [
  { id: "s1", name: "요구사항 정리", prompt: "p1", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
  { id: "s2", name: "구현", prompt: "p2", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
];

const sampleTemplate: Template = { id: "t1", name: "웹 프로그램 개발", description: "desc", stages };

const runningRun: RunRecord = {
  runId: "run1",
  templateId: "t1",
  targetDir: "/tmp/proj",
  status: "awaiting-checkpoint",
  currentStageIndex: 0,
  stages: [
    { id: "s1", status: "awaiting-checkpoint", sessionId: "sess1", log: [] },
    { id: "s2", status: "pending", sessionId: null, log: [] },
  ],
  resolvedStages: stages,
};

const gateRun: RunRecord = {
  ...runningRun,
  status: "awaiting-stage-start",
  currentStageIndex: 0,
  stages: [
    { id: "s1", status: "awaiting-start", sessionId: null, log: [] },
    { id: "s2", status: "pending", sessionId: null, log: [] },
  ],
};

beforeEach(() => {
  vi.mocked(onStageEvent).mockReset().mockResolvedValue(() => {});
  vi.mocked(approveCheckpoint).mockReset();
  vi.mocked(requestChanges).mockReset();
  vi.mocked(rejectCheckpoint).mockReset();
  vi.mocked(startStage).mockReset();
  vi.mocked(cancelRun).mockReset();
  vi.mocked(getRun).mockReset();
});
```

Every existing test in the file keeps working with these fixtures except `lets the user jump to the template editor from a failed run`, whose button was relabelled. Replace that one and append the new cases:

```tsx
  // F-D1 / PRD D-2
  it("offers both 실행 and 취소 at the pre-stage gate, never just one action", () => {
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "이 run 취소" })).toBeInTheDocument();
  });

  // F-E4a / PRD E-4
  it("pre-fills the gate panel from resolvedStages with the id shown but disabled", () => {
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("p1");
    expect(screen.getByLabelText("단계 이름")).toHaveValue("요구사항 정리");
    const idInput = screen.getByLabelText("단계 ID");
    expect(idInput).toHaveValue("s1");
    expect(idInput).toBeDisabled();
  });

  it("sends the edited draft and the rendered stage index to startStage", async () => {
    vi.mocked(startStage).mockResolvedValue({ ...gateRun, status: "awaiting-checkpoint" });
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("이 단계 프롬프트"), { target: { value: "이 과제에 맞게 수정한 프롬프트" } });
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    await waitFor(() =>
      expect(startStage).toHaveBeenCalledWith(
        "run1",
        0,
        expect.objectContaining({ id: "s1", prompt: "이 과제에 맞게 수정한 프롬프트" })
      )
    );
  });

  // F-E3 / PRD E-3
  it("replaces the draft with the next stage's values when the index advances", async () => {
    const advanced: RunRecord = {
      ...gateRun,
      currentStageIndex: 1,
      stages: [
        { id: "s1", status: "approved", sessionId: "sess1", log: [] },
        { id: "s2", status: "awaiting-start", sessionId: null, log: [] },
      ],
    };
    vi.mocked(startStage).mockResolvedValue(advanced);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("이 단계 프롬프트"), { target: { value: "이전 단계 초안" } });
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    // The stale draft must not survive the advance.
    await waitFor(() => expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("p2"));
    expect(screen.getByLabelText("단계 ID")).toHaveValue("s2");
  });

  // F-D4 — dec3360 double-click guard
  it("disables the gate buttons while startStage is in flight", async () => {
    let resolveStart: (value: RunRecord) => void = () => {};
    vi.mocked(startStage).mockImplementation(() => new Promise((resolve) => { resolveStart = resolve; }));
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "이 run 취소" })).toBeDisabled();

    resolveStart({ ...gateRun, status: "awaiting-checkpoint" });
    await waitFor(() => expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument());
  });

  // F-E6 / TRD 3.10 — a stale index refreshes the screen instead of showing an error
  it("silently refetches the run when startStage rejects with a stale stage index", async () => {
    vi.mocked(startStage).mockRejectedValue("STALE_STAGE_INDEX: run is at stage 1, caller expected 0");
    const fresh: RunRecord = {
      ...gateRun,
      currentStageIndex: 1,
      stages: [
        { id: "s1", status: "approved", sessionId: "sess1", log: [] },
        { id: "s2", status: "awaiting-start", sessionId: null, log: [] },
      ],
    };
    vi.mocked(getRun).mockResolvedValue(fresh);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    await waitFor(() => expect(getRun).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(screen.getByLabelText("이 단계 프롬프트")).toHaveValue("p2"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows an error for any startStage failure that is not a stale stage index", async () => {
    vi.mocked(startStage).mockRejectedValue("run 'run1' is not awaiting a stage start");
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("not awaiting a stage start");
    expect(getRun).not.toHaveBeenCalled();
  });

  // T-D1's frontend half / PRD D-1
  it("cancels the run and returns to the gallery", async () => {
    const onFinished = vi.fn();
    vi.mocked(cancelRun).mockResolvedValue({ ...gateRun, status: "cancelled" });
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={onFinished} onEditTemplate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "이 run 취소" }));

    await waitFor(() => expect(cancelRun).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });

  it("hides the gate panel when the run is not at a gate", () => {
    render(<PipelineRun initialRun={runningRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "승인" })).toBeInTheDocument();
  });

  // GAP-3 decision D1 / PRD D-3 — Failed is terminal and must say so
  it("states that a failed run is final and offers no retry", () => {
    const failedRun: RunRecord = { ...runningRun, status: "failed" };
    render(<PipelineRun initialRun={failedRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByText(/이 run은 실패로 종료되었습니다/)).toBeInTheDocument();
    expect(screen.getByText(/새 run을 시작하세요/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "이 단계 실행" })).not.toBeInTheDocument();
  });

  // PRD C-5 (IMP-013 decision D3) — the two edit paths must be distinguishable
  it("warns before sending the user to the original template editor", () => {
    const onEditTemplate = vi.fn();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={onEditTemplate} />);

    fireEvent.click(screen.getByRole("button", { name: "원본 템플릿 편집 (모든 향후 실행에 적용)" }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining("저장된 원본 템플릿"));
    expect(onEditTemplate).toHaveBeenCalledWith(sampleTemplate);
    confirmSpy.mockRestore();
  });

  it("does not navigate to the original template editor when the warning is declined", () => {
    const onEditTemplate = vi.fn();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<PipelineRun initialRun={gateRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={onEditTemplate} />);

    fireEvent.click(screen.getByRole("button", { name: "원본 템플릿 편집 (모든 향후 실행에 적용)" }));

    expect(onEditTemplate).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it("prefers the run's resolved stage names over the template's", () => {
    const renamed: RunRecord = {
      ...gateRun,
      resolvedStages: [{ ...stages[0], name: "이 run에서만 바꾼 이름" }, stages[1]],
    };
    render(<PipelineRun initialRun={renamed} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByText("이 run에서만 바꾼 이름: awaiting-start")).toBeInTheDocument();
  });
```

Delete the old `lets the user jump to the template editor from a failed run` test — the two `원본 템플릿 편집` cases above replace it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/pages/PipelineRun.test.tsx`
Expected: FAIL — `Unable to find an accessible element with the role "button" and name "이 단계 실행"`, and the mock factory throws because `../api` has no `startStage` yet if Task 12 was skipped (it was not).

- [ ] **Step 3: Write the implementation**

In `src/pages/PipelineRun.tsx`:

Replace the imports on lines 1-3 with:

```tsx
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
```

Replace `stageDotClass` and `statusBadgeClass` (lines 57-70) with:

```tsx
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
```

Replace `stageNameById` (line 74) with a version that prefers the run's own names, since a pre-start edit can rename a stage for this run only:

```tsx
  const stageNameById = new Map<string, string>([
    ...template.stages.map((s) => [s.id, s.name] as [string, string]),
    ...run.resolvedStages.map((s) => [s.id, s.name] as [string, string]),
  ]);
```

Add the draft state and its sync effect next to the other `useState` calls (after line 79's `error` state):

```tsx
  const [stageDraft, setStageDraft] = useState<Stage | null>(null);

  // PRD E-3: the draft follows the gate. When the index advances, the previous stage's
  // draft must not linger on screen.
  useEffect(() => {
    if (run.status === "awaiting-stage-start") {
      setStageDraft(run.resolvedStages[run.currentStageIndex] ?? null);
    } else {
      setStageDraft(null);
    }
  }, [run.status, run.currentStageIndex, run.resolvedStages]);
```

Add three handlers after `handleReject` (line 148):

```tsx
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
```

Insert the gate panel and the failed notice as siblings of the checkpoint card — directly after the `{run.status === "awaiting-checkpoint" && (...)}` block that ends on line 229:

```tsx
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
```

Finally, relabel the footer button (lines 231-234):

```tsx
        <div style={{ marginTop: 18, display: "flex", gap: 8 }}>
          <button className="btn btn-outline" onClick={handleEditTemplate}>
            원본 템플릿 편집 (모든 향후 실행에 적용)
          </button>
          <button className="btn btn-outline" onClick={onFinished}>
            갤러리로 돌아가기
          </button>
        </div>
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/pages/PipelineRun.test.tsx`
Expected: PASS — the original 11 cases (minus the replaced one) plus the 13 new ones.

Run: `npx tsc --noEmit`
Expected: only `src/App.tsx` and `src/App.test.tsx` still error, on the optimistic record's missing `resolvedStages`. Task 15 closes those.

- [ ] **Step 5: Commit**

```bash
git add src/pages/PipelineRun.tsx src/pages/PipelineRun.test.tsx
git commit -m "feat: add the pre-stage edit panel, cancel, and stale recovery to the run screen

The run screen now renders a gate panel pre-filled from resolvedStages with the
stage id shown but disabled (PRD E-4), and it always offers both '이 단계 실행'
and '이 run 취소' so the gate is never action-locked (PRD D-2). The draft
re-syncs whenever the index advances (E-3). A STALE_STAGE_INDEX rejection
refetches the run instead of showing an error, because it only means our copy
was stale (TRD 3.10 / F-E6).

Per decision D1 a failed run shows an explicit terminal notice (PRD D-3), and
per decision D3 the original-template button is relabelled and confirms first,
so the two edit scopes are distinguishable (PRD C-5)."
```

---

## Task 15: Move the run entry point back to the gallery (IMP-012, decision D2)

TRD §6.2 option B / PRD C-1, C-2, E-5. Today `TemplateEditor.tsx:92-113` routes **both** 저장 and 실행 through `persistTemplate()`, so pressing the editor's 실행 button (lines 153-155) always overwrites the saved template first — and since commit `a246384` removed the gallery's run button, the editor is the *only* run entry point. That makes "실행 = 원본 덮어쓰기" structural, and PRD C-1 ("원본 템플릿을 한 번도 쓰지 않고 실행이 완주된다") unreachable no matter what the backend does. Decision D2 breaks the coupling by moving the entry point: the editor loses its 실행 button and `onRun` prop, the gallery gets a per-card `실행` button back, and `a246384`'s actual goal — force the user past an edit screen before anything runs — is now met by Task 14's pre-stage gate, at the point where the edit actually applies.

This task also closes the two type errors Task 14 left behind: `App.tsx`'s optimistic `pendingRun` predates the new state machine and lacks the now-required `resolvedStages`.

**Files:**
- Modify: `src/pages/TemplateEditor.tsx:5-10` (`Props`), `:40` (destructuring), `:92-113` (`persistTemplate`/`handleSave`/`handleRun`), `:143-156` (the button row)
- Modify: `src/pages/TemplateEditor.test.tsx:31-85` (the whole `describe` block)
- Modify: `src/pages/TemplateGallery.tsx:5-8` (`Props`), `:10` (signature), `:53` (page subtitle), `:86-102` (the card footer)
- Modify: `src/pages/TemplateGallery.test.tsx:29-69` (add `onRun` to every render; two new cases)
- Modify: `src/App.tsx:63-84` (`handleRun`), `:86-112` (view wiring)
- Modify: `src/App.test.tsx:4-14` (the `./api` mock), `:23-46` (fixtures), `:48-108` (the routing tests)
- Test: the three `*.test.tsx` files above

**Interfaces:**
- Consumes: Task 12's `RunRecord` (with `resolvedStages`) and the `"awaiting-stage-start"` / `"awaiting-start"` spellings; Task 13's `StageFields` (indirectly, through the editor and the gate panel); Task 14's `PipelineRun`, whose props are unchanged (`{ initialRun, template, onFinished, onEditTemplate }`).
- Produces:
  - `TemplateEditor` props narrow to `{ initial: Template | null; onSaved: (template: Template) => void; onCancel: () => void }` — **`onRun` is removed**, and so is `persistTemplate`.
  - `TemplateGallery` props widen to `{ onEdit: (template: Template) => void; onNew: () => void; onRun: (template: Template) => void }`. The button label is exactly `실행` (Global Constraints).
  - `App`'s optimistic `pendingRun`: `status: "awaiting-stage-start"`, `stages[0].status: "awaiting-start"` with the rest `"pending"`, and `resolvedStages: template.stages`.
  - No backend, IPC, or `api.ts` change. `startPipelineRun`'s three arguments are untouched.

- [ ] **Step 1: Write the failing tests**

Three files change. Start with `src/pages/TemplateGallery.test.tsx`: every existing `render(<TemplateGallery ... />)` call (lines 32, 40, 47, 58, 66) needs the new required prop — add `onRun={vi.fn()}` to each. Then append inside the `describe("TemplateGallery", ...)` block:

```tsx
  // IMP-012 (decision D2): the gallery is the run entry point again.
  it("runs a template straight from its gallery card", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    const onRun = vi.fn();
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={onRun} />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));
    expect(onRun).toHaveBeenCalledWith(sample);
  });

  it("offers 실행 alongside 편집 and 삭제 on every card", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    render(<TemplateGallery onEdit={vi.fn()} onNew={vi.fn()} onRun={vi.fn()} />);
    await screen.findByText("웹 프로그램 개발");
    expect(screen.getByRole("button", { name: "실행" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "편집" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "삭제" })).toBeInTheDocument();
  });
```

Next, replace the whole `describe("TemplateEditor", ...)` block in `src/pages/TemplateEditor.test.tsx` (lines 31-85) with:

```tsx
describe("TemplateEditor", () => {
  it("disables save when a stage prompt is empty", () => {
    render(<TemplateEditor initial={existing} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("단계 1 프롬프트"), { target: { value: "" } });
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();
  });

  it("disables save when there are no stages", () => {
    render(<TemplateEditor initial={null} onSaved={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();
  });

  it("saves the edited template and calls onSaved", async () => {
    const onSaved = vi.fn();
    render(<TemplateEditor initial={existing} onSaved={onSaved} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("템플릿 이름"), { target: { value: "웹 프로그램 개발 v2" } });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() =>
      expect(saveTemplate).toHaveBeenCalledWith(expect.objectContaining({ name: "웹 프로그램 개발 v2" }))
    );
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
  });

  it("adds a new stage when '단계 추가' is clicked", () => {
    render(<TemplateEditor initial={null} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "단계 추가" }));
    expect(screen.getByLabelText("단계 1 프롬프트")).toBeInTheDocument();
  });

  it("shows an error when saveTemplate rejects", async () => {
    vi.mocked(saveTemplate).mockRejectedValue(new Error("disk full"));
    render(<TemplateEditor initial={existing} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
  });

  it("does not call onSaved when saveTemplate rejects", async () => {
    vi.mocked(saveTemplate).mockRejectedValue(new Error("disk full"));
    const onSaved = vi.fn();
    render(<TemplateEditor initial={existing} onSaved={onSaved} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(onSaved).not.toHaveBeenCalled();
  });

  // IMP-012 (decision D2) / PRD C-1: the editor is not a run entry point any more, so
  // saving can never happen as a side effect of running.
  it("offers no 실행 button", () => {
    render(<TemplateEditor initial={existing} onSaved={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "실행" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "저장" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "취소" })).toBeInTheDocument();
  });
});
```

Finally `src/App.test.tsx`. The `./api` mock at lines 4-14 must grow: `PipelineRun.tsx` now imports `startStage`, `cancelRun`, `getRun` and `isStaleStageIndexError` from the same module, and a factory that omits them makes the import throw. Replace lines 4-46 with:

```tsx
vi.mock("./api", async () => {
  const actual = await vi.importActual<typeof import("./api")>("./api");
  return {
    listTemplates: vi.fn(),
    deleteTemplate: vi.fn(),
    checkCli: vi.fn(),
    saveTemplate: vi.fn(),
    startPipelineRun: vi.fn(),
    onStageEvent: vi.fn(),
    approveCheckpoint: vi.fn(),
    requestChanges: vi.fn(),
    rejectCheckpoint: vi.fn(),
    startStage: vi.fn(),
    cancelRun: vi.fn(),
    getRun: vi.fn(),
    // The real predicate and prefix — App never calls them, but PipelineRun does.
    isStaleStageIndexError: actual.isStaleStageIndexError,
    STALE_STAGE_INDEX_PREFIX: actual.STALE_STAGE_INDEX_PREFIX,
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import { listTemplates, checkCli, saveTemplate, startPipelineRun, onStageEvent, startStage } from "./api";
import { open } from "@tauri-apps/plugin-dialog";
import App from "./App";
import type { RunRecord, Template } from "./types";

const sample: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "desc",
  stages: [
    {
      id: "s1",
      name: "요구사항",
      prompt: "PRD 작성",
      permissionMode: "acceptEdits",
      allowedTools: [],
      checkpoint: true,
    },
  ],
};

/** What the backend answers with once start_pipeline_run resolves. */
const backendRun: RunRecord = {
  runId: "run1",
  templateId: "t1",
  targetDir: "/tmp/new-project",
  status: "awaiting-stage-start",
  currentStageIndex: 0,
  stages: [{ id: "s1", status: "awaiting-start", sessionId: null, log: [] }],
  // Renamed on purpose, so a test can watch the optimistic record be replaced by the
  // authoritative one rather than just matching itself.
  resolvedStages: [{ ...sample.stages[0], name: "백엔드가 확정한 요구사항" }],
};

beforeEach(() => {
  vi.mocked(listTemplates).mockReset().mockResolvedValue([sample]);
  vi.mocked(checkCli).mockReset().mockResolvedValue("available:mock-claude 0.0.1");
  vi.mocked(saveTemplate).mockReset().mockResolvedValue(undefined);
  vi.mocked(onStageEvent).mockReset().mockResolvedValue(() => {});
  vi.mocked(startStage).mockReset();
  vi.mocked(open).mockReset();
  vi.mocked(startPipelineRun).mockReset();
});
```

Then replace the `describe("App routing", ...)` block (lines 48-108) with:

```tsx
describe("App routing", () => {
  it("starts on the template gallery", async () => {
    render(<App />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  // IMP-012 (decision D2): running starts at the gallery card, not in the editor.
  it("picks a target folder and starts a run when 실행 is clicked on a gallery card", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    let resolveStart: (value: RunRecord) => void = () => {};
    vi.mocked(startPipelineRun).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveStart = resolve;
        })
    );

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    await waitFor(() => expect(open).toHaveBeenCalledWith({ directory: true }));
    await waitFor(() =>
      expect(startPipelineRun).toHaveBeenCalledWith("t1", "/tmp/new-project", expect.any(String))
    );

    // The run view mounts on the optimistic record before startPipelineRun resolves, so
    // the stage-event listener is live from the very first stage — and that record must
    // already describe the gate, not a running stage (PRD A-1).
    expect(await screen.findByText("상태: awaiting-stage-start")).toBeInTheDocument();

    // ...and is then replaced by the authoritative record the backend created.
    resolveStart(backendRun);
    expect(await screen.findByText("백엔드가 확정한 요구사항: awaiting-start")).toBeInTheDocument();
  });

  // PRD C-1: no write to the saved template anywhere on the path into a run.
  it("never saves the template on the way to a run", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(startPipelineRun).mockResolvedValue(backendRun);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    await waitFor(() => expect(startPipelineRun).toHaveBeenCalled());
    expect(saveTemplate).not.toHaveBeenCalled();
  });

  // The optimistic record must carry resolvedStages, or the gate panel has nothing to
  // render and the user stares at an empty screen until the backend answers.
  it("renders the first stage's gate panel from the optimistic record", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(startPipelineRun).mockImplementation(() => new Promise(() => {}));

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    expect(await screen.findByLabelText("이 단계 프롬프트")).toHaveValue("PRD 작성");
    expect(screen.getByLabelText("단계 ID")).toHaveValue("s1");
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "이 run 취소" })).toBeInTheDocument();
  });

  it("stays on the gallery when the folder dialog is dismissed", async () => {
    vi.mocked(open).mockResolvedValue(null);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    await waitFor(() => expect(open).toHaveBeenCalled());
    expect(startPipelineRun).not.toHaveBeenCalled();
    expect(screen.getByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  it("shows an error and returns to the gallery when startPipelineRun rejects", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    vi.mocked(startPipelineRun).mockRejectedValue(new Error("target dir already contains a pipeline run"));

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실행" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("target dir already contains a pipeline run");
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  it("opens a pure editor from 편집 — no run button, no folder dialog", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "편집" }));

    expect(await screen.findByRole("button", { name: "저장" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "실행" })).not.toBeInTheDocument();
    expect(open).not.toHaveBeenCalled();
  });
});
```

The old `does not run a template directly from the gallery — only 편집/삭제 are offered` case asserted exactly the behaviour decision D2 reverses; it is deleted, and `runs a template straight from its gallery card` above is its replacement.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/App.test.tsx src/pages/TemplateEditor.test.tsx src/pages/TemplateGallery.test.tsx`
Expected: FAIL —
- `TemplateGallery.test.tsx`: `Unable to find an accessible element with the role "button" and name "실행"`.
- `TemplateEditor.test.tsx`: the `offers no 실행 button` case fails because the button is still there; the others fail type-check on the missing `onRun` prop.
- `App.test.tsx`: `Unable to find ... name "실행"` on the gallery, and `renders the first stage's gate panel` fails because the optimistic record has no `resolvedStages`.

- [ ] **Step 3: Add the 실행 button and the `onRun` prop to the gallery**

In `src/pages/TemplateGallery.tsx`, replace the `Props` interface and the component signature (lines 5-10) with:

```tsx
interface Props {
  onEdit: (template: Template) => void;
  onNew: () => void;
  /**
   * IMP-012 (decision D2). Running starts here again, from the saved template — the
   * editor no longer runs anything, so pressing 실행 can never overwrite what it runs.
   * Per-run edits happen at the run screen's pre-stage gate instead.
   */
  onRun: (template: Template) => void;
}

export default function TemplateGallery({ onEdit, onNew, onRun }: Props) {
```

Replace the page subtitle (line 53) with:

```tsx
          <p className="page-subtitle">
            템플릿을 실행하면 각 단계가 실행 직전에 멈춥니다 — 그 자리에서 확인·수정한 뒤 실행하세요.
          </p>
```

Replace the card footer's button row (lines 95-101) with:

```tsx
              <button className="btn btn-success" onClick={() => onRun(template)}>
                실행
              </button>
              <button className="btn btn-success-subtle" onClick={() => onEdit(template)}>
                편집
              </button>
              <span style={{ flex: 1 }} />
              <button className="btn btn-ghost" onClick={() => handleDelete(template.id)}>
                삭제
              </button>
```

- [ ] **Step 4: Strip the editor down to save and cancel**

In `src/pages/TemplateEditor.tsx`, replace the `Props` interface (lines 5-10) with:

```tsx
interface Props {
  initial: Template | null;
  onSaved: (template: Template) => void;
  onCancel: () => void;
}
```

Change the component signature (line 40) to:

```tsx
export default function TemplateEditor({ initial, onSaved, onCancel }: Props) {
```

Replace `persistTemplate`, `handleSave` and `handleRun` (lines 92-113) with a single handler:

```tsx
  // IMP-012 (decision D2): there is deliberately no run path here any more. Keeping a
  // shared persistTemplate() helper with one caller would just invite the save-then-run
  // coupling back, so it is folded into the only action the editor still performs.
  const handleSave = async () => {
    if (error) return;
    setSaveError(null);
    setBusy(true);
    try {
      await saveTemplate(template);
      onSaved(template);
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setBusy(false);
    }
  };
```

Replace the button row (lines 143-156) with:

```tsx
        <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
          <button className="btn btn-outline" onClick={() => setShowJson((s) => !s)}>
            {"{ } JSON"}
          </button>
          <button className="btn btn-outline" onClick={onCancel}>
            취소
          </button>
          <button className="btn btn-success" onClick={handleSave} disabled={!!error || busy}>
            저장
          </button>
        </div>
```

저장 takes over `btn btn-success` because it is now the only committing action on this screen.

- [ ] **Step 5: Rewire `App.tsx`**

Replace `handleRun` (lines 63-84) with:

```tsx
  const handleRun = async (template: Template) => {
    setError(null);
    try {
      const targetDir = await open({ directory: true });
      if (!targetDir || Array.isArray(targetDir)) return;
      const runId = crypto.randomUUID();
      // Optimistic record. It mounts the run screen — and with it the stage-event
      // listener — before start_pipeline_run answers, so it has to be shaped exactly
      // like what the backend's RunRecord::new produces: parked at the first stage's
      // gate with nothing executing (PRD A-1), not "running".
      const pendingRun: RunRecord = {
        runId,
        templateId: template.id,
        targetDir,
        status: "awaiting-stage-start",
        currentStageIndex: 0,
        stages: template.stages.map((s, i) => ({
          id: s.id,
          status: i === 0 ? "awaiting-start" : "pending",
          sessionId: null,
          log: [],
        })),
        // Required on RunRecord — there is no serde default on the Rust side (decision
        // D8) — and it is what the gate panel renders from, so omitting it would leave
        // the panel blank until the backend replied.
        resolvedStages: template.stages,
      };
      setView({ name: "run", run: pendingRun, template });
      const run = await startPipelineRun(template.id, targetDir, runId);
      setView({ name: "run", run, template });
    } catch (e) {
      setError(String(e));
      setView({ name: "gallery" });
    }
  };
```

`View` (lines 10-13) is unchanged — D2 moves a callback, not a screen.

Then drop `onRun` from the editor branch (lines 88-95) and add it to the gallery branch (lines 106-111):

```tsx
      <TemplateEditor
        initial={view.template}
        onSaved={() => setView({ name: "gallery" })}
        onCancel={() => setView({ name: "gallery" })}
      />
```

```tsx
      <TemplateGallery
        onEdit={(template) => setView({ name: "editor", template })}
        onNew={() => setView({ name: "editor", template: null })}
        onRun={handleRun}
      />
```

- [ ] **Step 6: Run the whole frontend suite and the type-check**

Run: `npx vitest run`
Expected: PASS — every file. `App.test.tsx` 7 cases, `TemplateEditor.test.tsx` 7, `TemplateGallery.test.tsx` 7, plus `PipelineRun.test.tsx`, `StageFields.test.tsx` and `api.test.ts` unchanged from Tasks 12-14.

Run: `npx tsc --noEmit`
Expected: **no errors at all.** This is the check that matters: Task 14 ended with `src/App.tsx` and `src/App.test.tsx` still erroring on the optimistic record's missing `resolvedStages`, and this task is what closes them. If anything still errors, do not proceed to Task 16.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — unchanged. This task touches no Rust; run it only to confirm that.

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.test.tsx src/pages/TemplateEditor.tsx src/pages/TemplateEditor.test.tsx src/pages/TemplateGallery.tsx src/pages/TemplateGallery.test.tsx
git commit -m "feat: move the run entry point back to the gallery (IMP-012)

TRD 6.2 option B. The editor's 실행 button and onRun prop are gone, so saving a
template can no longer be a side effect of running one and PRD C-1 holds for the
whole run rather than only after it starts. Running begins at a per-card 실행
button in the gallery, which is honest about what it runs: the saved template.
The reason a246384 routed running through the editor — force a look at the
prompts first — is now served by the pre-stage gate, at the point where the edit
actually applies and without touching the original.

App's optimistic record is rebuilt for the new state machine: awaiting-stage-start
with stage 0 at awaiting-start, and carrying resolvedStages, which RunRecord now
requires and the gate panel renders from. That closes the last two type errors
left over from the run-screen commit."
```

---

## Task 16: Documentation sweep — README, the eight `documents/` files, and the portal (IMP-011, IMP-022, decision D7)

TRD §3.13, §3.15, §5.4 / PRD B-5, F-2, F-4. Fourteen tasks changed what the app *is*: the only stop used to be after a checkpoint, and now every stage stops before it runs; there used to be two stores, and now there are three. Every sentence in `README.md` and `documents/` that describes the old shape is now wrong, and PRD F-4 makes correcting them a success criterion rather than a courtesy.

This task also carries the entire implementation of **decision D7 / TRD §6.7 option C**. That option's cost was "zero code, document the decision explicitly" — Step 2 is where the explicit documentation actually gets written, so skipping it leaves TRD §6.7 unresolved rather than decided, and PRD B-5's second clause unsatisfied. Decisions **D8** (legacy quarantine) and **D4** (a folder holds one run) are likewise surfaced to end users here, as a natural part of describing the third store.

No source file changes in this task. Steps 1-7 edit prose; Step 9 is the manual verification PRD F-2 demands, and it needs the real `claude` CLI, not the mock.

**Files:**
- Modify: `README.md:9-22` (How it works), `:58-82` (Project layout), `:84-95` (Known v1 limitations)
- Modify: `documents/database-design.md:30-42` (§1 저장소 목록), `:80-100` (§2.2 tables), append §2.3 after line 100, `:106-111` (§3 관계 표), `:130-139` (§5 마이그레이션)
- Modify: `documents/system-architecture.md:39-75` (mermaid), `:88-99` (컴포넌트 표), `:116-131` (§5 데이터 흐름), `:160-166` (§7 표)
- Modify: `documents/user-guide.md:132-145` (§4.1), `:147-154` (§4.2), `:156-170` (§4.3), `:192-201` (§6.1), `:244-256` (§8)
- Modify: `documents/api-design.md:64-82` (§3 에러 표), `:209-247` (`start_pipeline_run`), insert three commands after line 249
- Modify: `documents/api-user-guide.md:98-124` (시나리오 3), `:180-194` (§4)
- Modify: `documents/sequence-diagrams.md:54-113` (§2)
- Modify: `documents/class-diagram.md:43-75` (`RunRecord`/enums), `:127-137` (`Orchestrator`)
- Modify: `documents/er-diagram.md:10-50` (mermaid), `:60-70` (관계 설명)
- Modify: `documents/docs-portal.html` (the `#database-design`, `#system-architecture`, `#user-guide`, `#api-design`, `#api-user-guide`, `#sequence-diagrams`, `#class-diagram`, `#er-diagram` sections)
- Test: none automated (prose task). Step 7 re-runs the full suite as a regression guard; Step 8 is the PRD F-2 manual checklist.

**Interfaces:**
- Consumes: every user-visible outcome of Tasks 3-15 — `MANIFEST_DIR_NAME`, `runs-legacy-v1`, `RunStatus::AwaitingStageStart`, `StageStatus::AwaitingStart`, `resolved_stages`, `start_stage`/`cancel_run`/`get_run`, the gallery's `실행` button, and the run screen's `이 단계 실행` / `이 run 취소` labels.
- Produces: no code and no exported API. The only durable artifact is the written record of decision D7, which TRD §6.7 option C names as its deliverable.

- [ ] **Step 1: Update `README.md`**

Replace the `## How it works` section (lines 9-22) with:

```markdown
## How it works

1. Pick a template in the gallery, press **실행**, and choose (or create) a target
   folder. Nothing is executed yet — creating a run spawns no process at all.
2. **Every stage stops before it runs.** The run screen shows a
   "다음 단계 — 실행 전 확인/수정" panel pre-filled with that stage's prompt, permission
   mode, allowed tools and checkpoint flag. Anything you change there applies to this
   run only; the saved gallery template is never written.
3. Pressing **이 단계 실행** runs exactly that one stage as its own
   `claude --print --output-format stream-json` invocation, with `--resume <session_id>`
   chaining conversation context from one stage to the next so later stages know what
   earlier stages built.
4. When the stage finishes, the run returns to the *next* stage's gate — never straight
   into the next `claude` process. A stage marked as a checkpoint first shows its live
   log plus **승인 / 수정 요청 보내기 / 거부** controls.
5. Finishing or approving the last stage completes the run. **이 run 취소** is offered at
   every gate, so a run is never a dead end.

While a run exists, the stage definitions it is actually using are mirrored into
`<targetDir>/.claude-pipeline-wizard/run.json` and `pipeline.json`, so a folder can be
inspected without opening the app.

Templates, run records, and the two seed templates ("웹 프로그램 개발" and
"데스크톱 프로그램 개발 (Tauri)") are all plain JSON files — there's no embedded
database and no bundled AI SDK; the app is a thin, persistent orchestration layer
around the `claude` CLI you already have installed.
```

Replace the `## Project layout` code block (lines 60-82) with:

```
src/                      React frontend
  components/
    StageFields.tsx       The one stage form, shared by the editor and the run gate
  pages/
    TemplateGallery.tsx   Browse, run, edit, delete templates
    TemplateEditor.tsx    Form-based stage editor (add/remove/reorder, validation)
    PipelineRun.tsx       Pre-stage edit gate, live stage log, checkpoint approval
  api.ts                  Tauri invoke()/listen() wrappers
  types.ts                TypeScript mirrors of the Rust IPC types

src-tauri/src/
  template/               Template/Stage types, file-backed CRUD store, seed templates
  engine/
    run_record.rs         Run state machine (RunRecord/StageRun, gate + checkpoint transitions)
    project_manifest.rs   Writes <targetDir>/.claude-pipeline-wizard/{run,pipeline}.json
    legacy_migration.rs   One-shot startup quarantine of pre-resolvedStages run records
    stream_json.rs        Parser for the claude CLI's stream-json output
    executor.rs           Spawns the claude CLI per stage, streams events, enforces a timeout
    orchestrator.rs       One stage per start_stage call; owns every state transition
  commands.rs             Tauri commands exposed to the frontend
  bin/mock_claude.rs      Test-only stand-in for the claude CLI (used by the Rust test suite)

docs/superpowers/
  specs/                  Design spec
  plans/                  Implementation plan
```

Replace the `## Known v1 limitations` section (lines 84-95) with — note the existing three bullets are **kept**, per TRD §3.13, because GAP-1 is still open:

```markdown
## Known v1 limitations

- **New folders only** — a template runs against a folder you pick, and a folder that
  already contains `.claude-pipeline-wizard/run.json` is refused outright rather than
  overwritten. Running against an existing codebase isn't supported yet.
- **No cross-restart resume UI** — run state is persisted to disk per
  transition, but there's currently no command or screen to reattach to an
  existing run after the app restarts; recovery today means inspecting the
  JSON file under the app's `runs/` directory by hand.
- **The project-local manifest makes a run easier to look at, not resumable** —
  `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json` is rewritten on every
  transition so a folder's pipeline state can be read without the app, but nothing ever
  reads it back in. It does not solve the resume limitation above.
- **Run records written before the pre-stage gate can't be opened** — on the first
  launch of this version, records that predate the required `resolvedStages` field are
  moved to `<app_data_dir>/runs-legacy-v1/`. Nothing is deleted, but the app will not
  read them.
- **Sequential stages only** — no parallel or branching stages within a
  template.
```

- [ ] **Step 2: Update `documents/database-design.md` — the third store, and decision D7**

Replace the §1 table and the paragraph under it (lines 32-42) with:

```markdown
| # | 저장소 | 디렉터리 | 레코드 형태 | 예상 레코드 수 |
|---|--------|---------|------------|---------------|
| 1 | 템플릿 저장소 | `<app_data_dir>/templates/` | `Template` (JSON) | 수십 개 수준 (사용자가 직접 만드는 템플릿 + 시드 2개) |
| 2 | 실행 기록 저장소 | `<app_data_dir>/runs/` | `RunRecord` (JSON) | 실행할 때마다 1개씩 누적, 삭제 기능 없음 |
| 3 | **프로젝트 로컬 매니페스트** | `<targetDir>/.claude-pipeline-wizard/` | `run.json` (`RunRecord` 전체) + `pipeline.json` (`Template` 형태 스냅샷) | 대상 폴더 1개당 정확히 1세트 |
| — | 레거시 격리 보관소 | `<app_data_dir>/runs-legacy-v1/` | 구 형식 `RunRecord` (앱이 읽지 않음) | 이 버전 최초 기동 시 1회 이동된 파일 수 |

3번은 2번을 **대체하지 않는다.** 모든 상태 전이는 `runs/{run_id}.json`과 매니페스트
**양쪽에** 쓰인다(2.3절). 1·2번이 "앱이 무엇을 했는가"라면 3번은 "이 폴더가 무엇으로부터
만들어졌는가"이며, 앱 없이 폴더만 열어도 읽을 수 있는 것이 존재 이유다.

`<app_data_dir>`는 Tauri의 `app.path().app_data_dir()`이 반환하는 OS별 표준 앱 데이터
경로다(Windows: `%APPDATA%\com.suhwanju.claude-pipeline-wizard\`, macOS:
`~/Library/Application Support/com.suhwanju.claude-pipeline-wizard/`, Linux:
`~/.local/share/com.suhwanju.claude-pipeline-wizard/`). `identifier`는
`src-tauri/tauri.conf.json`에 정의되어 있다. `<targetDir>`는 사용자가 실행 시작 시 고른
대상 폴더 그 자체다.
```

In §2.2, replace the `status` row and append a `resolvedStages` row to the **RunRecord** table (lines 86-88):

```markdown
| `status` | enum | NOT NULL | `awaiting-stage-start` | `running` \| `awaiting-stage-start` \| `awaiting-checkpoint` \| `completed` \| `failed` \| `cancelled` | 실행 전체 상태. `awaiting-stage-start`는 **아무것도 실행되고 있지 않은 실행 전 게이트**를 뜻한다 |
| `currentStageIndex` | number | NOT NULL | `0` | `0 <= n < stages.length` | 현재 게이트에 있거나 실행 중인 단계의 배열 인덱스 |
| `stages` | `StageRun[]` | NOT NULL | - | 생성 시 템플릿의 단계 수와 1:1 매핑 | 단계별 실행 상태 목록 |
| `resolvedStages` | `Stage[]` | NOT NULL | 생성 시 템플릿 `stages`의 복사본 | `stages`와 같은 길이·같은 id 순서. **`#[serde(default)]` 없음 — 필수 필드** | 이 run이 실제로 실행할 단계 정의. 실행 전 게이트에서 수정한 내용이 여기 들어가며, 원본 템플릿 파일은 절대 쓰지 않는다 |
```

Replace the **StageRun** `status` row (line 94) with:

```markdown
| `status` | enum | NOT NULL | `pending` (0번 단계만 생성 시 `awaiting-start`) | `pending` \| `awaiting-start` \| `running` \| `awaiting-checkpoint` \| `approved` \| `failed` | 단계 상태. `awaiting-start`는 "이 단계 차례가 됐지만 아직 실행하지 않았다" |
```

Then insert a new subsection immediately after §2.2 (after line 100, before the `---` on line 102):

```markdown
### 2.3 &lt;targetDir&gt;/.claude-pipeline-wizard/ — 프로젝트 로컬 매니페스트

정의: `src-tauri/src/engine/project_manifest.rs`

| 파일 | 내용 | 생성/갱신 시점 |
|---|---|---|
| `run.json` | `RunRecord` 전체(`resolvedStages` 포함) — `runs/{run_id}.json`과 **같은 내용** | `start_run`, 그리고 그 뒤의 모든 상태 전이 |
| `pipeline.json` | `Template` 형태 스냅샷. `id`/`name` = `RunRecord.templateId`, `description` = 빈 문자열, `stages` = `resolvedStages` | 위와 동일 |

- **디렉터리 이름은 고정**이며(`MANIFEST_DIR_NAME = ".claude-pipeline-wizard"`) run별 하위
  디렉터리를 만들지 않는다. 따라서 **한 폴더는 run 하나만 담을 수 있다** — 이미 `run.json`이
  있는 폴더로 새 실행을 시작하면 `TargetDirInUse` 에러로 거부된다. 먼저 시작한 run의 스냅샷을
  조용히 덮어써서 잃어버리는 일이 구조적으로 불가능하다.
- **쓰기 실패는 치명적으로 취급한다.** 매니페스트 쓰기가 실패하면 `runs/`에 이미 반영된
  전이를 전이 직전 스냅샷으로 되돌리고(`Orchestrator::save_with_rollback`) 에러를 올린다.
  run은 직전 게이트 상태에 남으므로 재시도도 취소도 가능하다.
- `pipeline.json`은 **원본 템플릿이 아니라 `resolvedStages`의 스냅샷**이다. 실행 직전
  게이트에서 고친 프롬프트가 여기 남고, 갤러리의 저장된 템플릿은 그대로다.

**이 매니페스트가 기록하지 않는 것 — 명시적 결정 (IMP-020)**

> 매니페스트는 **파이프라인 정의의 스냅샷이지, 감사(audit) 목적의 완전한 실행 기록이
> 아니다.** 특히 **`request_changes`로 입력한 수정요청 피드백 프롬프트는 어디에도 기록하지
> 않는다** — `resolvedStages`에도, `pipeline.json`에도, `StageRun`의 별도 필드에도 남지
> 않는다. 피드백은 해당 단계를 1회 재실행하는 데만 쓰이는 일회성 입력으로 취급한다. 그
> 결과 "이 단계가 정확히 어떤 프롬프트로 재실행됐는가"는 `run.json`/`pipeline.json`만으로는
> 재구성할 수 없다.
>
> 남는 것은 있다. `StageRun.log`에 `StageEvent` 전체가 누적되므로 모델의 응답과 도구 사용
> 내역은 그대로 남는다. 남지 않는 것은 사용자가 입력한 피드백 텍스트뿐이다.
>
> 이렇게 정한 이유는 두 가지다. (1) `RunRecord` 스키마 변경을 `resolvedStages` 한 번으로
> 묶어 마이그레이션을 두 번 하지 않기 위해서, (2) 사용자가 입력한 자연어가 프로젝트 폴더의
> `run.json`에 그대로 남는 것을 피하기 위해서다. 완전한 감사 기록이 필요해지면 `StageRun`에
> 실행 프롬프트 이력 필드를 추가하는 것이 정공법이며, 그때는 별도의 마이그레이션이 필요하다.
```

In §3, replace the `RunRecord.templateId → Template.id` row (line 110) and append two rows:

```markdown
| RunRecord.templateId → Template.id | N:1 (소프트) | **강제 없음** | 참조 무결성 없음. 다만 **실행 중 단계 정의는 `resolvedStages`에서만 읽으므로**, 템플릿이 삭제·수정된 뒤에 체크포인트를 승인해도 실행이 실패하지 않는다. `templateId`는 표시·추적용 문자열로만 남는다 |
| RunRecord → Stage (`resolvedStages`) | 1:N (내장) | `RunRecord::new()`가 템플릿 `stages`를 복사 | 이 run 전용 파이프라인 사본. `apply_stage_override`가 현재 인덱스 1개만 교체하며, 원본 `templates/{id}.json`은 어떤 경로로도 쓰지 않는다 |
| RunRecord ↔ 프로젝트 로컬 매니페스트 | 1:1 (미러) | `Orchestrator::save()`가 두 곳에 연속으로 쓴다 | `runs/{run_id}.json`이 원본, `<targetDir>/.claude-pipeline-wizard/run.json`이 사본. 사본 쓰기 실패는 원본 쓰기를 롤백시킨다 |
```

Finally, replace §5's first two bullets (lines 132-139) with:

```markdown
- **하위 호환 원칙**: `Template`/`RunRecord` 구조체에 새 필드를 추가할 때는 `serde`의
  기본값 처리(`#[serde(default)]` 등)를 활용해 기존에 저장된 JSON 파일이 깨지지 않게
  하는 것이 원칙이다. **의도된 예외가 하나 있다**: `RunRecord.resolvedStages`는
  `#[serde(default)]` 없이 **필수 필드**로 추가됐다. 기본값을 주면 "실행할 단계 정의를
  모르는 run"이 조용히 만들어져 실행 전 게이트가 빈 화면이 되기 때문이다.
- **1회성 격리 마이그레이션 (IMP-018)**: 그 예외의 대가로, 앱 기동 시
  `engine::legacy_migration::quarantine_legacy_runs()`가 `runs/*.json`을 훑어
  `resolvedStages` 키가 없는 파일(그리고 아예 파싱되지 않는 파일)을
  `<app_data_dir>/runs-legacy-v1/`로 **이동**한다. 삭제하지 않고, 두 번째 기동부터는 옮길
  것이 없으므로 멱등(idempotent)이며, 이동 자체가 실패해도 로그만 남기고 앱은 정상
  기동한다 — 마이그레이션이 기동을 막지 않는다. 격리된 파일은 앱에서 열 수 없고, 필요하면
  사람이 직접 열어 확인해야 한다.
- **파괴적 변경 시 절차**: 필드 이름 변경이나 필수 필드 추가처럼 기존 JSON과 호환되지
  않는 변경이 또 필요해지면, 위 격리 방식(새 디렉터리로 이동)이나 1회성 변환 스크립트
  중 하나를 골라야 한다. 조용히 실패하게 두는 선택지는 없다.
- **손상 파일 대응**: `serde_json::from_str` 파싱 실패 시 `"json error: ..."`로 전체
  `list()` 호출 자체가 실패한다(→ [API 설계 문서](api-design.md) 3절). `runs/`의 손상 파일은
  위 격리 마이그레이션이 기동 시 자동으로 치워주지만, `templates/`에는 그런 장치가 없으므로
  디렉터리를 열어 손상된 `.json` 파일을 수동으로 찾아 제거/복구해야 한다.
```

- [ ] **Step 3: Update `documents/system-architecture.md` — the stop points**

In the §2 mermaid diagram, replace the `Backend` and `Storage` subgraphs (lines 39-50) with:

```
    subgraph Backend["백엔드 - Rust"]
        Commands["commands.rs - Tauri 커맨드"]
        Orchestrator["Orchestrator - 게이트/체크포인트 상태 머신"]
        Executor["executor.rs - claude 프로세스 스폰"]
        Manifest["project_manifest.rs - targetDir 매니페스트 쓰기"]
        Legacy["legacy_migration.rs - 기동 시 레거시 run 격리"]
        CliCheck["cli_check.rs - claude --version 확인"]
    end

    subgraph Storage["로컬 파일 시스템"]
        TemplatesJson[("templates/{id}.json")]
        RunsJson[("runs/{run_id}.json")]
        ManifestJson[("targetDir/.claude-pipeline-wizard/ - run.json + pipeline.json")]
    end
```

and add three edges next to `Orchestrator --> RunsJson` (line 66):

```
    Orchestrator --> Manifest
    Manifest --> ManifestJson
    Legacy --> RunsJson
```

Append two rows to the §3 component table (after line 99):

```markdown
| `project_manifest` (`engine/project_manifest.rs`) | Rust + `serde_json` | 대상 폴더 검증(절대 경로 요구)과 `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json` 쓰기. 상태 머신을 모르며, 넘겨받은 `RunRecord`를 직렬화할 뿐이다 |
| `legacy_migration` (`engine/legacy_migration.rs`) | Rust | 기동 시 1회, `resolvedStages`가 없는 구 형식 run 레코드를 `runs-legacy-v1/`로 이동. 실패해도 기동을 막지 않는다 |
```

Replace §5 in full (lines 116-131) with:

```markdown
## 5. 데이터 흐름

1. 앱 기동 시 `legacy_migration`이 `runs/`의 구 형식 레코드를 `runs-legacy-v1/`로
   격리한다(1회, 멱등, 실패해도 기동 계속).
2. 사용자가 갤러리 카드의 **실행**을 누르고 대상 폴더를 고른다 → `start_pipeline_run`
   호출. 이 커맨드는 **어떤 프로세스도 스폰하지 않는다** — 템플릿을 로드해 `RunRecord`를
   만들고 `runs/{run_id}.json`과 `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json`에
   저장한 뒤 즉시 반환한다. run은 `awaiting-stage-start`, 즉 **1단계 실행 전 게이트**에서
   멈춘다.
3. 실행 화면이 `resolvedStages[currentStageIndex]`로 편집 패널을 채운다. 사용자는
   프롬프트·권한 모드·허용 도구·체크포인트를 이 run에 한해 수정할 수 있다(원본 템플릿은
   쓰지 않는다). 단계 ID만은 표시되되 잠겨 있다.
4. **이 단계 실행**을 누르면 `start_stage(runId, expectedStageIndex, stageOverride)`가
   호출된다. run별 뮤텍스와 `expectedStageIndex` 세대 검사를 모두 통과한 호출만
   `Executor`로 내려가 `claude` 프로세스를 **정확히 하나** 스폰한다. stdout의 각 줄이
   `StageEvent`로 파싱되어 `pipeline://stage-event`로 프론트엔드까지 실시간 전달된다.
5. 프로세스가 종료되면 결과에 따라 `RunRecord`가 갱신되고 두 저장소에 다시 저장된다.
   - 실패/타임아웃 → `failed`. **종료 상태이며 이 run에서는 재시도할 수 없다**(새 run 필요).
   - 체크포인트 단계 → `awaiting-checkpoint`에서 승인/수정요청/거부를 기다린다.
   - 그 외 → **자동으로 다음 단계를 실행하지 않고** 다음 단계의 게이트로 복귀한다.
6. 체크포인트를 승인해도 결과는 같다 — 다음 단계의 게이트로 복귀할 뿐이다. 즉 **정지
   지점은 두 곳이다: 모든 단계의 실행 직전, 그리고 체크포인트 단계의 실행 직후.**
   `claude` 프로세스는 사용자가 **이 단계 실행**을 누를 때만 뜬다.
7. 마지막 단계가 승인/완료되면 `RunRecord.status`가 `completed`가 되며 흐름이 끝난다.
   어느 게이트에서든 **이 run 취소**로 `cancelled` 종료가 가능하다.

전체 흐름의 이벤트 단위 상세는 → [시퀀스 다이어그램](sequence-diagrams.md) 참고.
```

Replace the `확장성`/`복구성` rows of the §7 table (lines 165-166) with:

```markdown
| 확장성 | 단일 사용자 로컬 앱이므로 수평 확장 개념이 적용되지 않음. 여러 run을 동시에 열 수 있으며, run 하나의 load-check-save 구간은 run별 `tokio::sync::Mutex`로 직렬화된다(자식 프로세스 대기 구간에는 락을 잡지 않는다) |
| 복구성 | 모든 상태 전이가 `runs/{run_id}.json`과 `<targetDir>/.claude-pipeline-wizard/`에 즉시 저장된다. 다만 **매니페스트는 사람이 읽기 위한 것이지 앱이 다시 읽어들이지 않는다** — 앱 재시작 후 진행 중이던 run을 다시 여는 UI는 여전히 없다(v1 알려진 제약, → [사용 설명서](user-guide.md)) |
```

- [ ] **Step 4: Update `documents/user-guide.md`**

Replace the §4.1 ASCII gallery mock (lines 132-142) so the new button appears:

```
+--------------------------------------------------+
| 템플릿 갤러리                        [+ 새 템플릿] |
| CLI 상태: (초록) available:1.2.3                  |
+--------------------------------------------------+
| [웹 프로그램 개발]        | [데스크톱 프로그램...]  |
| web-app-dev  6단계        | desktop-app-dev... 5단계|
| ●요구사항 ●설계 ●프론트... | ●요구사항 ●설계 ●구현...|
| [실행][편집]        [삭제]| [실행][편집]      [삭제]|
+--------------------------------------------------+
```

Replace the last sentence of §4.2 (lines 153-154) with:

```markdown
템플릿의 원시 JSON을 바로 확인할 수 있다. 편집 화면에는 **실행 버튼이 없다** — 저장은
디스크에만 반영하며, 실행은 갤러리 카드의 **실행** 버튼에서 시작한다. 이렇게 분리해 둔
덕분에 "실행하려다 원본 템플릿이 덮어써지는" 일이 생기지 않는다. 이 run에서만 쓰고 싶은
수정은 저장하지 말고, 실행 화면의 실행 전 게이트에서 하면 된다.
```

Replace §4.3 in full (lines 156-170) with:

```markdown
### 4.3 파이프라인 실행 화면

좌측에 단계 타임라인(대기/실행 전 대기/진행/체크포인트 대기/승인됨/실패를 점 색으로
표시), 우측에 실시간 로그가 `pipeline://stage-event`를 통해 쌓인다.

**(1) 실행 전 게이트 — 모든 단계에 매번 나타난다**

run을 시작하면 아무것도 실행되지 않은 채 "다음 단계 — 실행 전 확인/수정" 패널이 먼저
뜬다. 여기에는 해당 단계의 이름·ID·프롬프트·권한 모드·허용 도구·체크포인트 스위치가
템플릿 값 그대로 채워져 있고, **단계 ID만 잠겨 있다**(바꾸면 실행 기록과 어긋난다).

| 버튼 | 동작 |
|---|---|
| **이 단계 실행** | 화면에 보이는 내용 그대로 이 단계 하나만 실행한다 |
| **이 run 취소** | run을 `cancelled`로 종료하고 갤러리로 돌아간다 |

> 여기서 고친 내용은 **이 run에만** 적용되며 저장된 템플릿은 바뀌지 않는다. 반대로 하단의
> **원본 템플릿 편집 (모든 향후 실행에 적용)** 버튼은 저장된 템플릿을 바꾸는 완전히 다른
> 경로이며, 진행 중인 run에는 반영되지 않는다 — 눌렀을 때 확인 창이 한 번 뜬다.

**(2) 체크포인트 — `checkpoint: true` 단계가 끝난 직후에만 나타난다**

| 버튼 | 동작 |
|---|---|
| **승인** | 현재 단계를 완료 처리하고 **다음 단계의 실행 전 게이트로 돌아간다**(마지막 단계면 파이프라인 완료) |
| **수정 요청 보내기** | 입력한 피드백 텍스트로 **같은 단계를 재실행**(원래 프롬프트를 완전히 대체). 이 피드백 텍스트는 어디에도 저장되지 않는다 |
| **거부** | 프로세스를 다시 실행하지 않고 파이프라인 자체를 취소(cancelled) 처리 |

변경된 파일 배지(Write/Edit 도구 호출에서 감지한 경로)가 체크포인트 카드 위에 같이
표시되어, 승인 전에 무엇이 바뀌었는지 훑어볼 수 있다.

**승인해도 다음 단계가 자동으로 시작되지 않는다** — 승인의 결과는 언제나 다음 단계의 실행 전
게이트다. 단계 실행이 실패하면 run은 `failed`로 종료되며, 그 run에서는 재시도할 수 없다.
프롬프트를 고쳐 다시 시도하려면 새 run을 시작해야 한다.
```

Replace the §6.1 data-location table (lines 192-195) with:

```markdown
| 데이터 | 경로 |
|---|---|
| 템플릿 | `<app_data_dir>/templates/{id}.json` |
| 실행 기록/로그 | `<app_data_dir>/runs/{run_id}.json` |
| 격리된 구 형식 실행 기록 | `<app_data_dir>/runs-legacy-v1/{run_id}.json` (앱이 읽지 않음) |
| 프로젝트 로컬 매니페스트 | `<targetDir>/.claude-pipeline-wizard/run.json`, `pipeline.json` |
```

Replace the §8 bullet list (lines 248-256) with:

```markdown
- **새로 만든 폴더만 지원** — 템플릿은 사용자가 고른 폴더에 대해 실행되며, 이미
  `.claude-pipeline-wizard/run.json`이 있는 폴더는 **거부된다**(덮어쓰지 않는다).
  기존 코드베이스에 대해 실행하는 것은 아직 지원되지 않는다.
- **재시작 후 재개(resume) UI 없음** — 실행 상태는 전이마다 디스크에 저장되지만,
  **앱을 재시작한 뒤 진행 중이던 런에 다시 연결할 커맨드나 화면이 없다.** 오늘 기준
  복구 방법은 `runs/` 디렉터리 아래의 JSON 파일을 직접 열어 확인하는 것뿐이다
  (6.4절 참고).
- **프로젝트 로컬 매니페스트는 조회를 쉽게 할 뿐 resume을 해결하지 않는다** —
  `<targetDir>/.claude-pipeline-wizard/`가 매 전이마다 갱신되므로 폴더만 열어도 상태를
  읽을 수 있지만, 앱은 이 파일을 **다시 읽어들이지 않는다.** 위 제약은 그대로다.
- **구 형식 실행 기록은 열 수 없다** — 이 버전 최초 기동 시 `resolvedStages`가 없는 기존
  레코드는 `runs-legacy-v1/`로 이동된다. 삭제되지는 않지만 앱에서 열 수는 없다.
- **실패한 run은 종료 상태다** — 실패한 단계를 같은 run에서 다시 실행할 수 없다. 새 run을
  시작해야 한다.
- **순차 단계만 지원** — 템플릿 내에서 단계의 병렬 실행이나 조건 분기는 지원하지
  않는다.
```

- [ ] **Step 5: Update `documents/api-design.md` and `documents/api-user-guide.md`**

In `documents/api-design.md` §3, append four rows to the error table (after line 80):

```markdown
| `OrchestratorError::NotAwaitingStageStart` | `"run '{id}' is not awaiting a stage start"` | 게이트에 있지 않은 run에 `start_stage` 호출 |
| `OrchestratorError::StaleStageIndex` | `"STALE_STAGE_INDEX: run is at stage {actual}, caller expected {expected}"` | 이미 지나간 게이트를 대상으로 한 `start_stage` 호출. **접두사가 계약이다** — 프론트엔드는 이 문자열로만 이 에러를 구분하고, `get_run`으로 조용히 재동기화한다 |
| `OrchestratorError::StageIdMismatch` | `"stage override id '{got}' does not match resolved stage '{expected}'"` | `stageOverride.id`가 현재 단계 id와 다름 |
| `OrchestratorError::TargetDirInUse` | `"target dir already contains a pipeline run: {path}"` | 이미 `.claude-pipeline-wizard/run.json`이 있는 폴더로 새 실행 시도 |
| `OrchestratorError::NotCancellable` | `"run '{id}' cannot be cancelled in its current state"` | 이미 종료된 run에 `cancel_run` 호출 |
```

Replace `start_pipeline_run`'s 동작/응답/에러 block (lines 226-247) with (the outer fence here is four backticks so the nested JSON block survives — write only the inner content into the file):

````markdown
**동작**: 템플릿을 로드·검증하고 `targetDir`을 검증(절대 경로 요구, 없으면 생성)한 뒤
`RunRecord`를 만들어 `runs/{runId}.json`과 `<targetDir>/.claude-pipeline-wizard/`에
저장하고 반환한다. **어떤 `claude` 프로세스도 스폰하지 않으며**, run은 1단계의 실행 전
게이트에서 멈춘다. 실제 실행은 `start_stage`가 담당한다.

**성공 응답**: `RunRecord`
```json
{
  "runId": "b3f1...",
  "templateId": "web-app-dev",
  "targetDir": "/Users/me/projects/my-new-app",
  "status": "awaiting-stage-start",
  "currentStageIndex": 0,
  "stages": [
    { "id": "requirements", "status": "awaiting-start", "sessionId": null, "log": [] },
    { "id": "design", "status": "pending", "sessionId": null, "log": [] }
  ],
  "resolvedStages": [ /* Stage[] — 이 run이 실행할 단계 정의의 사본 */ ]
}
```

**에러**: 템플릿을 찾을 수 없음, 상대 경로 `targetDir`, 폴더 생성 실패(`io error`),
매니페스트 쓰기 실패, 그리고 이미 실행 기록이 있는 폴더(`target dir already contains a
pipeline run`).
````

Insert three new command subsections after `start_pipeline_run` (after line 249's `---`) — again a four-backtick outer fence:

````markdown
#### `start_stage` — 게이트에 멈춘 단계를 하나 실행 (비동기)

**요청**
```ts
invoke("start_stage", {
  runId: "b3f1...",
  expectedStageIndex: 0,
  stageOverride: { id: "requirements", name: "요구사항", prompt: "이번엔 이렇게", permissionMode: "acceptEdits", allowedTools: ["Read"], checkpoint: true }
})
```

| 파라미터 | 타입 | 설명 |
|---|---|---|
| `runId` | string | 대상 실행 id |
| `expectedStageIndex` | number | **세대(generation) 가드.** 호출자가 화면에 렌더한 바로 그 `currentStageIndex`. run이 이미 다음 게이트로 넘어갔다면 `STALE_STAGE_INDEX:` 에러로 거부된다 |
| `stageOverride` | `Stage \| null` | 이 실행에만 적용할 단계 정의. `id`는 현재 단계와 같아야 하며, 통과하면 `resolvedStages`의 해당 인덱스를 교체한다. `null`이면 현재 `resolvedStages` 값을 그대로 쓴다 |

**동작**: run별 뮤텍스 안에서 상태·세대·단계 유효성을 검사하고 `RunRecord`를 `running`으로
전이시켜 저장한 뒤, **락을 놓고** `claude` 프로세스를 정확히 하나 스폰한다. 종료 후 결과에
따라 `awaiting-checkpoint` / 다음 게이트(`awaiting-stage-start`) / `completed` / `failed`로
전이한다. **원본 템플릿 파일은 어떤 경우에도 쓰지 않는다.**

**성공 응답**: 갱신된 `RunRecord`

---

#### `cancel_run` — 실행 취소 (동기)

**요청**
```ts
invoke("cancel_run", { runId: "b3f1..." })
```

**동작**: `claude` 프로세스를 실행하지 않고 run 상태를 `cancelled`로 표시해 두 저장소에
저장한다. 게이트나 체크포인트에서 언제든 호출할 수 있어, 사용자가 어느 지점에서도
빠져나올 수 있게 한다.

**성공 응답**: 갱신된 `RunRecord` (`status: "cancelled"`)
**에러**: 이미 종료된 run이면 `"run '{id}' cannot be cancelled in its current state"`.

---

#### `get_run` — 실행 기록 단건 조회 (동기, 읽기 전용)

**요청**
```ts
invoke("get_run", { runId: "b3f1..." })
```

**동작**: `runs/{runId}.json`을 읽어 그대로 반환한다. 상태를 바꾸지 않으므로 run 락을 잡지
않는다. 프론트엔드는 `STALE_STAGE_INDEX:` 거부를 받은 직후 이 커맨드로 화면을 조용히
재동기화한다.

**성공 응답**: `RunRecord`
**에러**: `"run '{id}' not found"`.

---
````

In `documents/api-user-guide.md`, rewrite 시나리오 3 (lines 98-124) so it reflects the two-call flow — `startPipelineRun`으로 run을 만들고, 게이트마다 `startStage(runId, run.currentStageIndex, editedStage)`를 호출하며, 종료 상태가 될 때까지 반복 — and append the four new wrappers to the §4 API list (after line 194):

```markdown
| `startStage(runId, expectedStageIndex, stageOverride?)` | `start_stage` | 게이트에 멈춘 단계 하나를 실행. 두 번째 인자는 선택이 아니라 **세대 가드**이므로 화면이 렌더한 인덱스를 그대로 넘겨야 한다 |
| `cancelRun(runId)` | `cancel_run` | 실행을 `cancelled`로 종료 |
| `getRun(runId)` | `get_run` | 읽기 전용 재조회 |
| `isStaleStageIndexError(error)` | — | `startStage` 거부가 세대 불일치인지 판별하는 유일한 지점. 참이면 에러를 띄우지 말고 `getRun`으로 화면을 갱신한다 |
```

- [ ] **Step 6: Update `documents/sequence-diagrams.md`, `class-diagram.md`, `er-diagram.md`**

`documents/sequence-diagrams.md` §2 (lines 54-113): the current diagram shows `start_pipeline_run` spawning stage 1 and auto-advancing on approval. Replace its message sequence with the gate loop — `start_pipeline_run` returns `RunRecord(awaiting-stage-start)` with **no** `Executor` participant involved; a separate `start_stage` interaction spawns exactly one `claude` process; and both the non-checkpoint completion path and the `approve_checkpoint` path end with a `RunRecord(awaiting-stage-start, index+1)` return rather than another spawn. Add the manifest write as a message from `Orchestrator` to a new `ProjectManifest` participant on every transition, and keep §2.1 (수정 요청) and §3 (실패/타임아웃) — they are still accurate, except that §3's "다음 단계로 자동 진행되지 않는다" note now applies to every stage, not only failed ones.

`documents/class-diagram.md`:
- `class RunRecord` (lines 43-52): add `+Vec~Stage~ resolvedStages` and the four new methods `+currentResolvedStage() Stage`, `+applyStageOverride(stage) Result`, `+beginCurrentStage()`, `+advanceToGate(sessionId) bool`.
- `class RunStatus` (line 63): add `AwaitingStageStart`. `class StageStatus` (line 70): add `AwaitingStart`.
- `class Orchestrator` (lines 127-137): remove `drive()`, add `startStage(runId, expectedStageIndex, stageOverride)`, `runCurrentStage(...)`, `cancelRun(runId)`, `save(record)`, `saveWithRollback(record, snapshot)`, `lockFor(runId)`, and the `runLocks` field.
- Add `class ProjectManifest` and `class StageOverrideError` with an `Orchestrator ..> ProjectManifest : 매 전이마다 씀` edge, and note in §"주요 설계 포인트" that the orchestrator no longer loops.

`documents/er-diagram.md`:
- In the mermaid block, add `Stage[] resolvedStages "이 run 전용 파이프라인 사본"` to `RUN_RECORD`, add `awaiting-start` / `awaiting-stage-start` to the status comments, and add a `PROJECT_MANIFEST` entity (`run.json`, `pipeline.json`) with a `RUN_RECORD ||--|| PROJECT_MANIFEST : "매 전이마다 미러링"` relation.
- In §"관계별 상세 설명", add the point that `RunRecord.resolvedStages`는 `Template.stages`에서 **복사**된 뒤 독립적으로 변하며, 이 복사가 원본 템플릿 오염을 구조적으로 막는다.

- [ ] **Step 7: Regenerate `documents/docs-portal.html`**

The portal is a `dev-docs-builder-v2` artifact, but **no generator script is checked into this repository** — `scripts/` holds only `build/`, `dev/` and `installer/`. Confirm that first:

```bash
ls scripts/
grep -rl "docs-portal" --include=*.sh --include=*.bat --include=*.ps1 --include=*.js . | grep -v node_modules
```
Expected: no generator. If one turns up, run it instead of hand-editing and skip to the verification below.

Otherwise edit the HTML directly. The portal inlines each markdown file as an HTML `<section>`; mirror Steps 2-6 into the matching section, converting each edited block to the surrounding markup style (`<table>` rows, `<li>` bullets, `<pre class="mermaid">` for diagrams). The anchors:

| Section | What to mirror | Current anchor |
|---|---|---|
| `#database-design` | the three-store table, `resolvedStages` rows, the whole new 2.3 including the D7 blockquote, the §3 rows, the §5 bullets | line 683 (`<h2>1. 저장소(테이블) 목록</h2>`), line 690 (the `runs/` row), line 780 (`<h2>5. "마이그레이션" 전략</h2>`) |
| `#system-architecture` | the mermaid subgraphs and the rewritten §5 | line 1354 (`<h2>5. 데이터 흐름</h2>`), line 1360 (the "체크포인트가 설정돼 있으면 여기서 멈추고" list item — **this sentence must be gone**) |
| `#user-guide` | §4.1/4.2/4.3, §6.1, §8 | line 1555 (`<h2>8. 알려진 v1 제약 사항</h2>`), lines 1558-1559 |
| `#api-design`, `#api-user-guide`, `#sequence-diagrams`, `#class-diagram`, `#er-diagram` | Steps 5-6 | the corresponding `<section id=...>` starting at lines 263, 519, 853, 1034, 789 |

The sidebar (line 247, "개발 문서 포털 · 8개 문서") is unchanged — no document is added or removed.

Verify the sweep mechanically:

```bash
grep -c "awaiting-stage-start" documents/docs-portal.html
grep -n "체크포인트가 설정돼 있으면 여기서 멈추고" documents/docs-portal.html
grep -c "claude-pipeline-wizard" documents/docs-portal.html
grep -rn "편집 화면의 실행 버튼" documents/ README.md
```
Expected: the first ≥ 3, the second **no match**, the third ≥ 3, the fourth **no match** (that phrase described the entry point decision D2 removed).

- [ ] **Step 8: Verify nothing regressed**

Run: `npm test`
Run: `npx tsc --noEmit`
Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS, unchanged from Task 15. This task edits no source, so any failure here means something else is wrong — stop and fix it before committing documentation that claims the app works.

- [ ] **Step 9: Run the manual smoke on the real CLI (PRD F-2 / TRD §5.4)**

This is the one part of the plan the automated suites cannot cover: every backend test runs against `mock_claude`, so nothing so far has proven the gate works with a real `claude` process. Run it against a **new, empty folder** — not one an earlier attempt already used, or (c) will be a `TargetDirInUse` refusal instead.

```bash
npx tauri dev
```

Then walk the five items and tick each only after seeing the pass condition with your own eyes:

- [ ] **(a) 템플릿 실행 시작** — gallery → **실행** → pick an empty folder.
      Pass: the **1단계 편집 패널**("다음 단계 — 실행 전 확인/수정") appears with *no process having run*, and the live log area is empty. If any log line appears before you press anything, `start_run` is still spawning and PRD A-1 fails.
- [ ] **(b) 1단계 프롬프트를 알아볼 수 있게 수정 후 '이 단계 실행'** — e.g. append
      "그리고 파일 맨 위에 SMOKE-B 라고 적어라".
      Pass: the live log shows the *edited* prompt's effect (the marker appears), not the template's original behaviour. Then confirm `templates/{id}.json` on disk is **unchanged** — that is PRD C-1 observed rather than asserted.
- [ ] **(c) targetDir을 파일 탐색기로 열어본다**
      Pass: `.claude-pipeline-wizard/run.json` and `pipeline.json` both exist; `pipeline.json` contains the **edited** stage-1 prompt from (b); and the files' contents change again after the next transition.
- [ ] **(d) `checkpoint: false` 단계 종료를 기다린다** — use a template whose stage has the checkpoint switch off (or turn it off at that stage's gate).
      Pass: when the stage ends, the app **returns to the next stage's edit panel** and starts nothing on its own.
- [ ] **(e) 체크포인트 단계에서 '승인'**
      Pass: approving likewise lands on the **next stage's edit panel**, not in a running stage.

Also spot-check the two escape hatches while you are here: **이 run 취소** at any gate returns to the gallery with the run marked `cancelled`, and a second **실행** against the same folder is refused with the `target dir already contains a pipeline run` message rather than silently overwriting (a) through (c)'s manifest.

If any item fails, it is a defect in Tasks 7-15, not in the documentation — fix it there and re-run this step before committing.

- [ ] **Step 10: Commit**

```bash
git add README.md documents/
git commit -m "docs: describe the pre-stage gate, the third store, and what is not recorded

Every stage now stops before it runs and the target folder carries a manifest, so
the README's 'pauses at checkpoints' story and documents/'s two-store model were
both stale. README gets the gate flow, the new engine and component files, and
three added limitations: the manifest aids inspection but does not make a run
resumable, pre-resolvedStages records are quarantined rather than read, and a
folder that already holds a run is refused.

database-design.md gains the project-local manifest as a third store and, per
IMP-020, states in writing that the manifest is a snapshot of the pipeline
definition and not an audit log — request_changes feedback prompts are recorded
nowhere. That sentence is the whole deliverable of TRD 6.7 option C; without it
the decision is merely unmade. system-architecture.md's single stop point becomes
two, and user-guide.md documents the gate's two buttons and the fact that approving
a checkpoint lands on the next gate rather than in the next process.

Closes the PRD F-2 manual smoke (a)-(e), run against the real CLI."
```
