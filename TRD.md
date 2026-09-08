# TRD — Claude Pipeline Wizard: 실행 전 단계 편집 게이트 & 프로젝트 로컬 매니페스트

- 문서 상태: **확정 (Approved)** — trd-reviewer 승인, 2026-09-08 (검토 3라운드 / 재작성 2회 후 승인). 검토 기록: `_workspace/p6_reviewer_trd-feedback.json`
- 대상 프로젝트: `D:\Utility\Agents\ClaudeCodeWizard`
- 대응 PRD: `D:\Utility\Agents\ClaudeCodeWizard\PRD.md` (확정, 2026-09-08 승인 — `_workspace/p4_reviewer_prd-feedback.json` `approved: true`)
- 커버 대상: PRD 3장 목표 A~F / 백로그 IMP-001 ~ IMP-022 (22건 전부)
- 스택: Tauri v2 + Rust 백엔드(`src-tauri/`), React 19 + TypeScript 프론트엔드(`src/`), Vitest + `cargo test`

> ### ⚠️ 이 TRD가 확정하지 않는 것
> PRD 6장(범위 제외)과 prd-reviewer의 `handoff_to_trd`에 따라, 아래 8개 항목은 **본 TRD가 임의로 결정하지 않는다.** 6장 "미해결 기술 질문"에 선택지와 구현 영향만 제시하며, 사용자 결정 전까지 해당 부분의 구현 스펙은 **미확정**이다.
>
> GAP-3(`Failed` 출구 간선) · IMP-015/GAP-4(매니페스트 쓰기 실패 정책) · IMP-018/GAP-6(기존 run.json 마이그레이션) · IMP-012(`persistTemplate` 저장-실행 결합) · IMP-013(실행 화면 '템플릿 편집' 버튼) · IMP-017(동일 targetDir 재실행) · IMP-019(targetDir 경로 검증) · IMP-020(피드백 프롬프트 감사 기록)
>
> 특히 **3.2절 상태 머신 전이표에 `Failed`로부터 나가는 간선은 그리지 않았다.** 이는 누락이 아니라 GAP-3이 미결이기 때문이며, 6.1절이 두 선택지를 제시한다.

---

## 1. 개요

### 1.1 이 TRD가 대응하는 PRD 목표

| PRD 목표 | 요약 | 이 TRD의 대응 섹션 |
|---|---|---|
| **A — 실행 모델** (PRD §3-목표A, §5.1) | 모든 단계가 실행 직전 `awaiting-stage-start`로 정지. 프로세스 스폰 경로를 `start_stage` 하나로 단일화 | §3.1 (IMP-002), §3.2 (IMP-001), §3.3 (IMP-004) |
| **B — 관측성** (PRD §3-목표B, §5.2) | `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json` 기록 | §3.4 (IMP-003) / §6.4·6.6·6.7 (IMP-017·019·020) |
| **C — 데이터 보전** (PRD §3-목표C, §5.3) | run 범위 편집이 원본 템플릿을 오염시키지 않음 | §3.5 (IMP-006), §3.6 (IMP-007) / §6.2·6.3 (IMP-012·013) |
| **D — 안정성** (PRD §3-목표D, §5.4) | 새 상태 머신이 `08e9144`류 상태 끼임을 재발시키지 않음 | §3.7 (IMP-014), §3.8 (IMP-016) / §6.5 (IMP-015), §6.8 (IMP-018) |
| **E — 사용성** (PRD §3-목표E, §5.5) | 실행 화면에서 다음 단계를 보고 고칠 수 있음 | §3.9 (IMP-008), §3.10 (IMP-009), §3.11 (IMP-010) |
| **F — 기반 정비** (PRD §3-목표F, §5.6) | 검증 경로·기준선·문서 정합 | §3.12 (IMP-005), §3.13 (IMP-011), §3.14 (IMP-021), §3.15 (IMP-022) |

### 1.2 이 TRD의 전제

1. **IMP-021(기준선 커밋)이 선행되어야 한다** (PRD §1.3, §4.4-1). 승계 대상 설계서 `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md`와 계획서 `docs/superpowers/plans/2026-09-07-pipeline-run-editing.md`가 git untracked인 한, PRD·TRD의 줄 번호 인용이 무효화된다. §3.14 참조.
2. **IMP-002(필드 추가)는 IMP-018(마이그레이션) 결정 없이 머지할 수 없다** (PRD §4.4-2). §3.1의 스펙은 확정이지만, **`resolved_stages`에 `#[serde(default)]`를 붙일지 여부는 §6.8의 결정에 종속**되며 그 결정 전까지 §3.1은 머지 금지다.
3. 본 TRD가 인용하는 모든 코드 줄 번호는 **2026-09-08 현재 워킹트리 기준**이다.

---

## 2. 현재 아키텍처 — 변경이 필요한 부분만

전체 아키텍처는 `documents/system-architecture.md`에 있으므로 재서술하지 않는다. 이번 변경이 건드리는 지점만 기술한다.

### 2.1 백엔드 실행 경로 (변경 대상)

```
src-tauri/src/
  lib.rs                     ← generate_handler! 커맨드 등록 (28-38행)
  commands.rs                ← Tauri 커맨드 9개, emit_stage_event (18-23행)
  template/mod.rs            ← Stage/Template/PermissionMode 모델, validate_template (53-73행), is_valid_id (46-51행)
  template/store.rs          ← TemplateStore::save = 통째 fs::write 덮어쓰기
  engine/mod.rs              ← pub mod executor/orchestrator/run_record/stream_json (4줄)
  engine/orchestrator.rs     ← Orchestrator, start_run/drive/approve_checkpoint/request_changes/reject_checkpoint
  engine/run_record.rs       ← RunStatus/StageStatus/StageRun/RunRecord/RunRecordStore
  engine/executor.rs         ← run_stage(): claude 프로세스 스폰 (유일한 하위 프로세스 경계, 이번에 변경 없음)
```

현재 구조의 문제 지점 4개:

**(a) 자동 전진 루프.** `orchestrator.rs:53-100`의 `drive()`는 `loop`로 `run_stage()`를 반복 호출하며, `stage.checkpoint == false`면 96-97행에서 `current_stage_index += 1; status = Running`으로 **정지 없이 다음 단계를 즉시 스폰**한다. 정지는 오직 `checkpoint == true`(85-88행), 실패(79-82행), 파이프라인 종료(92-95행) 세 경우뿐이다.

**(b) 프로세스 스폰 진입점이 3곳.** `start_run`(45행 `drive` 호출), `approve_checkpoint`(118행 `drive` 호출), `request_changes`(152행 `run_stage` 직접 호출). 즉 "실행 시작"과 "승인"이 각각 프로세스를 스폰한다.

**(c) 실행에 쓰인 stage 정의가 남지 않음.** `RunRecord`(`run_record.rs:36-45`)는 `template_id: String`만 갖고 stage 정의 사본이 없다. `StageRun`(27-34행)은 `{ id, status, session_id, log }`뿐이다. 그래서 `approve_checkpoint`(111행)와 `request_changes`(136행)는 매번 `template_store.load(&record.template_id)`로 **현재 시점의 원본 템플릿을 재로드**한다 — run 도중 템플릿이 수정되면 그 수정이 진행 중 run에 새어든다.

**(d) 저장소가 2개뿐.** `TemplateStore`(`app_data_dir/templates/`)와 `RunRecordStore`(`app_data_dir/runs/`, `run_record.rs:113-147`). targetDir에 무언가를 남기는 개념이 코드·문서 어디에도 없다.

### 2.2 프론트엔드 (변경 대상)

```
src/
  App.tsx                    ← View 유니온(10-13행), handleRun(63-84행) — 낙관적 pendingRun 생성(69-76행)
  api.ts                     ← invoke 래퍼 8개 + onStageEvent (41-43행)
  types.ts                   ← Rust 모델의 수작업 1:1 미러 (StageStatus 19행 / RunStatus 20행 / RunRecord 29-36행)
  pages/TemplateEditor.tsx   ← PERMISSION_MODES(12행)·KNOWN_TOOLS(13행)·ID_PATTERN(14행) 상수 보유,
                                stage 폼 JSX 인라인(206-294행), persistTemplate(92-105행)
  pages/PipelineRun.tsx      ← 실행 화면. awaiting-checkpoint 카드(191-229행), '템플릿 편집' 버튼(232-234행)
  pages/TemplateGallery.tsx  ← 직접 실행 버튼 없음(커밋 a246384로 제거됨)
  components/                ← ★ 디렉터리 자체가 존재하지 않음
```

**(e) `src/components/` 부재.** stage 편집 폼이 `TemplateEditor.tsx` 안에만 존재하며, 실행 화면이 재사용할 수 있는 형태가 아니다.

**(f) 낙관적 레코드 불일치.** `App.tsx:72-75`가 `status: "running"`, 각 stage `status: "pending"`으로 `pendingRun`을 만들고 즉시 `setView`한다. 새 모델에서는 실제 레코드가 절대 거치지 않는 상태다.

### 2.3 테스트 자산 (영향받음)

```
src-tauri/tests/orchestrator_test.rs  ← start_run 시그니처에 의존 (58, 72, 92, 114, 126, 180, 213행),
                                         reject 오류 변형 단언(133행)
src-tauri/tests/executor_test.rs      ← 변경 없음
src-tauri/tests/fixtures/             ← 템플릿 픽스처 (two-stage, hang-two-stage 등)
src/App.test.tsx, src/api.test.ts, src/pages/*.test.tsx (3개)
```

---

## 3. 변경 사항

각 항목은 PRD 백로그 `id` 기준이다. **[확정]** 표시 항목은 PRD가 명확히 서술한 내용을 코드 위치로 옮긴 것이고, **[미확정]** 표시 항목은 6장으로 이관된다.

### 3.1 IMP-002 [확정 / 단 §6.8 결정에 머지 종속] — 데이터 모델 확장

**대상 파일: `src-tauri/src/engine/run_record.rs`**

#### (1) 상태 열거형 확장

```rust
// run_record.rs:9-15 (RunStatus)
pub enum RunStatus {
    Running,
    AwaitingStageStart,   // ★ 신설 → 직렬화 "awaiting-stage-start"
    AwaitingCheckpoint,
    Completed,
    Failed,
    Cancelled,
}

// run_record.rs:17-25 (StageStatus)
pub enum StageStatus {
    Pending,              // 의미 불변: "아직 도달하지 않은 단계"
    AwaitingStart,        // ★ 신설 → 직렬화 "awaiting-start"
    Running,
    AwaitingCheckpoint,
    Approved,
    Failed,
}
```

기존 `#[serde(rename_all = "kebab-case")]`가 그대로 적용되므로 별도 attribute 불필요. 열거형 **값 추가는 기존 파일 읽기 방향으로 하위 호환**된다(기존 파일에 새 값이 없으므로).

#### (2) `RunRecord`에 `resolved_stages` 추가 — **§6.8 결정 대기 지점**

```rust
// run_record.rs:36-45
pub struct RunRecord {
    pub run_id: String,
    pub template_id: String,
    pub target_dir: String,
    pub status: RunStatus,
    pub current_stage_index: usize,
    pub stages: Vec<StageRun>,
    // ★ 신설. 직렬화 키 "resolvedStages" (구조체의 rename_all = "camelCase"에 의해)
    pub resolved_stages: Vec<crate::template::Stage>,
}
```

> ⚠️ **`#[serde(default)]` 부여 여부는 이 TRD가 결정하지 않는다.** PRD §4.4-2가 IMP-018과의 동시 결정을 요구하며, 필드를 그대로 넣으면 `app_data_dir/runs/`의 기존 `run.json`이 즉시 역직렬화 실패한다(`run_record.rs:145` `serde_json::from_str`). → **§6.8**

`use crate::template::Stage;`가 추가되며, `run_record.rs`가 `template` 모듈에 새로 의존하게 된다(현재는 `is_valid_id`만 import, 5행). 순환 의존은 없다 — `template` 모듈은 `engine`을 참조하지 않는다.

#### (3) 생성자·전이 메서드 변경

```rust
// run_record.rs:48 — 시그니처 변경: &[String] → &[Stage]
pub fn new(run_id: String, template_id: String, target_dir: String, stages: &[Stage]) -> Self
```
- `status`를 `RunStatus::Running`(53행) → **`RunStatus::AwaitingStageStart`**
- `stages[0].status`를 **`StageStatus::AwaitingStart`**, 나머지는 `Pending` 유지
- `resolved_stages: stages.to_vec()`

신설 메서드:

| 메서드 | 시그니처 | 동작 |
|---|---|---|
| `current_resolved_stage` | `(&self) -> &Stage` | `&self.resolved_stages[self.current_stage_index]` |
| `apply_stage_override` | `(&mut self, stage: Stage) -> Result<(), StageOverrideError>` | `stage.id != resolved_stages[idx].id`이면 `StageIdMismatch` 반환, 아니면 교체 |
| `begin_current_stage` | `(&mut self)` | 현 stage `Running`, run `Running` |
| `advance_to_gate` | `(&mut self, session_id: String) -> bool` | 현 stage `Approved`+session_id 기록 → 마지막이면 run `Completed`(true 반환), 아니면 `index += 1`, 다음 stage `AwaitingStart`, run `AwaitingStageStart`(false 반환) |

변경 메서드:
- **`approve_current`(84-94행)** — 88행 `self.status = RunStatus::Running`을 **`RunStatus::AwaitingStageStart`**로, 그리고 전진한 다음 stage의 status를 **`AwaitingStart`**로 설정. (PRD A-3 / 설계서 96-98행)
- `mark_current_awaiting_checkpoint`(71-75행), `mark_current_failed`(77-80행), `cancel`(96-98행) — 시그니처·동작 불변.

#### (4) 기존 단위 테스트 영향

`run_record.rs:150-247`의 `mod tests`는 헬퍼 `record()`(153-156행)가 `&[String]`을 넘기므로 **전부 수정 필요**. 특히:
- `new_starts_at_stage_zero_with_pending_stages`(158-165행) → 이름·단언을 `AwaitingStageStart` / `stages[0] == AwaitingStart` / 나머지 `Pending`으로 갱신
- `approve_current_advances_to_next_stage`(184-192행) → `status == AwaitingStageStart`, `stages[1].status == AwaitingStart` 단언으로 갱신

---

### 3.2 IMP-001 [확정] — 실행 전 편집 게이트 상태 머신

**대상 파일: `src-tauri/src/engine/orchestrator.rs`**

`drive()`(53-100행)의 `loop`를 **제거**한다. 자동 전진(96-97행)은 게이트 복귀로 대체된다. `drive`는 단일 단계만 실행하는 `run_current_stage(...)`로 축소된다(§3.3).

#### 상태 전이표

> **`Failed`로부터 나가는 간선은 이 표에 없다.** GAP-3 미결(§6.1).

