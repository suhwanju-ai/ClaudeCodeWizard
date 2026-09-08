# PRD — Claude Pipeline Wizard 개선 spec (실행 전 단계 편집 게이트 & 프로젝트 로컬 매니페스트)

- 문서 상태: **확정 (Approved)** — prd-reviewer 검토 완료 2026-09-08, critical 이슈 없음. 검토 기록: `_workspace/p4_reviewer_prd-feedback.json`
- 대상 프로젝트: `D:\Utility\Agents\ClaudeCodeWizard` (Claude Pipeline Wizard)
- 근거 백로그: `_workspace/p2_analyzer_improvement-backlog.json` (분석 시각 2026-09-08, items 22건 / gaps 7건)
- 승계 대상 설계서: `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md` (Status: Approved)
- 승계 대상 구현계획서: `docs/superpowers/plans/2026-09-07-pipeline-run-editing.md` (2038줄, 9개 태스크, 전 태스크 미착수)

---

## 1. 개요

### 1.1 이 spec이 다루는 것

Claude Pipeline Wizard는 사용자가 정의한 다단계 파이프라인(stage 목록)을 Claude CLI 프로세스로 순차 실행하는 Tauri 데스크톱 앱이다. 본 spec은 이 앱의 **파이프라인 실행 모델**을 다음 두 축으로 개선한다.

1. **실행 전 단계 편집 게이트(pre-stage edit gate)** — 모든 단계가 실행 직전에 정지하고, 사용자가 그 단계를 확인·편집한 뒤에야 프로세스가 스폰된다.
2. **프로젝트 로컬 매니페스트** — 실행 대상 폴더(`<targetDir>`) 안에 `.claude-pipeline-wizard/{run.json, pipeline.json}`을 남겨, 과제 폴더만 열어도 그 run이 무엇을 실행 중인지 확인·수정할 수 있게 한다.

그리고 이 두 축을 실제로 성립시키기 위해 반드시 함께 해결해야 하는 **원본 템플릿 오염 경로**, **상태 끼임/경합/마이그레이션 리스크**, **문서 정합성**을 함께 다룬다.

### 1.2 범위 구성 — 두 종류의 항목이 섞여 있음에 대한 명시

이 PRD에 담긴 22개 항목은 성격이 다른 두 묶음이다. 독자는 이 구분을 반드시 인지해야 한다.

| 묶음 | 항목 | 성격 |
|---|---|---|
| **A. 기존 설계 승계** | IMP-001 ~ IMP-011 | 이미 다른 세션에서 `Status: Approved`로 확정된 2026-09-07 설계서와 그 9개 태스크 구현계획서의 결정 사항을 **그대로 옮긴 것**. 이 PRD가 새로 발명한 내용이 아니다. |
| **B. 추가 발견** | IMP-012 ~ IMP-022 | 위 승인된 설계·계획서에 **없던** 항목. 원인 커밋과 선례 버그를 역추적하는 과정에서 "이 설계로는 해결되지 않는다"고 확인된 잔존 문제들이다. |

### 1.3 선행 조건

승계 대상 두 문서(설계서·계획서)가 현재 git untracked 상태다. 구현 착수 전 이 두 문서를 커밋해 기준선으로 고정해야 한다(IMP-021). 이 PRD·후속 TRD가 인용하는 모든 근거 줄 번호가 그 기준선에 종속된다.

---

## 2. 배경 및 문제 정의

### 2.1 현재 상태 — 문제의 구조

백로그 `summary`와 각 항목의 `evidence`가 지목하는 현재 시스템의 문제는 다음과 같다.

**(a) 실행이 시작되면 사용자가 개입할 수 있는 지점이 거의 없다.**
`src-tauri/src/engine/orchestrator.rs:32-100`의 `drive()` 루프는 `stage.checkpoint == false`이면 `current_stage_index += 1`로 자동 전진해 곧바로 다음 단계를 실행한다. 즉 현재 문서화된 유일한 정지 지점은 "체크포인트 단계의 **사후**"뿐이다(`documents/system-architecture.md:124-131`). 실행 도중 "다음 단계 프롬프트를 이 과제에 맞게 조금 손보고 싶다"는 요구를 받아줄 자리가 없다.

**(b) "실행에 실제로 쓰인 stage 정의"가 어디에도 남지 않는다.**
`RunRecord`는 `template_id` 문자열로만 템플릿을 참조하며 stage 정의 사본을 갖지 않는다(`src-tauri/src/engine/run_record.rs:36-45`). 저장소는 `templates/{id}.json`과 `runs/{run_id}.json` 두 개뿐이고, 타겟 폴더에 무언가를 남긴다는 개념 자체가 문서에 없다(`documents/database-design.md:1-70`). 그래서 템플릿이 나중에 수정되면 과거 run이 어떤 프롬프트로 돌았는지 재구성할 수 없다.

**(c) 한 과제를 위한 프롬프트 수정이 모든 향후 과제로 전파된다.**
`src-tauri/src/template/store.rs:20-81`의 `save()`는 통째로 `fs::write` 덮어쓰기이며 버전·이력·사본 개념이 없다. 템플릿에 한 번 쓰면 이전 내용이 즉시 소실된다. 그리고 아래 2.3에서 서술하듯, 현재 UI는 **실행하려면 반드시 이 덮어쓰기를 통과**하도록 되어 있다.

### 2.2 IMP-001 ~ IMP-011 — 기존 승인된 설계의 승계

> **이 부분은 새로 발명한 것이 아니다.** IMP-001 ~ IMP-011은 이미 다른 세션에서 승인된 설계 `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md` (Status: Approved)와 그에 대응하는 구현계획서 `docs/superpowers/plans/2026-09-07-pipeline-run-editing.md`의 결정 사항을 **그대로 승계**한 것이다. 본 PRD는 그 결정을 재심의하지 않으며, 표현만 개선 spec 형식으로 옮긴다.

승계되는 확정 결정은 다음과 같다.

