# Claude Pipeline Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri desktop app that runs template-defined, multi-stage Claude Code CLI pipelines against new project folders, pausing at per-stage checkpoints for user approval.

**Architecture:** Rust core (Tauri v2) owns template storage, run-record persistence, and a stage-execution engine that shells out to the `claude` CLI (`--print --output-format stream-json`, `--resume <session_id>` for continuity). A React/TypeScript frontend renders a template gallery, a form-based template editor, and a pipeline run view with a checkpoint approval panel, communicating with the Rust core via Tauri commands and events.

**Tech Stack:** Tauri v2, Rust (tokio, serde, thiserror, uuid), React 18 + TypeScript + Vite, Vitest + React Testing Library.

**Spec:** `docs/superpowers/specs/2026-09-06-claude-pipeline-wizard-design.md`

## Global Constraints

- No embedded Claude Agent SDK / Node.js runtime — the app shells out to the `claude` CLI directly.
- v1 targets newly created folders only — no pipelines against existing codebases.
- Strictly sequential stage execution — no parallel or branching stages.
- No database — templates and run records are flat JSON files on disk.
- The app depends on the Claude Code CLI already being installed and configured on the user's machine.

---

## Task 1: Project Scaffold

**Files:**
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `.gitignore`
- Create: `src/main.tsx`, `src/App.tsx`, `src/test-setup.ts`, `src/App.test.tsx`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: a buildable Tauri v2 + React/TS skeleton that later tasks extend. `App` component (default export from `src/App.tsx`). Rust lib crate `claude_pipeline_wizard_lib` with a public `run()` function.

- [ ] **Step 1: Create frontend config files**

`package.json`:
```json
{
  "name": "claude-pipeline-wizard",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "@tauri-apps/api": "^2.0.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@types/react": "^18.3.1",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.3",
    "vite": "^5.4.1",
    "vitest": "^2.0.5",
    "jsdom": "^25.0.0",
    "@testing-library/react": "^16.0.0",
    "@testing-library/jest-dom": "^6.5.0"
  }
}
```

`tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2021",
    "useDefineForClassFields": true,
    "lib": ["ES2021", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true
  },
  "include": ["src"]
}
```

`vite.config.ts`:
```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
});
```

`vitest.config.ts`:
```typescript
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
});
```

`src/test-setup.ts`:
```typescript
import "@testing-library/jest-dom";
```

`index.html`:
```html
<!doctype html>
<html lang="ko">
  <head>
    <meta charset="UTF-8" />
    <title>Claude Pipeline Wizard</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`.gitignore`:
```
node_modules/
dist/
src-tauri/target/
```

`src/main.tsx`:
```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

`src/App.tsx` (temporary empty shell — Step 2 below TDDs the real content, Task 16 replaces it with full routing):
```typescript
export default function App() {
  return <div></div>;
}
```

- [ ] **Step 2: Write a failing test for the app shell**

`src/App.test.tsx`:
```typescript
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";

describe("App", () => {
  it("renders the app shell heading", () => {
    render(<App />);
    expect(screen.getByText("Claude Pipeline Wizard")).toBeInTheDocument();
  });
});
```

Run: `npm install && npm run test`
Expected: FAIL (text not found).

- [ ] **Step 3: Make the test pass**

Update `src/App.tsx`:
```typescript
export default function App() {
  return <div>Claude Pipeline Wizard</div>;
}
```

Run: `npm run test`
Expected: PASS.

- [ ] **Step 4: Create the Rust scaffold**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "claude-pipeline-wizard"
version = "0.1.0"
edition = "2021"

[lib]
name = "claude_pipeline_wizard_lib"
path = "src/lib.rs"

[[bin]]
name = "claude-pipeline-wizard"
path = "src/main.rs"

[[bin]]
name = "mock_claude"
path = "src/bin/mock_claude.rs"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
thiserror = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "process", "io-util", "fs"] }

[dev-dependencies]
tempfile = "3"
```

`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "claude-pipeline-wizard",
  "version": "0.1.0",
  "identifier": "com.suhwanju.claude-pipeline-wizard",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Claude Pipeline Wizard",
        "width": 1100,
        "height": 750
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the main window",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

fn main() {
    claude_pipeline_wizard_lib::run();
}
```

`src-tauri/src/lib.rs`:
```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Placeholder for the mock binary (fleshed out in Task 6) — needed now only so the `[[bin]]` target compiles:

`src-tauri/src/bin/mock_claude.rs`:
```rust
fn main() {}
```

- [ ] **Step 5: Verify the Rust scaffold compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 6: Verify the frontend builds**

Run: `npm run build`
Expected: succeeds, produces `dist/`.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "chore: scaffold Tauri v2 + React/TS project"
```

---

## Task 2: Template Domain Types & Validation

**Files:**
- Create: `src-tauri/src/template/mod.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod template;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct Template { id, name, description, stages: Vec<Stage> }`, `pub struct Stage { id, name, prompt, permission_mode: PermissionMode, allowed_tools: Vec<String>, checkpoint: bool }`, `pub enum PermissionMode { AcceptEdits, BypassPermissions, Default }`, `pub enum TemplateValidationError`, `pub fn validate_template(&Template) -> Result<(), TemplateValidationError>`. All types `Serialize + Deserialize + Clone + Debug + PartialEq`, serialized as camelCase JSON matching the spec's schema.

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/template/mod.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    AcceptEdits,
    BypassPermissions,
    Default,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub permission_mode: PermissionMode,
    pub allowed_tools: Vec<String>,
    pub checkpoint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stages: Vec<Stage>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TemplateValidationError {
    #[error("template must have at least one stage")]
    NoStages,
    #[error("stage '{0}' has an empty prompt")]
    EmptyPrompt(String),
    #[error("duplicate stage id '{0}'")]
    DuplicateStageId(String),
}

pub fn validate_template(template: &Template) -> Result<(), TemplateValidationError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(id: &str, prompt: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: id.to_string(),
            prompt: prompt.to_string(),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec!["Read".to_string()],
            checkpoint: true,
        }
    }

    fn template(stages: Vec<Stage>) -> Template {
        Template {
            id: "t1".to_string(),
            name: "Test".to_string(),
            description: "desc".to_string(),
            stages,
        }
    }

    #[test]
    fn rejects_empty_stage_list() {
        assert_eq!(validate_template(&template(vec![])), Err(TemplateValidationError::NoStages));
    }

    #[test]
    fn rejects_empty_prompt() {
        let t = template(vec![stage("s1", "   ")]);
        assert_eq!(validate_template(&t), Err(TemplateValidationError::EmptyPrompt("s1".to_string())));
    }

    #[test]
    fn rejects_duplicate_stage_ids() {
        let t = template(vec![stage("s1", "do it"), stage("s1", "do it again")]);
        assert_eq!(validate_template(&t), Err(TemplateValidationError::DuplicateStageId("s1".to_string())));
    }

    #[test]
    fn accepts_valid_template() {
        let t = template(vec![stage("s1", "do it"), stage("s2", "do more")]);
        assert_eq!(validate_template(&t), Ok(()));
    }

    #[test]
    fn serializes_permission_mode_as_camel_case() {
        let json = serde_json::to_string(&PermissionMode::AcceptEdits).unwrap();
        assert_eq!(json, "\"acceptEdits\"");
    }
}
```

Add `pub mod template;` to `src-tauri/src/lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test template::`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement `validate_template`**

Replace the `todo!()` body:
```rust
pub fn validate_template(template: &Template) -> Result<(), TemplateValidationError> {
    if template.stages.is_empty() {
        return Err(TemplateValidationError::NoStages);
    }
    let mut seen = std::collections::HashSet::new();
    for stage in &template.stages {
        if stage.prompt.trim().is_empty() {
            return Err(TemplateValidationError::EmptyPrompt(stage.id.clone()));
        }
        if !seen.insert(stage.id.clone()) {
            return Err(TemplateValidationError::DuplicateStageId(stage.id.clone()));
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test template::`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add template domain types and validation"
```

---

## Task 3: Template File Store

**Files:**
- Create: `src-tauri/src/template/store.rs`
- Modify: `src-tauri/src/template/mod.rs` (add `pub mod store;`)

**Interfaces:**
- Consumes: `Template`, `validate_template`, `TemplateValidationError` from Task 2.
- Produces: `pub struct TemplateStore { .. }` with `pub fn new(dir: impl Into<PathBuf>) -> Self`, `pub fn list(&self) -> Result<Vec<Template>, StoreError>`, `pub fn load(&self, id: &str) -> Result<Template, StoreError>`, `pub fn save(&self, template: &Template) -> Result<(), StoreError>`, `pub fn delete(&self, id: &str) -> Result<(), StoreError>`. `pub enum StoreError { Io, Json, Validation, NotFound(String) }`.

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/template/store.rs`:
```rust
use std::fs;
use std::path::PathBuf;

use super::{validate_template, Template, TemplateValidationError};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(#[from] TemplateValidationError),
    #[error("template '{0}' not found")]
    NotFound(String),
}

pub struct TemplateStore {
    dir: PathBuf,
}

impl TemplateStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    pub fn list(&self) -> Result<Vec<Template>, StoreError> {
        todo!()
    }

    pub fn load(&self, id: &str) -> Result<Template, StoreError> {
        todo!()
    }

    pub fn save(&self, template: &Template) -> Result<(), StoreError> {
        todo!()
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    fn sample(id: &str) -> Template {
        Template {
            id: id.to_string(),
            name: "Sample".to_string(),
            description: "desc".to_string(),
            stages: vec![Stage {
                id: "s1".to_string(),
                name: "Stage 1".to_string(),
                prompt: "do it".to_string(),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec!["Read".to_string()],
                checkpoint: true,
            }],
        }
    }

    #[test]
    fn list_returns_empty_when_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path().join("does-not-exist"));
        assert_eq!(store.list().unwrap(), Vec::new());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        let template = sample("t1");
        store.save(&template).unwrap();
        assert_eq!(store.load("t1").unwrap(), template);
    }

    #[test]
    fn save_rejects_invalid_template() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        let mut template = sample("t1");
        template.stages.clear();
        assert!(matches!(store.save(&template), Err(StoreError::Validation(_))));
    }

    #[test]
    fn load_missing_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.load("missing"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn list_returns_saved_templates() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        store.save(&sample("a")).unwrap();
        store.save(&sample("b")).unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn delete_removes_template() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        store.save(&sample("t1")).unwrap();
        store.delete("t1").unwrap();
        assert!(matches!(store.load("t1"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn delete_missing_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.delete("missing"), Err(StoreError::NotFound(_))));
    }
}
```