| # | 출발 상태 | 트리거 | 도착 상태 | stage 상태 변화 | 프로세스 스폰 | 근거 |
|---|---|---|---|---|---|---|
| T1 | (없음) | `start_run` | `AwaitingStageStart` | `stages[0] = AwaitingStart`, 나머지 `Pending` | **없음** | IMP-004 / PRD A-1 |
| T2 | `AwaitingStageStart` | `start_stage(runId, expectedStageIndex, override?)` | `Running` | 현 stage `AwaitingStart → Running` | **있음 (유일)** | IMP-004 / PRD A-4 |
| T3 | `Running` | 단계 종료 `exit == 0`, `checkpoint == true` | `AwaitingCheckpoint` | 현 stage `AwaitingCheckpoint` + `session_id` | 없음 | 기존 동작 유지 |
| T4 | `Running` | 단계 종료 `exit == 0`, `checkpoint == false`, 다음 단계 있음 | **`AwaitingStageStart`** | 현 stage `Approved`, 다음 stage `AwaitingStart` | **없음** | ★ IMP-001 핵심 / PRD A-2 |
| T5 | `Running` | 단계 종료 `exit == 0`, 마지막 단계 | `Completed` | 현 stage `Approved` | 없음 | 기존 동작 유지 |
| T6 | `Running` | `exit != 0` 또는 스폰/IO 실패 | `Failed` | 현 stage `Failed` | 없음 | 기존 79-82행 |
| T7 | `AwaitingCheckpoint` | `approve_checkpoint`, 다음 단계 있음 | **`AwaitingStageStart`** | 현 stage `Approved`, 다음 stage `AwaitingStart` | **없음** | ★ IMP-001/004 / PRD A-3 |
| T8 | `AwaitingCheckpoint` | `approve_checkpoint`, 마지막 단계 | `Completed` | 현 stage `Approved` | 없음 | 기존 동작 유지 |
| T9 | `AwaitingCheckpoint` | `request_changes(feedback)` | `Running` | 현 stage `Running` | 있음 (재실행) | IMP-007 (§3.6) |
| T10 | `AwaitingCheckpoint` | `reject_checkpoint` | `Cancelled` | 변화 없음 | 없음 | 기존 171-179행 |
| T11 | **`AwaitingStageStart`** | **`cancel_run`** | **`Cancelled`** | 현 stage `AwaitingStart` 유지 | 없음 | ★ IMP-014 (§3.7) / PRD D-1 |
| — | `Completed` / `Cancelled` | — | (terminal) | — | — | |
| — | `Failed` | **미정** | **미정** | — | — | **GAP-3 → §6.1** |

> **세대 가드 각주 (T2)**: `expectedStageIndex`가 백엔드의 `currentStageIndex`와 불일치하면 **전이하지 않고** `StaleStageIndex`로 거부한다(§3.8). 이는 새로운 상태 전이가 아니라 **전이 거부**이므로 위 표에 행을 추가하지 않는다 — 거부 시 run의 상태·인덱스·stage 상태는 모두 그대로다. 마찬가지로 `status != AwaitingStageStart`인 호출은 `NotAwaitingStageStart`로 거부되며 역시 전이가 아니다.
>
> **세션 연속성 각주 (T2 / T4 / T7)**: T4·T7이 `AwaitingStageStart`로 복귀한 뒤 T2로 다음 단계를 스폰할 때, 그 단계는 **직전 단계의 Claude 세션을 `--resume`으로 이어받는다.** 즉 T3·T5·T7 행의 "기존 동작 유지"에는 세션 체이닝이 포함되며, T4가 대체하는 것은 `drive()`의 자동 전진뿐이고 세션 전달은 아니다. 공급원 규칙은 §3.3-(4a)에 확정 스펙으로 규정되어 있다.

#### 이 표가 보장하는 것 (PRD §5.1 성공 기준 대응)

- **A-1**: T1에 스폰 없음 — `start_run`이 `drive`를 호출하지 않는다.
- **A-2**: T4가 자동 전진을 대체 — N단계 파이프라인에서 T2가 정확히 N회 필요.
- **A-3**: T7이 `Running`이 아니라 `AwaitingStageStart`로 간다.
- **A-4**: 스폰이 T2·T9 두 트리거에서만 발생하고, 둘 다 `start_stage`/`request_changes` **단일 헬퍼 `run_current_stage()`**를 경유한다. `start_run`·`approve_checkpoint`에는 `run_stage` 호출이 남지 않는다.

> **명시적 비범위**: `Running` 상태에서의 사용자 취소 간선은 현재도 없으며 PRD가 요구하지 않으므로 신설하지 않는다. 실행 중 취소가 필요하다는 판단이 서면 별도 항목으로 올린다.

---

### 3.3 IMP-004 [확정] — `start_stage` 신설 및 실행 경로 단일화

**대상 파일: `src-tauri/src/engine/orchestrator.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/api.ts`**

#### (1) `OrchestratorError` 확장 (`orchestrator.rs:9-19`)

```rust
pub enum OrchestratorError {
    TemplateStore(#[from] TemplateStoreError),
    RunRecordStore(#[from] RunRecordStoreError),
    Io(#[from] std::io::Error),
    NotAwaitingCheckpoint(String),
    // ★ 신설
    #[error("run '{0}' is not awaiting a stage start")]
    NotAwaitingStageStart(String),
    #[error("stage override id '{got}' does not match resolved stage '{expected}'")]
    StageIdMismatch { expected: String, got: String },
    // ★ 신설 — 세대(generation) 가드. §3.8
    #[error("stage already advanced: run is at stage {actual}, caller expected {expected}")]
    StaleStageIndex { expected: usize, actual: usize },
    #[error(transparent)]
    StageValidation(#[from] crate::template::TemplateValidationError),
    #[error("failed to write project manifest: {0}")]
    ProjectManifest(String),
    #[error("run '{0}' cannot be cancelled in its current state")]
    NotCancellable(String),      // IMP-014, §3.7
}
```

#### (2) `start_run` — 동기화 + 비실행화 (`orchestrator.rs:32-48` 전면 교체)

```rust
pub fn start_run(
    &self,
    template_id: &str,
    target_dir: PathBuf,
    run_id: String,
) -> Result<RunRecord, OrchestratorError>
```

- `async`, `on_event: F` 제거 — 이벤트가 발생할 일이 없다.
- 동작: `template_store.load` → `validate_template`(선택) → `RunRecord::new(run_id, template.id, target_dir_string, &template.stages)` → `create_dir_all(&target_dir)` → `self.save(&record)` (§3.4의 이중 쓰기 헬퍼).
- **45행 `self.drive(...)` 호출을 삭제**한다. 이것이 PRD A-1의 핵심.
- 43-44행의 "타겟 디렉토리 비어있음 검사는 범위 밖" 주석이 있는 자리가 **IMP-017의 결정 삽입 지점**이다(§6.4).

#### (3) `start_stage` 신설

```rust
pub async fn start_stage<F: FnMut(&str, StageEvent)>(
    &self,
    run_id: &str,
    expected_stage_index: usize,   // ★ 세대 가드 (§3.8)
    stage_override: Option<Stage>,
    mut on_event: F,
) -> Result<RunRecord, OrchestratorError>
```

순서(선례 `0235a40`의 "스폰 전 Running을 디스크에 먼저 저장" 방어를 승계).

> **[확정] 락 범위 = 1~5단계 (load-check-save)뿐이다.** §3.8의 run별 뮤텍스는 1단계 진입 시 획득하고 **5단계의 `save`가 끝나는 즉시 해제**한다. **자식 프로세스 await(6단계)를 가로질러 뮤텍스를 잡지 않으며, 7·8단계(게이트 복귀 전이·저장)도 락 밖에서 수행한다.** 이유: 단계 실행은 수 분이 걸릴 수 있고, 같은 락을 `approve_checkpoint`/`request_changes`/`cancel_run`도 사용하므로(§3.8) 락을 6단계 이후까지 유지하면 실행 중인 run의 **취소조차 블로킹**된다 — 이는 IMP-014(§3.7)가 만들려는 탈출구를 무력화한다. 이 범위 확정은 §3.8의 세대 가드 서사와 §5.2 T-D4a/T-D4b의 전제이며, 세 곳이 동일한 전제를 공유한다.

1. `let mut record = self.run_store.load(run_id)?;`
2. 가드 (i): `record.status != RunStatus::AwaitingStageStart` → `NotAwaitingStageStart`
2b. 가드 (ii) — **세대 가드**: `record.current_stage_index != expected_stage_index` → `StaleStageIndex { expected: expected_stage_index, actual: record.current_stage_index }`. 근거와 필요성은 §3.8.
3. `if let Some(o) = stage_override`:
   a. `validate_stage(&o)?` (§3.12) → 실패 시 `StageValidation`
   b. `record.apply_stage_override(o)?` → id 불일치 시 `StageIdMismatch`
4. `record.begin_current_stage();`
5. `self.save(&record)?;` ← **§6.5(IMP-015)의 정책 삽입 지점.** 여기서 매니페스트 쓰기가 실패하면 `run_store`에는 이미 `Running`이 기록된 채 에러만 올라간다.
   — **여기까지가 락 구간이다. 5단계가 끝나면 뮤텍스 가드를 `drop`한다.**
6. (락 밖) `let resume = Self::resume_session_id_for(&record);` → `let outcome = self.run_current_stage(&mut record, resume, &mut on_event).await;`
   **`resume`의 값은 §3.3-(4a)가 확정 스펙으로 규정한다.** 구현자가 정책을 발명해서는 안 된다.
7. (락 밖) 결과 반영: `Ok(0)` → `checkpoint`에 따라 T3 또는 T4/T5, 그 외 → `record.mark_current_failed()` (T6). **`run_current_stage`가 `Err`를 반환해도 `?`로 조기 반환하지 않고 반드시 `mark_current_failed`로 흡수**한다 — `08e9144` 선례(`approve_checkpoint` 118-120행의 `.is_err()` 방어)와 동일한 패턴.
8. (락 밖) `self.save(&record)?;` → `Ok(record)`

> **락 밖 6~8단계가 안전한 이유**: 5단계가 디스크에 `Running`을 확정한 뒤 락이 풀리므로, 그 사이에 도착하는 다른 `start_stage` 호출은 락을 얻더라도 1단계에서 `Running`을 읽고 가드 (i)에 걸려 `NotAwaitingStageStart`로 거부된다. 즉 **"프로세스 2개 스폰 없음"(PRD D-4)은 뮤텍스 + 상태 가드만으로 이미 보장**되며, 세대 가드는 그 뒤 8단계까지 끝나 게이트로 복귀한 **이후에 도착하는 호출**을 담당한다(§3.8).

3-a의 override 검증에 **`validate_stage`가 필요**하므로 IMP-005(§3.12)가 IMP-004의 선행 조건이다.

#### (4) `run_current_stage` — 단일 단계 실행 헬퍼 (`drive` 대체)

```rust
async fn run_current_stage<F: FnMut(&str, StageEvent)>(
    &self,
    record: &mut RunRecord,
    resume_session_id: Option<String>,
    on_event: &mut F,
) -> Result<i32, OrchestratorError>
```

- 실행 대상 stage를 **`record.current_resolved_stage()`**에서 읽는다 (기존 63행 `&template.stages[stage_index]`가 아니다) — 이것이 IMP-006/007의 기반.
- `run_stage(&self.executor_config, stage, target_dir, resume_session_id, ...)` 1회 호출.
- 로그 수집·`latest_session_id` 추출 로직은 기존 65-77행을 그대로 이식.
- `loop` 없음. 종료 코드만 반환하고 상태 전이 판단은 호출자에게 맡긴다.

#### (4a) `resume_session_id`의 공급원 [확정 스펙]

`drive()`를 제거하면 현행 세션 체이닝 경로(비체크포인트 단계 종료 시 `orchestrator.rs:98`이 `resume_session_id = Some(session_id)`로 다음 단계에 세션을 넘기고, `approve_checkpoint`가 `orchestrator.rs:112`에서 체크포인트 단계의 `session_id`를 읽어 `drive`에 넘기는 것 — 118행)가 함께 사라진다. **그 경로를 대체하는 규칙을 여기서 확정한다.** 이 절의 목적은 현행 동작(단계 간 Claude 컨텍스트 연속성)을 **정확히 그대로 보존**하는 것이며, 새로운 동작을 도입하지 않는다.

**규칙 (a) — `start_stage` → `run_current_stage` (T2 경로, 즉 T4/T7로 게이트에 복귀한 뒤 사용자가 다음 단계를 실행하는 모든 경우)**

`start_stage`는 `run_current_stage`를 호출하기 직전에 다음 헬퍼로 `resume_session_id`를 계산해 넘긴다.

```rust
impl Orchestrator {
    /// 직전 단계의 Claude 세션을 이어받는다. 첫 단계는 새 세션으로 시작한다.
    fn resume_session_id_for(record: &RunRecord) -> Option<String> {
        let idx = record.current_stage_index;
        if idx == 0 {
            None
        } else {
            record.stages[idx - 1].session_id.clone()
        }
    }
}
```

- `current_stage_index > 0` → `record.stages[current_stage_index - 1].session_id.clone()`
- `current_stage_index == 0` → `None`
- 직전 단계의 `session_id`가 어떤 이유로 `None`이면 `None`이 그대로 전달된다 — 현행 `drive()`가 `latest_session_id`를 못 얻었을 때와 동일한 동작이다.

> **각주 — 완주한 단계의 `session_id`는 실제로 `None`이 되지 않는다.** 현행 `drive()`가 `latest_session_id.unwrap_or_default()`(`orchestrator.rs:84`)로 빈 문자열을 만들고 91행이 그것을 `Some("")`으로 기록하므로, 세션 id를 산출하지 못한 단계도 `None`이 아니라 **`Some("")`**로 남는다. 그 경우 다음 단계에는 `--resume ""`이 전달된다. **이는 현행 동작과 정확히 동일하므로 이번 변경의 회귀가 아니다** — 규칙 (a)는 값을 그대로 옮길 뿐 해석하지 않는다. `unwrap_or_default()`를 없애 빈 세션을 `None`으로 정규화하는 것은 관찰 동작을 바꾸는 개선이므로 **이번 범위 밖의 별도 항목**으로 둔다.

이 규칙이 성립하는 이유: 게이트 대기 시점에는 `stages[current_stage_index - 1].session_id`가 **항상 채워져 있다.** 기록 주체는 경로에 따라 다르다 — **비체크포인트 단계는 `advance_to_gate`(§3.1-(3))**가 그 단계를 `Approved`로 마감하면서 `session_id`를 `StageRun`에 기록하고, **체크포인트 단계는 그보다 앞서 `mark_current_awaiting_checkpoint`(`run_record.rs:71-75`)**가 이미 기록해 둔다(`approve_current`(`run_record.rs:84-94`)는 session_id를 기록하지 않고 `Approved` 마감과 `current_stage_index += 1`만 수행한다). 어느 경로든 인덱스가 전진하는 시점에는 직전 단계의 세션이 남아 있으므로, `stages[current_stage_index - 1].session_id`는 항상 "직전 단계가 실제로 사용한 Claude 세션"이다. 데이터는 이미 남아 있었고, 이 TRD는 그것을 **읽는 주체를 `drive()`에서 `start_stage`로 옮길 뿐**이다.

**규칙 (b) — `request_changes` (T9 경로): 변경 없음**

`request_changes`(`orchestrator.rs:126-169`)는 **같은 단계를 재실행**하므로 현행대로 **자기 stage의 세션**, 즉 `record.stages[record.current_stage_index].session_id.clone()`을 `resume_session_id`로 사용한다. 규칙 (a)의 헬퍼를 쓰지 않는다(그것은 직전 단계를 가리키므로 틀린 값이다). §3.6은 145-147행의 순서 유지만 언급했으나, 세션 인자에 대해서도 **현행 동작을 그대로 승계**한다는 것을 여기서 명시한다.

**규칙 (c) — 그 외**

`start_run`(T1)·`approve_checkpoint`(T7)·`cancel_run`(T11)·`reject_checkpoint`(T10)는 프로세스를 스폰하지 않으므로 `resume_session_id`를 다루지 않는다. 세션을 읽는 지점은 위 (a)·(b) 두 곳뿐이다.