- **Decision 1 (설계서 30-35행)** — 실행 전 편집 게이트의 적용 범위는 *모든 단계*다. 체크포인트 여부와 무관하게 모든 단계가 새 상태 `awaiting-stage-start`로 실행 직전에 정지하며, 기존의 "checkpoint:false 단계는 무정지 연속 실행" 빠른 경로는 제거된다. 승인은 이제 다음 단계를 즉시 실행하지 않고 다음 단계의 정지 지점까지만 진행시킨다(설계서 96-98행). → IMP-001
- **Data Model Changes (설계서 47-69행)** — `RunStatus::AwaitingStageStart`, `StageStatus::AwaitingStart` 신설, `RunRecord.resolvedStages: Stage[]` 추가. → IMP-002
- **Decision 2 (설계서 36-41행)** — `<targetDir>/.claude-pipeline-wizard/{run.json, pipeline.json}`을 모든 상태 전이마다 기록한다. 이는 기존 `app_data_dir/runs/` 사본을 대체하지 않고 **추가로** 쓴다. → IMP-003
- **Execution Flow (설계서 75-88행)** — `start_run`은 아무것도 실행하지 않고 레코드만 만들며, 신설 `start_stage(runId, stageOverride?)`가 **유일한 프로세스 스폰 경로**가 된다. → IMP-004
- **Decision 3 (설계서 42-45행)** — pre-start 게이트에서의 편집은 그 run의 `resolvedStages`만 갱신하고 저장된 갤러리 템플릿에는 **절대** 쓰지 않는다. → IMP-006
- **request_changes 변경 (설계서 102-108행)** — 재실행할 단계를 원본 템플릿이 아닌 `resolvedStages`에서 읽는다. → IMP-007

여기에 계획서가 정의한 지원 태스크가 따라붙는다: `validate_stage` 분리(Task 1 → IMP-005), 공유 `StageFields` 컴포넌트 추출(Task 7 → IMP-008), 실행 화면 편집 패널 + App.tsx 낙관적 레코드 정합화(Task 8 → IMP-009), 프론트 타입·API 래퍼 동기화(Task 6 → IMP-010), README 갱신 및 수동 스모크 5항목(Task 9 → IMP-011).

### 2.3 IMP-012 ~ IMP-022 — 기존 설계에 없던 추가 발견

아래 항목들은 승인된 설계서·계획서 어디에도 없다. 원인 커밋과 선례 버그를 역추적한 결과, **기존 설계를 그대로 구현해도 남는 문제**로 확인된 것들이다.

#### (1) 원인 커밋 a246384 / f3be3bf의 오염 경로가 기존 설계로 해결되지 않는다

이 프로젝트가 "실행 전에 프롬프트를 손보고 싶다"는 요구를 처음 받아들인 방식은 **갤러리의 직접 실행 버튼을 없애고 실행 진입점을 Template Editor 안으로 옮기는 것**이었다(커밋 `a246384` — "require editing before running a pipeline (remove direct run from gallery)", "so a user always passes through the manual edit screen before a run starts"). 그 결과 `src/pages/TemplateEditor.tsx:92-114`의 `persistTemplate()`은 저장 버튼과 실행 버튼 양쪽에서 `saveTemplate(template)`를 먼저 호출한 뒤 `onRun(template)`으로 넘어간다. 즉 **"실행"이 구조적으로 "원본 템플릿 덮어쓰기"를 동반**한다.

승인된 설계와 계획서는 실행이 *시작된 뒤*의 단계별 편집만 run 범위로 격리할 뿐, 이 진입점 자체는 손대지 않는다. 계획서 Task 7(1456-1462행)은 `TemplateEditor.tsx`를 `StageFields` 사용으로 리팩터링만 하고, Task 8(1693-1697행)은 `PipelineRun`/`App.tsx`만 수정하며, 계획서 전체에서 `persistTemplate`/`saveTemplate` 호출 경로를 바꾸는 단계가 **없다**(grep 결과 `saveTemplate`은 1389행 `api.test.ts` import 목록에만 등장). 따라서 pre-stage 게이트를 완성해도 "실행하려고 편집기를 지나가면서 원본이 오염되는" 최초 경로는 그대로 남는다. → **IMP-012**

같은 성격의 두 번째 경로가 실행 화면에 있다. 커밋 `f3be3bf`("show configured stage names instead of raw ids, add edit-and-retry" — "so a failed or completed run can be edited and re-run without losing context")는 실행 화면 하단에 '템플릿 편집' 버튼을 의도적으로 추가했고, 이 버튼은 `onEditTemplate(template)`으로 **원본** 템플릿 편집 화면으로 이동한다(`src/pages/PipelineRun.tsx:191-238`). 기존 설계는 실행 화면 *안*에 run 범위 편집 패널(IMP-009)을 추가하지만, 이 기존 버튼을 제거·경고·리라벨하지 않는다(계획서 Task 8 테스트는 `onEditTemplate={vi.fn()}`을 그대로 넘길 뿐 동작 변경 단계가 없음, 1765-1797행). 결과적으로 실행 화면에 "이 run만 고치는 경로"와 "원본을 고치는 경로"가 나란히 놓인다. → **IMP-013**

#### (2) 선례 버그 08e9144 / 0235a40 / dec3360의 교훈이 신규 상태에 부분적으로만 반영됐다

커밋 `08e9144`("Recover run status to Failed when drive() errors mid-checkpoint")는 run이 디스크에 `status:'running'`으로 영구히 끼어 승인·수정요청·취소가 모두 불가능해지던 버그를 고친 선례다. 원인은 `reject_checkpoint`가 `AwaitingCheckpoint` 상태만 허용한다는 점이었다. **동일한 문제가 신규 상태 `awaiting-stage-start`에 그대로 재현된다**: 계획서의 `reject_checkpoint`는 여전히 `status != AwaitingCheckpoint`이면 에러이고(1222-1231행), `start_stage`는 `AwaitingStageStart`만 허용하며(1097-1099행), 계획서 어디에도 `awaiting-stage-start`에서의 cancel/reject 전이가 없다. 실행 전 게이트에서 멈춘 run은 취소도 거부도 불가능하고 오직 '실행'만 가능하다. → **IMP-014**

