# Pipeline Run Editing & Project-Local Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pre-stage edit gate (every stage pauses for review/edit before it runs, not just after checkpointed stages) and a project-local run manifest (`<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json`), so a run's live state and its actually-executing (possibly per-run-edited) stage definitions live inside the target project folder, not only in the app's data directory.

**Architecture:** `RunRecord` gains a `resolvedStages: Stage[]` field (the per-run, possibly-edited stage list) and two new state values (`RunStatus::AwaitingStageStart`, `StageStatus::AwaitingStart`). The orchestrator's `start_run` now only creates the paused record (no execution); a new `start_stage` command applies an optional per-run stage override and then runs exactly one stage, mirroring today's single-stage execution logic. A new `engine::project_manifest` module writes `run.json` + `pipeline.json` into the target directory on every transition, alongside the existing `RunRecordStore` write. The frontend extracts the stage-fields form (prompt/permission mode/allowed tools/checkpoint) out of `TemplateEditor.tsx` into a shared `StageFields` component, reused by a new pre-stage edit panel in `PipelineRun.tsx`.

**Tech Stack:** Tauri v2, Rust (tokio, serde, thiserror, uuid), React 18 + TypeScript + Vite, Vitest + React Testing Library. Same as the base app — no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md` (extends `docs/superpowers/specs/2026-09-06-claude-pipeline-wizard-design.md`)

## Global Constraints

- Pre-stage edit gate applies to **every** stage, checkpointed or not — no stage may execute without first passing through `awaiting-stage-start`.
- The project-directory manifest (`.claude-pipeline-wizard/run.json` + `pipeline.json`) is written **in addition to** the existing `app_data_dir/runs/` copy, never instead of it.
- Per-run stage edits (the pre-stage override) are run-scoped only — they mutate `resolvedStages` on the run record, never the saved gallery `Template`.
- No new dependencies, no database — still flat JSON files on disk, same as the base app's constraints.
- Preserve all currently-passing behavior not explicitly changed here (template CRUD, checkpoint approve/request-changes/reject after a stage finishes, failed-stage → edit-template-and-restart flow).

---

## Task 1: Factor `validate_stage` Out of `validate_template`

**Files:**
- Modify: `src-tauri/src/template/mod.rs`

**Interfaces:**
- Consumes: existing `Stage`, `Template`, `TemplateValidationError`, `is_valid_id`.
- Produces: `pub fn validate_stage(stage: &Stage) -> Result<(), TemplateValidationError>` — validates one stage's `id` and `prompt` in isolation (no duplicate-id or empty-stage-list checks, which only make sense at the template level). `validate_template` is refactored to call it per-stage. Later tasks (orchestrator's `start_stage`) use this to validate a per-run stage override without needing a whole `Template`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src-tauri/src/template/mod.rs` (above the existing `serializes_permission_mode_as_camel_case` test):

```rust
    #[test]
    fn validate_stage_rejects_empty_prompt() {
        let s = stage("s1", "   ");
        assert_eq!(validate_stage(&s), Err(TemplateValidationError::EmptyPrompt("s1".to_string())));
    }

    #[test]
    fn validate_stage_rejects_invalid_id() {
        let s = stage("../bad", "do it");
        assert_eq!(validate_stage(&s), Err(TemplateValidationError::InvalidId("../bad".to_string())));
    }

    #[test]
    fn validate_stage_accepts_valid_stage() {
        let s = stage("s1", "do it");
        assert_eq!(validate_stage(&s), Ok(()));
    }
```

Add the (not yet implemented) function signature above `validate_template`:

```rust
pub fn validate_stage(stage: &Stage) -> Result<(), TemplateValidationError> {
    todo!()
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test template::`
Expected: FAIL — `todo!()` panics on the three new tests.

- [ ] **Step 3: Implement `validate_stage` and refactor `validate_template` to use it**