> **결론**: 단계 간 컨텍스트 연속성은 **현행과 동일하게 유지된다.** 이는 사용자에게 보이는 동작 변화가 아니므로 §4.4의 변화 목록에 항목으로 추가하지 않고, §4.4 말미에 "변하지 않는 것"으로 명시한다. 고정 테스트는 §5.2 T-A5.

#### (5) `approve_checkpoint` 비실행화 (`orchestrator.rs:102-124`)

- 111행 `template_store.load(&record.template_id)` **삭제** (IMP-007과 동시에 해소되는 결합).
- 116-121행의 `current_stage_mut().status = Running` + `drive(...)` 블록을 **삭제**하고, `record.approve_current()`가 반환한 결과에 따라 `save` 후 즉시 반환한다.
- 결과: `approve_checkpoint`는 완전 동기 로직이 되지만, 커맨드 시그니처 호환을 위해 함수는 `async`로 유지해도 무방하다(권장: 동기화하고 `commands.rs`에서 맞춘다).

#### (6) Tauri 커맨드 (`src-tauri/src/commands.rs`)

- `start_pipeline_run`(54-67행): `async fn` → **`fn`**, `app: AppHandle` 파라미터 제거(이벤트 없음), `State<'_, Orchestrator>` → `State<Orchestrator>`.
- **신설**:
```rust
#[tauri::command]
pub async fn start_stage(
    app: AppHandle,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
    expected_stage_index: usize,                       // ★ 세대 가드 (§3.8)
    stage_override: Option<crate::template::Stage>,
) -> Result<RunRecord, String> {
    orchestrator
        .start_stage(&run_id, expected_stage_index, stage_override, |stage_id, event| {
            emit_stage_event(&app, &run_id, stage_id, event);
        })
        .await
        .map_err(|e| e.to_string())
}
```
- `pipeline://stage-event` 발행 책임이 `start_pipeline_run`에서 `start_stage`로 이동한다. `emit_stage_event`(18-23행)는 변경 없음.
- **신설**: `cancel_run` (§3.7).

#### (7) 등록 (`src-tauri/src/lib.rs:28-38`)

`generate_handler!` 배열에 `commands::start_stage`, `commands::cancel_run`, `commands::get_run`(§3.11 — `StaleStageIndex` 복구용 읽기 커맨드) 추가.

---

### 3.4 IMP-003 [확정 — 단 정책 3건은 §6] — 프로젝트 로컬 매니페스트

**신설 파일: `src-tauri/src/engine/project_manifest.rs`**
**수정: `src-tauri/src/engine/mod.rs` (`pub mod project_manifest;` 1줄 추가)**

```rust
use std::path::{Path, PathBuf};
use crate::template::{Stage, Template};
use super::run_record::RunRecord;

pub const MANIFEST_DIR_NAME: &str = ".claude-pipeline-wizard";

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestError {
    #[error("io error: {0}")] Io(#[from] std::io::Error),
    #[error("json error: {0}")] Json(#[from] serde_json::Error),
    // ★ IMP-019 결정 시 여기에 InvalidTargetDir 변형이 추가될 수 있음 → §6.6
}

pub fn manifest_dir(target_dir: &Path) -> PathBuf {
    target_dir.join(MANIFEST_DIR_NAME)
}

/// 모든 상태 전이마다 run.json / pipeline.json 두 파일을 기록한다.
pub fn write_project_manifest(
    target_dir: &Path,
    record: &RunRecord,
) -> Result<(), ProjectManifestError> {
    let dir = manifest_dir(target_dir);
    std::fs::create_dir_all(&dir)?;
    // run.json — RunRecord 전체 (resolvedStages 포함)
    std::fs::write(dir.join("run.json"), serde_json::to_string_pretty(record)?)?;
    // pipeline.json — resolved_stages 기반 Template 스냅샷
    let snapshot = Template {
        id: record.template_id.clone(),
        name: record.template_id.clone(),   // 이름은 RunRecord에 없음 — 아래 주 참조
        description: String::new(),
        stages: record.resolved_stages.clone(),
    };
    std::fs::write(dir.join("pipeline.json"), serde_json::to_string_pretty(&snapshot)?)?;
    Ok(())
}
```

> **주**: `RunRecord`에 템플릿 `name`/`description`이 없어 `pipeline.json`의 그 두 필드를 채울 소스가 없다. 세 가지 처리(빈 문자열 / `template_id` 재사용 / `RunRecord`에 `template_name` 추가) 중 어느 쪽도 PRD가 규정하지 않으나, **관측 가능한 동작(사람이 폴더를 열어보는 것)에 실질 영향이 크지 않고 §6로 올릴 정책 결정 수준은 아니라고 판단**해 위와 같이 `template_id` 재사용으로 둔다. 리뷰어가 이견이 있으면 조정 대상이다.

#### 이중 쓰기 헬퍼 (`orchestrator.rs`)

`run_store.save`와 매니페스트 쓰기를 항상 함께 수행하는 단일 진입점을 둔다. 상태 전이 코드가 이 함수만 부르게 해서 "전이마다 매니페스트 갱신"(PRD B-1)을 구조적으로 보장한다.

```rust
impl Orchestrator {
    fn save(&self, record: &RunRecord) -> Result<(), OrchestratorError> {
        self.run_store.save(record)?;                       // 기존 app_data_dir/runs/ — 유지 (PRD B-2)
        let target = PathBuf::from(&record.target_dir);
        write_project_manifest(&target, record)
            .map_err(|e| OrchestratorError::ProjectManifest(e.to_string()))   // ★ §6.5 정책 삽입 지점
    }
}
```

호출 지점: `start_run` 말미, `start_stage` 5·8단계, `approve_checkpoint`, `request_changes`(전/후 2회), `reject_checkpoint`, `cancel_run`. 기존 `self.run_store.save(&record)?` 호출(47, 117, 122, 147, 167, 177행)을 전부 `self.save(&record)?`로 치환한다.

**PRD B-2 보장**: `run_store.save`를 먼저 호출하므로 `app_data_dir/runs/<runId>.json`은 대체되지 않고 계속 기록된다.

**미확정 3건** — 이 모듈에 다음이 아직 없다:
- 동일 targetDir 재실행 시 기존 매니페스트 처리 → **§6.4 (IMP-017)**
- `target_dir` 정규화/검증 → **§6.6 (IMP-019)**
- 쓰기 실패 시 정책(현재 코드는 `?`로 전파) → **§6.5 (IMP-015)**

---

### 3.5 IMP-006 [확정] — run 범위 편집 격리

**대상: `src-tauri/src/engine/orchestrator.rs` (쓰기 경로 없음을 유지), `src-tauri/tests/orchestrator_test.rs` (불변식 테스트 신설)**

설계상 격리는 §3.3-(3)-3-b에서 이미 성립한다 — `apply_stage_override`는 `record.resolved_stages`만 건드리고, `Orchestrator`의 어떤 실행 경로도 `self.template_store.save(...)`를 호출하지 않는다. 현재 `template_store.save`를 호출하는 유일한 지점은 `commands.rs:36-38`의 `save_template` 커맨드뿐이다.

**PRD C-2가 요구하는 것은 "이 불변식을 고정하는 자동화 테스트"**이므로, 다음 통합 테스트를 신설한다(§5.2 T-C2).

---

### 3.6 IMP-007 [확정] — `request_changes`를 `resolvedStages` 기준으로

**대상: `src-tauri/src/engine/orchestrator.rs:126-169`**

```rust
// 136행 삭제:  let template = self.template_store.load(&record.template_id)?;
// 138행 교체:
let mut feedback_stage = record.current_resolved_stage().clone();
feedback_stage.prompt = feedback.to_string();
```

- 재실행 stage의 `permission_mode` / `allowed_tools` / `checkpoint`가 **pre-start 게이트에서 편집된 값 그대로** 상속된다 (PRD C-3).
- 원본 템플릿을 재로드하지 않으므로 run 도중 템플릿 편집이 새어들지 않는다 (PRD C-4). `approve_checkpoint`의 111행 재로드 삭제(§3.3-(5))와 함께 이 결합이 완전히 해소된다.
- `feedback_stage.prompt` 덮어쓰기는 **로컬 클론에만** 적용되고 `record.resolved_stages`에는 기록하지 않는다(설계서 102-108행). 그 결과 실제 실행 프롬프트가 어디에도 남지 않는 문제 → **§6.7 (IMP-020)**.
- 145-147행의 `status = Running` + `save` 순서는 유지(0235a40 방어).

---

### 3.7 IMP-014 [확정] — `awaiting-stage-start`에서의 취소 경로

**대상: `src-tauri/src/engine/orchestrator.rs:171-179`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/api.ts`, `src/pages/PipelineRun.tsx`**

선례 `08e9144`가 고친 버그(가드가 좁아 run이 디스크에 끼임)의 재발을 막는다. PRD D-1/D-2가 "실행 전 게이트에서 멈춘 run을 거부/취소할 수 있어야 한다"고 명시하므로 이 전이(T11)는 확정 사항이다.

**백엔드** — 기존 `reject_checkpoint`를 유지한 채 가드를 넓힌 `cancel_run`을 신설한다:

```rust
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

> 기존 `reject_checkpoint`(171-179행)는 `AwaitingCheckpoint` 전용 가드를 그대로 두고 `cancel_run`에 위임하도록 하거나, 그대로 남긴다. `orchestrator_test.rs:133`이 `NotAwaitingCheckpoint` 변형을 단언하므로 **`reject_checkpoint`의 오류 변형을 바꾸지 않는 편**이 기존 테스트를 보존한다.

**커맨드**: `commands.rs`에 `cancel_run(orchestrator, run_id) -> Result<RunRecord, String>` 추가, `lib.rs`의 `generate_handler!`에 등록.

**프론트**: `src/api.ts`에 `cancelRun(runId): Promise<RunRecord>` 추가. `src/pages/PipelineRun.tsx`의 `awaiting-stage-start` 카드(§3.10)에 **"이 run 취소"** 버튼을 배치하고 `handleReject`와 동일한 `busy` 가드·`onFinished()` 처리를 적용한다. → PRD D-2 충족(액션이 '실행' 하나뿐인 상황 소거).

---

### 3.8 IMP-016 [확정] — `start_stage` load-check-save 구간 원자성

**대상: `src-tauri/src/engine/orchestrator.rs`, `src-tauri/tests/orchestrator_test.rs`**

§3.3-(3)의 1~5단계는 `run_store.load` → 가드 → `save` 사이에 어떤 상호배제도 없어 두 개의 동시 `start_stage` 호출이 같은 레코드를 읽고 각각 프로세스를 스폰할 수 있다. 단계별 수동 실행 버튼 도입으로 사용자가 실행을 트리거하는 **횟수 자체가 늘어나므로** `0235a40`이 막으려던 "동일 폴더 이중 claude 프로세스" 위험이 커진다.

**설계**: `Orchestrator`에 run별 비동기 뮤텍스를 둔다.

```rust
pub struct Orchestrator {
    pub template_store: TemplateStore,
    pub run_store: RunRecordStore,
    pub executor_config: ExecutorConfig,
    // ★ 신설: run_id -> 실행 직렬화 락
    run_locks: std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>,
}
```

`start_stage` / `request_changes` / `approve_checkpoint` / `cancel_run` 진입 시 해당 run의 `Arc<tokio::sync::Mutex<()>>`를 얻어 `.lock().await`를 유지한 채 **load-check-save 구간만** 수행한다. `Orchestrator::new`(28-30행) 시그니처는 유지하고 `run_locks`를 기본값으로 초기화한다.

> **[확정] 락 범위** — §3.3-(3)의 확정과 동일하다: **1~5단계(load → 가드 → override 적용 → `begin_current_stage` → `save`)까지만** 락을 유지하고, **자식 프로세스 실행(6단계)과 게이트 복귀 전이·저장(7·8단계)은 락 밖**에서 수행한다. 스폰을 락 안에 넣지 않는 이유는 §3.3-(3)의 주에 적었다(수 분짜리 실행이 같은 run의 `cancel_run`을 블로킹하게 되어 §3.7의 탈출구가 무력화된다). 이 문서의 §3.3 / §3.8 / §5.2는 **전부 이 하나의 범위**를 전제한다.

#### 동시 호출은 뮤텍스 + 상태 가드가 막는다

두 개의 `start_stage`가 동시에 도착하면, 먼저 락을 잡은 호출이 5단계에서 디스크에 `Running`을 확정하고 락을 놓는다. 뒤이어 락을 얻은 호출은 1단계에서 그 `Running`을 읽고 가드 (i)에 걸려 **`NotAwaitingStageStart`로 거부**된다. 프로세스는 1개만 스폰된다 → **PRD D-4는 이 두 장치(뮤텍스 + 상태 가드)만으로 충족된다.** 세대 가드는 이 경로에 관여하지 않는다. 고정 테스트는 §5.2 **T-D4a**.

#### 세대(generation) 가드가 막는 것 — 게이트 복귀 후 도착하는 지연·중복 호출

세대 가드가 필요한 상황은 **동시성이 아니라 시간차**다. 락이 이미 풀리고 단계가 끝나 run이 T4/T7로 **다음 단계의 게이트(`AwaitingStageStart`, index = 1)로 복귀한 뒤**, 이전 화면 상태에 기반한 **구식 IPC 호출**이 뒤늦게 도착하는 경우다. 구체적으로:

- 사용자가 '이 단계 실행'을 빠르게 두 번 눌러 두 번째 클릭 이벤트가 `disabled={busy}`가 걸리기 전/풀린 뒤에 새어 나간 경우,
- 프론트가 `setRun(updated)`로 화면을 갱신하기 전에(또는 갱신 실패·네트워크 지연으로) 옛 `run.currentStageIndex`를 들고 재시도가 발사된 경우.

이 호출은 `expected_stage_index = 0`을 들고 오지만 백엔드는 이미 index = 1의 게이트에 있다. **상태 가드는 이것을 잡지 못한다** — `status == AwaitingStageStart`가 참이기 때문이다. 상태값은 "지금 게이트에 있는가"만 보고 "**어느 단계의** 게이트인가"는 보지 않는다. 가드가 없으면 사용자가 확인·편집하지 않은 **stage 2가 스폰되고**, 이는 PRD A-2("사용자가 실행을 명시적으로 지시한 횟수가 N회와 일치")와 게이트의 취지를 백엔드 레벨에서 우회한다.

**채택안 (선택지 A — 위치 기반 세대 가드)**: `start_stage`가 호출자로부터 `expected_stage_index: usize`를 받아, **락을 획득한 뒤** `record.current_stage_index != expected_stage_index`이면 즉시 `OrchestratorError::StaleStageIndex { expected, actual }`로 거부한다(§3.3-(3) 가드 2b). 사용자가 화면에서 본 단계와 백엔드가 지금 실행하려는 단계가 다르면 실행하지 않는다는 뜻이며, 이는 "본 것을 실행한다"는 게이트의 계약 그 자체다. 고정 테스트는 §5.2 **T-D4b**(순차 호출로 결정적으로 재현된다 — 경합 타이밍에 의존하지 않는다).

전달 경로:

- `commands.rs`의 `start_stage` 커맨드가 `expected_stage_index: usize`를 그대로 받아 전달한다(§3.3-(6)).
- `src/api.ts`의 `startStage(runId, expectedStageIndex, stageOverride?)`가 인자를 넓힌다(§3.11).
- `src/pages/PipelineRun.tsx`의 `handleStartStage`가 **화면이 렌더한 그 시점의 `run.currentStageIndex`**를 넘긴다(§3.10). `stageDraft`와 같은 렌더 사이클의 값이므로, 사용자가 실제로 보고 있던 단계를 정확히 지목한다.