같은 유형의 재발 경로가 하나 더 있다. `start_stage`는 `begin_current_stage()`로 상태를 Running으로 올린 뒤 `save()`를 호출하는데(1110-1111행), `save`는 `run_store.save` 후 `write_project_manifest`를 하고 실패 시 `?`로 즉시 전파한다(1058-1068행). targetDir 삭제·읽기전용·네트워크 드라이브 단절로 매니페스트 쓰기가 실패하면 `app_data_dir` 사본에는 이미 Running이 기록된 채 에러만 올라가고, 그 run은 `start_stage`도 `approve/reject`도 불가능한 끼임 상태가 된다. 계획서는 `run_stage` 결과에 대해서는 `_ => mark_current_failed`로 방어를 명시했으나(1137-1152행) `save` 실패에 대한 정책은 없다. → **IMP-015**

경합 방어는 대체로 승계됐다: `dec3360`의 프론트 이중 클릭 가드는 계획서 Task 8의 `handleStartStage`가 `setBusy(true)`/`disabled={busy}`로(1832-1843, 1860행), `0235a40`의 "다음 단계 실행 전 Running 전이를 디스크에 먼저 저장" 방어는 `start_stage`의 `begin_current_stage() → save() → run_stage()` 순서로 승계했다. 다만 백엔드에서 두 `start_stage` 호출이 각각 `run_store.load`로 같은 레코드를 읽고 순차 save/spawn하는 TOCTOU 경합은 여전히 막히지 않는다(1096-1111행에 락 없음). 단계별 수동 실행 버튼이 생겨 사용자가 실행을 트리거하는 **횟수 자체가 늘어나므로**, `0235a40`이 막으려 했던 "동일 타겟 폴더에서 두 개의 claude 프로세스" 위험을 다시 점검해야 한다. → **IMP-016**

#### (3) 매니페스트 도입이 새로 만드는 문제

프로젝트 로컬 매니페스트는 경로가 targetDir당 고정(`.claude-pipeline-wizard/run.json`)이므로, 같은 폴더에 두 번째 run을 시작하는 순간 이전 run의 상태와 resolvedStages 스냅샷이 조용히 소실된다. 사본 도입 이전에는 runId별 파일이라 **없던 데이터 손실 경로**다. `orchestrator.rs:43-44`에는 이미 "타겟 디렉토리 비어있음 검사는 프론트 UI가 필요해 이번 범위 밖"이라는 미해결 주석이 남아 있고, 계획서의 새 `start_run`도 `create_dir_all`만 하고 기존 run 존재 여부를 검사하지 않는다(1076-1084행). → **IMP-017**

또한 새 매니페스트는 IPC 경계를 넘어온 `targetDir: String` 아래에 디렉터리를 만들고 파일을 쓰는데, `write_project_manifest`는 `target_dir`을 그대로 join할 뿐 정규화·심볼릭 링크·쓰기 가능 여부를 검사하지 않는다(510-512, 600-611행). 커밋 `74408c7`/`1b2ba18`이 template id/run id에 `is_valid_id`를 적용해 path traversal을 차단한 선례가 있으므로, targetDir 검증 정책도 명시적으로 정해 선례와 일관되게 두는 편이 안전하다. → **IMP-019**

매니페스트가 "실행 감사 기록"으로서 불완전한 지점도 남는다. `request_changes`의 피드백 프롬프트는 일회성이라 `resolvedStages`에 기록하지 않도록 설계에 명시돼 있고(설계서 102-108행), `StageRun { id, status, session_id, log }`에도 실행된 프롬프트를 담는 필드가 없다(`run_record.rs:27-34`). 따라서 수정요청으로 재실행한 단계는 "어떤 프롬프트로 실행됐는지"를 run.json/pipeline.json만으로 재구성할 수 없다. → **IMP-020**

#### (4) 스키마 변경 안전장치 부재

계획서 Task 2는 `RunRecord`에 `pub resolved_stages: Vec<Stage>`를 **필수 필드**로 추가하며 `#[serde(default)]` 등 하위 호환 장치를 두지 않는다(165-175행, 계획서 전체에 `serde(default)` 없음). 그 결과 앱 업데이트 후 `<app_data_dir>/runs/`에 남아 있던 기존 run.json들은 `resolvedStages` 키가 없어 `RunRecordStore::load`에서 역직렬화에 실패한다. `RunStatus`/`StageStatus`에 새 값이 추가되는 것은 읽기 방향으로 호환되지만 이 필드 추가는 아니다. `database-design.md`(2.4절)도 마이그레이션 도구·트랜잭션이 전혀 없음을 명시한다. → **IMP-018**

#### (5) 문서·리포지토리 위생

승계 대상 두 문서가 git untracked이고 전 태스크 체크박스가 미완료 상태다(IMP-021). 그리고 계획서 Task 9는 `README.md`만 수정 대상으로 잡고 있으나(1945-1946행), 이 프로젝트에는 `dev-docs-builder-v2`로 생성된 `documents/` 8종과 통합 포털 `docs-portal.html`이 있고 그중 여러 개가 현재 구조를 직접 기술한다 — `database-design.md`는 저장소가 두 개뿐이라고, `system-architecture.md`는 유일한 정지 지점이 체크포인트 사후라고 명시하며, `user-guide.md:246`은 README의 Known v1 limitations를 그대로 참조한다(IMP-022).

---

## 3. 개선 목표

백로그 22개 항목을 6개 목표로 재구성한다. 각 목표에 속한 항목 id를 병기한다.

### 목표 A — 실행 모델 목표: 모든 단계에 실행 전 편집 게이트를 도입한다
*(승계: 설계서 Decision 1 / Data Model Changes / Execution Flow)*