Add `pub mod store;` to `src-tauri/src/template/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test template::store::`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement the store methods**

```rust
    pub fn list(&self) -> Result<Vec<Template>, StoreError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut templates = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                templates.push(serde_json::from_str(&content)?);
            }
        }
        templates.sort_by(|a: &Template, b: &Template| a.name.cmp(&b.name));
        Ok(templates)
    }

    pub fn load(&self, id: &str) -> Result<Template, StoreError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, template: &Template) -> Result<(), StoreError> {
        validate_template(template)?;
        fs::create_dir_all(&self.dir)?;
        let content = serde_json::to_string_pretty(template)?;
        fs::write(self.path_for(&template.id), content)?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test template::store::`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add template file store with CRUD"
```

---

## Task 4: Run Record Model & State Transitions

**Files:**
- Create: `src-tauri/src/engine/mod.rs`, `src-tauri/src/engine/run_record.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod engine;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum RunStatus { Running, AwaitingCheckpoint, Completed, Failed, Cancelled }`, `pub enum StageStatus { Pending, Running, AwaitingCheckpoint, Approved, Failed }` (serialized kebab-case), `pub struct StageRun { id, status, session_id: Option<String>, log: Vec<serde_json::Value> }`, `pub struct RunRecord { run_id, template_id, target_dir, status, current_stage_index: usize, stages: Vec<StageRun> }` with `RunRecord::new(run_id, template_id, target_dir, stage_ids: &[String]) -> Self`, `current_stage_mut(&mut self) -> &mut StageRun`, `mark_current_awaiting_checkpoint(&mut self, session_id: String)`, `mark_current_failed(&mut self)`, `approve_current(&mut self) -> bool` (returns `true` when the run is now complete), `cancel(&mut self)`. Also `pub struct RunRecordStore` with `new`, `save`, `load` mirroring the template store's file-per-record pattern.

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/engine/mod.rs`:
```rust
pub mod run_record;
```

`src-tauri/src/engine/run_record.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
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
}

impl RunRecord {
    pub fn new(run_id: String, template_id: String, target_dir: String, stage_ids: &[String]) -> Self {
        todo!()
    }

    pub fn current_stage_mut(&mut self) -> &mut StageRun {
        &mut self.stages[self.current_stage_index]
    }

    pub fn mark_current_awaiting_checkpoint(&mut self, session_id: String) {
        todo!()
    }

    pub fn mark_current_failed(&mut self) {
        todo!()
    }

    /// Marks the current stage approved and advances to the next stage.
    /// Returns `true` if this was the last stage (run is now complete).
    pub fn approve_current(&mut self) -> bool {
        todo!()
    }

    pub fn cancel(&mut self) {
        todo!()
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
        fs::create_dir_all(&self.dir)?;
        let content = serde_json::to_string_pretty(record)?;
        fs::write(self.path_for(&record.run_id), content)?;
        Ok(())
    }

    pub fn load(&self, run_id: &str) -> Result<RunRecord, RunRecordStoreError> {
        let path = self.path_for(run_id);
        if !path.exists() {
            return Err(RunRecordStoreError::NotFound(run_id.to_string()));
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(stage_ids: &[&str]) -> RunRecord {
        let ids: Vec<String> = stage_ids.iter().map(|s| s.to_string()).collect();
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &ids)
    }

    #[test]
    fn new_starts_at_stage_zero_with_pending_stages() {
        let r = record(&["a", "b"]);
        assert_eq!(r.current_stage_index, 0);
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages.len(), 2);
        assert!(r.stages.iter().all(|s| s.status == StageStatus::Pending));
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
    fn approve_current_advances_to_next_stage() {
        let mut r = record(&["a", "b"]);
        let completed = r.approve_current();
        assert!(!completed);
        assert_eq!(r.current_stage_index, 1);
        assert_eq!(r.status, RunStatus::Running);
        assert_eq!(r.stages[0].status, StageStatus::Approved);
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
}
```

Add `pub mod engine;` to `src-tauri/src/lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test engine::run_record::`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement the transition methods**

```rust
    pub fn new(run_id: String, template_id: String, target_dir: String, stage_ids: &[String]) -> Self {
        Self {
            run_id,
            template_id,
            target_dir,
            status: RunStatus::Running,
            current_stage_index: 0,
            stages: stage_ids
                .iter()
                .map(|id| StageRun {
                    id: id.clone(),
                    status: StageStatus::Pending,
                    session_id: None,
                    log: Vec::new(),
                })
                .collect(),
        }
    }
```
```rust
    pub fn mark_current_awaiting_checkpoint(&mut self, session_id: String) {
        self.current_stage_mut().status = StageStatus::AwaitingCheckpoint;
        self.current_stage_mut().session_id = Some(session_id);
        self.status = RunStatus::AwaitingCheckpoint;
    }

    pub fn mark_current_failed(&mut self) {
        self.current_stage_mut().status = StageStatus::Failed;
        self.status = RunStatus::Failed;
    }

    pub fn approve_current(&mut self) -> bool {
        self.current_stage_mut().status = StageStatus::Approved;
        if self.current_stage_index + 1 < self.stages.len() {
            self.current_stage_index += 1;
            self.status = RunStatus::Running;
            false
        } else {
            self.status = RunStatus::Completed;
            true
        }
    }

    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test engine::run_record::`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add run record model with checkpoint state transitions"
```

---

## Task 5: Stream-JSON Line Parser

**Files:**
- Create: `src-tauri/src/engine/stream_json.rs`
- Modify: `src-tauri/src/engine/mod.rs` (add `pub mod stream_json;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum StageEvent { Init { session_id }, AssistantText { text }, ToolUse { name, input: Value }, ToolResult { content: Value }, Result { session_id, success: bool, result: Option<String> }, Unknown { raw: Value } }` — `Debug + Clone + PartialEq + Serialize`, internally tagged `#[serde(tag = "kind", rename_all = "camelCase")]` so the frontend receives `{"kind":"init","sessionId":"..."}` etc. `pub struct ParseWarning(pub String)`. `pub fn parse_line(line: &str) -> Result<StageEvent, ParseWarning>`.

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/engine/stream_json.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StageEvent {
    Init { session_id: String },
    AssistantText { text: String },
    ToolUse { name: String, input: serde_json::Value },
    ToolResult { content: serde_json::Value },
    Result { session_id: String, success: bool, result: Option<String> },
    Unknown { raw: serde_json::Value },
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("invalid stream-json line: {0}")]
pub struct ParseWarning(pub String);

pub fn parse_line(line: &str) -> Result<StageEvent, ParseWarning> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_init_event() {
        let line = r#"{"type":"system","session_id":"sess-1"}"#;
        assert_eq!(parse_line(line).unwrap(), StageEvent::Init { session_id: "sess-1".to_string() });
    }

    #[test]
    fn parses_assistant_text_event() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(parse_line(line).unwrap(), StageEvent::AssistantText { text: "hello".to_string() });
    }

    #[test]
    fn parses_tool_use_event() {
        let line = r#"{"type":"tool_use","name":"Write","input":{"path":"a.txt"}}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::ToolUse { name: "Write".to_string(), input: serde_json::json!({"path": "a.txt"}) }
        );
    }

    #[test]
    fn parses_tool_result_event() {
        let line = r#"{"type":"tool_result","content":"ok"}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::ToolResult { content: serde_json::json!("ok") }
        );
    }

    #[test]
    fn parses_successful_result_event() {
        let line = r#"{"type":"result","subtype":"success","session_id":"sess-1","result":"done"}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::Result { session_id: "sess-1".to_string(), success: true, result: Some("done".to_string()) }
        );
    }

    #[test]
    fn parses_failed_result_event() {
        let line = r#"{"type":"result","subtype":"error","session_id":"sess-1"}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::Result { session_id: "sess-1".to_string(), success: false, result: None }
        );
    }

    #[test]
    fn unknown_type_becomes_unknown_event() {
        let line = r#"{"type":"future_event","foo":"bar"}"#;
        let event = parse_line(line).unwrap();
        assert!(matches!(event, StageEvent::Unknown { .. }));
    }

    #[test]
    fn invalid_json_returns_parse_warning() {
        assert!(parse_line("not json").is_err());
    }

    #[test]
    fn serializes_with_camel_case_kind_tag() {
        let event = StageEvent::Init { session_id: "sess-1".to_string() };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"kind":"init","sessionId":"sess-1"}"#);
    }
}
```

Add `pub mod stream_json;` to `src-tauri/src/engine/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test engine::stream_json::`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement `parse_line`**

```rust
pub fn parse_line(line: &str) -> Result<StageEvent, ParseWarning> {
    let trimmed = line.trim();
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| ParseWarning(format!("{e}: {trimmed}")))?;
    let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "system" => {
            let session_id = value.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(StageEvent::Init { session_id })
        }
        "assistant" => {
            let text = value
                .pointer("/message/content")
                .and_then(|c| c.as_array())
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            Ok(StageEvent::AssistantText { text })
        }
        "tool_use" => {
            let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let input = value.get("input").cloned().unwrap_or(serde_json::Value::Null);
            Ok(StageEvent::ToolUse { name, input })
        }
        "tool_result" => {
            let content = value.get("content").cloned().unwrap_or(serde_json::Value::Null);
            Ok(StageEvent::ToolResult { content })
        }
        "result" => {
            let session_id = value.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let subtype = value.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
            let result = value.get("result").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(StageEvent::Result { session_id, success: subtype == "success", result })
        }
        _ => Ok(StageEvent::Unknown { raw: value }),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test engine::stream_json::`
Expected: PASS (9 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add stream-json line parser"
```