> **왜 `stage_override`의 id 검사에 의존하지 않는가**: 실무적으로는 §3.10의 `handleStartStage`가 항상 `stageDraft`를 override로 보내므로 `apply_stage_override`의 id 불일치 검사가 세대 가드 역할을 겸한다. 그러나 (1) `stage_override`가 `Option`이라 백엔드 계약상 **없이도 호출 가능**하고, (2) 서로 다른 두 단계가 같은 `id`를 갖는 것을 `validate_template`이 금지하더라도 이 보호는 프론트 구현 디테일에 의존하는 **우연한 보호**다. 세대 가드는 override 유무와 무관하게 백엔드 단독으로 성립해야 하므로, `expected_stage_index`를 **필수 인자**로 둔다. `apply_stage_override`의 id 검사는 그대로 유지되며 이중 방어로 남는다.

**한계 명시**: 이는 **단일 프로세스 내** 직렬화일 뿐 앱을 두 번 띄운 경우나 외부 프로세스와의 경합은 막지 못한다. targetDir 수준의 파일 락은 이번 범위 밖으로 두되, PRD D-4가 요구하는 "백엔드 레벨 동시 호출에서 프로세스가 2개 스폰되지 않음"은 **뮤텍스 + 상태 가드**로 §5.2 T-D4a에, 그와 별개인 "게이트 복귀 후 구식 호출이 다음 단계를 스폰하지 않음"은 **세대 가드**로 §5.2 T-D4b에 각각 고정된다. 두 장치는 서로 다른 실패 시나리오를 담당하며 겹치지 않는다.

---

### 3.9 IMP-008 [확정] — 공유 `StageFields` 컴포넌트

**신설 파일: `src/components/StageFields.tsx`** (디렉터리 `src/components/`도 함께 신설)
**수정: `src/pages/TemplateEditor.tsx`**

```tsx
export const PERMISSION_MODES: PermissionMode[] = ["acceptEdits", "bypassPermissions", "default"];
export const KNOWN_TOOLS = ["Read", "Write", "Edit", "Glob", "Grep", "Bash", "WebSearch", "WebFetch"];

interface Props {
  stage: Stage;
  onChange: (patch: Partial<Stage>) => void;
  idPrefix: string;         // input id 충돌 방지 ("stage-0" / "run-stage")
  promptLabel: string;      // "단계 3 프롬프트" / "이 단계 프롬프트"
  lockId: boolean;          // true면 id 입력을 표시하되 disabled
}
export default function StageFields({ stage, onChange, idPrefix, promptLabel, lockId }: Props)
```

- `TemplateEditor.tsx:12-13`의 `PERMISSION_MODES`·`KNOWN_TOOLS` 상수를 이 파일로 **이동**하고 `TemplateEditor.tsx`는 재-import한다. `ID_PATTERN`(14행)은 템플릿 레벨 검증에만 쓰이므로 `TemplateEditor.tsx`에 남긴다.
- `TemplateEditor.tsx:206-294`의 인라인 폼(이름/ID/프롬프트/권한 모드/허용 도구/체크포인트 스위치)을 `<StageFields ... lockId={false} idPrefix={`stage-${selectedIndex}`} />`로 치환한다. 단계 삭제 버튼(201-203행)과 위/아래 이동(296-311행)은 템플릿 편집 고유 기능이므로 컴포넌트 밖에 남긴다.
- `lockId`는 **id 입력을 숨기지 않고 `disabled`로만 둔다**(PRD E-4). run 도중 stage id가 바뀌면 `StageRun.id`·`resolvedStages` 정합이 깨지고 백엔드 `StageIdMismatch`에 걸린다.
- 기존 `id` 속성 명명 규칙(`stage-name-${i}` / `stage-id-${i}` / `stage-prompt-${i}`)은 `idPrefix`로 재구성되므로 **`src/pages/TemplateEditor.test.tsx`의 셀렉터를 함께 갱신**해야 한다.

---

### 3.10 IMP-009 [확정] — 실행 화면 편집 패널 + 낙관적 레코드 정합화

**대상: `src/pages/PipelineRun.tsx`, `src/App.tsx`**

#### (1) `PipelineRun.tsx`

`awaiting-checkpoint` 카드(191-229행)와 **형제**로 `awaiting-stage-start` 카드를 추가한다:

```tsx
const [stageDraft, setStageDraft] = useState<Stage | null>(null);

useEffect(() => {
  if (run.status === "awaiting-stage-start") {
    setStageDraft(run.resolvedStages[run.currentStageIndex] ?? null);
  } else {
    setStageDraft(null);
  }
}, [run.status, run.currentStageIndex, run.resolvedStages]);   // PRD E-3
```

```tsx
{run.status === "awaiting-stage-start" && stageDraft && (
  <div className="card">
    <div className="section-label">다음 단계 — 실행 전 확인/수정</div>
    <StageFields stage={stageDraft} onChange={(p) => setStageDraft(s => s && {...s, ...p})}
                 idPrefix="run-stage" promptLabel="이 단계 프롬프트" lockId />
    <button className="btn btn-success" onClick={handleStartStage} disabled={busy}>이 단계 실행</button>
    <button className="btn btn-danger-outline" onClick={handleCancelRun} disabled={busy}>이 run 취소</button>  {/* IMP-014 */}
  </div>
)}
```

- `handleStartStage`는 `setBusy(true)` → `await startStage(run.runId, run.currentStageIndex, stageDraft)` → `setRun(updated)` → `finally setBusy(false)` 패턴을 기존 `handleApprove`(106-120행)와 동일하게 따른다. **`disabled={busy}`가 `dec3360`의 이중 클릭 가드 승계**이고, 두 번째 인자 `run.currentStageIndex`가 §3.8의 세대 가드에 넘기는 값이다 — `stageDraft`와 같은 렌더 사이클의 값이므로 사용자가 화면에서 본 그 단계를 지목한다.
- **`StaleStageIndex` 수신 시 처리**: `startStage`가 `StaleStageIndex`로 실패하면 **에러 알림을 띄우지 않는다.** 이 오류는 정의상 "프론트의 `run` 상태가 낡았다(백엔드가 이미 다음 단계로 전진했다)"는 뜻이므로, 에러 문구를 보여주면 사용자는 낡은 단계를 화면에 둔 채 막히게 된다. 대신 **최신 `RunRecord`를 재조회해 `setRun(fresh)`**하고(`await getRun(run.runId)` — §3.11에서 신설), 그러면 위 (1)의 `useEffect`가 `stageDraft`를 실제 현재 단계 값으로 다시 잡아준다. 즉 사용자에게 보이는 결과는 "**화면이 실제 단계로 갱신됨**"뿐이다. 그 밖의 오류(`StageValidation`, `NotAwaitingStageStart` 등)는 기존대로 에러를 표시한다. `disabled={busy}` 가드 때문에 정상 UI 조작으로는 거의 도달하지 않는 복구 경로이지만, 도달했을 때의 동작을 여기서 확정한다. → 사용자에게 보이는 결과가 화면 갱신에 그치므로 **§4.4(릴리스 노트)에는 항목을 추가하지 않는다.**
- `stageDotClass`(57-62행)에 `awaiting-start` 케이스, `statusBadgeClass`(64-70행)에 `awaiting-stage-start` 케이스를 추가한다.
- 74행 `stageNameById`는 현재 `template.stages`에서 만들지만, run 범위 편집으로 이름이 바뀔 수 있으므로 **`run.resolvedStages` 기준으로 변경**한다(`template` prop 폴백 유지).

#### (2) `App.tsx` — 낙관적 레코드 (69-76행)

```tsx
const pendingRun: RunRecord = {
  runId, templateId: template.id, targetDir,
  status: "awaiting-stage-start",                                    // ← "running" 에서 변경
  currentStageIndex: 0,
  stages: template.stages.map((s, i) => ({
    id: s.id,
    status: i === 0 ? "awaiting-start" : "pending",                  // ← 전부 "pending" 에서 변경
    sessionId: null, log: [],
  })),
  resolvedStages: template.stages,                                   // ← 신설 필드
};
```
→ PRD E-1(실제 레코드가 거치지 않는 `status:'running'`이 화면에 잠깐 보이지 않음) 충족.

`handleRun`(63-84행)의 나머지 흐름(`open({directory:true})` → `startPipelineRun`)은 유지된다. 다만 `handleRun`의 **호출 지점**은 IMP-012 결정에 종속된다(§6.2).

---

### 3.11 IMP-010 [확정] — 프론트 타입·API 래퍼 동기화

**대상: `src/types.ts`, `src/api.ts`**

```ts
// types.ts:19-20
export type StageStatus = "pending" | "awaiting-start" | "running" | "awaiting-checkpoint" | "approved" | "failed";
export type RunStatus = "running" | "awaiting-stage-start" | "awaiting-checkpoint" | "completed" | "failed" | "cancelled";

// types.ts:29-36 — RunRecord
export interface RunRecord {
  runId: string; templateId: string; targetDir: string;
  status: RunStatus; currentStageIndex: number; stages: StageRun[];
  resolvedStages: Stage[];      // ★ 신설
}
```

```ts
// api.ts
// expectedStageIndex는 §3.8의 세대 가드에 넘기는 필수 인자다.
export function startStage(
  runId: string,
  expectedStageIndex: number,
  stageOverride?: Stage,
): Promise<RunRecord> {
  return invoke("start_stage", { runId, expectedStageIndex, stageOverride: stageOverride ?? null });
}
export function cancelRun(runId: string): Promise<RunRecord> {
  return invoke("cancel_run", { runId });
}

// ★ 신설 — StaleStageIndex 복구용 읽기 경로(§3.10). 현재 api.ts에는 run을 조회하는
//   함수가 없다(모든 RunRecord가 변경 커맨드의 반환값으로만 들어온다).
export function getRun(runId: string): Promise<RunRecord> {
  return invoke("get_run", { runId });
}
```

`getRun`을 위해 `commands.rs`에 `get_run(orchestrator, run_id) -> Result<RunRecord, String>`(내부적으로 `run_store.load`를 감싼 읽기 전용 커맨드)를 추가하고 `lib.rs`의 `generate_handler!`에 등록한다(§3.3-(7)에 함께 기재). 상태를 바꾸지 않으므로 뮤텍스(§3.8)를 잡지 않는다.

**리스크 명시**: `src/types.ts`는 Rust 모델(`run_record.rs`, `template/mod.rs`)의 **수작업 1:1 미러**다. 컴파일 타임에 어긋남을 잡아주는 장치가 없어, 이번처럼 양쪽 열거형이 동시에 늘어날 때 드리프트가 발생하기 쉽다. `ts-rs`/`specta` 등으로 타입을 생성하는 것이 근본 해법이나 **이번 범위 밖**으로 두고 리스크로만 기록한다(PRD IMP-010 서술과 동일).

---

### 3.12 IMP-005 [확정] — `validate_stage` 분리