- **IMP-001** — 모든 단계가 실행 직전 `awaiting-stage-start`로 정지. `checkpoint:false` 무정지 연속 실행 빠른 경로 제거. `checkpoint:true`의 의미(단계 종료 *후* 정지해 승인/수정요청/거부)는 불변이며, 승인은 다음 단계의 정지 지점까지만 진행시킨다.
- **IMP-002** — `RunStatus::AwaitingStageStart` / `StageStatus::AwaitingStart` 신설(`pending`은 "아직 도달하지 않은 단계"용으로 유지), `RunRecord.resolved_stages: Vec<Stage>` 추가. `RunRecord::new` 시그니처가 `&[String]` → `&[Stage]`로 변경되고, `current_resolved_stage` / `apply_stage_override` / `begin_current_stage`가 추가되며, `approve_current`는 다음 단계를 `Running`이 아니라 `AwaitingStart`로 전진시킨다.
- **IMP-004** — `start_run`은 동기 함수가 되어 레코드 생성·저장만 하고 아무것도 실행하지 않는다. 신설 `start_stage(run_id, stage_override, on_event)`가 유일한 프로세스 스폰 경로가 되며, override는 id 일치 검사 + `validate_stage` 후 `resolvedStages[currentStageIndex]`를 교체·즉시 영속화한다. Tauri 커맨드 `start_stage` 추가 및 `lib.rs generate_handler` 등록, `pipeline://stage-event` 발행 책임 이동, `OrchestratorError`에 `NotAwaitingStageStart`/`StageIdMismatch`/`StageValidation`/`ProjectManifest` 추가.

### 목표 B — 관측성 목표: 과제 폴더만 열어도 run 상태를 확인·수정할 수 있게 한다
*(승계: 설계서 Decision 2 + 그 도입이 새로 만드는 문제 3건)*

- **IMP-003** — `engine/project_manifest.rs` 신설. `manifest_dir(target_dir) = target_dir.join(".claude-pipeline-wizard")`, `write_project_manifest(...)`가 모든 상태 전이마다 `run.json`(전체 RunRecord)과 `pipeline.json`(resolved_stages 기반 Template 스냅샷)을 기록. 기존 `app_data_dir/runs/` 사본을 대체하지 않고 추가로 쓴다.
- **IMP-017** — 동일 타겟 폴더 재실행 시 매니페스트가 경고 없이 덮어쓰이는 문제. 기존 run 감지 시 **차단·경고·runId별 하위 경로** 중 어느 정책을 쓸지 결정이 필요하다.
- **IMP-019** — targetDir 경로 안전성 검토(`74408c7`/`1b2ba18` 선례 적용). 절대 경로 요구/정규화 후 검사 등 검증 정책을 명시적으로 결정한다.
- **IMP-020** — `request_changes` 피드백 프롬프트가 어디에도 기록되지 않아 매니페스트가 실행 감사 기록으로 불완전. Decision 3(템플릿·resolvedStages 비오염)을 지키면서 실행 로그 측에 실제 사용 프롬프트를 남길지 결정한다.

### 목표 C — 데이터 보전 목표: 한 과제를 위한 편집이 원본 템플릿을 오염시키지 않게 한다
*(승계 Decision 3 + 그 설계가 놓친 오염 경로 2건)*

- **IMP-006** — pre-start 편집은 그 run의 `resolvedStages`(및 스냅샷 `pipeline.json`)만 갱신하고 `<app_data_dir>/templates/{id}.json`에는 절대 쓰지 않는다. 이 방향의 쓰기 경로가 생기지 않았음을 **테스트로 고정**해야 한다.
- **IMP-007** — `request_changes`가 원본 템플릿 재로드 대신 `record.resolved_stages[stage_index]`에서 재실행 단계를 읽어 pre-start 편집을 상속한다. prompt만 피드백 텍스트로 덮어쓰되 그 덮어쓰기는 일회성이므로 `resolvedStages`에 기록하지 않는다. 동시에 `approve_checkpoint`가 매번 원본을 재로드해 run 도중 템플릿 편집이 새어들던 결합도 해소된다.
- **IMP-012** — `persistTemplate()`의 "저장 후 실행" 결합 해소. 실행 시 저장 없이 진행 가능하게 할지, 편집기의 실행 버튼을 제거하고 갤러리 직접 실행 + pre-stage 게이트로 대체할지 결정이 필요하다.
- **IMP-013** — 실행 화면의 '템플릿 편집'(원본) 버튼과 신규 run 범위 편집 패널의 구분. UI에서 두 경로를 명시(라벨/설명/확인 절차)하거나 실행 중 원본 편집 진입을 제한할지 결정이 필요하다.

### 목표 D — 안정성 목표: 새 상태 머신이 선례 버그를 재발시키지 않게 한다
*(전부 추가 발견 — 기존 설계에 없음)*

- **IMP-014** — `awaiting-stage-start`에서의 cancel/reject 전이를 상태 머신과 UI 양쪽에 정의. (`08e9144` 재발 방지)
- **IMP-015** — `start_stage`의 `save` 실패 시 Running 고착 방지. 매니페스트 쓰기 실패를 치명적 오류로 볼지(상태 롤백) 경고로 흘릴지 정책 결정.
- **IMP-016** — `start_stage`의 load-check-save 구간 원자성. TOCTOU 경합을 최소한 통합 테스트로 고정하고 동일 폴더 이중 claude 프로세스 위험을 재점검.
- **IMP-018** — `resolvedStages` 필드 추가에 대한 역직렬화 마이그레이션 전략. `#[serde(default)]` 부여 / 기존 레코드 폐기·이관 / 스키마 버전 필드 도입 중 하나를 결정.

### 목표 E — 사용성 목표: 실행 화면에서 다음 단계를 보고 고칠 수 있게 한다
*(승계: 계획서 Task 6/7/8)*