---

## Task 6: Mock Claude Test Binary

**Files:**
- Modify: `src-tauri/src/bin/mock_claude.rs`

**Interfaces:**
- Consumes: nothing (standalone binary).
- Produces: a `mock_claude` executable (built by `cargo build`/`cargo test` via the `[[bin]]` target from Task 1) used by Task 8/9 integration tests as a stand-in for the real `claude` CLI. Behavior: `mock_claude --version` prints `mock-claude 0.0.1` and exits 0. `mock_claude ... -p FIXTURE:<path>` reads `<path>` line-by-line and echoes each line to stdout, then exits with the code found in `<path>.exitcode` (default 0 if that file doesn't exist).

- [ ] **Step 1: Implement the mock binary**

There is no meaningful "unit test" for a standalone test-fixture binary in isolation (its behavior is exercised by the integration tests in Tasks 8 and 9, which fail to compile/run without it). Implement it directly:

`src-tauri/src/bin/mock_claude.rs`:
```rust
use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version") {
        println!("mock-claude 0.0.1");
        exit(0);
    }

    let prompt = args
        .iter()
        .position(|a| a == "-p")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();

    if let Some(fixture_path) = prompt.strip_prefix("FIXTURE:") {
        let content = fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("mock_claude: cannot read fixture {fixture_path}: {e}"));
        for line in content.lines() {
            println!("{line}");
        }
        let exit_code_path = format!("{fixture_path}.exitcode");
        let code: i32 = fs::read_to_string(&exit_code_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        exit(code);
    }

    exit(0);
}
```

- [ ] **Step 2: Verify it builds and behaves as expected**

Run: `cd src-tauri && cargo build --bin mock_claude`
Expected: builds successfully.

Run (from `src-tauri`, PowerShell): `& .\target\debug\mock_claude.exe --version`
Expected output: `mock-claude 0.0.1`

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test: implement mock_claude fixture binary"
```

---

## Task 7: Claude CLI Availability Check

**Files:**
- Create: `src-tauri/src/cli_check.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod cli_check;`)

**Interfaces:**
- Consumes: nothing (takes a binary name/path as a parameter).
- Produces: `pub enum CliCheckResult { Available(String), NotFound }`, `pub fn check_claude_cli(binary: &str) -> CliCheckResult`.

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/cli_check.rs`:
```rust
use std::process::Command;

#[derive(Debug, PartialEq)]
pub enum CliCheckResult {
    Available(String),
    NotFound,
}

pub fn check_claude_cli(binary: &str) -> CliCheckResult {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_not_found_for_missing_binary() {
        assert_eq!(check_claude_cli("definitely-not-a-real-binary-xyz"), CliCheckResult::NotFound);
    }
}
```

Add `pub mod cli_check;` to `src-tauri/src/lib.rs` (must be `pub` so the integration test below can reach it).

`env!("CARGO_BIN_EXE_mock_claude")` only resolves inside integration test binaries (files under `src-tauri/tests/`), not unit tests inside `src/` — that's why the "available" case is a separate integration test rather than a case in the `mod tests` block above. Create `src-tauri/tests/cli_check_test.rs`:
```rust
use claude_pipeline_wizard_lib::cli_check::{check_claude_cli, CliCheckResult};

#[test]
fn returns_available_with_version_for_mock_binary() {
    let binary = env!("CARGO_BIN_EXE_mock_claude");
    assert_eq!(
        check_claude_cli(binary),
        CliCheckResult::Available("mock-claude 0.0.1".to_string())
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test cli_check`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement `check_claude_cli`**

```rust
pub fn check_claude_cli(binary: &str) -> CliCheckResult {
    match Command::new(binary).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            CliCheckResult::Available(version)
        }
        _ => CliCheckResult::NotFound,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test cli_check`
Expected: PASS (2 tests total: 1 unit + 1 integration).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add claude CLI availability check"
```

---

## Task 8: Stage Process Executor

**Files:**
- Create: `src-tauri/src/engine/executor.rs`
- Create: `src-tauri/tests/fixtures/executor_success.jsonl`
- Create: `src-tauri/tests/fixtures/executor_failure.jsonl`, `src-tauri/tests/fixtures/executor_failure.jsonl.exitcode`
- Create: `src-tauri/tests/executor_test.rs`
- Modify: `src-tauri/src/engine/mod.rs` (add `pub mod executor;`)

**Interfaces:**
- Consumes: `Stage`, `PermissionMode` (Task 2); `StageEvent`, `parse_line` (Task 5); `mock_claude` binary (Task 6).
- Produces: `pub struct ExecutorConfig { pub claude_binary: String }` (`Default` = `"claude"`), `pub async fn run_stage<F: FnMut(StageEvent)>(config: &ExecutorConfig, stage: &Stage, target_dir: &Path, resume_session_id: Option<&str>, on_event: F) -> Result<i32, std::io::Error>` — spawns the CLI, streams parsed events to `on_event`, returns the process exit code.

- [ ] **Step 1: Create fixture files**

`src-tauri/tests/fixtures/executor_success.jsonl`:
```
{"type":"system","session_id":"sess-exec-1"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Working on it"}]}}
{"type":"result","subtype":"success","session_id":"sess-exec-1","result":"done"}
```

`src-tauri/tests/fixtures/executor_failure.jsonl`:
```
{"type":"system","session_id":"sess-exec-2"}
```

`src-tauri/tests/fixtures/executor_failure.jsonl.exitcode`:
```
1
```

- [ ] **Step 2: Write the failing implementation stub and integration test**

`src-tauri/src/engine/executor.rs`:
```rust
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::template::{PermissionMode, Stage};

use super::stream_json::{parse_line, StageEvent};

pub struct ExecutorConfig {
    pub claude_binary: String,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self { claude_binary: "claude".to_string() }
    }
}

fn permission_mode_arg(mode: &PermissionMode) -> &'static str {
    match mode {
        PermissionMode::AcceptEdits => "acceptEdits",
        PermissionMode::BypassPermissions => "bypassPermissions",
        PermissionMode::Default => "default",
    }
}

pub async fn run_stage<F: FnMut(StageEvent)>(
    config: &ExecutorConfig,
    stage: &Stage,
    target_dir: &Path,
    resume_session_id: Option<&str>,
    mut on_event: F,
) -> Result<i32, std::io::Error> {
    todo!()
}
```

Add `pub mod executor;` to `src-tauri/src/engine/mod.rs`.

`src-tauri/tests/executor_test.rs`:
```rust
use std::path::{Path, PathBuf};

use claude_pipeline_wizard_lib::engine::executor::{run_stage, ExecutorConfig};
use claude_pipeline_wizard_lib::engine::stream_json::StageEvent;
use claude_pipeline_wizard_lib::template::{PermissionMode, Stage};

fn stage(prompt: &str) -> Stage {
    Stage {
        id: "s1".to_string(),
        name: "Stage 1".to_string(),
        prompt: prompt.to_string(),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: vec![],
        checkpoint: true,
    }
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[tokio::test]
async fn streams_parsed_events_and_returns_success_exit_code() {
    let config = ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string() };
    let fixture = fixture_path("executor_success.jsonl");
    let prompt = format!("FIXTURE:{}", fixture.to_string_lossy());
    let target_dir = std::env::temp_dir();

    let mut events = Vec::new();
    let exit_code = run_stage(&config, &stage(&prompt), &target_dir, None, |e| events.push(e))
        .await
        .unwrap();

    assert_eq!(exit_code, 0);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], StageEvent::Init { session_id: "sess-exec-1".to_string() });
    assert!(matches!(events[1], StageEvent::AssistantText { .. }));
    assert_eq!(
        events[2],
        StageEvent::Result { session_id: "sess-exec-1".to_string(), success: true, result: Some("done".to_string()) }
    );
}

#[tokio::test]
async fn returns_non_zero_exit_code_on_failure() {
    let config = ExecutorConfig { claude_binary: env!("CARGO_BIN_EXE_mock_claude").to_string() };
    let fixture = fixture_path("executor_failure.jsonl");
    let prompt = format!("FIXTURE:{}", fixture.to_string_lossy());
    let target_dir = std::env::temp_dir();

    let mut events = Vec::new();
    let exit_code = run_stage(&config, &stage(&prompt), &target_dir, None, |e| events.push(e))
        .await
        .unwrap();

    assert_eq!(exit_code, 1);
    assert_eq!(events.len(), 1);
}
```

For the integration test to compile, `engine` must be `pub mod engine;` (already is, from Task 4) and `template` must be `pub mod template;` (already is, from Task 2).

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --test executor_test`
Expected: FAIL — `todo!()` panics (or compile error until implemented).

- [ ] **Step 4: Implement `run_stage`**

