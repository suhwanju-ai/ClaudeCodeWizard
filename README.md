# Claude Pipeline Wizard

A Tauri v2 + React/TypeScript desktop app that stores reusable, multi-stage
**pipeline templates** (e.g. "웹 프로그램 개발", "데스크톱 프로그램 개발 (Tauri)")
and runs them end-to-end against a newly created project folder — one stage
at a time, by shelling out to the [Claude Code](https://claude.com/claude-code)
CLI — pausing at per-stage checkpoints for human approval before continuing.

## How it works

1. Pick a template from the gallery and choose (or create) an empty target folder.
2. Each stage runs as its own `claude --print --output-format stream-json` invocation,
   with `--resume <session_id>` chaining conversation context from one stage to the next
   so later stages know what earlier stages built.
3. When a stage finishes, the run pauses (if the stage has a checkpoint) and shows a
   live log plus **Approve / Request changes / Reject** controls.
4. Approving the last stage's checkpoint completes the run.

Templates, run records, and the two seed templates ("웹 프로그램 개발" and
"데스크톱 프로그램 개발 (Tauri)") are all plain JSON files — there's no embedded
database and no bundled AI SDK; the app is a thin, persistent orchestration layer
around the `claude` CLI you already have installed.

## Requirements

- [Node.js](https://nodejs.org/) 18+ and npm
- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain) and the
  [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS
- The [`claude` CLI](https://claude.com/claude-code) installed and available on
  your `PATH`, logged in with API access — the app shells out to it for every
  pipeline stage and will show a warning banner in the gallery if it can't find it

## Getting started

```bash
npm install
npx tauri dev
```

This starts the Vite dev server and launches the Tauri desktop window. On first
launch, the two seed templates are created automatically in the app's data
directory (editable and duplicable, just like any other template).

## Development

```bash
npm run dev       # Vite dev server only (frontend)
npm run build     # Type-check + production frontend build
npm run test      # Frontend tests (Vitest)
npx tsc --noEmit  # Frontend type-check only

cd src-tauri
cargo check       # Backend compile check
cargo test        # Backend unit + integration tests
cargo clippy --all-targets
```

## Project layout

```
src/                      React frontend
  pages/
    TemplateGallery.tsx   Browse, run, edit, delete templates
    TemplateEditor.tsx    Form-based stage editor (add/remove/reorder, validation)
    PipelineRun.tsx       Live stage log + checkpoint approval panel
  api.ts                  Tauri invoke()/listen() wrappers
  types.ts                TypeScript mirrors of the Rust IPC types

src-tauri/src/
  template/               Template/Stage types, file-backed CRUD store, seed templates
  engine/
    run_record.rs         Run state machine (RunRecord/StageRun, checkpoint transitions)
    stream_json.rs        Parser for the claude CLI's stream-json output
    executor.rs           Spawns the claude CLI per stage, streams events, enforces a timeout
    orchestrator.rs        Drives the checkpoint state machine end to end
  commands.rs             Tauri commands exposed to the frontend
  bin/mock_claude.rs      Test-only stand-in for the claude CLI (used by the Rust test suite)

docs/superpowers/
  specs/                  Design spec
  plans/                  Implementation plan
```

## Known v1 limitations

- **New folders only** — a template runs against a folder you pick, which must
  not already have a pipeline run in it. Running against an existing codebase
  isn't supported yet.
- **No cross-restart resume UI** — run state is persisted to disk per
  transition, but there's currently no command or screen to reattach to an
  existing run after the app restarts; recovery today means inspecting the
  JSON file under the app's `runs/` directory by hand.
- **Sequential stages only** — no parallel or branching stages within a
  template.