- **IMP-009** — `run.status === 'awaiting-stage-start'`일 때 "다음 단계 — 실행 전 확인/수정" 카드에 `StageFields(lockId)`를 `resolvedStages[currentStageIndex]`로 프리필하고, '이 단계 실행' 버튼이 `startStage(run.runId, stageDraft)`를 호출한다. 편집 초안은 `run.status`/`currentStageIndex`/`resolvedStages` 변화에 `useEffect`로 동기화. `App.tsx`의 낙관적 `pendingRun`도 `status:'awaiting-stage-start'`, `stage0:'awaiting-start'`, `resolvedStages:template.stages`로 교체해야 실제 레코드가 거치지 않는 `status:'running'`이 화면에 잠깐 보이는 일이 없다.
- **IMP-008** — `src/components/StageFields.tsx` 신설(디렉터리 자체가 아직 없음). name/id/prompt/permissionMode/allowedTools/checkpoint 폼과 `PERMISSION_MODES`·`KNOWN_TOOLS` 상수를 이동해 Template Editor와 실행 화면이 단일 출처를 공유. props는 `{ stage, onChange, idPrefix, promptLabel, lockId }`이며 `lockId`는 id 입력을 숨기지 않고 비활성화한다.
- **IMP-010** — `types.ts`에 `'awaiting-start'`/`'awaiting-stage-start'`/`resolvedStages: Stage[]` 추가, `api.ts`에 `startStage(runId, stageOverride?): Promise<RunRecord>` 추가. 프론트 타입이 Rust 모델의 수작업 1:1 미러라는 이중 정의 구조를 유지보수 리스크로 함께 인지한다.

### 목표 F — 기반 정비 목표: 검증 경로·기준선·문서를 코드와 일치시킨다

- **IMP-005** — `validate_stage(stage: &Stage) -> Result<(), TemplateValidationError>` 분리(id 문자셋 + 빈 프롬프트만 검사). `validate_template`은 이를 per-stage 호출하도록 리팩터링하되 중복 id·빈 stage 목록 검사는 템플릿 레벨에 유지하며, **관찰 가능한 `validate_template` 동작은 변하지 않아야 한다**.
- **IMP-011** — README의 'How it works'를 "모든 단계가 실행 전에 멈춘다"로 수정, Project layout에 `components/StageFields.tsx`·`engine/project_manifest.rs` 추가, Known v1 limitations에 "프로젝트 로컬 매니페스트는 조회를 쉽게 할 뿐 resume을 해결하지 않는다" 추가(기존 'No cross-restart resume UI' 항목은 유지). 아울러 실 CLI(`npx tauri dev`)로 다음 수동 스모크 5항목을 실행한다 — (a) run 시작 시 아무것도 실행되기 전에 1단계 편집 패널에 도달, (b) 편집한 프롬프트가 실제로 실행됨(라이브 로그로 확인), (c) targetDir의 `run.json`/`pipeline.json`이 존재하고 전이마다 갱신, (d) non-checkpoint 단계 종료 후 자동 실행 없이 다음 단계 편집 패널로 복귀, (e) 체크포인트 승인 후에도 자동 실행 없이 편집 패널로 복귀.
- **IMP-021** — 승계 대상 두 문서를 커밋해 기준선 고정. 그 밖의 미커밋 변경(`package.json`, `Cargo.toml/lock`, `tauri.conf.json` — installer bump-version 부산물)은 분리 커밋.
- **IMP-022** — 문서 갱신 범위를 `documents/` 8종 + `docs-portal.html`까지 확장. 특히 `database-design.md`에 프로젝트 로컬 매니페스트를 **세 번째 저장소**로 추가하고, `system-architecture.md`의 정지 지점 기술과 `user-guide.md`의 한계 참조를 갱신한다.

---

## 4. 우선순위

백로그의 priority 값을 그대로 유지한다. 각 항목에 백로그 `id`를 병기해 TRD에서 역참조할 수 있게 한다.

### 4.1 High (11건)

| id | 항목 | 카테고리 | 목표 | 묶음 |
|---|---|---|---|---|
| IMP-001 | 모든 단계에 실행 전 편집 게이트(`awaiting-stage-start`) 도입 — 비체크포인트 단계 자동 연속 실행 제거 | architecture | A | 승계 |
| IMP-002 | `RunRecord.resolvedStages` 추가 및 신규 상태값 2개 도입 | architecture | A | 승계 |
| IMP-003 | 프로젝트 로컬 매니페스트 writer 신설 — `<targetDir>/.claude-pipeline-wizard/{run,pipeline}.json` | architecture | B | 승계 |
| IMP-004 | `start_stage` 커맨드 신설 및 `start_run`/`approve_checkpoint` 비실행화 — 단계 실행 경로 단일화 | architecture | A | 승계 |
| IMP-006 | 런타임 단계 편집은 run 범위 한정 — 저장된 갤러리 템플릿에 절대 역기록하지 않음 | architecture | C | 승계 |
| IMP-009 | PipelineRun에 실행 전 편집 패널 추가 및 App.tsx 낙관적 레코드 정합화 | ux | E | 승계 |
| IMP-012 | 커밋 a246384가 만든 결합(실행 = 원본 템플릿 강제 저장)을 이 설계가 해결하지 않음 | ux | C | 추가 |
| IMP-013 | 커밋 f3be3bf가 추가한 실행 화면의 '템플릿 편집' 버튼(원본 오염 경로)이 설계에서 미처리 | ux | C | 추가 |
| IMP-014 | `awaiting-stage-start`에서 취소/중단 경로가 없음 — 08e9144 '상태 끼임' 선례 미반영 | bug | D | 추가 |
| IMP-017 | 동일 타겟 폴더 재실행 시 매니페스트가 경고 없이 덮어쓰기됨 | bug | B | 추가 |
| IMP-018 | `resolvedStages` 역직렬화 마이그레이션 전략 부재 — 기존 run.json 로드 불가 | bug | D | 추가 |

### 4.2 Medium (11건)