```rust
pub async fn run_stage<F: FnMut(StageEvent)>(
    config: &ExecutorConfig,
    stage: &Stage,
    target_dir: &Path,
    resume_session_id: Option<&str>,
    mut on_event: F,
) -> Result<i32, std::io::Error> {
    let mut cmd = Command::new(&config.claude_binary);
    cmd.current_dir(target_dir)
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--permission-mode")
        .arg(permission_mode_arg(&stage.permission_mode));

    if let Some(session_id) = resume_session_id {
        cmd.arg("--resume").arg(session_id);
    }

    cmd.arg("-p").arg(&stage.prompt);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");
    let mut reader = BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await? {
        match parse_line(&line) {
            Ok(event) => on_event(event),
            Err(warning) => on_event(StageEvent::Unknown {
                raw: serde_json::json!({ "parseWarning": warning.0 }),
            }),
        }
    }

    let status = child.wait().await?;
    Ok(status.code().unwrap_or(-1))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --test executor_test`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add stage process executor"
```

---

## Task 9: Pipeline Orchestrator

**Files:**
- Create: `src-tauri/src/engine/orchestrator.rs`
- Create: `src-tauri/tests/fixtures/orchestrator_stage1.jsonl`, `orchestrator_stage2.jsonl`, `orchestrator_feedback.jsonl`
- Create: `src-tauri/tests/orchestrator_test.rs`
- Modify: `src-tauri/src/engine/mod.rs` (add `pub mod orchestrator;`)

**Interfaces:**
- Consumes: `TemplateStore`, `StoreError`, `Template` (Task 3); `RunRecord`, `RunRecordStore`, `RunRecordStoreError`, `RunStatus`, `StageStatus` (Task 4); `StageEvent` (Task 5); `run_stage`, `ExecutorConfig` (Task 8).
- Produces: `pub struct Orchestrator { pub template_store: TemplateStore, pub run_store: RunRecordStore, pub executor_config: ExecutorConfig }` with `new(...)`, `pub async fn start_run<F: FnMut(&str, StageEvent)>(&self, template_id: &str, target_dir: PathBuf, run_id: String, on_event: F) -> Result<RunRecord, OrchestratorError>`, `pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(&self, run_id: &str, on_event: F) -> Result<RunRecord, OrchestratorError>`, `pub async fn request_changes<F: FnMut(&str, StageEvent)>(&self, run_id: &str, feedback: &str, on_event: F) -> Result<RunRecord, OrchestratorError>`, `pub fn reject_checkpoint(&self, run_id: &str) -> Result<RunRecord, OrchestratorError>`. `pub enum OrchestratorError` wraps the store errors plus `Io` and `NotAwaitingCheckpoint(String)`.

- [ ] **Step 1: Create fixture files**

`src-tauri/tests/fixtures/orchestrator_stage1.jsonl`:
```
{"type":"system","session_id":"sess-orch-1"}
{"type":"result","subtype":"success","session_id":"sess-orch-1","result":"stage1 done"}
```

`src-tauri/tests/fixtures/orchestrator_stage2.jsonl`:
```
{"type":"system","session_id":"sess-orch-2"}
{"type":"result","subtype":"success","session_id":"sess-orch-2","result":"stage2 done"}
```

`src-tauri/tests/fixtures/orchestrator_feedback.jsonl`:
```
{"type":"result","subtype":"success","session_id":"sess-orch-1-revised","result":"revised"}
```

- [ ] **Step 2: Write the failing implementation stub**

`src-tauri/src/engine/orchestrator.rs`:
```rust
use std::path::PathBuf;

use crate::template::{store::StoreError as TemplateStoreError, store::TemplateStore, Template};

use super::executor::{run_stage, ExecutorConfig};
use super::run_record::{RunRecord, RunRecordStore, RunRecordStoreError, RunStatus, StageStatus};
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

    pub async fn start_run<F: FnMut(&str, StageEvent)>(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        todo!()
    }

    pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
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
        record.cancel();
        self.run_store.save(&record)?;
        Ok(record)
    }
}
```

Add `pub mod orchestrator;` to `src-tauri/src/engine/mod.rs`. Note: `TemplateStore` needs to be reachable as `crate::template::store::TemplateStore` — confirm `src-tauri/src/template/mod.rs` has `pub mod store;` (added in Task 3) and that `store::StoreError`/`store::TemplateStore` are `pub`.

`src-tauri/tests/orchestrator_test.rs`:
```rust
use std::path::Path;

use claude_pipeline_wizard_lib::engine::executor::ExecutorConfig;
use claude_pipeline_wizard_lib::engine::orchestrator::Orchestrator;
use claude_pipeline_wizard_lib::engine::run_record::{RunRecordStore, RunStatus, StageStatus};
use claude_pipeline_wizard_lib::engine::stream_json::StageEvent;
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
                checkpoint: true,
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

#[tokio::test]
async fn start_run_stops_at_first_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    let record = orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].status, StageStatus::AwaitingCheckpoint);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1".to_string()));
}

#[tokio::test]
async fn approve_checkpoint_advances_and_then_completes() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    let mut events = Vec::new();
    let record = orchestrator
        .approve_checkpoint("run1", |stage_id, event| events.push((stage_id.to_string(), event)))
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(record.stages[0].status, StageStatus::Approved);
    assert_eq!(record.stages[1].status, StageStatus::AwaitingCheckpoint);
    assert!(events.iter().any(|(id, _)| id == "stage2"));
}

#[tokio::test]
async fn request_changes_reruns_current_stage_and_stays_awaiting_checkpoint() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    let mut template = orchestrator.template_store.load("two-stage").unwrap();
    template.stages[0].prompt = fixture_prompt("orchestrator_feedback.jsonl");
    orchestrator.template_store.save(&template).unwrap();

    let record = orchestrator
        .request_changes("run1", fixture_prompt("orchestrator_feedback.jsonl").as_str(), |_, _| {})
        .await
        .unwrap();

    assert_eq!(record.status, RunStatus::AwaitingCheckpoint);
    assert_eq!(record.current_stage_index, 0);
    assert_eq!(record.stages[0].session_id, Some("sess-orch-1-revised".to_string()));
}