**대상: `src-tauri/src/template/mod.rs:53-73`**

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
    if !is_valid_id(&template.id) { return Err(TemplateValidationError::InvalidId(template.id.clone())); }
    if template.stages.is_empty() { return Err(TemplateValidationError::NoStages); }
    let mut seen = std::collections::HashSet::new();
    for stage in &template.stages {
        validate_stage(stage)?;                       // ← 62-67행을 위임
        if !seen.insert(stage.id.clone()) {
            return Err(TemplateValidationError::DuplicateStageId(stage.id.clone()));
        }
    }
    Ok(())
}
```

**검사 순서 보존이 중요하다.** 현재 코드는 stage마다 `InvalidId` → `EmptyPrompt` → `DuplicateStageId` 순서로 검사하며(62-70행), 위 리팩터링은 이 순서를 그대로 유지한다. `NoStages`·`DuplicateStageId`는 템플릿 레벨에 남는다. → **PRD F-1: `validate_template`의 관찰 가능한 동작 불변, `template/mod.rs:99-160`의 기존 테스트 10개가 무수정 통과해야 한다.**

`validate_stage`는 `orchestrator.rs`의 `start_stage`(§3.3-(3)-3-a)가 호출한다.

---

### 3.13 IMP-011 [확정] — README 갱신 + 수동 스모크

**대상: `README.md`**

- **'How it works'** — "체크포인트 단계에서만 멈춘다"는 서술을 **"모든 단계가 실행 전에 멈추고, 사용자가 확인·편집한 뒤에만 실행된다"**로 수정.
- **'Project layout'** — `src/components/StageFields.tsx`, `src-tauri/src/engine/project_manifest.rs` 추가.
- **'Known v1 limitations'** — "프로젝트 로컬 매니페스트는 조회를 쉽게 할 뿐 resume을 해결하지 않는다" 항목 추가. 기존 'No cross-restart resume UI' 항목은 **유지**(GAP-1이 미결이므로 제거하지 않는다).

수동 스모크 5항목(`npx tauri dev`)은 §5.4에 검증 절차로 기술한다.

---

### 3.14 IMP-021 [확정] — 기준선 커밋

**대상: 리포지토리 상태**

1. `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md`와 `docs/superpowers/plans/2026-09-07-pipeline-run-editing.md` 2개 파일만 담은 커밋을 **첫 커밋으로** 만든다.
2. installer bump-version 부산물(`package.json`, `src-tauri/Cargo.toml`, `Cargo.lock`, `src-tauri/tauri.conf.json`)은 **별도 커밋**으로 분리한다.
3. 이후 구현 커밋을 쌓는다.

**이 순서가 지켜지지 않으면 PRD·TRD의 모든 줄 번호 인용이 무효**가 되므로, 구현 착수 전 게이트로 취급한다(PRD §4.4-1).

---

### 3.15 IMP-022 [확정] — 문서 갱신

**대상: `documents/` 하위 8종 + 포털**

| 파일 | 갱신 내용 |
|---|---|
| `documents/database-design.md` | **프로젝트 로컬 매니페스트를 세 번째 저장소로 추가.** 현재 "저장소는 `templates/`와 `runs/` 두 개"라는 서술(1-70행)을 3개로 정정하고, `<targetDir>/.claude-pipeline-wizard/{run.json, pipeline.json}`의 스키마·수명주기·`app_data_dir/runs/`와의 관계(대체 아님, 추가)를 기술. `RunRecord`에 `resolvedStages` 필드 추가 반영. 2.4절(마이그레이션 도구 없음)에 §6.8 결정 결과를 반영. |
| `documents/system-architecture.md` | 정지 지점 기술(124-131행) 정정 — "유일한 정지 지점은 체크포인트 사후" → "모든 단계 실행 전 + 체크포인트 사후". `start_stage` 커맨드와 `project_manifest` 모듈 추가. |
| `documents/api-design.md` | `start_stage`, `cancel_run` 커맨드 추가. `start_pipeline_run`이 프로세스를 스폰하지 않게 됨을 반영. |
| `documents/sequence-diagrams.md` | run 시작 → 게이트 → `start_stage` → 게이트 복귀 흐름의 시퀀스 다이어그램 추가/교체. |
| `documents/class-diagram.md` / `er-diagram.md` | `RunRecord.resolvedStages`, 신규 상태 2개, `project_manifest` 모듈 반영. |
| `documents/user-guide.md` | 246행의 "README Known v1 limitations 참조"가 가리키는 내용이 §3.13에서 바뀌므로 함께 검토. 단계별 실행 조작 절차 추가. |
| `documents/api-user-guide.md` | `startStage`/`cancelRun` 프론트 API 추가. |
| `documents/docs-portal.html` | 위 8종의 갱신 내용을 반영해 재생성(`dev-docs-builder-v2` 산출물). |

---

## 4. 마이그레이션 / 롤아웃

### 4.1 기존 데이터에 대한 영향 — **미확정 (§6.8)**

이번 변경이 만드는 데이터 영향은 **하나**다: `RunRecord`에 `resolved_stages` 필수 필드가 추가되면 `app_data_dir/runs/*.json`의 기존 파일이 `run_record.rs:145`의 `serde_json::from_str`에서 실패한다. 열거형 값 추가(`AwaitingStageStart`/`AwaitingStart`)는 읽기 방향으로 호환되므로 문제가 아니다.

**이 마이그레이션 전략은 §6.8의 결정 대상이며, 결정 전까지 §3.1-(2)를 머지할 수 없다.** 결정에 따라 롤아웃 절차가 달라지므로, 아래 4.2는 결정 이후 확정되는 부분과 무관하게 성립하는 순서만 기술한다.

### 4.2 롤아웃 순서 (권장)

각 단계 끝에서 `cargo test` + `npm test`가 통과해야 다음으로 넘어간다.

| # | 단계 | 항목 | 롤백 |
|---|---|---|---|
| 0 | **기준선 커밋** | IMP-021 | — (문서 커밋만) |
| 1 | `validate_stage` 분리 | IMP-005 | 단일 커밋 revert. 관찰 동작 불변이므로 독립적으로 안전. |
| 2 | `StageFields` 추출 | IMP-008 | 단일 커밋 revert. 순수 프론트 리팩터링. |
| 3 | **§6.8 결정 반영** + 데이터 모델 확장 | IMP-002, IMP-018 | ★ 여기부터 기존 run 레코드에 영향. revert 시 이미 새 형식으로 저장된 레코드는 구버전이 읽지 못한다 — **4.3 참조**. |
| 4 | 매니페스트 모듈 + 이중 쓰기 | IMP-003 (+ §6.4·6.5·6.6 결정) | revert 시 targetDir에 남은 `.claude-pipeline-wizard/`는 고아가 되지만 앱 동작에 영향 없음(읽는 코드가 없다). |
| 5 | 상태 머신 + `start_stage` | IMP-001, IMP-004, IMP-007 | 3·4와 함께 revert해야 정합. |
| 6 | 취소 경로 + 동시성 | IMP-014, IMP-016 | 독립 revert 가능. |
| 7 | 프론트 타입·API·화면 | IMP-010, IMP-009 (+ §6.2·6.3 결정 시 IMP-012·013) | 5 없이 7만 배포하면 `start_stage` 커맨드 부재로 런타임 실패 — **5 이후에만 배포**. |
| 8 | 불변식 테스트 고정 | IMP-006 | — |
| 9 | README + 문서 | IMP-011, IMP-022 | — |

### 4.3 롤백 시 데이터 비대칭 (경고)

3단계 이후 생성된 `run.json`은 `resolvedStages` 키를 포함한다. serde는 기본적으로 **모르는 키를 무시**하므로 구버전 `RunRecord`가 이 파일을 읽는 것 자체는 성공하지만, `status: "awaiting-stage-start"` 값은 구버전 `RunStatus`에 없어 **역직렬화가 실패**한다. 즉 3~5단계 롤백은 그 사이에 생성된 run 레코드를 구버전에서 읽을 수 없게 만든다. 롤백 절차에 "해당 기간 run 레코드 파일 이동/삭제"를 포함해야 한다.

### 4.4 사용자에게 보이는 동작 변화 (릴리스 노트 대상)

1. 파이프라인을 시작해도 **아무것도 실행되지 않고** 1단계 확인 화면이 먼저 뜬다.
2. `checkpoint: false` 단계도 이제 **매번 실행 전에 멈춘다.** N단계 파이프라인은 N번의 '이 단계 실행' 클릭이 필요하다. → PRD GAP-2(조작 부담)가 미결이므로, "남은 단계 일괄 실행" 같은 완화 경로는 이번 릴리스에 없다.
3. 체크포인트 승인이 다음 단계를 **즉시 실행하지 않는다.**
4. 실행 대상 폴더에 `.claude-pipeline-wizard/` 디렉터리가 생긴다. → **GAP-5(`.gitignore` 처리·완료 후 정리·민감 프롬프트 잔존)가 미결**이므로 릴리스 노트에 "이 폴더가 생긴다"는 사실만 알리고 정리 정책은 안내하지 않는다.

**변하지 않는 것 (릴리스 노트에 '변경 없음'으로 명시)**: **단계 간 Claude 세션 연속성.** 2번째 이후 단계는 여전히 직전 단계의 세션을 `--resume`으로 이어받으며(§3.3-(4a) 규칙 (a)), `request_changes` 재실행도 여전히 자기 단계의 세션을 이어받는다(규칙 (b)). 이것이 §3.2 전이표 T2·T4·T7이 "기존 세션 연속성 유지"를 만족한다고 단언할 수 있는 근거다 — `drive()` 제거로 사라진 것은 자동 전진뿐이고, 세션 전달 책임은 `start_stage`가 그대로 승계한다. 따라서 "단계마다 새 Claude 세션으로 시작한다"는 동작 변화는 **이번 릴리스에 없다.**

---

## 5. 테스트 전략

### 5.1 도구

- Rust 단위: `src-tauri/src/**` 내 `#[cfg(test)] mod tests` — `cargo test`
- Rust 통합: `src-tauri/tests/*.rs` + `src-tauri/tests/fixtures/` (mock claude 바이너리 기반)
- 프론트: Vitest + Testing Library — `npm test`
- 수동: `npx tauri dev` (실 Claude CLI)

### 5.2 신설/변경 테스트 (PRD 성공 기준 ↔ 테스트 대응)

#### Rust 단위 — `src-tauri/src/engine/run_record.rs`

| ID | 테스트 | 검증 대상 |
|---|---|---|
| U-1 | `new_starts_awaiting_stage_start_with_stage0_awaiting` | A-5 / IMP-002 |
| U-2 | `new_stores_resolved_stages_copy` | IMP-002 |
| U-3 | `approve_current_advances_to_awaiting_start_gate` | A-3 / IMP-002 |
| U-4 | `apply_stage_override_replaces_only_current_index` | IMP-006 |
| U-5 | `apply_stage_override_rejects_id_mismatch` | IMP-004 |
| U-6 | `begin_current_stage_sets_running_on_stage_and_run` | IMP-002 |
| U-7 | `serializes_new_statuses_as_kebab_case` (`"awaiting-stage-start"` / `"awaiting-start"`) | A-5 |

#### Rust 단위 — `src-tauri/src/template/mod.rs`

| ID | 테스트 | 검증 대상 |
|---|---|---|
| U-8 | `validate_stage_rejects_empty_prompt` / `..._invalid_id` / `accepts_valid_stage` | F-1 / IMP-005 |
| U-9 | **기존 10개 테스트(99-160행)가 무수정 통과** | F-1 (관찰 동작 불변) |

#### Rust 통합 — `src-tauri/tests/orchestrator_test.rs` (기존 파일 전면 개정)

| ID | 테스트 | 검증 대상 |
|---|---|---|
| T-A1 | `start_run_spawns_nothing_and_returns_awaiting_stage_start` — mock 바이너리 호출 횟수 0 | **A-1** |
| T-A2 | `non_checkpoint_stage_completion_returns_to_gate` — `checkpoint:false` 2단계 픽스처로 `start_stage` 1회 후 상태가 `AwaitingStageStart`, index 1 | **A-2** |
| T-A3 | `approve_checkpoint_does_not_spawn_and_returns_to_gate` | **A-3** |
| T-A4 | `full_pipeline_requires_n_start_stage_calls` — N단계 완주에 정확히 N회 | **A-2** |
| T-A5 | `start_stage_resumes_previous_stage_session` — 아래 상세 | **§3.3-(4a) 규칙 (a)** / T2·T4·T7 세션 연속성 |
| T-A6 | `request_changes_resumes_own_stage_session` — 체크포인트에서 `request_changes` 시 mock 인자에 **자기 단계**의 `session_id`가 `--resume`으로 전달됨(직전 단계의 것이 아님) | **§3.3-(4a) 규칙 (b)** |
| T-B1 | `manifest_files_written_and_updated_on_each_transition` — 전이 전후 `run.json`/`pipeline.json` 해시 비교 | **B-1** |
| T-B2 | `app_data_dir_run_record_still_written_alongside_manifest` | **B-2** |
| T-C1 | `full_run_never_writes_template_store` — 실행 전후 `templates/{id}.json` mtime+바이트 동일 | **C-1** |
| T-C2 | `stage_override_does_not_touch_saved_template` — override로 프롬프트 변경 후 완주, `templates/{id}.json` **바이트 단위 동일** | **C-2** |
| T-C3 | `request_changes_inherits_pre_start_edits` — override로 `permission_mode`/`allowed_tools`/`checkpoint` 변경 → 체크포인트에서 `request_changes` → 재실행에 그 값이 쓰였는지 mock이 기록한 인자로 검증 | **C-3** |
| T-C4 | `template_edited_mid_run_does_not_leak_into_run` — run 진행 중 `template_store.save`로 원본 변경 → 이후 단계가 `resolvedStages` 값으로 실행 | **C-4** |
| T-D1 | `cancel_run_from_awaiting_stage_start_sets_cancelled` + 디스크 레코드 확인 | **D-1** |
| T-D4a | `concurrent_start_stage_spawns_only_one_process` — 동시 2호출 경합. 아래 상세 | **D-4** (뮤텍스 + 상태 가드) |
| T-D4b | `stale_stage_index_is_rejected_after_gate_advance` — 게이트 복귀 후 구식 index로 재호출. 아래 상세 | **D-4** (세대 가드, §3.8) |
| T-F1 | `start_stage_rejects_invalid_override` (빈 프롬프트 / 잘못된 id) | F-1 / IMP-004 |
| T-E1 | `start_stage_on_non_gate_status_returns_not_awaiting_stage_start` | IMP-004 가드 |

**T-A5 상세 — `start_stage_resumes_previous_stage_session`**

- **픽스처**: `checkpoint:false`인 2단계 템플릿(`two-stage` 계열). mock claude 바이너리는 stage 1에서 알려진 `session_id`(예: `sess-stage-1`)를 stream-json으로 내보내고, **호출될 때마다 받은 argv 전체를 파일로 덤프**하도록 확장한다(기존 mock의 인자 덤프 기능을 재사용/추가).
- **절차**: `start_run` → `start_stage(run_id, 0, None)` (stage 1 실행, T4로 게이트 복귀) → `start_stage(run_id, 1, None)` (stage 2 실행).
- **단언**:
  1. 1번째 호출의 argv 덤프에 `--resume`이 **없다** (`current_stage_index == 0` → `None`).
  2. 2번째 호출의 argv 덤프에 `--resume sess-stage-1`이 **있다** — 즉 `record.stages[0].session_id`가 그대로 전달됐다.
  3. 디스크의 `run.json`에서 `stages[0].sessionId == "sess-stage-1"`.
- **이 테스트가 고정하는 것**: `drive()` 제거가 단계 간 세션 체이닝을 **무너뜨리지 않았다는 것**. 이 단언이 없으면 구현자가 `resume_session_id`에 `None`을 넣어도 다른 어떤 테스트도 실패하지 않으며, 파이프라인의 단계 간 컨텍스트 연속성이 조용히 사라진다.
- 체크포인트를 경유하는 경로(T7)도 같은 성질을 가지므로, `checkpoint:true` 2단계 픽스처로 `approve_checkpoint` → `start_stage(run_id, 1, None)` 시 `--resume`이 전달되는지를 같은 테스트의 두 번째 케이스로 둔다.

**T-D4는 두 개의 독립 테스트로 분리한다.** 두 테스트는 서로 다른 방어 장치를 검증하며, 하나가 다른 하나를 대체하지 못한다. 분리의 근거는 §3.3-(3)이 확정한 락 범위(1~5단계, 자식 프로세스 await를 가로지르지 않음)다 — 이 범위에서 동시 호출은 **상태 가드**에, 지연 호출은 **세대 가드**에 걸린다.

**T-D4a 상세 — `concurrent_start_stage_spawns_only_one_process` (뮤텍스 + 상태 가드)**

- **픽스처 (고정)**: **`checkpoint:false`인 2단계 템플릿** — T-A2가 쓰는 것과 동일한 형태. (T-D4b와 픽스처를 공유해 두 테스트가 같은 조건 위에서 비교되게 한다.)
- **절차**: `start_run` 직후, 동일 `run_id`에 대해 `start_stage(run_id, 0, None)`을 **2개의 태스크로 거의 동시에 호출**하고 `join`한다.
- **기대값**: mock 바이너리 **총 호출 횟수 = 1**. 한 호출은 `Ok(record)`, 나머지 한 호출은 **`Err(OrchestratorError::NotAwaitingStageStart(run_id))`**.
- **왜 `NotAwaitingStageStart`인가**: 락 범위가 1~5단계이므로, 먼저 락을 잡은 호출은 5단계에서 디스크에 `status = Running`을 확정한 **직후 락을 놓고** 그 다음에 프로세스를 스폰한다. 두 번째 호출은 곧바로 락을 얻지만 1단계에서 `Running`을 읽고 가드 (i)에 걸린다. 첫 호출이 아직 실행 중이므로 게이트 복귀(T4)는 일어나지 않았고, 따라서 `current_stage_index`는 여전히 0이다.
- **세대 가드는 이 테스트에서 실행되지 않는다** — 가드 (i)에서 이미 반환되므로 가드 (ii)에 도달하지 않는다. 이 사실을 **테스트 주석으로 명시**해서, 이 테스트가 세대 가드의 커버리지가 아님을 독자가 오해하지 않게 한다. 세대 가드의 검증은 T-D4b가 담당한다.
- **음성 대조**: 뮤텍스를 제거하면 두 호출이 모두 `AwaitingStageStart`를 읽고 각각 스폰해 호출 횟수가 2가 된다 — 즉 이 테스트가 고정하는 것은 **뮤텍스의 존재**다.

**T-D4b 상세 — `stale_stage_index_is_rejected_after_gate_advance` (세대 가드)**

- **픽스처 (고정)**: T-D4a와 동일한 **`checkpoint:false` 2단계 템플릿**. 이 픽스처가 필수인 이유는, 1단계가 `checkpoint:true`이거나 단일 단계인 픽스처에서는 stage 0 완주 후 상태가 `AwaitingCheckpoint`/`Completed`가 되어 **상태 가드만으로 거부**되고 세대 가드에 도달하지 않기 때문이다. `checkpoint:false` + 후속 단계가 있어야만 완주 후 상태가 **다시 `AwaitingStageStart`**(단, index는 1)가 되어, 상태 가드를 통과하고 세대 가드가 판정하는 유일한 경로가 열린다.
- **절차 (순차, 경합 없음)**:
  1. `start_run` → `start_stage(run_id, 0, None)`을 **끝까지 await**한다. stage 0이 완주하고 T4로 `current_stage_index = 1`인 게이트에 복귀한다(반환된 record로 확인).
  2. 그 다음 **순차적으로** `start_stage(run_id, 0, None)`을 한 번 더 호출한다 — 즉 게이트가 이미 전진한 뒤 **구식 `expected_stage_index = 0`**을 들고 도착하는 지연·중복 호출을 재현한다(§3.8의 중복 클릭 / 늦게 도착한 재시도 시나리오).
- **기대값**: **`Err(OrchestratorError::StaleStageIndex { expected: 0, actual: 1 })`**. mock 바이너리 **총 호출 횟수 = 1**(stage 0의 1회뿐, stage 1은 스폰되지 않음). 디스크 레코드의 `status`는 `AwaitingStageStart`, `current_stage_index`는 1로 **변하지 않는다**(거부는 전이를 만들지 않는다, §3.2 각주).
- **이것이 세대 가드가 실제로 발동하는 유일한 경로다**: 2단계 호출은 상태 가드 (i)를 **통과한다**(`status == AwaitingStageStart`가 참이다). 오직 가드 (ii)의 `record.current_stage_index(1) != expected_stage_index(0)` 비교만이 이 호출을 막는다.
- **음성 대조 (권장, 리뷰 시 수동 1회 검증)**: **세대 가드(가드 2b)를 제거하면 이 테스트의 2단계 호출이 상태 가드를 통과해 stage 1을 스폰하므로 mock 호출 횟수가 2가 되고 테스트가 실패한다.** 즉 이 테스트는 가드를 빼면 반드시 깨지며, 세대 가드가 실제로 일하고 있음을 증명한다. (같은 대조를 T-D4a에 적용하면 여전히 1회이므로 참이 아니다 — T-D4a는 상태 가드가 막는 경로이기 때문이다.)
- **이 테스트가 T-D4a보다 안정적인 이유**: 경합 타이밍에 의존하지 않는 **결정적 순차 테스트**다. CI에서 flaky하지 않으며, 세대 가드의 회귀를 확실히 잡는다.

기존 테스트 중 `start_run_stops_at_first_checkpoint`(55행), 72·92·114·126·180·213행의 `start_run(...)` 호출은 시그니처 변경으로 **전부 개정**된다. 특히 `approve_checkpoint_marks_run_failed_instead_of_stuck_running_on_drive_timeout`(163행)은 `drive` 제거로 무의미해지므로, **동등한 보호를 `start_stage` 대상으로 재작성**한다(`run_current_stage` 실패가 `mark_current_failed`로 흡수되는지) — `08e9144` 선례 보호를 잃지 않기 위한 필수 조치다.

**미검증 성공 기준**: B-3(IMP-017) / B-4(IMP-019) / B-5(IMP-020) / C-5(IMP-013) / D-3(IMP-015) / D-5(IMP-018)에 대응하는 테스트는 **§6의 결정이 내려진 뒤에 설계**한다. 결정 전에 테스트를 쓰면 그 테스트가 곧 임의 결정이 된다.

#### 프론트 — Vitest

| ID | 파일 | 테스트 | 검증 대상 |
|---|---|---|---|
| F-E1 | `src/App.test.tsx` | 낙관적 `pendingRun`이 `awaiting-stage-start` / `stages[0]='awaiting-start'` / `resolvedStages` 포함 | **E-1** |
| F-E3 | `src/pages/PipelineRun.test.tsx` | `currentStageIndex` 전진 시 편집 초안이 다음 단계 값으로 교체되고 이전 초안이 남지 않음 | **E-3** |
| F-E4a | `src/pages/PipelineRun.test.tsx` | `StageFields`가 렌더되고 stage id 입력이 **표시되지만 disabled** | **E-4** |
| F-E4b | `src/pages/TemplateEditor.test.tsx` | 동일 `StageFields`를 사용하며 id 입력이 편집 가능 | **E-4** |
| F-E5 | `src/api.test.ts` | `startStage`가 `invoke("start_stage", { runId, expectedStageIndex, stageOverride })`를 호출(§3.8 세대 가드 인자 포함), `cancelRun`이 `invoke("cancel_run", ...)` 호출 | **E-5** |
| F-E6 | `src/pages/PipelineRun.test.tsx` | `startStage`가 `StaleStageIndex`로 reject되면 **에러 문구가 표시되지 않고** `getRun`이 호출되어 화면이 최신 `currentStageIndex`로 갱신됨(§3.10) | IMP-009 / §3.8 |
| F-D1 | `src/pages/PipelineRun.test.tsx` | `awaiting-stage-start` 카드에 '이 단계 실행'과 '이 run 취소'가 **둘 다** 존재 | **D-2** |
| F-D4 | `src/pages/PipelineRun.test.tsx` | '이 단계 실행' 클릭 직후 버튼이 `disabled`(이중 클릭 가드, `dec3360` 승계) | IMP-009 |

`src/pages/TemplateEditor.test.tsx`는 §3.9의 input id 규칙 변경으로 셀렉터 갱신이 필요하다.

### 5.3 회귀 방지 관점 (선례 버그 대응)

| 선례 커밋 | 원 문제 | 이번 변경에서의 대응 | 고정 테스트 |
|---|---|---|---|
| `08e9144` | 좁은 가드로 run이 디스크에 끼임 | `awaiting-stage-start`에 취소 경로 추가(§3.7), `run_current_stage` 실패를 `mark_current_failed`로 흡수(§3.3-(3)-7) | T-D1, 재작성된 163행 테스트 |
| `0235a40` | 스폰 전 상태를 디스크에 안 남김 | `begin_current_stage → save → run_current_stage` 순서 고정(§3.3-(3)). **락 해제 전에 `Running`이 디스크에 확정되는 것이 T-D4a 기대값의 근거**다. | T-D4a |
| `dec3360` | 프론트 이중 클릭 | `setBusy(true)` / `disabled={busy}`(§3.10) | F-D4 |
| `74408c7` / `1b2ba18` | id path traversal | `is_valid_id`는 그대로 유효. targetDir에 대한 동등 방어는 **미결(§6.6)** | — (§6.6 결정 후) |
| `a246384` / `f3be3bf` | 실행 진입점이 원본 템플릿을 오염 | **미결(§6.2, §6.3)** | — (결정 후) |

### 5.4 수동 스모크 (PRD F-2 / IMP-011) — `npx tauri dev`

| # | 절차 | 합격 조건 |
|---|---|---|
| (a) | 템플릿 실행 시작 | 어떤 프로세스도 실행되기 전에 **1단계 편집 패널**에 도달. 로그 영역이 비어 있음 |
| (b) | 1단계 프롬프트를 알아볼 수 있게 수정 후 '이 단계 실행' | 라이브 로그에 **수정된 프롬프트**의 결과가 나타남 |
| (c) | targetDir을 파일 탐색기로 열어본다 | `.claude-pipeline-wizard/run.json`·`pipeline.json` 존재. 각 상태 전이 후 내용이 갱신됨 |
| (d) | `checkpoint:false` 단계 종료를 기다린다 | 자동 실행 없이 **다음 단계 편집 패널로 복귀** |
| (e) | 체크포인트 단계에서 '승인' | 자동 실행 없이 **다음 단계 편집 패널로 복귀** |

---

## 6. 미해결 기술 질문

> **아래 8개 항목은 PRD가 선택지만 제시했거나 명시적으로 미결로 남긴 것이다. 이 TRD는 어느 쪽도 고르지 않았다.** 각 항목마다 선택지와 구현 영향을 제시하며, 사용자 결정 전까지 해당 부분은 구현 스펙이 없다. 결정이 내려지면 이 절의 내용을 3장으로 승격시킨다.

---

### 6.1 GAP-3 — `Failed` 상태에서 나가는 전이 간선의 존재 여부 ⚠️ 최우선

**미결의 근거**: 2026-09-06 원 설계서 Error Handling 절(163-165행)은 실패 단계에 대해 "Retry(같은 세션 재개) / Edit prompt & retry / Cancel run" 세 가지를 제공한다고 기술한다. 반면 2026-09-07 addendum의 Execution Flow 4단계는 "비정상 종료 → stage failed, run failed(기존 실패 UX 유지)"로만 기술하고 **실패 후 재시도가 새 상태 머신에서 어떤 전이인지 명시하지 않는다.** 두 문서가 서로 다른 동작을 말하며 근거만으로 어느 쪽이 현재 의도인지 확정할 수 없다.

**따라서 §3.2 전이표에 `Failed`의 출구 간선을 그리지 않았다.** 현재 표대로면 `Failed`는 `Completed`/`Cancelled`와 동일한 terminal 상태다.

#### 선택지 A — `Failed`는 terminal. 재시도하려면 새 run을 시작한다

- **전이표 영향**: 변경 없음. §3.2 표가 그대로 최종안이 된다.
- **`run_record.rs`**: 추가 메서드 없음.
- **`orchestrator.rs`**: 추가 커맨드 없음. `start_stage`의 가드는 `AwaitingStageStart`만 허용한 채로 유지.
- **`PipelineRun.tsx`**: `status === "failed"`일 때 카드 없이 "갤러리로 돌아가기"만 노출. **단, PRD D-3이 "최종 상태(Failed 등)임이 UI에 명시적으로 드러난다"를 요구**하므로 실패 사유와 "이 run은 종료되었습니다 — 다시 하려면 새 run을 시작하세요" 안내를 명시해야 한다.
- **`documents/`**: `system-architecture.md`·`sequence-diagrams.md`에 "실패는 terminal"을 명시.
- **비용**: 구현 비용 0. 대신 **N단계 파이프라인의 8단계에서 실패하면 앞의 7단계를 다시 실행해야 한다.** 매니페스트에 `resolvedStages`가 남아 있어 재구성은 가능하지만 재실행 비용은 그대로 발생한다. 2026-09-06 설계서가 약속한 "Edit prompt & retry"는 제공되지 않는다.
- **테스트**: `failed_run_rejects_start_stage` 1건 추가.

#### 선택지 B — `Failed`에서 편집 게이트(`AwaitingStageStart`)로 복귀할 수 있다

- **전이표 영향**: 신규 간선 **T12: `Failed` --`retry_stage(runId)`--> `AwaitingStageStart`** (현 stage `Failed → AwaitingStart`, `current_stage_index` 불변). 이후 T2로 이어져 사용자가 프롬프트를 고쳐 재실행 → 2026-09-06 설계서의 "Edit prompt & retry"가 pre-stage 게이트로 자연스럽게 흡수된다.
- **`run_record.rs`**: `reset_current_stage_to_gate(&mut self)` 신설 — 현 stage `AwaitingStart`, run `AwaitingStageStart`.
- **`orchestrator.rs`**: `retry_stage(run_id)` 신설 + `OrchestratorError::NotFailed`. **`Failed`가 더 이상 terminal이 아니게 되므로 `cancel_run`의 가드(§3.7)도 `Failed`를 포함하도록 넓혀야 한다** — 그러지 않으면 "재시도만 가능하고 취소는 불가능한" 새로운 반쪽 상태가 생겨 `08e9144` 패턴이 재발한다.
- **세션 재개 정책이 함께 필요하다**: 실패한 stage의 `session_id`를 `run_stage`의 `--resume`으로 넘길지(= 원 설계서의 "Retry, 같은 세션 재개") 새 세션으로 시작할지. `StageRun.session_id`(`run_record.rs:32`)는 실패 시에도 남아 있을 수 있어 **둘 다 구현 가능하며, 이 역시 결정 사항이다.**
- **`commands.rs` / `lib.rs` / `api.ts` / `PipelineRun.tsx`**: `retry_stage` 커맨드·래퍼·"이 단계 재시도" 버튼 추가.
- **비용**: 백엔드 1 메서드 + 1 커맨드 + 프론트 1 버튼. 테스트 3건(`retry_stage_from_failed_returns_to_gate`, `retry_stage_on_non_failed_rejects`, `cancel_run_from_failed_sets_cancelled`).
- **부수 효과**: PRD D-3("run이 무한 대기로 남지 않는다")이 훨씬 쉽게 충족된다. 대신 `Failed`가 비-terminal이 되면서 `PipelineRun.tsx:112`의 `onFinished()` 조건(`completed || cancelled || failed`)에서 `failed`를 빼야 하고, `documents/`의 상태 다이어그램이 모두 바뀐다.

**결정이 필요한 이유**: 이 선택은 §3.2 전이표, `cancel_run` 가드 범위, `PipelineRun`의 종료 처리, 그리고 문서 8종의 상태 다이어그램에 동시에 영향을 준다. 구현 도중에 뒤집으면 재작업 범위가 크다.

---

### 6.2 IMP-012 / 목표 C — `TemplateEditor`의 "저장 후 실행" 결합을 어떻게 풀 것인가

**현재 상태**: `src/pages/TemplateEditor.tsx:92-113`. `persistTemplate()`이 `saveTemplate(template)`를 호출하고, `handleSave`(107-109행)와 `handleRun`(111-113행) **양쪽이 이를 통과**한다. 즉 실행 버튼을 누르면 반드시 원본 템플릿이 덮어써진다. 커밋 `a246384`가 갤러리의 직접 실행 버튼을 제거하면서 이 편집기가 유일한 실행 진입점이 되었기 때문에, **실행 = 원본 덮어쓰기**가 구조적으로 성립한다. 승인된 설계·계획서는 이 경로를 손대지 않는다.

이 결정 없이는 **PRD C-1("원본 템플릿을 한 번도 쓰지 않고 실행이 완주된다")이 성립할 수 없다.** §3.5의 백엔드 격리는 이 프론트 경로를 막지 못한다.

#### 선택지 A — 조건부 저장: 실행 버튼이 `saveTemplate`을 호출하지 않는다

```tsx
const handleRun = () => { if (!error) onRun(template); };   // persistTemplate 경유 없음
```
- **변경 범위**: `TemplateEditor.tsx` 3줄. 최소 변경.
- **영향**: 편집기에서 프롬프트를 고치고 '실행'을 누르면 그 수정은 **이 run에만** 적용되고(→ `App.tsx:handleRun`이 인메모리 `template`을 그대로 `startPipelineRun`에 넘기지 않고 `resolvedStages`로 전달해야 함) 갤러리 템플릿은 그대로 남는다.
- **주의**: `App.tsx:78`의 `startPipelineRun(template.id, targetDir, runId)`은 **템플릿 id만 넘긴다.** 백엔드 `start_run`은 `template_store.load(template_id)`로 **디스크에서** 읽으므로, 저장하지 않은 편집 내용은 **사라진다.** 따라서 이 선택지는 `start_run`이 stage 배열을 직접 받도록 시그니처를 바꾸거나(`start_run(template_id, stages, target_dir, run_id)`), 편집 내용을 첫 stage의 override로만 흘리거나, "편집기의 미저장 변경은 실행에 반영되지 않는다"는 UX를 감수해야 한다. **이 세 갈래가 다시 결정 사항이다.**
- **사용자 혼란 리스크**: 저장하지 않고 실행하면 다음에 갤러리에서 볼 때 수정이 없다 — 현재 동작과 반대라 안내가 필요하다.

#### 선택지 B — 별도 실행 진입점 신설: 갤러리에 직접 실행 버튼을 되돌리고 편집기의 실행 버튼을 제거

- **변경 범위**: `TemplateEditor.tsx`(실행 버튼 153-155행 및 `onRun` prop 제거), `src/pages/TemplateGallery.tsx`(실행 버튼 추가), `App.tsx`(`View` 유니온과 `handleRun` 배선 변경), 각 테스트.
- **영향**: 커밋 `a246384`의 의도("사용자가 실행 전에 반드시 수동 편집 화면을 지나가게 한다")를 **pre-stage 게이트가 대신 충족**한다 — 이제 실행 직전에 반드시 편집 패널이 뜨므로 편집기 경유를 강제할 이유가 사라졌다. 즉 이 선택지는 `a246384`를 되돌리는 것이 아니라 **그 목적을 더 나은 자리로 옮기는 것**이다.
- **비용**: A보다 크지만 의미론적으로 깔끔하다. `startPipelineRun`이 디스크 템플릿을 읽는 현재 구조와 모순이 없다(갤러리 실행 = 저장된 템플릿 실행).
- **부수 효과**: 편집기는 순수 편집 화면이 되고, run 범위 편집은 전적으로 실행 화면에서 이뤄진다. IMP-013(§6.3)과 방향이 일치한다.

#### 선택지 C — 현행 유지 (실행이 저장을 동반)

- **변경 범위**: 0.
- **영향**: **PRD C-1이 무효가 되어 성공 기준 개정이 필요하다** (PRD §5.3 C-1의 단서에 명시됨). "실행 = 원본 저장"이 의도된 동작이라고 문서화하고, 오염 방어를 run 시작 이후 구간(IMP-006/007)으로만 한정한다.

**참고**: PRD §4.4-3은 IMP-012/013을 IMP-006과 **함께 판단**할 것을 권고한다(단, 이는 백로그 명시가 아니라 PRD가 도출한 제약이므로 재확인 대상).

---

### 6.3 IMP-013 / 목표 C — 실행 화면의 '템플릿 편집' 버튼을 어떻게 할 것인가

**현재 상태**: `src/pages/PipelineRun.tsx:232-234`. 커밋 `f3be3bf`가 "실패하거나 완료된 run을 컨텍스트를 잃지 않고 편집·재실행"하기 위해 의도적으로 추가한 버튼으로, `onEditTemplate(template)` → `App.tsx:102`가 `setView({ name: "editor", template })`로 **원본 템플릿 편집 화면**으로 이동시킨다.

§3.10이 실행 화면 **안**에 run 범위 편집 패널을 추가하면, 같은 화면에 "이 run만 고치는 경로"와 "원본을 고치는 경로"가 나란히 놓인다. PRD C-5는 두 경로가 UI상 구분되고 사용자가 후자의 영향 범위를 인지할 수 있어야 한다고 요구한다.

#### 선택지 A — 원본 이동 유지 + 명시적 구분

- 버튼 라벨을 **"원본 템플릿 편집(모든 향후 실행에 적용)"**으로 리라벨하고 설명 텍스트를 붙인다.
- 클릭 시 확인 다이얼로그: "이 편집은 저장된 템플릿을 바꾸며 진행 중인 이 run에는 반영되지 않습니다."
- **run 진행 중(`Running`/`AwaitingCheckpoint`/`AwaitingStageStart`)에는 disabled**로 두고 terminal 상태에서만 활성화하는 변형도 가능하다.
- **변경 범위**: `PipelineRun.tsx` 라벨 + 확인 절차 ~15줄, 테스트 1~2건.
- **영향**: `f3be3bf`의 원 의도(완료/실패 run에서 컨텍스트 유지 편집)를 보존한다. §3.6(IMP-007)에 의해 원본 편집이 진행 중 run에 새어들지 않으므로 **기능적으로는 안전**하고, 남는 것은 사용자 오해 리스크뿐이다.

#### 선택지 B — 이 run 전용 사본 편집으로 전환

- 버튼을 **"이 run의 파이프라인 편집"**으로 바꾸고, `resolvedStages` 전체(현재 단계 이후 단계 포함)를 편집하는 화면으로 보낸다. 저장 시 `template_store`가 아니라 **`record.resolved_stages`를 갱신**한다.
- **필요한 신규 백엔드**: `update_resolved_stages(run_id, stages)` 커맨드 — 현재 `start_stage`의 override는 **현재 단계 1개만** 교체할 수 있어서 이후 단계를 미리 손볼 수단이 없다. `apply_stage_override`(§3.1-(3))와 별개로 `replace_resolved_stages`가 필요하고, 이미 실행된 단계는 변경 금지, stage 개수·id 집합 변경 허용 여부 등 **추가 규칙이 줄줄이 따라온다.**
- **변경 범위**: 백엔드 커맨드 1개 + 검증 규칙 + `TemplateEditor`의 "run 모드" 분기 + `App.tsx` View 유니온 확장 + 테스트 다수. **A보다 훨씬 크다.**
- **영향**: 원본 오염 경로가 실행 화면에서 완전히 사라진다. 대신 "원본을 고치고 싶다"는 요구는 갤러리로 돌아가야만 가능해진다.

#### 선택지 C — 버튼 제거

- 실행 화면에서 원본 편집 진입을 없애고 갤러리 경유만 남긴다. 변경 범위 최소(3줄 + `onEditTemplate` prop 제거, `App.tsx:102` 정리).
- `f3be3bf`가 의도한 "실패한 run을 컨텍스트를 잃지 않고 편집·재실행" 동선이 사라진다 — **이는 §6.1(GAP-3) 선택지 B와 상호작용한다.** GAP-3에서 B(Failed → 게이트 복귀)를 고르면 이 버튼 없이도 실패 단계를 그 자리에서 고쳐 재실행할 수 있으므로 C의 손실이 작아진다. **두 결정을 함께 보는 것이 좋다.**

---

### 6.4 IMP-017 / 목표 B — 동일 `targetDir` 재실행 시 기존 매니페스트 처리

**현재 상태**: 매니페스트 경로가 targetDir당 고정(`.claude-pipeline-wizard/run.json`)이다. 같은 폴더에 두 번째 run을 시작하면 이전 run의 상태·`resolvedStages` 스냅샷이 **조용히 소실**된다. `app_data_dir/runs/`는 runId별 파일이라 이 손실 경로가 없었으므로, **매니페스트 도입이 새로 만드는 데이터 손실**이다. `orchestrator.rs:43-44`에는 이미 "타겟 디렉토리 비어있음 검사는 프론트 UI가 필요해 범위 밖"이라는 미해결 주석이 남아 있다.

이 결정 없이 §3.4를 구현하면 PRD B-3("이전 run 매니페스트가 조용히 소실되지 않는다")을 만족할 수 없다.

#### 선택지 A — 차단: 기존 매니페스트가 있으면 `start_run` 실패

- `start_run`에서 `manifest_dir(&target_dir).join("run.json").exists()` 검사 → `OrchestratorError::TargetDirInUse(path)` 반환.
- **영향**: 가장 안전하지만 **완료된 run의 폴더를 재사용할 수 없다** — 같은 과제 폴더에서 2차 작업을 하려면 사용자가 수동으로 폴더를 지워야 한다. 실사용 마찰이 크다.
- 완화: 기존 `run.json`의 `status`가 terminal(`completed`/`failed`/`cancelled`)이면 허용하고 진행 중일 때만 차단 — 이 변형 역시 결정 대상이다.
- **프론트**: `App.tsx:handleRun`에 에러 처리 + 안내 필요.

#### 선택지 B — 경고 후 진행: 사용자가 확인하면 덮어쓴다

- 신규 커맨드 `inspect_target_dir(targetDir) -> Option<ExistingRunSummary>`를 만들어 `App.tsx`의 폴더 선택 직후 호출, 기존 run이 있으면 확인 다이얼로그.
- **영향**: 유연하지만 **"조용히 소실되지 않는다"만 만족하고 소실 자체는 막지 못한다.** 사용자가 확인을 누르면 이전 스냅샷은 사라진다.
- **변경 범위**: 백엔드 커맨드 1 + `api.ts` + `App.tsx` 다이얼로그 + 테스트.

#### 선택지 C — runId별 하위 경로: `.claude-pipeline-wizard/runs/<runId>/{run,pipeline}.json`

- 충돌이 원천적으로 없다. `latest`를 심볼릭 링크나 복사본으로 두면 "폴더만 열어서 확인" 요구도 유지된다.
- **영향**: **설계서 Decision 2가 명시한 경로(`.claude-pipeline-wizard/{run.json, pipeline.json}`)에서 벗어난다.** 승계 대상 승인 설계의 변경이므로 별도 승인이 필요하다. 또 폴더가 계속 누적되어 GAP-5(프로젝트 폴더에 잔존물이 쌓이는 문제, 정리 정책 미결)를 악화시킨다.
- **변경 범위**: `project_manifest.rs`의 경로 함수와 문서(`database-design.md`)만. 코드 비용은 오히려 작다.

**연관**: 어느 선택지든 `documents/database-design.md`의 "세 번째 저장소" 기술(§3.15)이 그에 맞춰 달라진다.

---

### 6.5 IMP-015 / GAP-4 / 목표 D — 매니페스트 쓰기 실패 시 정책

**현재 상태(§3.4 초안)**: `Orchestrator::save()`가 `run_store.save` **후** `write_project_manifest`를 하고 실패 시 `?`로 즉시 전파한다. `start_stage`의 5단계(`begin_current_stage` 직후 `save`)에서 이 일이 벌어지면 — targetDir 삭제, 읽기 전용, 네트워크 드라이브 단절 등 — **`app_data_dir` 사본에는 이미 `Running`이 기록된 채 에러만 올라간다.** 그 run은 `start_stage`(가드가 `AwaitingStageStart`)도 `approve`/`reject`(가드가 `AwaitingCheckpoint`)도 받지 못하는 끼임 상태가 된다. 이는 `08e9144`가 고친 것과 **동일한 유형의 버그**다.

계획서는 `OrchestratorError::ProjectManifest` 변형 추가만 명시하고 복구/폴백 정책을 정하지 않았다. PRD D-3은 "run이 어떤 액션도 받지 않는 무한 대기로 남지 않는다"만 요구하며, 세 선택지 모두 이를 만족할 수 있다.

#### 선택지 A — 치명적 오류 + 상태 롤백

- `save()`에서 매니페스트 쓰기가 실패하면 **메모리 상 `record`를 전이 이전 상태로 되돌리고 `run_store.save`를 다시 호출**해 디스크를 원상 복구한 뒤 에러를 반환한다.
- **구현**: `start_stage`가 `begin_current_stage` 전에 `let snapshot = record.clone();`을 잡아두고, 매니페스트 실패 시 `self.run_store.save(&snapshot)`로 되돌린다. `RunRecord`는 이미 `Clone`이다(`run_record.rs:36`).
- **영향**: run이 `AwaitingStageStart`로 남아 **재시도 가능**. 가장 안전하고 PRD D-3을 확실히 만족한다.
- **약점**: 롤백 저장 자체가 실패할 수 있다(이중 실패). 그 경우의 처리를 또 정해야 한다 — 보통은 로그만 남기고 원 에러를 반환.
- **비용**: `orchestrator.rs` ~10줄 + 테스트 1~2건(읽기 전용 targetDir 픽스처 필요. **Windows에서 읽기 전용 디렉터리 재현이 까다로워** 테스트는 존재하지 않는 드라이브 경로나 파일을 디렉터리 자리에 두는 방식이 현실적이다).

#### 선택지 B — 경고로 흘리기: `app_data_dir` 사본만 유지하고 진행

- `write_project_manifest` 실패를 `eprintln!`/이벤트로 알리고 `Ok(())` 반환. 실행은 정상 진행.
- **영향**: 실행이 중단되지 않아 사용자 마찰이 가장 적다. 대신 **매니페스트가 "모든 전이마다 갱신된다"는 PRD B-1 보장이 깨진다** — 매니페스트를 신뢰할 수 없는 부분 기록으로 격하시키는 셈이다.
- **필요 부수 작업**: 실패를 사용자에게 알릴 채널(`pipeline://stage-event`에 경고 종류 추가 또는 별도 이벤트)과 `PipelineRun.tsx`의 경고 배너. 조용히 실패하면 관측성 목표(B) 자체가 무의미해진다.
- **비용**: 백엔드 ~5줄 + 이벤트/배너.

#### 선택지 C — 최초 1회만 치명적, 이후는 경고

- `start_run`에서의 매니페스트 쓰기 실패는 치명적(targetDir이 애초에 쓸 수 없다면 실행 자체가 무의미)으로 처리하고, 이후 전이에서의 실패는 경고로 흘린다.
- **영향**: A와 B의 절충. "폴더가 원래부터 못 쓰는 경우"와 "실행 도중 드라이브가 끊긴 경우"를 다르게 다룬다.
- **비용**: 분기 로직이 늘어 `save()`가 컨텍스트 파라미터(`is_initial: bool`)를 받아야 한다. 테스트도 두 갈래.

**어느 선택지든 필요한 공통 안전장치**: `start_stage`의 실패 경로가 `?`로 조기 반환하는 대신 **run을 항상 액션 가능한 상태로 남기는지**를 통합 테스트로 고정해야 한다(PRD D-3).

---

### 6.6 IMP-019 / 목표 B — `targetDir` 경로 검증 정책 (선례 `74408c7`)

**현재 상태**: `targetDir`은 IPC 경계를 넘어온 `String`이다(`commands.rs:57`). `start_run`이 `create_dir_all`을 하고(`orchestrator.rs:42`), §3.4의 `write_project_manifest`가 그 아래에 디렉터리와 파일 2개를 만든다. **정규화·심볼릭 링크 해석·절대 경로 요구·쓰기 가능 여부 검사가 하나도 없다.**

선례가 있다: 커밋 `74408c7`/`1b2ba18`이 template id와 run id에 `is_valid_id`(`template/mod.rs:46-51`)를 적용해 path traversal을 차단했고, `RunRecordStore::save/load`(`run_record.rs:127, 137`)와 `validate_template`(54행)이 이를 강제한다. **targetDir만 이 방어에서 빠져 있다.**

> **위협 모델에 대한 주의**: 현재 targetDir은 `App.tsx:66`의 `open({ directory: true })` 네이티브 다이얼로그로만 생성되므로 실사용 공격면은 좁다. 그러나 커맨드는 IPC로 임의 문자열을 받을 수 있고, 매니페스트 도입으로 **앱이 targetDir 아래에 파일을 쓰기 시작**했다는 점이 새롭다. 정책을 명시적으로 정해 선례와 일관되게 두는 편이 안전하다는 것이 PRD의 판단이다.

#### 선택지 A — 절대 경로 요구 + 정규화 후 존재/쓰기 가능 검사

```rust
// project_manifest.rs 또는 orchestrator.rs
fn validate_target_dir(p: &Path) -> Result<PathBuf, ...> {
    if !p.is_absolute() { return Err(NotAbsolute); }
    let canonical = std::fs::canonicalize(p)?;   // 심볼릭 링크 해석
    if !canonical.is_dir() { return Err(NotADirectory); }
    Ok(canonical)
}
```
- **영향**: 상대 경로·존재하지 않는 경로가 거부된다. **문제: 현재 `start_run`은 `create_dir_all`로 없는 폴더를 만들 수 있는데, `canonicalize`는 존재하는 경로에만 동작한다.** 따라서 "새 폴더 생성 후 실행"을 계속 허용하려면 `create_dir_all` → `canonicalize` 순서로 하거나, 부모 디렉터리만 canonicalize해야 한다. **이 세부가 또 결정 사항이다.**
- **Windows 주의**: `canonicalize`가 `\\?\` UNC 접두사를 붙인 경로를 반환한다. 이 값이 `RunRecord.target_dir`에 저장되고 UI(`PipelineRun.tsx:155`)에 그대로 표시되므로 **표시 경로가 지저분해진다.** 저장은 원본, 검사만 정규화 값으로 하는 분리가 필요할 수 있다.
- **비용**: 함수 1개 + 에러 변형 + 테스트 3~4건.

#### 선택지 B — 거부 목록 방식: 시스템 디렉터리·홈 루트 등만 차단

- 절대 경로만 요구하고, 정규화 후 결과가 시스템 경로(`C:\Windows`, `/`, `/usr` 등)나 사용자 홈 루트 자체이면 거부.
- **영향**: 유연하지만 **완전하지 않고 플랫폼마다 목록이 달라진다.** 유지보수 부담이 크고 선례(`is_valid_id`의 화이트리스트 방식)와 철학이 반대다.

#### 선택지 C — 검증하지 않음 (현행 유지) + 명시적 문서화

- "targetDir은 네이티브 폴더 선택 다이얼로그로만 들어오며 IPC를 직접 호출하는 주체는 이미 앱과 동일 권한을 가진다"는 위협 모델을 문서화하고 검증을 추가하지 않는다.
- **영향**: 비용 0. **PRD B-4("targetDir 검증 정책이 코드에 존재하고, 정책 위반 입력에 대해 매니페스트 쓰기가 시도되지 않는다")를 만족하지 못하므로 성공 기준 개정이 필요하다.**

---

### 6.7 IMP-020 / 목표 B — `request_changes` 피드백 프롬프트를 감사 기록에 남길 것인가

**현재 상태**: 설계서 102-108행이 "피드백 프롬프트는 일회성이므로 `resolvedStages`에 기록하지 않는다"고 명시했고 §3.6이 이를 따른다. 한편 `StageRun { id, status, session_id, log }`(`run_record.rs:27-34`)에는 실행된 프롬프트를 담는 필드가 없다. 결과적으로 **수정요청으로 재실행된 단계는 "어떤 프롬프트로 실행됐는지"를 `run.json`/`pipeline.json`만으로 재구성할 수 없다.**

> 다만 `StageRun.log`(33행)는 `StageEvent` 전체를 축적하므로 **모델의 응답과 도구 사용은 남는다.** 남지 않는 것은 사용자가 입력한 피드백 텍스트다.

PRD B-5는 "재구성할 수 있다 — **또는** '기록하지 않음'이 명시적 결정으로 문서화되어 있다"로 양쪽을 허용한다.

#### 선택지 A — `StageRun`에 실행 프롬프트 이력 필드 추가

```rust
pub struct StageRun {
    pub id: String, pub status: StageStatus,
    pub session_id: Option<String>, pub log: Vec<serde_json::Value>,
    pub executed_prompts: Vec<String>,   // ★ 신설 — 실제 실행에 쓰인 프롬프트를 순서대로
}
```
- `start_stage`와 `request_changes`가 실행 직전에 `push`한다.
- **영향**: 감사 기록이 완전해진다. **비용: 이것도 `RunRecord` 스키마 변경이므로 §6.8의 마이그레이션 결정 범위에 포함되어야 한다** — `resolvedStages`와 함께 결정하면 추가 비용이 거의 없지만, 나중에 따로 추가하면 마이그레이션을 두 번 하게 된다. **타이밍상 지금 결정하는 것이 유리하다.**
- **프라이버시**: 프롬프트가 targetDir의 `run.json`에 그대로 남는다 → **GAP-5(민감한 프롬프트가 프로젝트 폴더에 잔존)를 악화**시킨다.
- **크기**: 긴 피드백이 반복되면 `run.json`이 커진다. `log` 필드가 이미 훨씬 큰 것을 감안하면 상대적으로 미미하다.

#### 선택지 B — `log`에 합성 이벤트로 기록

- `StageEvent`에 `{ kind: "promptSubmitted", prompt: String }` 변형을 추가하고 실행 직전에 `log`에 넣는다(`stream_json.rs` 수정).
- **영향**: `RunRecord` 구조 변경이 없어 **마이그레이션 부담이 없다.** 프론트 로그 뷰(`PipelineRun.tsx:27-47` `describeEvent`)에 케이스를 추가하면 실행 프롬프트가 라이브 로그에도 보여 UX 이득이 있다.
- **약점**: `StageEvent`는 원래 claude CLI의 stream-json을 미러링하는 타입이라 **앱이 합성한 이벤트를 섞으면 의미가 흐려진다.** `types.ts:38-45`의 프론트 미러도 함께 늘어난다.

#### 선택지 C — 기록하지 않음 (현행 설계 유지) + 명시적 문서화

- `documents/database-design.md`와 README에 "매니페스트는 파이프라인 정의의 스냅샷이며 일회성 피드백 프롬프트는 포함하지 않는다. 감사 목적의 완전한 실행 기록이 아니다"를 명시한다.
- **영향**: 비용 0. PRD B-5의 후자 조항으로 충족된다. GAP-5 악화도 없다.

---

### 6.8 IMP-018 / GAP-6 / 목표 D — 기존 `run.json` 역직렬화 마이그레이션 ⚠️ IMP-002와 동시 결정 필요

**현재 상태**: §3.1-(2)가 `RunRecord`에 `resolved_stages`를 **필수 필드**로 추가한다. 계획서(165-175행)도 `#[serde(default)]` 등 하위 호환 장치를 두지 않는다. 그 결과 앱 업데이트 후 `<app_data_dir>/runs/`에 남아 있던 기존 `run.json`들은 `resolvedStages` 키가 없어 **`run_record.rs:145`의 `serde_json::from_str`에서 실패**한다.

열거형 값 추가(`AwaitingStageStart`/`AwaitingStart`)는 읽기 방향으로 호환되지만 **필드 추가는 아니다.** `documents/database-design.md` 2.4절도 마이그레이션 도구·트랜잭션이 전혀 없음을 명시한다.

**PRD §4.4-2가 이 결정을 IMP-002와 동시에 내릴 것을 요구하며, 그래서 §3.1-(2)의 `#[serde(default)]` 부여 여부를 이 TRD가 비워 두었다.** GAP-6(진행 중이던 run들의 처리)도 같은 결정에 묶인다.

#### 선택지 A — `#[serde(default)]` 부여

```rust
#[serde(default)]
pub resolved_stages: Vec<crate::template::Stage>,
```
- **영향**: 기존 파일이 **로드는 된다.** 그러나 `resolved_stages`가 **빈 벡터**가 되어, 그 run에 대해 `current_resolved_stage()`가 인덱스 범위를 벗어나 **패닉**한다. 즉 로드 실패가 런타임 패닉으로 바뀔 뿐이다.
- **따라서 A는 단독으로 불충분하다.** 반드시 (i) 로드 시 `resolved_stages.is_empty()`이면 `template_store`에서 채우는 지연 마이그레이션, 또는 (ii) 빈 경우를 "레거시 run"으로 판정해 UI에서 읽기 전용 처리하는 처리와 짝지어야 한다. **어느 짝을 쓸지가 결정 사항이다.**
- (i)의 위험: 템플릿이 그 사이 수정되었으면 채워지는 stage가 **실제 실행에 쓰인 정의와 다르다** — "실행에 쓰인 정의를 보존한다"는 목표 B의 취지와 상충하는 재구성이 된다. 템플릿이 삭제되었으면 채울 수도 없다.
- **비용**: attribute 1줄 + 짝 처리 로직 + 테스트 2~3건.

#### 선택지 B — 기존 레코드 폐기 / 격리 이관

- 앱 시작 시 `app_data_dir/runs/`를 스캔해 `resolvedStages`가 없는 파일을 `app_data_dir/runs-legacy-v1/`로 **이동**하고, 사용자에게 1회 안내한다.
- **영향**: 새 코드가 레거시 형식을 전혀 모르므로 **코드가 가장 깨끗**하다. 데이터는 지우지 않고 보존한다. GAP-6(진행 중이던 `running`/`awaiting-checkpoint` run 처리)도 이 이관에 자연스럽게 흡수된다 — 그 run들은 어차피 앱 재시작 후 재개 불가(README v1 한계 2)이므로 실질 손실이 작다.
- **비용**: `lib.rs:15-27`의 `setup` 훅에 마이그레이션 함수 1개(~30줄) + 안내 UI + 테스트.
- **약점**: 사용자가 과거 run 기록을 앱에서 더 이상 볼 수 없다. 다만 **현재 앱에는 과거 run 목록 UI 자체가 없다**(`App.tsx`의 View는 gallery/editor/run 3개뿐이고 run은 방금 시작한 것만 본다) — 따라서 실사용 손실이 거의 없다는 것이 이 선택지의 근거다.

#### 선택지 C — 스키마 버전 필드 도입

```rust
pub struct RunRecord {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,   // 신규 레코드는 2, 없으면 1
    ...
}
```
- 로드 시 버전으로 분기해 v1 → v2 변환기를 태운다.
- **영향**: 가장 확장성 있는 구조로, 이후 스키마 변경(예: §6.7 선택지 A의 `executed_prompts`)에도 재사용된다. **§6.7에서 A를 고른다면 C의 가치가 크게 올라간다.**
- **비용**: 가장 크다. 버전 상수·변환기·양방향 테스트. 현재 이 앱의 규모(저장소 2~3개, 레코드 타입 2개)에 비해 과할 수 있다.
- **주의**: v1 → v2 변환기 안에서 `resolved_stages`를 무엇으로 채울지는 여전히 A의 (i)과 같은 문제를 안는다.

**공통 요구**: 어느 선택지든 PRD D-5("원인 불명 역직렬화 실패로 앱 기능이 막히지 않는다")를 만족해야 하고, 결정 결과를 `documents/database-design.md` 2.4절(§3.15)에 반영해야 한다.

---

### 6.9 결정 대기 항목 요약

| # | 항목 | PRD 근거 | 블로킹 대상 | 함께 결정해야 할 것 |
|---|---|---|---|---|
| 6.1 | GAP-3 — `Failed` 출구 간선 | PRD §6 GAP-3 (최우선) | §3.2 전이표, §3.7 `cancel_run` 가드, `PipelineRun` 종료 처리, 문서 8종 | 6.3 |
| 6.2 | IMP-012 — `persistTemplate` 결합 | PRD §3-목표C, §5.3 C-1 | 성공 기준 C-1 성립 여부, `App.tsx` 실행 배선 | 6.3, (PRD §4.4-3에 따라 IMP-006) |
| 6.3 | IMP-013 — 실행 화면 '템플릿 편집' | PRD §5.3 C-5 | §3.10 화면 구성 | 6.1, 6.2 |
| 6.4 | IMP-017 — 동일 targetDir 재실행 | PRD §5.2 B-3 | §3.4 매니페스트 경로·`start_run` 가드 | GAP-5 |
| 6.5 | IMP-015 / GAP-4 — 매니페스트 쓰기 실패 | PRD §5.4 D-3, §6 GAP-4 | §3.4 `Orchestrator::save`, §3.3 `start_stage` 5단계 | — |
| 6.6 | IMP-019 — targetDir 경로 검증 | PRD §5.2 B-4 | §3.4 `write_project_manifest` 진입 | 6.4 |
| 6.7 | IMP-020 — 피드백 프롬프트 기록 | PRD §5.2 B-5 | §3.6, `StageRun` 스키마 | **6.8 (스키마 변경 타이밍)**, GAP-5 |
| 6.8 | IMP-018 / GAP-6 — 역직렬화 마이그레이션 | PRD §4.4-2, §5.4 D-5, §6 GAP-6 | **§3.1-(2) 머지 자체** | 6.7 |

### 6.10 PRD 6장의 나머지 GAP에 대한 이 TRD의 처리

- **GAP-1(resume UI)**: 범위 밖 유지. §3.13에서 README의 기존 'No cross-restart resume UI' 한계 항목을 **제거하지 않고 유지**하는 것으로 반영했다.
- **GAP-2(모든 단계 정지에 따른 조작 부담)**: "남은 단계 일괄 실행" 같은 예외 경로를 **설계하지 않았다.** §4.4-2에 사용자에게 보이는 변화로만 기록했다. 필요 판단이 서면 별도 항목으로 올라와야 한다.
- **GAP-5(`.claude-pipeline-wizard/` 잔존 정책)**: `.gitignore` 처리·완료 후 정리·민감 프롬프트 잔존에 대해 아무 동작도 설계하지 않았다. §6.4와 §6.7의 선택이 이 문제의 크기를 바꾸므로 함께 검토 대상이다.
- **GAP-7(우선순위 실사용 근거 부재)**: §4.2 롤아웃 순서는 **기술적 의존성**만으로 짰다(PRD 우선순위를 그대로 따르지 않았다). 실사용 근거가 확보되면 순서 재조정이 가능하다.

---

## 부록 A — PRD 커버리지 자체 확인

| 백로그 id | 상태 | 이 TRD에서의 위치 |
|---|---|---|
| IMP-001 | 확정 | §3.2 (상태 전이표 T4/T7), §5.2 T-A2·T-A4 |
| IMP-002 | 확정 (단 §6.8에 머지 종속) | §3.1, §5.2 U-1~U-7 |
| IMP-003 | 확정 (정책 3건 §6.4·6.5·6.6) | §3.4, §5.2 T-B1·T-B2 |
| IMP-004 | 확정 | §3.3 (세션 재개 규칙 §3.3-(4a) 포함), §5.2 T-A1·T-A3·T-A5·T-A6·T-E1 |
| IMP-005 | 확정 | §3.12, §5.2 U-8·U-9 |
| IMP-006 | 확정 | §3.5, §5.2 T-C1·T-C2 |
| IMP-007 | 확정 | §3.6, §5.2 T-C3·T-C4 |
| IMP-008 | 확정 | §3.9, §5.2 F-E4a·F-E4b |
| IMP-009 | 확정 | §3.10, §5.2 F-E1·F-E3·F-D4 |
| IMP-010 | 확정 | §3.11, §5.2 F-E5 |
| IMP-011 | 확정 | §3.13, §5.4 (a)~(e) |
| **IMP-012** | **미해결** | **§6.2** (선택지 A/B/C) |
| **IMP-013** | **미해결** | **§6.3** (선택지 A/B/C) |
| IMP-014 | 확정 | §3.7 (전이 T11), §5.2 T-D1·F-D1 |
| **IMP-015** | **미해결** | **§6.5** (선택지 A/B/C) |
| IMP-016 | 확정 | §3.8 (run별 뮤텍스[락 범위 1~5단계] + `expected_stage_index` 세대 가드), §5.2 T-D4a·T-D4b |
| **IMP-017** | **미해결** | **§6.4** (선택지 A/B/C) |
| **IMP-018** | **미해결** | **§6.8** (선택지 A/B/C) — §3.1-(2) 머지 블로킹 |
| **IMP-019** | **미해결** | **§6.6** (선택지 A/B/C) |
| **IMP-020** | **미해결** | **§6.7** (선택지 A/B/C) |
| IMP-021 | 확정 | §3.14, §4.2 단계 0 |
| IMP-022 | 확정 | §3.15 |

**커버리지: 22 / 22 (100%)** — 확정 15건, 미해결 7건. 생략된 항목 없음.

| PRD GAP | 처리 |
|---|---|
| GAP-1 | §6.10 (범위 밖 유지, README 한계 항목 존치) |
| GAP-2 | §6.10 (완화 경로 미설계, §4.4-2에 사용자 영향으로 기록) |
| **GAP-3** | **§6.1 (선택지 A/B — `Failed` 출구 간선을 전이표에 그리지 않음)** |
| **GAP-4** | **§6.5 (IMP-015와 동일 항목)** |
| GAP-5 | §6.10 (미설계, §6.4·6.7과 연관 명시) |
| **GAP-6** | **§6.8 (IMP-018과 동일 항목)** |
| GAP-7 | §6.10 (롤아웃 순서를 기술 의존성으로만 구성) |

**PRD 성공 기준 커버리지**: A-1~A-5 / B-1·B-2 / C-1~C-4 / D-1·D-2·D-4 / E-1·E-3·E-4·E-5 / F-1~F-4는 §5.2·§5.4에 검증 수단이 대응된다. **B-3·B-4·B-5·C-5·D-3·D-5는 §6의 결정 이후에 테스트를 설계**한다(§5.2 말미에 명시).