| id | 항목 | 카테고리 | 목표 | 묶음 |
|---|---|---|---|---|
| IMP-005 | `validate_stage`를 `validate_template`에서 분리 — 단일 stage override 검증 경로 확보 | tech-debt | F | 승계 |
| IMP-007 | `request_changes`를 원본 템플릿이 아닌 `resolvedStages` 기준으로 재실행 | bug | C | 승계 |
| IMP-008 | 공유 `StageFields` 컴포넌트 추출 — Template Editor와 실행 화면이 같은 폼 사용 | tech-debt | E | 승계 |
| IMP-010 | 프론트 타입·API 래퍼를 신규 상태/필드/커맨드에 맞춰 동기화 | tech-debt | E | 승계 |
| IMP-011 | README 갱신 및 수동 스모크 테스트 5항목을 합격 기준으로 실행 | tech-debt | F | 승계 |
| IMP-015 | `start_stage`에서 매니페스트 쓰기 실패 시 run이 Running으로 고착 — 08e9144류 재발 경로 | bug | D | 추가 |
| IMP-016 | `start_stage`의 load-check-save 구간에 원자성 보장이 없음 | bug | D | 추가 |
| IMP-019 | targetDir 하위 매니페스트 쓰기에 대한 경로 안전성 검토 — 74408c7/1b2ba18 선례 적용 | tech-debt | B | 추가 |
| IMP-020 | `request_changes` 피드백 프롬프트가 기록되지 않아 매니페스트가 감사 기록으로 불완전 | architecture | B | 추가 |
| IMP-021 | 승인된 설계서와 구현 계획서가 git untracked 상태 — 기준선 고정 필요 | tech-debt | F | 추가 |
| IMP-022 | 문서 갱신 범위가 README에 한정 — `documents/` 8종·포털·세 번째 저장소 반영 누락 | tech-debt | F | 추가 |

### 4.3 Low (0건)

백로그에 `priority: "low"` 항목은 없다. 임의로 항목을 강등하거나 생성하지 않는다.

### 4.4 실행 순서에 대한 제약

우선순위와 별개로 다음 순서 제약이 있다. 1·2는 백로그에 명시된 제약이고, 3은 백로그 항목들로부터 이 PRD가 도출한 제약이므로 TRD 단계에서 재확인이 필요하다.

1. **IMP-021(기준선 커밋)은 모든 구현에 선행**한다 — 이 PRD·TRD가 인용하는 근거 문서가 버전 관리 밖에 있으면 안 된다. *(백로그 IMP-021 명시: "구현 착수 전에 두 문서를 커밋해 기준선으로 고정해야 한다")*
2. **IMP-018(마이그레이션)은 IMP-002(필드 추가)와 동시에 결정**되어야 한다 — 필드가 먼저 들어가면 기존 레코드가 즉시 로드 불가가 된다. *(백로그 IMP-018 및 gaps 6번이 이 동시 결정을 요구)*
3. **IMP-012/IMP-013(오염 경로)은 IMP-006(run 범위 격리)과 함께 판단**하는 것을 권고한다 — IMP-006만 구현하면 "run 안은 깨끗한데 진입할 때 오염된다"는 절반의 해결에 그친다. *(이 제약은 백로그에 명시된 것이 아니라 IMP-006·012·013의 내용에서 도출한 것이다.)*

---

## 5. 성공 기준

각 목표마다 측정 가능한 완료 조건을 정의한다. 괄호 안은 해당 기준이 커버하는 백로그 id다.

### 5.1 목표 A — 실행 전 편집 게이트

- **A-1** run을 시작해도 `start_run` 반환 시점까지 claude 프로세스가 **0개** 스폰된다. 첫 프로세스는 사용자가 '이 단계 실행'을 누른 뒤에만 생긴다. (IMP-001, IMP-004)
- **A-2** `checkpoint: false`인 단계가 종료된 뒤, 자동 실행 없이 run이 다음 단계의 `awaiting-stage-start`에 멈춘다. 전체 N단계 파이프라인에서 사용자가 실행을 명시적으로 지시한 횟수가 **N회**와 일치한다. (IMP-001)
- **A-3** 체크포인트 승인 후에도 다음 단계가 즉시 실행되지 않고 `awaiting-stage-start`에 멈춘다. (IMP-001)
- **A-4** 프로세스 스폰 코드 경로가 `start_stage` **한 곳**뿐임이 코드/테스트로 확인된다(`start_run`·`approve_checkpoint`에 스폰 없음). (IMP-004)
- **A-5** `RunRecord` 직렬화 결과에 `resolvedStages` 배열이 존재하고, `RunStatus`에 `awaiting-stage-start`, `StageStatus`에 `awaiting-start` 값이 실제로 관측된다. `pending`은 여전히 "아직 도달하지 않은 단계"에만 쓰인다. (IMP-002)

### 5.2 목표 B — 프로젝트 로컬 매니페스트

- **B-1** 실행 대상 폴더에 `.claude-pipeline-wizard/run.json`과 `pipeline.json`이 존재하며, 상태 전이가 일어날 때마다 두 파일의 내용이 갱신된다(전이 전후 파일 해시/mtime 비교로 검증). (IMP-003)
- **B-2** 매니페스트 도입 후에도 기존 `app_data_dir/runs/<runId>.json`이 계속 기록된다(대체가 아니라 추가임을 확인). (IMP-003)
- **B-3** 이미 `.claude-pipeline-wizard/`가 존재하는 폴더에 두 번째 run을 시작하려 하면, 결정된 정책(차단/경고/runId별 하위 경로) 중 하나가 실제로 발동하며 **이전 run 매니페스트가 조용히 소실되지 않는다**. (IMP-017)
- **B-4** targetDir 검증 정책이 코드에 존재하고, 정책 위반 입력(상대 경로 등 결정된 거부 대상)에 대해 매니페스트 쓰기가 시도되지 않는다. (IMP-019)
- **B-5** 수정요청으로 재실행된 단계에 대해, 실제 실행에 쓰인 프롬프트를 run 기록만으로 재구성할 수 있다 — 또는 "기록하지 않음"이 명시적 결정으로 문서화되어 있다. (IMP-020)

### 5.3 목표 C — 원본 템플릿 오염 차단