#[tokio::test]
async fn reject_checkpoint_cancels_run() {
    let (orchestrator, _t, _r, target_dir) = setup();
    orchestrator
        .start_run("two-stage", target_dir.path().to_path_buf(), "run1".to_string(), |_, _| {})
        .await
        .unwrap();

    let record = orchestrator.reject_checkpoint("run1").unwrap();
    assert_eq!(record.status, RunStatus::Cancelled);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --test orchestrator_test`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 4: Implement `start_run` and the shared `drive` loop**

```rust
impl Orchestrator {
    pub async fn start_run<F: FnMut(&str, StageEvent)>(
        &self,
        template_id: &str,
        target_dir: PathBuf,
        run_id: String,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let template = self.template_store.load(template_id)?;
        let stage_ids: Vec<String> = template.stages.iter().map(|s| s.id.clone()).collect();
        let mut record = RunRecord::new(run_id, template.id.clone(), target_dir.to_string_lossy().to_string(), &stage_ids);
        std::fs::create_dir_all(&target_dir)?;
        self.drive(&template, &mut record, &target_dir, None, &mut on_event).await?;
        self.run_store.save(&record)?;
        Ok(record)
    }

    /// Runs stages starting at `record.current_stage_index`, auto-advancing
    /// through any stage with `checkpoint == false`, and stops at the first
    /// checkpointed stage, a failure, or the end of the pipeline.
    async fn drive<F: FnMut(&str, StageEvent)>(
        &self,
        template: &Template,
        record: &mut RunRecord,
        target_dir: &PathBuf,
        mut resume_session_id: Option<String>,
        on_event: &mut F,
    ) -> Result<(), OrchestratorError> {
        loop {
            let stage_index = record.current_stage_index;
            let stage = &template.stages[stage_index];
            let stage_id = stage.id.clone();
            let mut latest_session_id = resume_session_id.clone();
            let mut collected_log = Vec::new();

            let exit_code = run_stage(&self.executor_config, stage, target_dir, resume_session_id.as_deref(), |event| {
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
                return Ok(());
            }

            let session_id = latest_session_id.unwrap_or_default();
            if stage.checkpoint {
                record.mark_current_awaiting_checkpoint(session_id);
                return Ok(());
            }

            record.current_stage_mut().status = StageStatus::Approved;
            record.current_stage_mut().session_id = Some(session_id.clone());
            if record.current_stage_index + 1 >= record.stages.len() {
                record.status = RunStatus::Completed;
                return Ok(());
            }
            record.current_stage_index += 1;
            record.status = RunStatus::Running;
            resume_session_id = Some(session_id);
        }
    }
}
```

- [ ] **Step 5: Implement `approve_checkpoint`**

```rust
    pub async fn approve_checkpoint<F: FnMut(&str, StageEvent)>(
        &self,
        run_id: &str,
        mut on_event: F,
    ) -> Result<RunRecord, OrchestratorError> {
        let mut record = self.run_store.load(run_id)?;
        if record.status != RunStatus::AwaitingCheckpoint {
            return Err(OrchestratorError::NotAwaitingCheckpoint(run_id.to_string()));
        }
        let template = self.template_store.load(&record.template_id)?;
        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());
        let completed = record.approve_current();
        if !completed {
            self.drive(&template, &mut record, &target_dir, resume_session_id, &mut on_event).await?;
        }
        self.run_store.save(&record)?;
        Ok(record)
    }
```

- [ ] **Step 6: Implement `request_changes`**

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
        let mut feedback_stage = template.stages[stage_index].clone();
        feedback_stage.prompt = feedback.to_string();
        let stage_id = feedback_stage.id.clone();

        let resume_session_id = record.current_stage_mut().session_id.clone();
        let target_dir = PathBuf::from(record.target_dir.clone());
        let mut latest_session_id = resume_session_id.clone();
        let mut collected_log = Vec::new();

        let exit_code = run_stage(&self.executor_config, &feedback_stage, &target_dir, resume_session_id.as_deref(), |event| {
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
            record.mark_current_awaiting_checkpoint(latest_session_id.unwrap_or_default());
        }
        self.run_store.save(&record)?;
        Ok(record)
    }
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --test orchestrator_test`
Expected: PASS (4 tests).

Run: `cd src-tauri && cargo test`
Expected: PASS (all tests across all tasks so far).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: add pipeline orchestrator with checkpoint control flow"
```

---

## Task 10: Tauri Commands & App Wiring

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `Orchestrator`, `OrchestratorError` (Task 9); `Template` (Task 2); `RunRecord` (Task 4); `StageEvent` (Task 5).
- Produces: Tauri commands `list_templates`, `load_template`, `save_template`, `delete_template`, `start_pipeline_run`, `approve_checkpoint`, `request_changes`, `reject_checkpoint`, `check_cli`, registered on the app and callable from the frontend via `@tauri-apps/api/core`'s `invoke`. Emits `pipeline://stage-event` with payload `{ runId, stageId, event }`.

The command functions are thin glue over already-tested orchestrator/store logic (Tasks 2-9); the verification for this task is that the app compiles and starts, exercised by `cargo check` here and by the manual smoke test in Task 16 which drives every command through the real UI.

- [ ] **Step 1: Implement the commands module**

`src-tauri/src/commands.rs`:
```rust
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::cli_check::{check_claude_cli, CliCheckResult};
use crate::engine::orchestrator::Orchestrator;
use crate::engine::run_record::RunRecord;
use crate::engine::stream_json::StageEvent;
use crate::template::Template;

#[derive(Serialize, Clone)]
struct StageEventPayload {
    run_id: String,
    stage_id: String,
    event: StageEvent,
}

fn emit_stage_event(app: &AppHandle, run_id: &str, stage_id: &str, event: StageEvent) {
    let _ = app.emit(
        "pipeline://stage-event",
        StageEventPayload { run_id: run_id.to_string(), stage_id: stage_id.to_string(), event },
    );
}

#[tauri::command]
pub fn list_templates(orchestrator: State<Orchestrator>) -> Result<Vec<Template>, String> {
    orchestrator.template_store.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_template(orchestrator: State<Orchestrator>, id: String) -> Result<Template, String> {
    orchestrator.template_store.load(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_template(orchestrator: State<Orchestrator>, template: Template) -> Result<(), String> {
    orchestrator.template_store.save(&template).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_template(orchestrator: State<Orchestrator>, id: String) -> Result<(), String> {
    orchestrator.template_store.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_cli(orchestrator: State<Orchestrator>) -> String {
    match check_claude_cli(&orchestrator.executor_config.claude_binary) {
        CliCheckResult::Available(version) => format!("available:{version}"),
        CliCheckResult::NotFound => "not-found".to_string(),
    }
}

#[tauri::command]
pub async fn start_pipeline_run(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    template_id: String,
    target_dir: String,
) -> Result<RunRecord, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    orchestrator
        .start_run(&template_id, target_dir.into(), run_id.clone(), |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn approve_checkpoint(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
) -> Result<RunRecord, String> {
    orchestrator
        .approve_checkpoint(&run_id, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn request_changes(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
    feedback: String,
) -> Result<RunRecord, String> {
    orchestrator
        .request_changes(&run_id, &feedback, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reject_checkpoint(orchestrator: State<Orchestrator>, run_id: String) -> Result<RunRecord, String> {
    orchestrator.reject_checkpoint(&run_id).map_err(|e| e.to_string())
}
```

Note: this uses `run_id.clone()` captured into the closure while `run_id` (the command parameter) is also used afterward only implicitly — double check borrow: in `start_pipeline_run`, `run_id` is moved into `orchestrator.start_run(&template_id, target_dir.into(), run_id.clone(), closure)`; the closure itself captures `run_id` by reference for `emit_stage_event(&app, &run_id, ...)`. Since `run_id.clone()` is passed by value as the second positional arg and the original `run_id` is borrowed by the closure afterward in the same call expression, this is fine — Rust evaluates the `run_id.clone()` argument before the closure captures the original `run_id`, and the closure only borrows `run_id` (not moves it), so both coexist without conflict.

- [ ] **Step 2: Wire commands and orchestrator state into `lib.rs`**

`src-tauri/src/lib.rs`:
```rust
pub mod cli_check;
pub mod commands;
pub mod engine;
pub mod template;

use engine::executor::ExecutorConfig;
use engine::orchestrator::Orchestrator;
use engine::run_record::RunRecordStore;
use tauri::Manager;
use template::store::TemplateStore;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("failed to resolve app data dir");
            let orchestrator = Orchestrator::new(
                TemplateStore::new(app_data_dir.join("templates")),
                RunRecordStore::new(app_data_dir.join("runs")),
                ExecutorConfig::default(),
            );
            app.manage(orchestrator);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_templates,
            commands::load_template,
            commands::save_template,
            commands::delete_template,
            commands::check_cli,
            commands::start_pipeline_run,
            commands::approve_checkpoint,
            commands::request_changes,
            commands::reject_checkpoint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`cli_check` must change from `mod cli_check;` (Task 7) to `pub mod cli_check;` here (already done above) since `commands.rs` imports from it.

- [ ] **Step 3: Verify everything compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

Run: `cd src-tauri && cargo test`
Expected: PASS (all existing tests still pass — this task adds no new automated tests, per the rationale above).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: wire Tauri commands and orchestrator state"
```

---

## Task 11: Frontend Types & API Wrapper

**Files:**
- Create: `src/types.ts`, `src/api.ts`, `src/api.test.ts`

**Interfaces:**
- Consumes: nothing (mirrors Rust JSON shapes from Tasks 2, 4, 5, 10 by convention).
- Produces: TS types `PermissionMode`, `Stage`, `Template`, `StageStatus`, `RunStatus`, `StageRun`, `RunRecord`, `StageEvent`, `StageEventPayload`. API functions: `listTemplates`, `loadTemplate`, `saveTemplate`, `deleteTemplate`, `checkCli`, `startPipelineRun`, `approveCheckpoint`, `requestChanges`, `rejectCheckpoint`, `onStageEvent` — used by Tasks 12-14.

- [ ] **Step 1: Write the types module (no test needed — pure type declarations)**

`src/types.ts`:
```typescript
export type PermissionMode = "acceptEdits" | "bypassPermissions" | "default";

export interface Stage {
  id: string;
  name: string;
  prompt: string;
  permissionMode: PermissionMode;
  allowedTools: string[];
  checkpoint: boolean;
}

export interface Template {
  id: string;
  name: string;
  description: string;
  stages: Stage[];
}

export type StageStatus = "pending" | "running" | "awaiting-checkpoint" | "approved" | "failed";
export type RunStatus = "running" | "awaiting-checkpoint" | "completed" | "failed" | "cancelled";

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
}

export type StageEvent =
  | { kind: "init"; sessionId: string }
  | { kind: "assistantText"; text: string }
  | { kind: "toolUse"; name: string; input: unknown }
  | { kind: "toolResult"; content: unknown }
  | { kind: "result"; sessionId: string; success: boolean; result: string | null }
  | { kind: "unknown"; raw: unknown };

export interface StageEventPayload {
  runId: string;
  stageId: string;
  event: StageEvent;
}
```

- [ ] **Step 2: Write the failing API wrapper tests**

`src/api.test.ts`:
```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import {
  listTemplates,
  loadTemplate,
  saveTemplate,
  deleteTemplate,
  checkCli,
  startPipelineRun,
  approveCheckpoint,
  requestChanges,
  rejectCheckpoint,
  onStageEvent,
} from "./api";
import type { Template } from "./types";

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
});

describe("api wrapper", () => {
  it("listTemplates invokes list_templates", async () => {
    invokeMock.mockResolvedValue([]);
    await listTemplates();
    expect(invokeMock).toHaveBeenCalledWith("list_templates");
  });

  it("loadTemplate invokes load_template with id", async () => {
    invokeMock.mockResolvedValue({});
    await loadTemplate("t1");
    expect(invokeMock).toHaveBeenCalledWith("load_template", { id: "t1" });
  });

  it("saveTemplate invokes save_template with template", async () => {
    invokeMock.mockResolvedValue(undefined);
    const template: Template = { id: "t1", name: "T", description: "d", stages: [] };
    await saveTemplate(template);
    expect(invokeMock).toHaveBeenCalledWith("save_template", { template });
  });

  it("deleteTemplate invokes delete_template with id", async () => {
    invokeMock.mockResolvedValue(undefined);
    await deleteTemplate("t1");
    expect(invokeMock).toHaveBeenCalledWith("delete_template", { id: "t1" });
  });

  it("checkCli invokes check_cli", async () => {
    invokeMock.mockResolvedValue("not-found");
    await checkCli();
    expect(invokeMock).toHaveBeenCalledWith("check_cli");
  });

  it("startPipelineRun invokes start_pipeline_run with templateId and targetDir", async () => {
    invokeMock.mockResolvedValue({});
    await startPipelineRun("t1", "/tmp/proj");
    expect(invokeMock).toHaveBeenCalledWith("start_pipeline_run", { templateId: "t1", targetDir: "/tmp/proj" });
  });

  it("approveCheckpoint invokes approve_checkpoint with runId", async () => {
    invokeMock.mockResolvedValue({});
    await approveCheckpoint("run1");
    expect(invokeMock).toHaveBeenCalledWith("approve_checkpoint", { runId: "run1" });
  });

  it("requestChanges invokes request_changes with runId and feedback", async () => {
    invokeMock.mockResolvedValue({});
    await requestChanges("run1", "fix this");
    expect(invokeMock).toHaveBeenCalledWith("request_changes", { runId: "run1", feedback: "fix this" });
  });

  it("rejectCheckpoint invokes reject_checkpoint with runId", async () => {
    invokeMock.mockResolvedValue({});
    await rejectCheckpoint("run1");
    expect(invokeMock).toHaveBeenCalledWith("reject_checkpoint", { runId: "run1" });
  });

  it("onStageEvent listens on pipeline://stage-event and forwards the payload", async () => {
    const handler = vi.fn();
    listenMock.mockImplementation((_event, cb) => {
      cb({ payload: { runId: "run1", stageId: "s1", event: { kind: "init", sessionId: "sess1" } } });
      return Promise.resolve(() => {});
    });
    await onStageEvent(handler);
    expect(handler).toHaveBeenCalledWith({ runId: "run1", stageId: "s1", event: { kind: "init", sessionId: "sess1" } });
  });
});
```

This test file imports `./api`, which does not exist yet.

- [ ] **Step 3: Run tests to verify they fail**

Run: `npm run test`
Expected: FAIL — cannot resolve `./api`.

- [ ] **Step 4: Implement the API wrapper**

`src/api.ts`:
```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { RunRecord, StageEventPayload, Template } from "./types";

export function listTemplates(): Promise<Template[]> {
  return invoke("list_templates");
}

export function loadTemplate(id: string): Promise<Template> {
  return invoke("load_template", { id });
}

export function saveTemplate(template: Template): Promise<void> {
  return invoke("save_template", { template });
}

export function deleteTemplate(id: string): Promise<void> {
  return invoke("delete_template", { id });
}

export function checkCli(): Promise<string> {
  return invoke("check_cli");
}

export function startPipelineRun(templateId: string, targetDir: string): Promise<RunRecord> {
  return invoke("start_pipeline_run", { templateId, targetDir });
}

export function approveCheckpoint(runId: string): Promise<RunRecord> {
  return invoke("approve_checkpoint", { runId });
}

export function requestChanges(runId: string, feedback: string): Promise<RunRecord> {
  return invoke("request_changes", { runId, feedback });
}

export function rejectCheckpoint(runId: string): Promise<RunRecord> {
  return invoke("reject_checkpoint", { runId });
}

export function onStageEvent(handler: (payload: StageEventPayload) => void): Promise<UnlistenFn> {
  return listen<StageEventPayload>("pipeline://stage-event", (event) => handler(event.payload));
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm run test`
Expected: PASS (10 tests).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add frontend types and Tauri API wrapper"
```

---

## Task 12: Template Gallery Page

**Files:**
- Create: `src/pages/TemplateGallery.tsx`, `src/pages/TemplateGallery.test.tsx`

**Interfaces:**
- Consumes: `listTemplates`, `deleteTemplate`, `checkCli` (Task 11); `Template` type (Task 11).
- Produces: `export default function TemplateGallery(props: { onRun: (t: Template) => void; onEdit: (t: Template) => void; onNew: () => void }): JSX.Element` — used by `App.tsx` in Task 16.

- [ ] **Step 1: Write the failing tests**

`src/pages/TemplateGallery.test.tsx`:
```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("../api", () => ({
  listTemplates: vi.fn(),
  deleteTemplate: vi.fn(),
  checkCli: vi.fn(),
}));

import { listTemplates, deleteTemplate, checkCli } from "../api";
import TemplateGallery from "./TemplateGallery";
import type { Template } from "../types";

const sample: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "요구사항부터 배포까지",
  stages: [],
};