```rust
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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test template::`
Expected: PASS (all existing `template::` tests plus the 3 new ones — the refactor must not change `validate_template`'s observable behavior).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: factor validate_stage out of validate_template"
```

---

## Task 2: RunRecord — Pre-Stage-Start State & Resolved Stages

**Files:**
- Modify: `src-tauri/src/engine/run_record.rs` (full rewrite of the non-store portion; `RunRecordStore` itself is unchanged)

**Interfaces:**
- Consumes: `Stage` from `crate::template`.
- Produces: `RunStatus` gains `AwaitingStageStart` (serializes as `"awaiting-stage-start"`); `StageStatus` gains `AwaitingStart` (serializes as `"awaiting-start"`, distinct from the existing `Pending`, which is now reserved for stages not yet reached). `RunRecord` gains `pub resolved_stages: Vec<Stage>`. `RunRecord::new(run_id, template_id, target_dir, stages: &[Stage]) -> Self` (signature change: takes `&[Stage]` instead of `&[String]`, seeds `resolved_stages` from it, starts `status: AwaitingStageStart`, stage 0 `AwaitingStart`, the rest `Pending`). New methods: `pub fn current_resolved_stage(&self) -> &Stage`, `pub fn apply_stage_override(&mut self, stage: Stage)`, `pub fn begin_current_stage(&mut self)` (sets current stage + run status to `Running`). `approve_current` now advances the next stage to `AwaitingStart` / run status `AwaitingStageStart` instead of immediately marking it `Running`.

- [ ] **Step 1: Write the failing tests (replace the file's `mod tests` block and struct/impl)**

Replace the top of `src-tauri/src/engine/run_record.rs` (everything above `#[cfg(test)] mod tests`) with:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::template::{is_valid_id, Stage};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    AwaitingStageStart,
    Running,
    AwaitingCheckpoint,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StageStatus {
    Pending,
    AwaitingStart,
    Running,
    AwaitingCheckpoint,
    Approved,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageRun {
    pub id: String,
    pub status: StageStatus,
    pub session_id: Option<String>,
    pub log: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    pub template_id: String,
    pub target_dir: String,
    pub status: RunStatus,
    pub current_stage_index: usize,
    pub stages: Vec<StageRun>,
    pub resolved_stages: Vec<Stage>,
}

impl RunRecord {
    pub fn new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self {
        todo!()
    }

    pub fn current_stage_mut(&mut self) -> &mut StageRun {
        &mut self.stages[self.current_stage_index]
    }

    pub fn current_resolved_stage(&self) -> &Stage {
        &self.resolved_stages[self.current_stage_index]
    }

    pub fn apply_stage_override(&mut self, stage: Stage) {
        todo!()
    }

    pub fn begin_current_stage(&mut self) {
        todo!()
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

    /// Marks the current stage approved and advances to the next stage,
    /// which becomes `AwaitingStart` (paused for the pre-stage edit gate).
    /// Returns `true` if this was the last stage (run is now complete).
    pub fn approve_current(&mut self) -> bool {
        todo!()
    }

    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunRecordStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("run '{0}' not found")]
    NotFound(String),
    #[error("invalid id '{0}': ids must be non-empty, contain only letters, digits, '.', '_', or '-', and not be '.' or '..'")]
    InvalidId(String),
}

pub struct RunRecordStore {
    dir: PathBuf,
}

impl RunRecordStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, run_id: &str) -> PathBuf {
        self.dir.join(format!("{run_id}.json"))
    }

    pub fn save(&self, record: &RunRecord) -> Result<(), RunRecordStoreError> {
        if !is_valid_id(&record.run_id) {
            return Err(RunRecordStoreError::InvalidId(record.run_id.clone()));
        }
        fs::create_dir_all(&self.dir)?;
        let content = serde_json::to_string_pretty(record)?;
        fs::write(self.path_for(&record.run_id), content)?;
        Ok(())
    }

    pub fn load(&self, run_id: &str) -> Result<RunRecord, RunRecordStoreError> {
        if !is_valid_id(run_id) {
            return Err(RunRecordStoreError::InvalidId(run_id.to_string()));
        }
        let path = self.path_for(run_id);
        if !path.exists() {
            return Err(RunRecordStoreError::NotFound(run_id.to_string()));
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }
}
```

Replace the `#[cfg(test)] mod tests` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::PermissionMode;

    fn stage(id: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: id.to_string(),
            prompt: "do it".to_string(),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec![],
            checkpoint: true,
        }
    }

    fn record(ids: &[&str]) -> RunRecord {
        let stages: Vec<Stage> = ids.iter().map(|id| stage(id)).collect();
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &stages)
    }

    #[test]
    fn new_starts_at_stage_zero_awaiting_start() {
        let r = record(&["a", "b"]);
        assert_eq!(r.current_stage_index, 0);
        assert_eq!(r.status, RunStatus::AwaitingStageStart);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingStart);
        assert_eq!(r.stages[1].status, StageStatus::Pending);
        assert_eq!(r.resolved_stages.len(), 2);
        assert_eq!(r.resolved_stages[0].id, "a");
    }

    #[test]
    fn begin_current_stage_marks_running() {
        let mut r = record(&["a"]);
        r.begin_current_stage();
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages[0].status, StageStatus::Running);
    }

    #[test]
    fn apply_stage_override_replaces_resolved_stage() {
        let mut r = record(&["a", "b"]);
        let mut edited = r.resolved_stages[0].clone();
        edited.prompt = "edited prompt".to_string();
        r.apply_stage_override(edited);
        assert_eq!(r.resolved_stages[0].prompt, "edited prompt");
        assert_eq!(r.resolved_stages[1].prompt, "do it");
    }

    #[test]
    fn mark_current_awaiting_checkpoint_updates_stage_and_run() {
        let mut r = record(&["a", "b"]);
        r.begin_current_stage();
        r.mark_current_awaiting_checkpoint("sess1".to_string());
        assert_eq!(r.status, RunStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].status, StageStatus::AwaitingCheckpoint);
        assert_eq!(r.stages[0].session_id, Some("sess1".to_string()));
    }

    #[test]
    fn mark_current_failed_updates_stage_and_run() {
        let mut r = record(&["a"]);
        r.begin_current_stage();
        r.mark_current_failed();
        assert_eq!(r.status, RunStatus::Failed);
        assert_eq!(r.stages[0].status, StageStatus::Failed);
    }

    #[test]
    fn approve_current_advances_to_awaiting_start() {
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
        let completed = r.approve_current();
        assert!(completed);
        assert_eq!(r.status, RunStatus::Completed);
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

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test engine::run_record::`
Expected: FAIL to compile / `todo!()` panics (`new`, `apply_stage_override`, `approve_current` are unimplemented).

- [ ] **Step 3: Implement the transition methods**

```rust
    pub fn new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self {
        let stage_runs = stages
            .iter()
            .enumerate()
            .map(|(i, s)| StageRun {
                id: s.id.clone(),
                status: if i == 0 { StageStatus::AwaitingStart } else { StageStatus::Pending },
                session_id: None,
                log: Vec::new(),
            })
            .collect();
        Self {
            run_id,
            template_id,
            target_dir,
            status: RunStatus::AwaitingStageStart,
            current_stage_index: 0,
            stages: stage_runs,
            resolved_stages: stages.to_vec(),
        }
    }
```

```rust
    pub fn apply_stage_override(&mut self, stage: Stage) {
        self.resolved_stages[self.current_stage_index] = stage;
    }

    pub fn begin_current_stage(&mut self) {
        self.current_stage_mut().status = StageStatus::Running;
        self.status = RunStatus::Running;
    }
```

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test engine::run_record::`
Expected: PASS (14 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(run-record): add pre-stage-start state and resolved stages"
```

---

## Task 3: Project-Local Manifest Writer

**Files:**
- Create: `src-tauri/src/engine/project_manifest.rs`
- Modify: `src-tauri/src/engine/mod.rs` (add `pub mod project_manifest;`)

**Interfaces:**
- Consumes: `Template`, `Stage` (`crate::template`); `RunRecord` (Task 2).
- Produces: `pub fn manifest_dir(target_dir: &Path) -> PathBuf` (returns `target_dir.join(".claude-pipeline-wizard")`), `pub fn write_project_manifest(target_dir: &Path, template_id: &str, template_name: &str, template_description: &str, record: &RunRecord) -> Result<(), ProjectManifestError>` — writes `run.json` (the full record) and `pipeline.json` (a `Template`-shaped snapshot built from `record.resolved_stages`) into `manifest_dir(target_dir)`, creating it if needed. `pub enum ProjectManifestError { Io, Json }`.

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/engine/project_manifest.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use crate::template::Template;

use super::run_record::RunRecord;

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn manifest_dir(target_dir: &Path) -> PathBuf {
    target_dir.join(".claude-pipeline-wizard")
}

/// Writes the run's live state (`run.json`) and a resolved-template
/// snapshot (`pipeline.json`, reflecting any per-run stage edits) into a
/// hidden directory inside the run's target project folder, so the run is
/// inspectable/editable by hand even without the app's own data directory.
pub fn write_project_manifest(
    target_dir: &Path,
    template_id: &str,
    template_name: &str,
    template_description: &str,
    record: &RunRecord,
) -> Result<(), ProjectManifestError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    fn sample_record() -> RunRecord {
        let stages = vec![Stage {
            id: "s1".to_string(),
            name: "Stage 1".to_string(),
            prompt: "do it".to_string(),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec![],
            checkpoint: true,
        }];
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &stages)
    }

    #[test]
    fn writes_run_json_and_pipeline_json_under_hidden_dir() {
        let target_dir = tempfile::tempdir().unwrap();
        let record = sample_record();

        write_project_manifest(target_dir.path(), "tpl1", "Template", "desc", &record).unwrap();

        let dir = manifest_dir(target_dir.path());
        let run_json: RunRecord =
            serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(run_json, record);

        let pipeline_json: Template =
            serde_json::from_str(&fs::read_to_string(dir.join("pipeline.json")).unwrap()).unwrap();
        assert_eq!(pipeline_json.id, "tpl1");
        assert_eq!(pipeline_json.name, "Template");
        assert_eq!(pipeline_json.stages, record.resolved_stages);
    }

    #[test]
    fn overwrites_existing_manifest_on_repeated_writes() {
        use crate::engine::run_record::RunStatus;

        let target_dir = tempfile::tempdir().unwrap();
        let mut record = sample_record();
        write_project_manifest(target_dir.path(), "tpl1", "Template", "desc", &record).unwrap();

        record.approve_current();
        write_project_manifest(target_dir.path(), "tpl1", "Template", "desc", &record).unwrap();

        let dir = manifest_dir(target_dir.path());
        let run_json: RunRecord =
            serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(run_json.status, RunStatus::Completed);
    }
}
```

Add `pub mod project_manifest;` to `src-tauri/src/engine/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test engine::project_manifest::`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement `write_project_manifest`**

```rust
pub fn write_project_manifest(
    target_dir: &Path,
    template_id: &str,
    template_name: &str,
    template_description: &str,
    record: &RunRecord,
) -> Result<(), ProjectManifestError> {
    let dir = manifest_dir(target_dir);
    fs::create_dir_all(&dir)?;

    fs::write(dir.join("run.json"), serde_json::to_string_pretty(record)?)?;

    let snapshot = Template {
        id: template_id.to_string(),
        name: template_name.to_string(),
        description: template_description.to_string(),
        stages: record.resolved_stages.clone(),
    };
    fs::write(dir.join("pipeline.json"), serde_json::to_string_pretty(&snapshot)?)?;

    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test engine::project_manifest::`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(engine): write run state and resolved template snapshot into the project directory"
```

---

## Task 4: Orchestrator — Pre-Stage-Start Gate

**Files:**
- Modify: `src-tauri/src/engine/orchestrator.rs` (full rewrite)
- Modify (full rewrite): `src-tauri/tests/orchestrator_test.rs`
- Create: `src-tauri/tests/fixtures/orchestrator_stage1_edited.jsonl`

**Interfaces:**
- Consumes: `Stage`, `Template`, `validate_stage`, `TemplateValidationError` (Task 1); `RunRecord`, `RunStatus`, `StageStatus` w/ new variants (Task 2); `write_project_manifest`, `ProjectManifestError` (Task 3); `run_stage`, `ExecutorConfig` (unchanged, existing); `StageEvent` (unchanged, existing).
- Produces: `Orchestrator::start_run(&self, template_id: &str, target_dir: PathBuf, run_id: String) -> Result<RunRecord, OrchestratorError>` (now **sync**, no `on_event` param — it no longer executes anything). `Orchestrator::start_stage<F: FnMut(&str, StageEvent)>(&self, run_id: &str, stage_override: Option<Stage>, on_event: F) -> Result<RunRecord, OrchestratorError>` (new — runs exactly the current, paused stage). `Orchestrator::approve_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError>` (now **sync**, no `on_event` param — it only advances state). `Orchestrator::request_changes` and `Orchestrator::reject_checkpoint` keep their existing signatures. `OrchestratorError` gains `NotAwaitingStageStart(String)`, `StageIdMismatch(String)`, `StageValidation(#[from] TemplateValidationError)`, `ProjectManifest(#[from] ProjectManifestError)`.

- [ ] **Step 1: Create the new fixture**

`src-tauri/tests/fixtures/orchestrator_stage1_edited.jsonl`:
```
{"type":"system","session_id":"sess-orch-1-edited"}
{"type":"result","subtype":"success","session_id":"sess-orch-1-edited","result":"stage1 edited done"}
```

(`orchestrator_stage1.jsonl`, `orchestrator_stage2.jsonl`, `orchestrator_feedback.jsonl` already exist from the base plan and are reused unchanged.)

- [ ] **Step 2: Write the failing stub and replace the integration test file**

Replace `src-tauri/src/engine/orchestrator.rs` in full:

```rust
use std::path::{Path, PathBuf};

use crate::template::{
    store::StoreError as TemplateStoreError, store::TemplateStore, validate_stage, Stage, Template,
    TemplateValidationError,
};

use super::executor::{run_stage, ExecutorConfig};
use super::project_manifest::{write_project_manifest, ProjectManifestError};
use super::run_record::{RunRecord, RunRecordStore, RunRecordStoreError, RunStatus, StageStatus};
use super::stream_json::StageEvent;

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    TemplateStore(#[from] TemplateStoreError),
    #[error(transparent)]
    RunRecordStore(#[from] RunRecordStoreError),
    #[error(transparent)]
    StageValidation(#[from] TemplateValidationError),
    #[error(transparent)]
    ProjectManifest(#[from] ProjectManifestError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("run '{0}' is not awaiting a checkpoint")]
    NotAwaitingCheckpoint(String),
    #[error("run '{0}' is not awaiting a stage start")]
    NotAwaitingStageStart(String),
    #[error("stage override id '{0}' does not match the current stage")]
    StageIdMismatch(String),
}

pub struct Orchestrator {
    pub template_store: TemplateStore,
    pub run_store: RunRecordStore,
    pub executor_config: ExecutorConfig,
}

impl Orchestrator {
    pub fn new(template_store: TemplateStore, run_store: RunRecordStore, executor_config: ExecutorConfig) -> Self {
        Self { template_store, run_store, executor_config }
    }

    fn save(&self, template: &Template, record: &RunRecord) -> Result<(), OrchestratorError> {
        todo!()
    }

    pub fn start_run(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
    ) -> Result<RunRecord, OrchestratorError> {
        todo!()
    }

    pub async fn start_stage<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        stage_override: Option<Stage>,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        todo!()
    }

    pub fn approve_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        todo!()
    }

    pub async fn request_changes<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        feedback: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        todo!()
    }

    pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        record.cancel();
        self.save(&template, &record)?;
        Ok(record)
    }
}
```

Replace `src-tauri/tests/orchestrator_test.rs` in full:

```rust
use std::fs;
use std::path::Path;

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::{Orchestrator, OrchestratorError};
use claude_pipeline_wizard_lib::engine::run_record::{RunRecordStore, RunStatus, StageStatus};
use claude_pipeline_wizard_lib::template::store::TemplateStore;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage, Template};

fn fixture_prompt(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    format!("FIXTURE:{}", path.to_string_lossy())
}

fn two_stage_template() -> Template {
    Template {
        id: "two-stage".to_string(),
        name: "Two Stage".to_string(),
        description: "desc".to_string(),
        stages: vec![
            Stage {
                id: "stage1".to_string(),
                name: "Stage 1".to_string(),
                prompt: fixture_prompt("orchestrator_stage1.jsonl"),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            },
            Stage {
                id: "stage2".to_string(),
                name: "Stage 2".to_string(),
                prompt: fixture_prompt("orchestrator_stage2.jsonl"),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: false,
            },
        ],
    }
}

fn setup() -> (Orchestrator, tempfile::TempDir, tempfile::TempDir, tempfile::TempDir) {
    let template_dir = tempfile::tempdir().unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let template_store = TemplateStore::new(template_dir.path());
    template_store.save(&two_stage_template()).unwrap();
    let orchestrator = Orchestrator::new(
        template_store,
        RunRecordStore::new(run_dir.path()),
        ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string() },
    );
    (orchestrator, template_dir, run_dir, target_dir)
}

#[test]
fn start_run_creates_record_awaiting_first_stage_without_executing() {
    let (orchestrator, _t, _r, target_dir) = setup();
    let record = orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string())
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[0].session_id, None);

    let manifest_dir = target_dir.path().join(".claude-pipeline-wizard");
    assert!(manifest_dir.join("run.json").exists());
    assert!(manifest_dir.join("pipeline.json").exists());
}

#[tokio::test]
async fn start_stage_without_override_runs_current_stage_and_pauses_at_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();

    let mut events = Vec::new();
    let record = orchestrator
        .start_stage("run1", None, |stage_id, event| events.push((stage_id.to_string(), event)))
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingCheckpoint);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1".to_string()));
    assert!(events.iter().any(|(id, _)| id == "stage1"));
    assert_eq!(record.resolved_stages[0].prompt, fixture_prompt("orchestrator_stage1.jsonl"));
}

#[tokio::test]
async fn start_stage_with_override_replaces_resolved_stage_prompt_before_running() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();

    let mut edited = orchestrator.template_store.load("two-stage").unwrap().stages[0].clone();
    edited.prompt = fixture_prompt("orchestrator_stage1_edited.jsonl");

    let record = orchestrator.start_stage("run1", Some(edited), |_, _| {}).await.unwrap();

    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-edited".to_string()));
    assert_eq!(record.resolved_stages[0].prompt, fixture_prompt("orchestrator_stage1_edited.jsonl"));

    // Run-scoped only: the saved gallery template is untouched.
    let saved_template = orchestrator.template_store.load("two-stage").unwrap();
    assert_eq!(saved_template.stages[0].prompt, fixture_prompt("orchestrator_stage1.jsonl"));

    let pipeline_json_path = target_dir.path().join(".claude-pipeline-wizard/pipeline.json");
    let snapshot: Template = serde_json::from_str(&fs::read_to_string(pipeline_json_path).unwrap()).unwrap();
    assert_eq!(snapshot.stages[0].prompt, fixture_prompt("orchestrator_stage1_edited.jsonl"));
}

#[tokio::test]
async fn start_stage_rejects_override_with_mismatched_id() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();

    let mut wrong = orchestrator.template_store.load("two-stage").unwrap().stages[0].clone();
    wrong.id = "not-stage1".to_string();

    let result = orchestrator.start_stage("run1", Some(wrong), |_, _| {}).await;
    assert!(matches!(result, Err(OrchestratorError::StageIdMismatch(id)) if id == "not-stage1"));
}

#[tokio::test]
async fn approve_checkpoint_advances_to_awaiting_start_without_running_next_stage() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let record = orchestrator.approve_checkpoint("run1").unwrap();

    assert_eq!(record.status, RunStatus::AwaitingStageStart);
    assert_eq!(record.current_stage_index, 1);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::AwaitingStart);
    assert_eq!(record.stages[1].session_id, None);
}

#[tokio::test]
async fn full_two_stage_run_completes_after_starting_each_stage_explicitly() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();
    orchestrator.approve_checkpoint("run1").unwrap();

    let mut events = Vec::new();
    let record = orchestrator
        .start_stage("run1", None, |stage_id, event| events.push((stage_id.to_string(), event)))
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::Approved);
    assert!(events.iter().any(|(id, _)| id == "stage2"));
}

#[tokio::test]
async fn request_changes_reruns_current_stage_using_resolved_prompt_override() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let record = orchestrator
        .request_changes("run1", &fixture_prompt("orchestrator_feedback.jsonl"), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-revised".to_string()));
    // The stage's canonical (resolved) prompt is unchanged by feedback.
    assert_eq!(record.resolved_stages[0].prompt, fixture_prompt("orchestrator_stage1.jsonl"));
}

#[tokio::test]
async fn reject_checkpoint_cancels_run() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap();

    let record = orchestrator.reject_checkpoint("run1").unwrap();
    assert_eq!(record.status, RunStatus::Cancelled);
}

#[tokio::test]
async fn start_stage_when_not_awaiting_stage_start_errors() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator.start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string()).unwrap();
    orchestrator.start_stage("run1", None, |_, _| {}).await.unwrap(); // now AwaitingCheckpoint

    let result = orchestrator.start_stage("run1", None, |_, _| {}).await;
    assert!(matches!(result, Err(OrchestratorError::NotAwaitingStageStart(id)) if id == "run1"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --test orchestrator_test`
Expected: FAIL — `todo!()` panics. Every method (including `reject_checkpoint`, whose own body wasn't stubbed) routes its persistence through the still-stubbed `save()` helper, so every test panics there.

- [ ] **Step 4: Implement `save`, `start_run`, `start_stage`, `approve_checkpoint`, `request_changes`**

```rust
    fn save(&self, template: &Template, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.run_store.save(record)?;
        write_project_manifest(
            Path::new(&record.target_dir),
            &template.id,
            &template.name,
            &template.description,
            record,
        )?;
        Ok(())
    }

    pub fn start_run(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
    ) -> Result<RunRecord, OrchestratorError> {
        let template = self.template_store.load(template_id)?;
        std::fs::create_dir_all(&target_dir)?;
        let record = RunRecord::new(
            run_id,
            template.id.clone(),
            target_dir.to_string_lossy().to_string(),
            &template.stages,
        );
        self.save(&template, &record)?;
        Ok(record)
    }
```

```rust
    pub async fn start_stage<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        stage_override: Option<Stage>,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingStageStart {
            return Err(OrchestratorError::NotAwaitingStageStart(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;

        if let Some(stage) = stage_override {
            if stage.id != record.current_resolved_stage().id {
                return Err(OrchestratorError::StageIdMismatch(stage.id));
            }
            validate_stage(&stage)?;
            record.apply_stage_override(stage);
        }

        record.begin_current_stage();
        self.save(&template, &record)?;

        let stage_index = record.current_stage_index;
        let stage = record.current_resolved_stage().clone();
        let stage_id = stage.id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());
        let resume_session_id = if stage_index == 0 {
            None
        } else {
            record.stages[stage_index - 1].session_id.clone()
        };

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let exit_code = run_stage(&self.executor_config, &stage, &target_dir, resume_session_id.as_deref(), |event| {
            if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                latest_session_id = Some(session_id.clone());
            }
            collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
            on_event(&stage_id, event);
        })
        .await?;

        record.stages[stage_index].log.extend(collected_log);

        if exit_code != 0 {
            record.mark_current_failed();
        } else {
            let session_id = latest_session_id.unwrap_or_default();
            if stage.checkpoint {
                record.mark_current_awaiting_checkpoint(session_id);
            } else {
                record.current_stage_mut().session_id = Some(session_id);
                record.approve_current();
            }
        }

        self.save(&template, &record)?;
        Ok(record)
    }
```

```rust
    pub fn approve_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        record.approve_current();
        self.save(&template, &record)?;
        Ok(record)
    }
```

```rust
    pub async fn request_changes<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        feedback: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        let stage_index = record.current_stage_index;
        let mut feedback_stage = record.resolved_stages[stage_index].clone();
        feedback_stage.prompt = feedback.to_string();
        let stage_id = feedback_stage.id.clone();

        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());

        record.status = RunStatus::Running;
        record.current_stage_mut().status = StageStatus::Running;
        self.save(&template, &record)?;

        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let result = run_stage(&self.executor_config, &feedback_stage, &target_dir, resume_session_id.as_deref(), |event| {
            if let StageEvent::Init { session_id } | StageEvent::Result { session_id, .. } = &event {
                latest_session_id = Some(session_id.clone());
            }
            collected_log.push(serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
            on_event(&stage_id, event);
        })
        .await;

        record.stages[stage_index].log.extend(collected_log);

        match result {
            Ok(0) => record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default()),
            _ => record.mark_current_failed(),
        }
        self.save(&template, &record)?;
        Ok(record)
    }
```

Also update `reject_checkpoint` to use the new `save` helper (replace its `self.run_store.save(&record)?;` line with `self.save(&template, &record)?;`, loading `template` the same way the other methods do) so a cancelled run's project manifest reflects the cancellation too:

```rust
    pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        record.cancel();
        self.save(&template, &record)?;
        Ok(record)
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --test orchestrator_test`
Expected: PASS (9 tests).

Run the full backend suite to confirm nothing else regressed: `cd src-tauri && cargo test`
Expected: PASS (all unit + integration tests across `template::`, `engine::`, `cli_check`, `executor_test`, `orchestrator_test`).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(orchestrator): add pre-stage-start gate, remove auto-advance for non-checkpoint stages"
```

---

## Task 5: Tauri Commands & App Wiring

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `Orchestrator::start_run` (now sync, no `on_event`), `Orchestrator::start_stage` (new), `Orchestrator::approve_checkpoint` (now sync, no `on_event`) from Task 4; `Stage` from `crate::template`.
- Produces: Tauri command `start_stage(run_id: String, stage_override: Option<Stage>) -> Result<RunRecord, String>` (new, emits `pipeline://stage-event` same as `start_pipeline_run`/`approve_checkpoint` did before). `start_pipeline_run` and `approve_checkpoint` commands keep their existing names/params but no longer emit events (they do no execution).

- [ ] **Step 1: Update `commands.rs`**

In `src-tauri/src/commands.rs`, change the import line to include `Stage`:

```rust
use crate::template::{Stage, Template};
```

Replace the `start_pipeline_run` command:

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

Add the new `start_stage` command directly after it:

```rust
#[tauri::command]
pub async fn start_stage(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
    stage_override: Option<Stage>,
) -> Result<RunRecord, String> {
    orchestrator
        .start_stage(&run_id, stage_override, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}
```

Replace the `approve_checkpoint` command:

```rust
#[tauri::command]
pub fn approve_checkpoint(orchestrator: State<Orchestrator>, run_id: String) -> Result<RunRecord, String> {
    orchestrator.approve_checkpoint(&run_id).map_err(|e| e.to_string())
}
```

`request_changes` and `reject_checkpoint` are unchanged.

- [ ] **Step 2: Register the new command in `lib.rs`**

In `src-tauri/src/lib.rs`, add `commands::start_stage,` to the `tauri::generate_handler![...]` list, right after `commands::start_pipeline_run,`:

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
        ])
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors. (There is no dedicated unit test for `commands.rs`, matching the existing codebase — it is thin glue over the already-tested `Orchestrator`; it's exercised indirectly by the frontend Vitest suite's mocked `invoke` calls in Tasks 6 and 8, and by the manual smoke test in Task 9.)

Run the full backend suite once more to be sure the command-layer signature changes didn't break anything: `cd src-tauri && cargo test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(commands): add start_stage command, stop auto-executing on run start/checkpoint approval"
```

---

## Task 6: Frontend Types & API Wrapper

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api.ts`
- Modify: `src/api.test.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces: `StageStatus` gains `"awaiting-start"`; `RunStatus` gains `"awaiting-stage-start"`; `RunRecord` gains `resolvedStages: Stage[]`. `api.ts` gains `export function startStage(runId: string, stageOverride?: Stage): Promise<RunRecord>`.

- [ ] **Step 1: Update `types.ts`**

```typescript
export type StageStatus = "pending" | "awaiting-start" | "running" | "awaiting-checkpoint" | "approved" | "failed";
export type RunStatus = "awaiting-stage-start" | "running" | "awaiting-checkpoint" | "completed" | "failed" | "cancelled";
```

Add `resolvedStages: Stage[];` to the `RunRecord` interface:

```typescript
export interface RunRecord {
  runId: string;
  templateId: string;
  targetDir: string;
  status: RunStatus;
  currentStageIndex: number;
  stages: StageRun[];
  resolvedStages: Stage[];
}
```

- [ ] **Step 2: Write the failing test for `startStage`**

Add to `src/api.test.ts`, inside the `import { ... } from "./api"` list, `startStage`:

```typescript
import {
  listTemplates,
  loadTemplate,
  saveTemplate,
  deleteTemplate,
  checkCli,
  startPipelineRun,
  startStage,
  approveCheckpoint,
  requestChanges,
  rejectCheckpoint,
  onStageEvent,
} from "./api";
import type { Stage, Template } from "./types";
```

Add the test (after the `startPipelineRun` test):

```typescript
  it("startStage invokes start_stage with runId and an optional stage override", async () => {
    invokeMock.mockResolvedValue({});
    const stage: Stage = {
      id: "s1",
      name: "Stage 1",
      prompt: "edited prompt",
      permissionMode: "acceptEdits",
      allowedTools: [],
      checkpoint: true,
    };
    await startStage("run1", stage);
    expect(invokeMock).toHaveBeenCalledWith("start_stage", { runId: "run1", stageOverride: stage });

    await startStage("run1");
    expect(invokeMock).toHaveBeenCalledWith("start_stage", { runId: "run1", stageOverride: null });
  });
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `npm run test -- api.test`
Expected: FAIL — `startStage` is not exported from `./api`.

- [ ] **Step 4: Implement `startStage` in `api.ts`**

Add `Stage` to the `types` import and add the function after `startPipelineRun`:

```typescript
import type { RunRecord, Stage, StageEventPayload, Template } from "./types";
```

```typescript
export function startStage(runId: string, stageOverride?: Stage): Promise<RunRecord> {
  return invoke("start_stage", { runId, stageOverride: stageOverride ?? null });
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `npm run test -- api.test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(frontend): add awaiting-stage-start types and startStage API wrapper"
```

---

## Task 7: Extract Shared `StageFields` Component

**Files:**
- Create: `src/components/StageFields.tsx`
- Create: `src/components/StageFields.test.tsx`
- Modify: `src/pages/TemplateEditor.tsx`

**Interfaces:**
- Consumes: `Stage`, `PermissionMode` (Task 6's `types.ts`, unchanged shape).
- Produces: `export default function StageFields({ stage, onChange, idPrefix, promptLabel, lockId }: Props)` — renders the name/id/prompt/permission-mode/allowed-tools/checkpoint fields shared by the Template Editor's stage panel and the new pre-stage edit panel (Task 8). `onChange: (patch: Partial<Stage>) => void`. `lockId?: boolean` (default `false`) disables the id input without hiding it. Exports `PERMISSION_MODES` and `KNOWN_TOOLS` constants (moved from `TemplateEditor.tsx`) so both call sites share one source of truth.

- [ ] **Step 1: Write the failing test**

`src/components/StageFields.test.tsx`:

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

describe("StageFields", () => {
  it("renders the prompt with the given label and current value", () => {
    render(<StageFields stage={stage} onChange={vi.fn()} idPrefix="stage-0" promptLabel="단계 1 프롬프트" />);
    expect(screen.getByLabelText("단계 1 프롬프트")).toHaveValue("PRD 작성");
  });

  it("calls onChange with the edited prompt", () => {
    const onChange = vi.fn();
    render(<StageFields stage={stage} onChange={onChange} idPrefix="stage-0" promptLabel="프롬프트" />);
    fireEvent.change(screen.getByLabelText("프롬프트"), { target: { value: "새 프롬프트" } });
    expect(onChange).toHaveBeenCalledWith({ prompt: "새 프롬프트" });
  });

  it("toggles an allowed tool chip", () => {
    const onChange = vi.fn();
    render(<StageFields stage={stage} onChange={onChange} idPrefix="stage-0" promptLabel="프롬프트" />);
    fireEvent.click(screen.getByRole("button", { name: "Write" }));
    expect(onChange).toHaveBeenCalledWith({ allowedTools: ["Read", "Write"] });
  });

  it("disables the id input when lockId is set", () => {
    render(<StageFields stage={stage} onChange={vi.fn()} idPrefix="stage-0" promptLabel="프롬프트" lockId />);
    expect(screen.getByLabelText("단계 ID")).toBeDisabled();
  });

  it("leaves the id input enabled by default", () => {
    render(<StageFields stage={stage} onChange={vi.fn()} idPrefix="stage-0" promptLabel="프롬프트" />);
    expect(screen.getByLabelText("단계 ID")).not.toBeDisabled();
  });
});
```

Create a minimal failing `src/components/StageFields.tsx` (empty shell so the test file compiles and fails on assertions, not on a missing module):

```tsx
import type { Stage } from "../types";

interface Props {
  stage: Stage;
  onChange: (patch: Partial<Stage>) => void;
  idPrefix: string;
  promptLabel: string;
  lockId?: boolean;
}

export default function StageFields(_props: Props) {
  return null;
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test -- StageFields`
Expected: FAIL — none of the fields render.

- [ ] **Step 3: Implement `StageFields`**

```tsx
import type { PermissionMode, Stage } from "../types";

export const PERMISSION_MODES: PermissionMode[] = ["acceptEdits", "bypassPermissions", "default"];
export const KNOWN_TOOLS = ["Read", "Write", "Edit", "Glob", "Grep", "Bash", "WebSearch", "WebFetch"];

interface Props {
  stage: Stage;
  onChange: (patch: Partial<Stage>) => void;
  idPrefix: string;
  promptLabel: string;
  lockId?: boolean;
}

export default function StageFields({ stage, onChange, idPrefix, promptLabel, lockId = false }: Props) {
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
                onClick={() => {
                  const allowedTools = selected
                    ? stage.allowedTools.filter((t) => t !== tool)
                    : [...stage.allowedTools, tool];
                  onChange({ allowedTools });
                }}
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

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test -- StageFields`
Expected: PASS (5 tests).

- [ ] **Step 5: Refactor `TemplateEditor.tsx` to use `StageFields`**

Remove the module-level `PERMISSION_MODES` and `KNOWN_TOOLS` constants from `src/pages/TemplateEditor.tsx` (now owned by `StageFields.tsx`) and add the import:

```typescript
import StageFields from "../components/StageFields";
```

Replace the whole inline fields block — everything from `<div style={{ display: "flex", gap: 12, marginBottom: 12 }}>` (the name/id row) through the closing `</div>` of the `switch-row` (i.e. everything between the "단계 편집 · {name}" header and the move-up/move-down button row) — with:

```tsx
              <StageFields
                stage={selectedStage}
                onChange={(patch) => updateStage(selectedIndex, patch)}
                idPrefix={`stage-${selectedIndex}`}
                promptLabel={`단계 ${selectedIndex + 1} 프롬프트`}
              />
```

- [ ] **Step 6: Run the full frontend test suite to verify no regression**

Run: `npm run test`
Expected: PASS — `TemplateEditor.test.tsx` (which asserts on `단계 1 프롬프트`, `템플릿 이름`, etc.) must pass unchanged, since `StageFields` reproduces the exact same ids/labels/markup.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(frontend): extract StageFields component out of TemplateEditor"
```

---

## Task 8: Pre-Stage Edit Panel in `PipelineRun`

**Files:**
- Modify: `src/pages/PipelineRun.tsx`
- Modify: `src/pages/PipelineRun.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

**Interfaces:**
- Consumes: `StageFields` (Task 7); `startStage` (Task 6); `RunRecord.resolvedStages` (Task 6).
- Produces: `PipelineRun` renders an editable stage-fields panel with a **이 단계 실행** button whenever `run.status === "awaiting-stage-start"`, calling `startStage(run.runId, editedStage)`.

- [ ] **Step 1: Write the failing tests**

In `src/pages/PipelineRun.test.tsx`, add `startStage` to the `vi.mock("../api", ...)` factory and import list:

```tsx
vi.mock("../api", () => ({
  onStageEvent: vi.fn(),
  approveCheckpoint: vi.fn(),
  requestChanges: vi.fn(),
  rejectCheckpoint: vi.fn(),
  startStage: vi.fn(),
}));

import { onStageEvent, approveCheckpoint, requestChanges, rejectCheckpoint, startStage } from "../api";
```

Add `resolvedStages` to the existing `runningRun` fixture (required by the updated `RunRecord` type):

```tsx
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
  resolvedStages: [
    { id: "s1", name: "요구사항 정리", prompt: "p1", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
    { id: "s2", name: "구현", prompt: "p2", permissionMode: "acceptEdits", allowedTools: [], checkpoint: true },
  ],
};

const pendingRun: RunRecord = {
  ...runningRun,
  status: "awaiting-stage-start",
  stages: [
    { id: "s1", status: "awaiting-start", sessionId: null, log: [] },
    { id: "s2", status: "pending", sessionId: null, log: [] },
  ],
};
```

Reset the new mock in `beforeEach`:

```tsx
beforeEach(() => {
  vi.mocked(onStageEvent).mockReset();
  vi.mocked(onStageEvent).mockResolvedValue(() => {});
  vi.mocked(approveCheckpoint).mockReset();
  vi.mocked(requestChanges).mockReset();
  vi.mocked(rejectCheckpoint).mockReset();
  vi.mocked(startStage).mockReset();
});
```

Add these tests at the end of the `describe("PipelineRun", ...)` block:

```tsx
  it("shows the pre-stage edit panel prefilled with the current stage's prompt when awaiting stage start", () => {
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.getByRole("button", { name: "이 단계 실행" })).toBeInTheDocument();
    expect(screen.getByLabelText("프롬프트")).toHaveValue("p1");
  });

  it("sends the edited stage via startStage when running the stage", async () => {
    vi.mocked(startStage).mockResolvedValue({ ...pendingRun, status: "awaiting-checkpoint" });
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("프롬프트"), { target: { value: "수정된 프롬프트" } });
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    await waitFor(() =>
      expect(startStage).toHaveBeenCalledWith("run1", expect.objectContaining({ id: "s1", prompt: "수정된 프롬프트" }))
    );
  });

  it("runs the stage unedited when the user clicks run without changing anything", async () => {
    vi.mocked(startStage).mockResolvedValue({ ...pendingRun, status: "awaiting-checkpoint" });
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    await waitFor(() =>
      expect(startStage).toHaveBeenCalledWith("run1", expect.objectContaining({ id: "s1", prompt: "p1" }))
    );
  });

  it("shows an error when startStage rejects", async () => {
    vi.mocked(startStage).mockRejectedValue(new Error("claude CLI not found"));
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "이 단계 실행" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("claude CLI not found");
  });

  it("does not show the checkpoint approval panel while awaiting stage start", () => {
    render(<PipelineRun initialRun={pendingRun} template={sampleTemplate} onFinished={vi.fn()} onEditTemplate={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "승인" })).not.toBeInTheDocument();
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm run test -- PipelineRun`
Expected: FAIL — `types.ts` requires `resolvedStages` (fixtures now provide it, so that part compiles), but there is no "이 단계 실행" button yet, and `startStage` is never called.

- [ ] **Step 3: Implement the pre-stage edit panel in `PipelineRun.tsx`**

Add imports:

```tsx
import { approveCheckpoint, onStageEvent, rejectCheckpoint, requestChanges, startStage } from "../api";
import type { RunRecord, Stage, StageEventPayload, StageStatus, Template } from "../types";
import StageFields from "../components/StageFields";
```

Add state for the draft stage and keep it in sync with the paused stage:

```tsx
  const [stageDraft, setStageDraft] = useState<Stage>(run.resolvedStages[run.currentStageIndex]);

  useEffect(() => {
    if (run.status === "awaiting-stage-start") {
      setStageDraft(run.resolvedStages[run.currentStageIndex]);
    }
  }, [run.status, run.currentStageIndex, run.resolvedStages]);
```

Add the handler, next to `handleApprove`/`handleRequestChanges`/`handleReject`:

```tsx
  const handleStartStage = async () => {
    setError(null);
    setBusy(true);
    try {
      const updated = await startStage(run.runId, stageDraft);
      setRun(updated);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
```

Add the panel markup, directly above the existing `{run.status === "awaiting-checkpoint" && ( ... )}` block:

```tsx
        {run.status === "awaiting-stage-start" && (
          <div className="card">
            <div className="section-label">다음 단계 — 실행 전 확인/수정</div>
            <StageFields
              stage={stageDraft}
              onChange={(patch) => setStageDraft((prev) => ({ ...prev, ...patch }))}
              idPrefix="pending-stage"
              promptLabel="프롬프트"
              lockId
            />
            <div style={{ display: "flex", marginTop: 12 }}>
              <button className="btn btn-success" onClick={handleStartStage} disabled={busy}>
                이 단계 실행
              </button>
            </div>
          </div>
        )}
```

- [ ] **Step 4: Update `src/App.tsx`'s optimistic pending record**

`handleRun`'s optimistic `pendingRun` (shown while `startPipelineRun` is in flight) must match the new initial shape returned by the backend, otherwise the live view briefly shows a stale `status: "running"` that the real record will never actually pass through first. Replace:

```tsx
      const pendingRun: RunRecord = {
        runId,
        templateId: template.id,
        targetDir,
        status: "running",
        currentStageIndex: 0,
        stages: template.stages.map((s) => ({ id: s.id, status: "pending", sessionId: null, log: [] })),
      };
```

with:

```tsx
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
        resolvedStages: template.stages,
      };
```

- [ ] **Step 5: Update `src/App.test.tsx`**

Add `resolvedStages` to the `runRecord` fixture in the "opens the editor from 편집..." test:

```tsx
    const runRecord: RunRecord = {
      runId: "run1",
      templateId: "t1",
      targetDir: "/tmp/new-project",
      status: "awaiting-checkpoint",
      currentStageIndex: 0,
      stages: [{ id: "s1", status: "awaiting-checkpoint", sessionId: "sess1", log: [] }],
      resolvedStages: sample.stages,
    };
```

Update the assertion that checked the optimistic record's status:

```tsx
    // the run view mounts with a pending record before startPipelineRun resolves,
    // so the stage-event listener is live for the entire first stage
    expect(await screen.findByText("상태: awaiting-stage-start")).toBeInTheDocument();
```

- [ ] **Step 6: Run the full frontend suite**

Run: `npm run test`
Expected: PASS — all of `PipelineRun.test.tsx`, `App.test.tsx`, `TemplateEditor.test.tsx`, `TemplateGallery.test.tsx`, `StageFields.test.tsx`, `api.test.ts`.

Run: `npx tsc --noEmit`
Expected: no type errors.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(frontend): add pre-stage edit panel, wire startStage into the run view"
```

---

## Task 9: Documentation & Manual Smoke Test

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: nothing (docs-only task).
- Produces: an accurate description of the new pre-stage edit gate and the project-local manifest, so a new contributor isn't misled by the old "checkpoint-only pause" description.

- [ ] **Step 1: Update the "How it works" section**

In `README.md`, replace step 2-3 of "How it works" with:

```markdown
2. Every stage pauses *before* it runs so you can review or edit its prompt,
   permission mode, allowed tools, or checkpoint toggle for this run only —
   click **이 단계 실행** to run it as shown (or as edited).
3. Each stage runs as its own `claude --print --output-format stream-json` invocation,
   with `--resume <session_id>` chaining conversation context from one stage to the next
   so later stages know what earlier stages built.
4. When a stage finishes, if it's marked as a checkpoint, the run pauses again and shows a
   live log plus **Approve / Request changes / Reject** controls before moving on to the
   next stage's pre-run edit pause.
5. Approving the last stage's checkpoint (or a non-checkpoint last stage finishing) completes the run.
```

Renumber the following paragraph accordingly and update the "plain JSON files" paragraph to mention the project-local copy:

```markdown
Templates, run records, and the two seed templates ("웹 프로그램 개발" and
"데스크톱 프로그램 개발 (Tauri)") are all plain JSON files — there's no embedded
database and no bundled AI SDK. Run state and the resolved (per-run-edited)
stage definitions are written to both the app's own data directory and to
`.claude-pipeline-wizard/{run.json,pipeline.json}` inside the run's target
project folder, so a run is inspectable — and its `pipeline.json` hand-editable
— from the project folder itself.
```

- [ ] **Step 2: Update "Project layout"**

Add the new files to the `src-tauri/src/engine/` and `src/` listings:

```markdown
src/                      React frontend
  pages/
    TemplateGallery.tsx   Browse, run, edit, delete templates
    TemplateEditor.tsx    Form-based stage editor (add/remove/reorder, validation)
    PipelineRun.tsx       Pre-stage edit panel, live stage log, checkpoint approval panel
  components/
    StageFields.tsx       Shared prompt/permission/tools/checkpoint fields (editor + run view)
  api.ts                  Tauri invoke()/listen() wrappers
  types.ts                TypeScript mirrors of the Rust IPC types

src-tauri/src/
  template/               Template/Stage types, file-backed CRUD store, seed templates
  engine/
    run_record.rs         Run state machine (RunRecord/StageRun, pre-stage-start + checkpoint transitions)
    project_manifest.rs   Writes run.json/pipeline.json into the target project folder
    stream_json.rs        Parser for the claude CLI's stream-json output
    executor.rs            Spawns the claude CLI per stage, streams events, enforces a timeout
    orchestrator.rs        Drives the pre-stage-start + checkpoint state machine end to end
  commands.rs             Tauri commands exposed to the frontend
  bin/mock_claude.rs      Test-only stand-in for the claude CLI (used by the Rust test suite)
```

- [ ] **Step 3: Update "Known v1 limitations"**

Add a note clarifying what the project-local manifest does and does not solve:

```markdown
- **Project-local manifest eases inspection, not resume** — `.claude-pipeline-wizard/run.json`
  and `pipeline.json` inside the target folder make a paused/failed run's state and
  actually-executing stage definitions inspectable (and `pipeline.json` hand-editable)
  without digging through the app's data directory, but there is still no in-app command
  or screen to reattach to an existing run after an app restart (see the point below).
```

Keep the existing "No cross-restart resume UI" bullet as-is (still true).

- [ ] **Step 4: Manual smoke test**

Run the app against a real target folder with the actual `claude` CLI (same manual check the base plan calls for), confirming:
- Starting a run lands on the "다음 단계 — 실행 전 확인/수정" panel for stage 1 *before* anything executes.
- Editing the prompt and clicking **이 단계 실행** runs the edited prompt (visible in the live log).
- `<targetDir>/.claude-pipeline-wizard/run.json` and `pipeline.json` exist and update after each transition.
- A non-checkpoint stage, after finishing, returns to the pre-stage edit panel for the next stage rather than auto-running it.
- Approving a checkpoint also returns to the pre-stage edit panel for the next stage rather than auto-running it.

Run: `npx tauri dev`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: describe the pre-stage edit gate and project-local manifest"
```