- **C-1** 동일 targetDir에 대해 **원본 템플릿을 한 번도 쓰기(`save_template`/`template_store.save`)하지 않고** 실행이 처음부터 끝까지 진행된다. IMP-012가 요구하는 결정(실행 시 저장 없이 진행 / 편집기의 실행 버튼 제거 후 갤러리 직접 실행 + pre-stage 게이트)이 어느 쪽으로 내려지든 이 기준은 동일하게 적용된다. 단, "현행 유지"가 선택되는 경우 이 기준은 무효가 되므로 그 선택 시 본 기준을 함께 개정해야 한다. (IMP-006, IMP-012)
- **C-2** pre-start 게이트에서 프롬프트를 편집한 뒤 run을 완주해도 `<app_data_dir>/templates/{id}.json`의 내용이 실행 전과 **바이트 단위로 동일**하다. 이 불변식을 고정하는 자동화 테스트가 존재한다. (IMP-006)
- **C-3** pre-start 게이트에서 permission mode / allowed tools / checkpoint를 편집한 단계를 체크포인트에서 수정요청으로 재실행하면, **그 편집이 유지된 채** 재실행된다(원본 템플릿 값으로 되돌아가지 않는다). (IMP-007)
- **C-4** run 진행 중에 다른 화면에서 템플릿을 수정해도, 진행 중인 run의 이후 단계에 그 수정이 반영되지 않는다. (IMP-007)
- **C-5** 실행 화면에서 "이 run만 고치는 경로"와 "원본 템플릿을 고치는 경로"가 UI상 구분되며, 사용자가 후자를 선택할 때 그 영향 범위를 인지할 수 있다(리라벨/설명/확인 절차/진입 제한 중 결정된 방식이 실제로 적용됨). (IMP-013)

### 5.4 목표 D — 안정성

- **D-1** **실행 전 게이트에서 멈춘 run을 거부(reject)/취소(cancel)할 수 있다** — UI에 액션이 존재하고, 수행 후 run이 최종 상태로 전이하며 디스크 레코드도 그 상태로 기록된다. (IMP-014)
- **D-2** `awaiting-stage-start` 상태에서 수행 가능한 액션이 '실행' 하나뿐인 상황이 더 이상 존재하지 않는다. (IMP-014)
- **D-3** targetDir을 쓰기 불가 상태로 만든 뒤 `start_stage`를 호출하면, run이 `Running`으로 고착되지 않는다 — 결정된 정책에 따라 상태가 롤백되거나 Failed로 전이하거나 경고 후 진행하며, 어느 경우든 **run이 "어떤 액션도 받지 않는 무한 대기" 상태로 남지 않는다** — 후속 액션이 가능하거나, 최종 상태(Failed 등)임이 UI에 명시적으로 드러난다. (`Failed`에서 재시도로 나가는 간선의 존재 여부 자체는 GAP-3 미결 사항이므로 이 기준이 그것을 확정하지 않는다.) (IMP-015)
- **D-4** 동일 run에 대한 `start_stage` 동시 호출(백엔드 레벨)에서 claude 프로세스가 2개 스폰되지 않음을 통합 테스트가 고정한다. (IMP-016)
- **D-5** `resolvedStages` 필드가 없는 **기존 형식의 run.json**을 앱이 로드했을 때, 결정된 정책(기본값 주입 / 명시적 폐기 안내 / 스키마 버전 기반 이관)대로 동작하며 **원인 불명 역직렬화 실패로 앱 기능이 막히지 않는다**. (IMP-018)

### 5.5 목표 E — 실행 화면 편집 UI

- **E-1** run 시작 직후 화면이 `status:'running'`을 거치지 않고 곧바로 1단계 실행 전 편집 패널에 도달한다(낙관적 레코드가 실제 상태 머신과 일치). (IMP-009)
- **E-2** 편집 패널에서 수정한 프롬프트가 **실제로 실행됨**이 라이브 로그로 확인된다. (IMP-009, IMP-011(b))
- **E-3** 편집 패널의 초안이 `currentStageIndex` 전진 시 다음 단계 값으로 자동 갱신된다(이전 단계 초안이 남지 않는다). (IMP-009)
- **E-4** stage 필드 폼이 `src/components/StageFields.tsx` 한 곳에 정의되고 TemplateEditor와 PipelineRun 양쪽이 이를 import한다. 실행 중 stage id 입력은 비활성(disabled)이되 표시된다. (IMP-008)
- **E-5** 프론트 타입/`api.ts`가 신규 상태·필드·커맨드를 포함하며 기존 프론트 테스트가 통과한다. (IMP-010)

### 5.6 목표 F — 기반 정비

- **F-1** `validate_stage`가 독립 함수로 존재하고 `start_stage`가 이를 호출한다. `validate_template`의 관찰 가능한 동작(중복 id·빈 stage 목록 검사 포함)은 리팩터링 전후 동일하며 기존 테스트가 무수정으로 통과한다. (IMP-005)
- **F-2** README의 'How it works', Project layout, Known v1 limitations가 갱신되고, 수동 스모크 5항목 (a)~(e)가 실 CLI(`npx tauri dev`)로 전부 통과한다. (IMP-011)
- **F-3** 설계서·계획서 두 문서가 git에 커밋되어 있고, installer bump-version 부산물 변경은 별도 커밋으로 분리되어 있다. (IMP-021)
- **F-4** `documents/database-design.md`에 프로젝트 로컬 매니페스트가 세 번째 저장소로 기술되고, `system-architecture.md`의 정지 지점 기술과 `user-guide.md`의 한계 참조가 갱신되며, `docs-portal.html`이 갱신된 내용을 반영한다. (IMP-022)

---

## 6. 범위 제외 (이번 spec에서 다루지 않음)

아래 7개 항목은 백로그의 `gaps`를 그대로 옮긴 것이다. **근거가 없어 판단을 보류한 지점이며, 이 PRD가 임의로 결론을 채우지 않는다.** TRD 작성자와 승인자는 이 항목들에 대해 별도 결정을 받아야 한다.

### ⚠️ GAP-3 (최우선 확인 필요) — 실패한 단계의 재시도 경로: 두 설계 문서 간 정합성 불명확

