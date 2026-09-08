# Claude Pipeline Wizard

A Tauri v2 + React/TypeScript desktop app that stores reusable, multi-stage
**pipeline templates** (e.g. "웹 프로그램 개발", "데스크톱 프로그램 개발 (Tauri)")
and runs them end-to-end against a newly created project folder — one stage
at a time, by shelling out to the [Claude Code](https://claude.com/claude-code)
CLI — pausing at per-stage checkpoints for human approval before continuing.

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
    executor.rs            Spawns the claude CLI per stage, streams events, enforces a timeout
    orchestrator.rs        One stage per start_stage call; owns every state transition
  commands.rs             Tauri commands exposed to the frontend
  bin/mock_claude.rs      Test-only stand-in for the claude CLI (used by the Rust test suite)

docs/superpowers/
  specs/                  Design spec
  plans/                  Implementation plan
```

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