beforeEach(() => {
  vi.mocked(listTemplates).mockReset();
  vi.mocked(deleteTemplate).mockReset();
  vi.mocked(checkCli).mockReset();
  vi.mocked(checkCli).mockResolvedValue("available:mock-claude 0.0.1");
});

describe("TemplateGallery", () => {
  it("renders templates returned by listTemplates", async () => {
    vi.mocked(listTemplates).mockResolvedValue([sample]);
    render(<TemplateGallery onRun={vi.fn()} onEdit={vi.fn()} onNew={vi.fn()} />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
    expect(screen.getByText("요구사항부터 배포까지")).toBeInTheDocument();
  });

  it("shows a warning banner when the CLI is not found", async () => {
    vi.mocked(listTemplates).mockResolvedValue([]);
    vi.mocked(checkCli).mockResolvedValue("not-found");
    render(<TemplateGallery onRun={vi.fn()} onEdit={vi.fn()} onNew={vi.fn()} />);
    expect(await screen.findByText(/Claude Code CLI/)).toBeInTheDocument();
  });

  it("calls deleteTemplate and refetches when delete is clicked", async () => {
    vi.mocked(listTemplates).mockResolvedValueOnce([sample]).mockResolvedValueOnce([]);
    vi.mocked(deleteTemplate).mockResolvedValue(undefined);
    render(<TemplateGallery onRun={vi.fn()} onEdit={vi.fn()} onNew={vi.fn()} />);
    const deleteButton = await screen.findByRole("button", { name: "삭제" });
    deleteButton.click();
    await waitFor(() => expect(deleteTemplate).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(listTemplates).toHaveBeenCalledTimes(2));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test`
Expected: FAIL — cannot resolve `./TemplateGallery`.

- [ ] **Step 3: Implement `TemplateGallery`**

`src/pages/TemplateGallery.tsx`:
```typescript
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add template gallery page"
```

---

## Task 13: Template Editor Page

**Files:**
- Create: `src/pages/TemplateEditor.tsx`, `src/pages/TemplateEditor.test.tsx`

**Interfaces:**
- Consumes: `saveTemplate` (Task 11); `Template`, `Stage`, `PermissionMode` types (Task 11).
- Produces: `export default function TemplateEditor(props: { initial: Template | null; onSaved: (t: Template) => void; onCancel: () => void }): JSX.Element` — used by `App.tsx` in Task 16. `initial === null` means "creating a new template".

- [ ] **Step 1: Write the failing tests**

`src/pages/TemplateEditor.test.tsx`:
```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

vi.mock("../api", () => ({ saveTemplate: vi.fn() }));

import { saveTemplate } from "../api";
import TemplateEditor from "./TemplateEditor";
import type { Template } from "../types";

beforeEach(() => {
  vi.mocked(saveTemplate).mockReset();
  vi.mocked(saveTemplate).mockResolvedValue(undefined);
});

const existing: Template = {
  id: "t1",
  name: "웹 프로그램 개발",
  description: "desc",
  stages: [
    {
      id: "s1",
      name: "요구사항",
      prompt: "PRD.md 작성",
      permissionMode: "acceptEdits",
      allowedTools: ["Read"],
      checkpoint: true,
    },
  ],
};

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
    await Promise.resolve();
    expect(saveTemplate).toHaveBeenCalledWith(expect.objectContaining({ name: "웹 프로그램 개발 v2" }));
    expect(onSaved).toHaveBeenCalled();
  });

  it("adds a new stage when '단계 추가' is clicked", () => {
    render(<TemplateEditor initial={null} onSaved={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "단계 추가" }));
    expect(screen.getByLabelText("단계 1 프롬프트")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test`
Expected: FAIL — cannot resolve `./TemplateEditor`.

- [ ] **Step 3: Implement `TemplateEditor`**

`src/pages/TemplateEditor.tsx`:
```typescript
import { useState } from "react";
import { saveTemplate } from "../api";
import type { PermissionMode, Stage, Template } from "../types";

interface Props {
  initial: Template | null;
  onSaved: (template: Template) => void;
  onCancel: () => void;
}

const PERMISSION_MODES: PermissionMode[] = ["acceptEdits", "bypassPermissions", "default"];
const KNOWN_TOOLS = ["Read", "Write", "Edit", "Glob", "Grep", "Bash", "WebSearch", "WebFetch"];

function emptyStage(index: number): Stage {
  return {
    id: `stage-${index}-${Date.now()}`,
    name: `단계 ${index + 1}`,
    prompt: "",
    permissionMode: "acceptEdits",
    allowedTools: [],
    checkpoint: true,
  };
}

function validate(template: Template): string | null {
  if (template.stages.length === 0) return "단계가 최소 1개 필요합니다.";
  const ids = new Set<string>();
  for (const stage of template.stages) {
    if (stage.prompt.trim() === "") return `단계 '${stage.name}'의 프롬프트가 비어 있습니다.`;
    if (ids.has(stage.id)) return `단계 id '${stage.id}'가 중복되었습니다.`;
    ids.add(stage.id);
  }
  return null;
}

export default function TemplateEditor({ initial, onSaved, onCancel }: Props) {
  const [template, setTemplate] = useState<Template>(
    initial ?? { id: `template-${Date.now()}`, name: "", description: "", stages: [] }
  );

  const error = validate(template);

  const updateStage = (index: number, patch: Partial<Stage>) => {
    setTemplate((t) => ({
      ...t,
      stages: t.stages.map((s, i) => (i === index ? { ...s, ...patch } : s)),
    }));
  };

  const addStage = () => {
    setTemplate((t) => ({ ...t, stages: [...t.stages, emptyStage(t.stages.length)] }));
  };

  const removeStage = (index: number) => {
    setTemplate((t) => ({ ...t, stages: t.stages.filter((_, i) => i !== index) }));
  };

  const moveStage = (index: number, direction: -1 | 1) => {
    setTemplate((t) => {
      const target = index + direction;
      if (target < 0 || target >= t.stages.length) return t;
      const stages = [...t.stages];
      [stages[index], stages[target]] = [stages[target], stages[index]];
      return { ...t, stages };
    });
  };

  const handleSave = async () => {
    if (error) return;
    await saveTemplate(template);
    onSaved(template);
  };

  return (
    <div>
      <label>
        템플릿 이름
        <input value={template.name} onChange={(e) => setTemplate((t) => ({ ...t, name: e.target.value }))} />
      </label>
      <label>
        설명
        <input
          value={template.description}
          onChange={(e) => setTemplate((t) => ({ ...t, description: e.target.value }))}
        />
      </label>

      {template.stages.map((stage, index) => (
        <fieldset key={stage.id}>
          <legend>{stage.name}</legend>
          <label htmlFor={`stage-name-${index}`}>단계 {index + 1} 이름</label>
          <input
            id={`stage-name-${index}`}
            value={stage.name}
            onChange={(e) => updateStage(index, { name: e.target.value })}
          />
          <label htmlFor={`stage-prompt-${index}`}>단계 {index + 1} 프롬프트</label>
          <textarea
            id={`stage-prompt-${index}`}
            aria-label={`단계 ${index + 1} 프롬프트`}
            value={stage.prompt}
            onChange={(e) => updateStage(index, { prompt: e.target.value })}
          />
          <label htmlFor={`stage-mode-${index}`}>권한 모드</label>
          <select
            id={`stage-mode-${index}`}
            value={stage.permissionMode}
            onChange={(e) => updateStage(index, { permissionMode: e.target.value as PermissionMode })}
          >
            {PERMISSION_MODES.map((mode) => (
              <option key={mode} value={mode}>
                {mode}
              </option>
            ))}
          </select>
          {KNOWN_TOOLS.map((tool) => (
            <label key={tool}>
              <input
                type="checkbox"
                checked={stage.allowedTools.includes(tool)}
                onChange={(e) => {
                  const allowedTools = e.target.checked
                    ? [...stage.allowedTools, tool]
                    : stage.allowedTools.filter((t) => t !== tool);
                  updateStage(index, { allowedTools });
                }}
              />
              {tool}
            </label>
          ))}
          <label>
            체크포인트
            <input
              type="checkbox"
              checked={stage.checkpoint}
              onChange={(e) => updateStage(index, { checkpoint: e.target.checked })}
            />
          </label>
          <button onClick={() => moveStage(index, -1)}>위로</button>
          <button onClick={() => moveStage(index, 1)}>아래로</button>
          <button onClick={() => removeStage(index)}>단계 삭제</button>
        </fieldset>
      ))}

      <button onClick={addStage}>단계 추가</button>
      {error && <p role="alert">{error}</p>}
      <button onClick={handleSave} disabled={!!error}>
        저장
      </button>
      <button onClick={onCancel}>취소</button>
    </div>
  );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add template editor page"
```

---

## Task 14: Pipeline Run View

**Files:**
- Create: `src/pages/PipelineRun.tsx`, `src/pages/PipelineRun.test.tsx`

**Interfaces:**
- Consumes: `onStageEvent`, `approveCheckpoint`, `requestChanges`, `rejectCheckpoint` (Task 11); `RunRecord`, `StageEventPayload` types (Task 11).
- Produces: `export default function PipelineRun(props: { initialRun: RunRecord; onFinished: () => void }): JSX.Element` — used by `App.tsx` in Task 16.

- [ ] **Step 1: Write the failing tests**

`src/pages/PipelineRun.test.tsx`:
```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("../api", () => ({
  onStageEvent: vi.fn(),
  approveCheckpoint: vi.fn(),
  requestChanges: vi.fn(),
  rejectCheckpoint: vi.fn(),
}));

import { onStageEvent, approveCheckpoint, requestChanges, rejectCheckpoint } from "../api";
import PipelineRun from "./PipelineRun";
import type { RunRecord } from "../types";

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
};

beforeEach(() => {
  vi.mocked(onStageEvent).mockReset();
  vi.mocked(onStageEvent).mockResolvedValue(() => {});
  vi.mocked(approveCheckpoint).mockReset();
  vi.mocked(requestChanges).mockReset();
  vi.mocked(rejectCheckpoint).mockReset();
});

describe("PipelineRun", () => {
  it("shows the checkpoint panel when the run is awaiting checkpoint", () => {
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    expect(screen.getByRole("button", { name: "승인" })).toBeInTheDocument();
  });

  it("appends incoming stage events to the live log", async () => {
    let capturedHandler: (payload: unknown) => void = () => {};
    vi.mocked(onStageEvent).mockImplementation((handler) => {
      capturedHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    capturedHandler({ runId: "run1", stageId: "s1", event: { kind: "assistantText", text: "작업 중입니다" } });
    expect(await screen.findByText("작업 중입니다")).toBeInTheDocument();
  });

  it("calls approveCheckpoint and updates run state on approve click", async () => {
    const updated: RunRecord = { ...runningRun, status: "completed", currentStageIndex: 1 };
    vi.mocked(approveCheckpoint).mockResolvedValue(updated);
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "승인" }));
    await waitFor(() => expect(approveCheckpoint).toHaveBeenCalledWith("run1"));
    expect(await screen.findByText(/completed/)).toBeInTheDocument();
  });

  it("submits feedback via requestChanges", async () => {
    const updated: RunRecord = { ...runningRun };
    vi.mocked(requestChanges).mockResolvedValue(updated);
    render(<PipelineRun initialRun={runningRun} onFinished={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("수정 요청 내용"), { target: { value: "더 자세히 써줘" } });
    fireEvent.click(screen.getByRole("button", { name: "수정 요청 보내기" }));
    await waitFor(() => expect(requestChanges).toHaveBeenCalledWith("run1", "더 자세히 써줘"));
  });

  it("calls rejectCheckpoint and onFinished when rejected", async () => {
    const onFinished = vi.fn();
    const cancelled: RunRecord = { ...runningRun, status: "cancelled" };
    vi.mocked(rejectCheckpoint).mockResolvedValue(cancelled);
    render(<PipelineRun initialRun={runningRun} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "거부" }));
    await waitFor(() => expect(rejectCheckpoint).toHaveBeenCalledWith("run1"));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test`
Expected: FAIL — cannot resolve `./PipelineRun`.

- [ ] **Step 3: Implement `PipelineRun`**

`src/pages/PipelineRun.tsx`:
```typescript
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
    const updated = await approveCheckpoint(run.runId);
    setRun(updated);
    if (updated.status === "completed" || updated.status === "cancelled" || updated.status === "failed") {
      onFinished();
    }
  };

  const handleRequestChanges = async () => {
    const updated = await requestChanges(run.runId, feedback);
    setRun(updated);
    setFeedback("");
  };

  const handleReject = async () => {
    const updated = await rejectCheckpoint(run.runId);
    setRun(updated);
    onFinished();
  };

  return (
    <div>
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
          <button onClick={handleApprove}>승인</button>
          <button onClick={handleReject}>거부</button>
          <label htmlFor="feedback">수정 요청 내용</label>
          <textarea id="feedback" value={feedback} onChange={(e) => setFeedback(e.target.value)} />
          <button onClick={handleRequestChanges}>수정 요청 보내기</button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add pipeline run view with checkpoint controls"
```

---

## Task 15: Seed Templates

**Files:**
- Create: `src-tauri/src/template/seed.rs`
- Modify: `src-tauri/src/template/mod.rs` (add `pub mod seed;`)
- Modify: `src-tauri/src/lib.rs` (call seeding at startup)

**Interfaces:**
- Consumes: `TemplateStore`, `Template`, `Stage`, `PermissionMode` (Tasks 2-3).
- Produces: `pub fn seed_default_templates(store: &TemplateStore) -> Result<(), StoreError>` — saves the two default templates only if a template with the same id doesn't already exist (so user edits survive relaunch).

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/template/seed.rs`:
```rust
use super::store::{StoreError, TemplateStore};
use super::Template;

pub fn seed_default_templates(store: &TemplateStore) -> Result<(), StoreError> {
    todo!()
}

fn default_templates() -> Vec<Template> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    #[test]
    fn creates_default_templates_when_store_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        seed_default_templates(&store).unwrap();
        let templates = store.list().unwrap();
        assert_eq!(templates.len(), 2);
        assert!(templates.iter().any(|t| t.id == "web-app-dev"));
        assert!(templates.iter().any(|t| t.id == "desktop-app-dev-tauri"));
    }

    #[test]
    fn does_not_overwrite_a_customized_existing_template() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        let customized = Template {
            id: "web-app-dev".to_string(),
            name: "내가 수정한 이름".to_string(),
            description: "desc".to_string(),
            stages: vec![Stage {
                id: "only-stage".to_string(),
                name: "Only".to_string(),
                prompt: "do it".to_string(),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            }],
        };
        store.save(&customized).unwrap();

        seed_default_templates(&store).unwrap();

        let loaded = store.load("web-app-dev").unwrap();
        assert_eq!(loaded.name, "내가 수정한 이름");
    }

    #[test]
    fn seeded_templates_pass_validation() {
        for template in default_templates() {
            crate::template::validate_template(&template).unwrap();
        }
    }
}
```

Add `pub mod seed;` to `src-tauri/src/template/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test template::seed`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement the seed templates and seeding function**

```rust
fn stage(id: &str, name: &str, prompt: &str, allowed_tools: &[&str]) -> Stage {
    Stage {
        id: id.to_string(),
        name: name.to_string(),
        prompt: prompt.to_string(),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: allowed_tools.iter().map(|s| s.to_string()).collect(),
        checkpoint: true,
    }
}

fn default_templates() -> Vec<Template> {
    vec![
        Template {
            id: "web-app-dev".to_string(),
            name: "웹 프로그램 개발".to_string(),
            description: "요구사항 정의부터 배포까지 웹 애플리케이션 개발 전체 파이프라인".to_string(),
            stages: vec![
                stage(
                    "requirements",
                    "요구사항 정의",
                    "PRD.md를 작성하라. project-architect 스킬을 사용해 프로젝트의 목적, 사용자, 핵심 기능, 성공 기준을 정리하라.",
                    &["Read", "Write", "Glob", "Grep", "WebSearch"],
                ),
                stage(
                    "design",
                    "아키텍처 설계",
                    "PRD.md를 참고해 시스템 아키텍처와 데이터 모델을 설계하고 ARCHITECTURE.md에 문서화하라. project-architect 스킬을 사용하라.",
                    &["Read", "Write", "Glob", "Grep"],
                ),
                stage(
                    "frontend",
                    "프론트엔드 구현",
                    "ARCHITECTURE.md를 참고해 프론트엔드 UI를 구현하라. frontend-craftsman 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Glob", "Grep", "Bash"],
                ),
                stage(
                    "backend",
                    "백엔드 구현",
                    "ARCHITECTURE.md를 참고해 백엔드 API를 구현하라. api-engineer 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Glob", "Grep", "Bash"],
                ),
                stage(
                    "testing",
                    "테스트",
                    "구현된 기능에 대한 테스트를 작성하고 실행하라. qa-tester 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Bash"],
                ),
                stage(
                    "deployment",
                    "배포 스크립트",
                    "빌드 및 배포 스크립트를 작성하라. devops-builder 스킬을 사용하라.",
                    &["Read", "Write", "Bash"],
                ),
            ],
        },
        Template {
            id: "desktop-app-dev-tauri".to_string(),
            name: "데스크톱 프로그램 개발 (Tauri)".to_string(),
            description: "요구사항 정의부터 패키징까지 Tauri 데스크톱 애플리케이션 개발 전체 파이프라인".to_string(),
            stages: vec![
                stage(
                    "requirements",
                    "요구사항 정의",
                    "PRD.md를 작성하라. project-architect 스킬을 사용해 프로젝트의 목적, 사용자, 핵심 기능, 성공 기준을 정리하라.",
                    &["Read", "Write", "Glob", "Grep", "WebSearch"],
                ),
                stage(
                    "design",
                    "아키텍처 설계",
                    "PRD.md를 참고해 Tauri(Rust 백엔드 + 프론트엔드) 아키텍처를 설계하고 ARCHITECTURE.md에 문서화하라. project-architect 스킬을 사용하라.",
                    &["Read", "Write", "Glob", "Grep"],
                ),
                stage(
                    "implementation",
                    "구현",
                    "ARCHITECTURE.md를 참고해 Tauri 프론트엔드와 Rust 백엔드를 구현하라. frontend-craftsman 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Glob", "Grep", "Bash"],
                ),
                stage(
                    "testing",
                    "테스트",
                    "구현된 기능에 대한 테스트를 작성하고 실행하라. qa-tester 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Bash"],
                ),
                stage(
                    "packaging",
                    "패키징",
                    "MSI/DMG/AppImage 인스톨 패키지를 생성하는 배포 스크립트를 작성하라. devops-builder 스킬을 사용하라.",
                    &["Read", "Write", "Bash"],
                ),
            ],
        },
    ]
}

pub fn seed_default_templates(store: &TemplateStore) -> Result<(), StoreError> {
    for template in default_templates() {
        if store.load(&template.id).is_err() {
            store.save(&template)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test template::seed`
Expected: PASS (3 tests).

- [ ] **Step 5: Call seeding at app startup**

Modify the `.setup()` closure in `src-tauri/src/lib.rs`, right after constructing `orchestrator` and before `app.manage(orchestrator)`:
```rust
            if let Err(e) = template::seed::seed_default_templates(&orchestrator.template_store) {
                eprintln!("failed to seed default templates: {e}");
            }
```

- [ ] **Step 6: Verify the app still compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: seed default pipeline templates on startup"
```

---

## Task 16: App Routing, Target Folder Picker & Manual Smoke Test

**Files:**
- Modify: `src/App.tsx`
- Create: `src/App.test.tsx` (replace Task 1's version)
- Modify: `src-tauri/Cargo.toml`, `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`, `package.json`

**Interfaces:**
- Consumes: `TemplateGallery` (Task 12), `TemplateEditor` (Task 13), `PipelineRun` (Task 14), `startPipelineRun` (Task 11).
- Produces: the fully wired application entry point.

- [ ] **Step 1: Add the dialog plugin dependency**

Add to `src-tauri/Cargo.toml`'s `[dependencies]`:
```toml
tauri-plugin-dialog = "2"
```

Add to `package.json`'s `dependencies`:
```json
    "@tauri-apps/plugin-dialog": "^2.0.0"
```

Run: `npm install`

- [ ] **Step 2: Register the plugin and grant its permission**

Modify `src-tauri/src/lib.rs` — add `.plugin(tauri_plugin_dialog::init())` to the builder chain, before `.setup(...)`:
```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
```

Modify `src-tauri/capabilities/default.json`'s `permissions` array to include the dialog permission:
```json
  "permissions": ["core:default", "dialog:allow-open"]
```

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 3: Write the failing App routing test**

Replace `src/App.test.tsx`:
```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("./api", () => ({
  listTemplates: vi.fn(),
  deleteTemplate: vi.fn(),
  checkCli: vi.fn(),
  saveTemplate: vi.fn(),
  startPipelineRun: vi.fn(),
  onStageEvent: vi.fn(),
  approveCheckpoint: vi.fn(),
  requestChanges: vi.fn(),
  rejectCheckpoint: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import { listTemplates, checkCli, startPipelineRun, onStageEvent } from "./api";
import { open } from "@tauri-apps/plugin-dialog";
import App from "./App";
import type { RunRecord, Template } from "./types";

const sample: Template = { id: "t1", name: "웹 프로그램 개발", description: "desc", stages: [] };

beforeEach(() => {
  vi.mocked(listTemplates).mockReset().mockResolvedValue([sample]);
  vi.mocked(checkCli).mockReset().mockResolvedValue("available:mock-claude 0.0.1");
  vi.mocked(onStageEvent).mockReset().mockResolvedValue(() => {});
  vi.mocked(open).mockReset();
  vi.mocked(startPipelineRun).mockReset();
});

describe("App routing", () => {
  it("starts on the template gallery", async () => {
    render(<App />);
    expect(await screen.findByText("웹 프로그램 개발")).toBeInTheDocument();
  });

  it("picks a target folder and starts a run when '불러와서 실행' is clicked", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/new-project");
    const runRecord: RunRecord = {
      runId: "run1",
      templateId: "t1",
      targetDir: "/tmp/new-project",
      status: "awaiting-checkpoint",
      currentStageIndex: 0,
      stages: [{ id: "s1", status: "awaiting-checkpoint", sessionId: "sess1", log: [] }],
    };
    vi.mocked(startPipelineRun).mockResolvedValue(runRecord);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "불러와서 실행" }));

    await waitFor(() => expect(open).toHaveBeenCalledWith({ directory: true }));
    await waitFor(() => expect(startPipelineRun).toHaveBeenCalledWith("t1", "/tmp/new-project"));
    expect(await screen.findByText("상태: awaiting-checkpoint")).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `npm run test`
Expected: FAIL — `App.tsx` does not yet render `TemplateGallery` or call `startPipelineRun`.

- [ ] **Step 5: Implement the routed `App`**

`src/App.tsx`:
```typescript
import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { startPipelineRun } from "./api";
import TemplateGallery from "./pages/TemplateGallery";
import TemplateEditor from "./pages/TemplateEditor";
import PipelineRun from "./pages/PipelineRun";
import type { RunRecord, Template } from "./types";

type View =
  | { name: "gallery" }
  | { name: "editor"; template: Template | null }
  | { name: "run"; run: RunRecord };

export default function App() {
  const [view, setView] = useState<View>({ name: "gallery" });

  const handleRun = async (template: Template) => {
    const targetDir = await open({ directory: true });
    if (!targetDir || Array.isArray(targetDir)) return;
    const run = await startPipelineRun(template.id, targetDir);
    setView({ name: "run", run });
  };

  if (view.name === "editor") {
    return (
      <TemplateEditor
        initial={view.template}
        onSaved={() => setView({ name: "gallery" })}
        onCancel={() => setView({ name: "gallery" })}
      />
    );
  }

  if (view.name === "run") {
    return <PipelineRun initialRun={view.run} onFinished={() => setView({ name: "gallery" })} />;
  }

  return (
    <TemplateGallery
      onRun={handleRun}
      onEdit={(template) => setView({ name: "editor", template })}
      onNew={() => setView({ name: "editor", template: null })}
    />
  );
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `npm run test`
Expected: PASS.

- [ ] **Step 7: Full automated verification**

Run: `cd src-tauri && cargo test`
Expected: PASS (all Rust unit + integration tests).

Run: `npm run test`
Expected: PASS (all frontend tests).

Run: `npm run build && cd src-tauri && cargo check`
Expected: both succeed.

- [ ] **Step 8: Manual smoke test (not automatable — requires a real, configured `claude` CLI)**

1. Ensure `claude --version` works in a terminal and the CLI is logged in / has API access configured.
2. Run: `npm run tauri dev` (from the repo root; requires `@tauri-apps/cli` from Task 1 — if `tauri` isn't on PATH, run `npx tauri dev`).
3. In the app, confirm the "웹 프로그램 개발" and "데스크톱 프로그램 개발 (Tauri)" seed templates appear with no CLI warning banner.
4. Click "편집" on the web template, temporarily reduce it to a single fast stage (e.g. prompt: "README.md 파일에 'hello' 라고만 적어라"), save.
5. Click "불러와서 실행", pick an empty temp folder.
6. Confirm the pipeline run view shows live log lines and, after the stage finishes, the checkpoint panel with 승인/거부/수정 요청 controls.
7. Click 승인 and confirm the run reaches `completed` and `README.md` was created in the target folder.
8. Restore the template's original stages (or delete the temp copy) afterward.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: wire app routing, target folder picker, and dialog plugin"
```