> **TRD 작성자가 반드시 알아야 할 미해결 지점.**
>
> - **원 설계문서(2026-09-06) Error Handling 절**은 실패한 단계에 대해 **"Retry(같은 세션 재개), Edit prompt & retry, Cancel run"** 세 가지를 제공한다고 기술한다.
> - **이번 addendum(2026-09-07) Execution Flow 4단계**는 **"비정상 종료 → stage failed, run failed(기존 실패 UX 유지)"** 로만 기술하고, **실패 후 재시도가 새 상태 머신에서 어떤 전이인지 명시하지 않는다.**
>
> 두 문서가 서로 다른 실패 후 동작을 말하고 있으며, 어느 쪽이 현재 의도인지 근거만으로는 확정할 수 없다. TRD에서 상태 머신 전이표를 그릴 때 **`Failed` 상태로부터 나가는 간선이 존재하는지 여부가 미정**이라는 뜻이다. 이 항목은 IMP-014(실행 전 게이트에서의 cancel)와도 인접하지만 별개의 미결 사항이다. **임의 확정 금지 — 사용자 결정 필요.**

### GAP-1 — 재시작 후 run 재접속(resume) UI

설계서 Non-Goals(110-116행)는 이를 명시적으로 범위 밖에 두고 "프로젝트 로컬 매니페스트는 조회·수동 수정만 쉽게 한다"고 못박았으나, `README.md:84-95`의 v1 한계 2는 이를 여전히 실사용 제약으로 남긴다. 사용자가 이 한계를 이번 범위에서 계속 감수하기로 했는지에 대한 직접 근거가 원자료에 없어 판단을 보류한다.

### GAP-2 — 긴 파이프라인에서 모든 단계가 정지함에 따른 조작 부담

Decision 1은 "checkpoint:false 무정지 연속 실행" 경로를 완전히 제거하며 이는 사용자 확정 사항으로 기록돼 있다. 다만 이후 `checkpoint` 플래그의 의미가 사실상 "사후 정지 여부"로만 축소되는 점, 그리고 "남은 단계 일괄 실행" 같은 예외 경로가 필요한지에 대한 근거가 없다. 임의 판단하지 않고 보류한다.

### GAP-4 — 매니페스트 쓰기 실패 시 정책

치명적 오류로 run을 중단할지, 경고로 흘리고 `app_data_dir` 사본만 유지할지에 대한 결정 근거가 없다. 계획서는 `OrchestratorError::ProjectManifest` 변형 추가만 명시하고 복구/폴백 정책을 정하지 않았다. (IMP-015가 이 결정을 요구하는 항목이며, 결정 자체는 이 spec 밖에서 내려져야 한다.)

### GAP-5 — 타겟 프로젝트 폴더에 `.claude-pipeline-wizard/`를 남기는 것에 대한 정책

대상 프로젝트의 `.gitignore` 처리, run 완료 후 정리 여부, 민감한 프롬프트가 프로젝트 폴더에 남는 문제 — 설계서·계획서·원자료 어디에도 언급이 없다.

### GAP-6 — 기존에 진행 중이던 run들의 처리

`app_data_dir/runs/`에 남아 있는 `status:'running'` 또는 `'awaiting-checkpoint'` 레코드를 새 상태 머신으로 어떻게 처리할지(이관/폐기/차단)에 대한 결정 근거가 없다. **IMP-018의 역직렬화 문제와 함께 결정되어야 하나 임의로 확정하지 않는다.**

### GAP-7 — 우선순위의 실사용 근거 부재

이 프로젝트에는 `CLAUDE.md`, 이슈 트래커, `.claude/agents`·`skills`가 없어(수집 단계에서 `sources_absent`로 기록) 사용자 피드백·불만의 1차 기록을 확인할 수 없었다. 사용자가 겪은 문제에 대한 근거는 git 커밋 메시지와 코드 구조에서 역추적한 것뿐이므로, **실제 사용 빈도·심각도 기반의 우선순위 재조정은 사용자 확인이 필요하다.** 즉 4장의 High/Medium 배치는 코드 근거 기반의 잠정치다.

---

## 부록 A — 백로그 id 커버리지 표

| id | 등장 섹션 |
|---|---|
| IMP-001 | 2.2, 3-목표A, 4.1, 5.1(A-1~A-3) |
| IMP-002 | 2.2, 3-목표A, 4.1, 5.1(A-5) |
| IMP-003 | 2.2, 3-목표B, 4.1, 5.2(B-1, B-2) |
| IMP-004 | 2.2, 3-목표A, 4.1, 5.1(A-1, A-4) |
| IMP-005 | 2.2, 3-목표F, 4.2, 5.6(F-1) |
| IMP-006 | 2.2, 3-목표C, 4.1, 4.4, 5.3(C-1, C-2) |
| IMP-007 | 2.2, 3-목표C, 4.2, 5.3(C-3, C-4) |
| IMP-008 | 2.2, 3-목표E, 4.2, 5.5(E-4) |
| IMP-009 | 2.2, 3-목표E, 4.1, 5.5(E-1~E-3) |
| IMP-010 | 2.2, 3-목표E, 4.2, 5.5(E-5) |
| IMP-011 | 2.2, 3-목표F, 4.2, 5.5(E-2), 5.6(F-2) |
| IMP-012 | 2.3(1), 3-목표C, 4.1, 4.4, 5.3(C-1) |
| IMP-013 | 2.3(1), 3-목표C, 4.1, 4.4, 5.3(C-5) |
| IMP-014 | 2.3(2), 3-목표D, 4.1, 5.4(D-1, D-2) |
| IMP-015 | 2.3(2), 3-목표D, 4.2, 5.4(D-3), 6-GAP-4 |
| IMP-016 | 2.3(2), 3-목표D, 4.2, 5.4(D-4) |
| IMP-017 | 2.3(3), 3-목표B, 4.1, 5.2(B-3) |
| IMP-018 | 2.3(4), 3-목표D, 4.1, 4.4, 5.4(D-5), 6-GAP-6 |
| IMP-019 | 2.3(3), 3-목표B, 4.2, 5.2(B-4) |
| IMP-020 | 2.3(3), 3-목표B, 4.2, 5.2(B-5) |
| IMP-021 | 2.3(5), 1.3, 3-목표F, 4.2, 4.4, 5.6(F-3) |
| IMP-022 | 2.3(5), 3-목표F, 4.2, 5.6(F-4) |

**커버리지: 22 / 22 (100%).** 백로그에 없는 개선 항목은 추가하지 않았다.
