# PRD — Claude Pipeline Wizard 개선 spec (실행 전 단계 편집 게이트 & 프로젝트 로컬 매니페스트 & 실행 화면 파일 탐색·편집)

- 문서 상태: **확정 (Approved)** — 1차(IMP-001~IMP-022) prd-reviewer 검토 완료 2026-09-08 / 2차(IMP-023~IMP-038, 목표 G·§7) 검토 완료 2026-09-09, 양차 모두 critical 이슈 없음. 검토 기록: `_workspace/p4_reviewer_prd-feedback.json`(2차 검토 결과로 갱신됨)
- 대상 프로젝트: `D:\Utility\Agents\ClaudeCodeWizard` (Claude Pipeline Wizard)
- 근거 백로그 (1차): `_workspace/p2_analyzer_improvement-backlog.json` (분석 시각 2026-09-08, items 22건 / gaps 7건, IMP-001 ~ IMP-022)
- 근거 백로그 (2차): `_workspace/p2_analyzer_improvement-backlog.json` (분석 시각 2026-09-09, items 16건 / gaps 8건, IMP-023 ~ IMP-038 — §1.4·§2.4·목표 G·§4.5·§5.7·§7·부록 B의 근거)
- 승계 대상 설계서: `docs/superpowers/specs/2026-09-07-pipeline-run-editing-design.md` (Status: Approved)
- 승계 대상 구현계획서: `docs/superpowers/plans/2026-09-07-pipeline-run-editing.md` (2038줄, 9개 태스크, 전 태스크 미착수)

---

## 1. 개요

### 1.1 이 spec이 다루는 것

Claude Pipeline Wizard는 사용자가 정의한 다단계 파이프라인(stage 목록)을 Claude CLI 프로세스로 순차 실행하는 Tauri 데스크톱 앱이다. 본 spec은 이 앱의 **파이프라인 실행 모델**을 다음 두 축으로 개선한다. *(이 두 축은 이미 구현·머지가 완료됐으며, 그 위에 얹는 세 번째 축은 §1.4에서 정의한다.)*

1. **실행 전 단계 편집 게이트(pre-stage edit gate)** — 모든 단계가 실행 직전에 정지하고, 사용자가 그 단계를 확인·편집한 뒤에야 프로세스가 스폰된다.
2. **프로젝트 로컬 매니페스트** — 실행 대상 폴더(`<targetDir>`) 안에 `.claude-pipeline-wizard/{run.json, pipeline.json}`을 남겨, 과제 폴더만 열어도 그 run이 무엇을 실행 중인지 확인·수정할 수 있게 한다.

그리고 이 두 축을 실제로 성립시키기 위해 반드시 함께 해결해야 하는 **원본 템플릿 오염 경로**, **상태 끼임/경합/마이그레이션 리스크**, **문서 정합성**을 함께 다룬다.

### 1.2 범위 구성 — 두 종류의 항목이 섞여 있음에 대한 명시

이 PRD의 1차 범위 22개 항목(IMP-001 ~ IMP-022)은 성격이 다른 두 묶음이다. 독자는 이 구분을 반드시 인지해야 한다. *(2차 범위 IMP-023 ~ IMP-038은 이 두 묶음 어디에도 속하지 않는 전부 신규 항목이며, 그 성격은 §1.4.2에서 따로 설명한다.)*

| 묶음 | 항목 | 성격 |
|---|---|---|
| **A. 기존 설계 승계** | IMP-001 ~ IMP-011 | 이미 다른 세션에서 `Status: Approved`로 확정된 2026-09-07 설계서와 그 9개 태스크 구현계획서의 결정 사항을 **그대로 옮긴 것**. 이 PRD가 새로 발명한 내용이 아니다. |
| **B. 추가 발견** | IMP-012 ~ IMP-022 | 위 승인된 설계·계획서에 **없던** 항목. 원인 커밋과 선례 버그를 역추적하는 과정에서 "이 설계로는 해결되지 않는다"고 확인된 잔존 문제들이다. |

### 1.3 선행 조건

승계 대상 두 문서(설계서·계획서)가 현재 git untracked 상태다. 구현 착수 전 이 두 문서를 커밋해 기준선으로 고정해야 한다(IMP-021). 이 PRD·후속 TRD가 인용하는 모든 근거 줄 번호가 그 기준선에 종속된다.

### 1.4 세 번째 축 — 실행 화면에서 targetDir을 들여다보고 고칠 수 있게 한다

§1.1이 정의한 두 축(실행 전 단계 편집 게이트 / 프로젝트 로컬 매니페스트)은 구현·머지가 완료됐다. 본 절 이하 IMP-023 ~ IMP-038은 그 위에 얹는 **세 번째 축**이다.

**사용자 요청 원문:** "template을 실행 하면 디렉토리를 선택 하는데, 실행 main 화면에 탭을 추가 해서 지정된 디렉토리의 서브 디렉토리와 파일등의 리스트를 보고 편집 기능을 추가 했으면해."

즉 파이프라인 실행 화면(`PipelineRun`)에 탭을 추가해, 그 run의 `targetDir` 아래 파일·서브디렉토리 목록을 보고 파일을 편집할 수 있게 한다.

#### 1.4.1 이 축이 앞의 두 축과 맺는 관계

- **목표 B(관측성)의 연장선이되 방향이 반대다.** 목표 B는 "앱을 열지 않고도 폴더를 들여다볼 수 있게" 매니페스트를 targetDir에 남겼다. 이번 축은 반대로 "앱을 떠나지 않고도 폴더를 들여다볼 수 있게" 한다.
- **목표 E(실행 화면 편집 UI)와 화면을 공유하되 편집 대상이 다르다.** 목표 E가 편집하는 것은 *다음 단계의 프롬프트*(`resolvedStages`)이고, 이번 축이 편집하는 것은 *claude가 만들어낸 산출물 파일*이다. 같은 화면에 두 종류의 "편집"이 놓이므로 UI에서 구분되어야 한다.
- **목표 D(안정성)가 세운 불변식을 깨뜨리지 않아야 한다.** 이번 축은 상태 머신에 새 전이를 추가하지 않는 **순수 부수 기능**으로 설계된다(IMP-027). 이는 이번 축의 설계 제약이자 성공 기준이다.

#### 1.4.2 이 축의 항목 성격 — 앞의 두 묶음과 다르다

§1.2가 구분한 "A. 기존 설계 승계 / B. 추가 발견"과 달리, IMP-023 ~ IMP-038은 **승계할 승인된 설계서가 존재하지 않는 전부 신규 항목**이다. 백로그가 확인한 대로 이 저장소에는 파일 탐색기·편집기를 시도한 커밋이 0건이고 관련 TODO·FIXME 주석도 0건이다. 따라서 이 축에는 "이미 확정된 결정을 옮긴 것"이 하나도 없으며, 반대로 **사용자 결정을 기다리는 미결 항목의 비중이 앞의 두 축보다 크다**(§7의 GAP-F1 ~ GAP-F8, 8건).

#### 1.4.3 선행 조건

이번 축은 IMP-001 ~ IMP-022의 구현 결과 위에서만 성립한다. 특히 `<targetDir>/.claude-pipeline-wizard/`(IMP-003), `run.targetDir`을 프론트가 보유하는 구조(IMP-009/IMP-010), `TargetDirInUse` 차단(IMP-017)이 전제다. 이 전제는 이미 머지되어 충족돼 있다.

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

### 2.4 IMP-023 ~ IMP-038 — 파일 탐색·편집이 마주하는 문제 구조

백로그 `summary`가 지목하는 핵심은 이렇다. **이 요청은 코드베이스에 존재하지 않는 두 가지를 동시에 신설해야 한다** — 탭이라는 UI 패턴과, 프론트가 호출 가능한 파일시스템 IPC. 그리고 가장 위험한 축은 기능이 아니라 **보안 경계**다.

#### (1) 놓을 자리가 없다 — 탭 UI도, 파일 IPC도 선례가 0이다

실행 화면은 탭이 없는 단일 2단 스크린이다. 최상위가 `display:flex` 2단이고, 좌측 240px 고정폭에 템플릿명·targetDir 표시·stage 타임라인이, 우측 `flex:1` 컬럼에 에러 alert → 상태 배지 → 라이브 로그 카드 → (조건부) 체크포인트 카드 → (조건부) 실행 전 편집 패널 → (조건부) 실패 안내 → 하단 고정 버튼 2개가 세로로 쌓인다(`src/pages/PipelineRun.tsx:231-358`). '파일' 탭을 넣으려면 우측 컬럼 전체를 탭 컨테이너로 감싸고 기존 7개 블록을 '실행' 탭 콘텐츠로 옮겨야 한다. `src` 전체 grep 결과 탭 컴포넌트·CSS 클래스가 전무해(매치 2건은 `StageFields.test.tsx`의 'editable' 오탐) 참고할 내부 선례가 없다. 사이드바 3항목 내비는 있으나 화면 안쪽 탭은 없으며, 새 탭은 사이드바가 아니라 `PipelineRun` 내부에 두는 것이 기존 구조와 맞는다. → **IMP-023**

백엔드 쪽도 같다. `lib.rs`의 `generate_handler!`에 등록된 커맨드는 12개(`list_templates`, `load_template`, `save_template`, `delete_template`, `check_cli`, `start_pipeline_run`, `start_stage`, `approve_checkpoint`, `request_changes`, `reject_checkpoint`, `cancel_run`, `get_run`)이고, **디렉토리 나열·파일 읽기·파일 쓰기 커맨드는 하나도 없다**(`src-tauri/src/lib.rs:13-14, 37-50`). 플러그인도 `tauri_plugin_dialog` 하나뿐이다. 프론트는 `run.targetDir` 문자열만 갖고 있을 뿐 그 폴더 내용을 볼 수단이 없다. 다만 `tokio`에 `fs` feature가 이미 켜져 있어(`src-tauri/Cargo.toml:22-32`) 새 크레이트 없이 비동기 나열·읽기 구현이 가능하고, 새 읽기 커맨드는 `get_run`이 남긴 "read-only, 상태를 바꾸지 않으므로 run lock을 잡지 않는다"는 선례를 그대로 재사용할 수 있다(`src-tauri/src/commands.rs:120-123`). → **IMP-024**

#### (2) 가장 위험한 축 — 프론트발 경로가 파일시스템에 닿는 두 번째 지점이 열린다

현재 프론트발 경로가 파일시스템에 닿는 유일한 지점은 `target_dir`이고, 그 검증은 `validate_target_dir` 하나다. 그런데 이 함수는 ① 절대경로 여부, ② `create_dir_all`, ③ `canonicalize` 후 `is_dir()`만 확인하고 **canonical 형태를 의도적으로 버린다** — Windows `canonicalize`가 `\\?\` UNC를 반환하는데 `target_dir`이 UI에 그대로 표시되기 때문이다(`src-tauri/src/engine/project_manifest.rs:36-54`). 즉 저장·표시되는 `target_dir`은 정규화되지 않은 원본 문자열이며, **하위 경로에 대한 containment 검사는 존재하지 않는다.**

다른 방어 장치인 `is_valid_id`는 슬래시·백슬래시를 문자셋에서 제외해 path traversal을 막지만(`src-tauri/src/template/mod.rs:46-51`), 파일 탐색기의 상대 경로는 슬래시를 포함할 수밖에 없어 **그대로 재사용할 수 없다.** 따라서 새 커맨드는 `../` 상위 탈출, 심볼릭 링크 경유 탈출, 절대경로 주입을 모두 차단하는 별도 정책(세그먼트 단위 검사 + canonical prefix 비교 등)을 스스로 설계·테스트해야 한다. 이 저장소가 경로 검증을 반복적으로 사후 보강해 온 이력(`74408c7`, `1b2ba18`, `0235a40`)을 감안하면, 이번에는 사후가 아니라 **최초 설계 시점에** 고정해야 한다. 현존 파일 I/O 4곳은 전부 백엔드가 경로를 조립하며, 프론트 전달 문자열 경로를 그대로 열어주는 커맨드는 지금 하나도 없다. → **IMP-025 (이번 축의 최우선 보안 항목)**

권한 경계 자체도 백지다. capability 파일은 `default.json` 하나뿐이고 permissions가 `["core:default", "dialog:allow-open"]` 두 줄이며, `tauri.conf.json`(전체 33줄)에는 **`app.security` 블록 자체가 없다**(`src-tauri/capabilities/default.json:1-7`). 이 프로젝트의 확립된 관례는 모든 파일 I/O를 Rust 커맨드 안에 두고 프론트에는 권한을 주지 않는 것이다. `tauri-plugin-fs` + `fs:allow-read`/`fs:allow-write` + scope로 여는 방식은 프론트에 임의 경로 접근 권한을 주게 되어 이 관례를 깨뜨린다. 어느 쪽을 택하든 결정과 근거를 문서에 남기고 `app.security`/capabilities를 명시적으로 갱신해야 한다. → **IMP-026**

#### (3) 기존 불변식을 건드리면 대가가 크다

TRD §3.2는 전이 T1~T11을 고정하며 그 핵심 불변식이 "프로세스 스폰은 T2(`start_stage`)와 T9(`request_changes`) 두 곳뿐이고 둘 다 `run_current_stage()` 단일 헬퍼를 경유한다"는 것이다(`TRD.md:182-219`). `Failed`에서 나가는 간선은 기존 GAP-3 미결로 의도적으로 없고, terminal은 `Completed`/`Cancelled`이며, "Running 상태에서의 사용자 취소 간선은 신설하지 않는다"가 명시적 비범위다. 새 파일 커맨드가 `RunStatus`/`StageStatus`를 건드리면 이 표 전체와 `orchestrator_test.rs`(30.6KB)를 재검증해야 한다. 따라서 읽기 커맨드는 `get_run`처럼 run lock을 잡지 않는 read-only로, 쓰기 커맨드도 상태 전이 없이 파일시스템만 만지는 형태로 둔다. 그렇게 하지 못할 이유가 생기면 그때 TRD 전이표를 정식으로 갱신해야 한다. → **IMP-027**

#### (4) 새로 생기는 데이터 손실 경로 — 이번 축의 유일한 bug 항목

`executor`의 `run_stage`는 `Command::new(claude_binary).current_dir(target_dir)`로 CLI를 스폰하고, `--permission-mode`(`acceptEdits`/`bypassPermissions`/`default`)에 따라 CLI가 **그 폴더의 파일을 직접 수정한다.** 스테이지 타임아웃은 30분이므로(`src-tauri/src/engine/executor.rs:42-53, 12`) `run.status === 'running'` 구간이 길게 유지될 수 있다. 이 구간에 파일 편집 탭이 저장을 허용하면 두 주체가 같은 파일을 동시에 쓰게 되어 **한쪽 변경이 조용히 사라진다 — 명백한 데이터 손실 위험이다.** 최소한 "실행 중 저장 차단" 또는 "저장 직전 mtime/내용 재확인 후 충돌 경고" 중 하나를 spec이 명시해야 한다. 이 저장소는 경합 방어를 반복적으로 사후 보강해 온 이력(`dec3360`, `0235a40`)이 있으므로 같은 실수를 반복하지 않아야 한다. 어떤 정책을 택할지는 §7 GAP-F4로 분리했다. → **IMP-028**

#### (5) 화면에 정보원이 둘이 된다

실행 화면은 이미 파일 경로를 부분적으로 추적한다. 스테이지 이벤트 중 `toolUse`이고 도구명이 `Write` 또는 `Edit`이면 `input.path`를 뽑아 `changedFiles`에 누적하고, 체크포인트 카드에서만 `.badge.mono` 칩으로 표시한다(`src/pages/PipelineRun.tsx:93, 117-132, 276-284`). 다만 이것은 실제 파일시스템을 읽은 결과가 아니라 **CLI 이벤트에서 파싱한 문자열**이고 `currentStageIndex`가 바뀌면 초기화된다. 새 탐색기가 실제 디렉토리를 보여주면 두 정보원이 화면에 공존하므로, '변경된 파일 강조' 형태로 연계하거나 최소한 두 목록의 의미 차이(이벤트 유래 vs 실측)를 UI에서 구분해야 한다. → **IMP-029**

그리고 `start_run`은 run 생성 시 `<targetDir>/.claude-pipeline-wizard/` 아래 `run.json`·`pipeline.json`을 **항상** 쓰므로(`src-tauri/src/engine/orchestrator.rs:113-135`), 탐색기가 보게 될 폴더에는 예외 없이 이 디렉토리가 존재한다. 이를 남기는 정책은 기존 §6 GAP-5로 이미 미결이고, 탐색기가 이를 사용자 눈앞에 노출하면 그 미결 항목이 다시 문제가 된다. 동시에 `README.md:27-29`는 "앱을 열지 않고도 폴더를 들여다볼 수 있다"를 매니페스트의 가치로 서술하고 있어 숨기는 선택과 긴장 관계다. 노출/숨김/토글 중 무엇을 택할지는 §7 GAP-F5로 분리했다. → **IMP-030**

#### (6) 기존 유지보수 부담이 그대로 확대된다

- `src/types.ts`는 TRD §3.11(IMP-010)이 명시하듯 Rust 모델의 **수작업 1:1 미러**이며 이중 정의가 유지보수 리스크로 이미 인지돼 있다. `RunRecord`만 해도 7개 필드를 손으로 맞춘다. 파일 탐색용 새 타입(디렉토리 엔트리, 파일 내용 응답 등)을 추가하면 같은 부담이 그대로 늘어난다. 최소한 명명 규약(Rust는 snake_case + `#[serde(rename_all = "camelCase")]`, TS는 camelCase)과 동기화 책임을 문서에 못 박아야 한다. → **IMP-031**
- `api.ts`는 `STALE_STAGE_INDEX_PREFIX` 상수와 `isStaleStageIndexError` 헬퍼를 갖고 있고, 주석이 밝히듯 **Tauri 커맨드는 모든 백엔드 에러를 문자열로 붕괴시키므로** 특별 처리가 필요한 에러는 Rust 쪽 prefix와 짝을 이루는 문자열 규약으로만 구분할 수 있다(`src/api.ts:73-79`). 파일 탐색·편집에도 '파일이 사라짐', '경로가 targetDir 밖', '파일이 너무 큼', '바이너리라 표시 불가'처럼 프론트가 다르게 반응해야 하는 에러가 생기므로 같은 규약을 확장하고 `documents/api-design.md` §3 에러 메시지 소스 표에 항목을 추가해야 한다. → **IMP-032**
- 프론트 테스트 5개 중 `PipelineRun.test.tsx`가 16,827바이트로 가장 크고 실행 화면 동작을 광범위하게 고정한다. 우측 컬럼을 탭 컨테이너로 감싸면 기존 블록들이 **탭 선택 상태에 종속된 조건부 렌더링**이 되므로 이 파일의 쿼리 다수가 영향을 받는다. 테스트 개정을 별도 작업 항목으로 잡고, 기존 실행 화면 동작(체크포인트 승인/수정요청/거부, 실행 전 편집, 실패 안내, 로그 표시)이 '실행' 탭 안에서 그대로 유지된다는 회귀 방어를 남겨야 한다. → **IMP-033**

#### (7) 마운트 타이밍 함정

`App.tsx`의 `handleRun`은 네이티브 다이얼로그로 고른 절대경로를 그대로 넣은 **낙관적 `pendingRun`**을 만들어 화면을 먼저 띄우고, 백엔드 `startPipelineRun` 응답이 오면 레코드를 교체한다(`src/App.tsx:63-97`). 이 낙관적 레코드 단계에서는 아직 백엔드가 폴더를 `validate_target_dir`로 검증하거나 `create_dir_all`로 생성하지 않은 상태다. 파일 탭이 마운트 즉시 `run.targetDir`을 나열하면 이 창에서 "존재하지 않는 디렉토리" 에러가 나거나, 더 나쁘게는 사용자가 원인을 모르는 빈 목록을 보게 된다. 탭은 백엔드 응답으로 레코드가 교체된 이후에만 나열을 시작하거나, 이 상태를 명시적 로딩/안내로 표시해야 한다. → **IMP-034**

#### (8) UI 수준 결정이 곧 의존성 정책 결정이 된다

프론트 dependencies는 `react`, `react-dom`, `@tauri-apps/api`, `@tauri-apps/plugin-dialog` **네 개뿐**이다(`package.json:13-30`). 트리 뷰, Monaco/CodeMirror 같은 코드 에디터, 신택스 하이라이터, 상태 관리 라이브러리가 모두 없다. Rust 쪽도 `.gitignore` 파싱이나 파일 타입 감지 크레이트가 없어 무시 규칙·텍스트 판별을 직접 구현해야 한다. 즉 "탐색기"와 "편집기"의 UI 수준을 어디까지 잡느냐가 곧 의존성 도입 여부 결정이며, 이 프로젝트의 기존 취향(의존성 최소, 단일 `styles.css`, CSS-in-JS·Tailwind 없음)과 충돌하지 않게 정해야 한다. 라이브러리 도입 여부 자체는 §7 GAP-F6으로 분리했다. → **IMP-035**

반면 시각 요소는 새로 만들 필요가 없다. 탭 전용 클래스는 없지만 `.segmented` / `.segmented__option` / `.segmented__option--selected`가 이미 존재하며 현재 permissionMode 선택 UI에서 쓰인다(`src/styles.css:442-470`). 이를 탭 스트립으로 전용하면 새 시각 언어를 도입하지 않고도 기존 화면과 일관된 모양을 얻는다. `.card`, `.badge` 계열, `.field`/`.input`/`.textarea`, `.mono`, `.help-text`, `.alert`도 파일 탭 내부에서 그대로 재사용 가능하다. → **IMP-036**

#### (9) 문서 위생 — 기존 선례(IMP-011/IMP-022)를 따른다

이 저장소는 코드 변경과 문서 갱신을 함께 가져가는 선례를 갖고 있다. 이번 기능은 세 곳을 건드린다. ① `README.md:74-98`의 Project layout 트리는 각 파일을 1줄로 설명하는 형식이므로 신규 컴포넌트·모듈 줄을 추가해야 한다. ② `documents/system-architecture.md:182`의 보안 표는 프로세스 권한(`permissionMode`)만 다루고 **앱 자체의 파일 접근 범위 항목이 없으므로** 새 항목이 필요하며, 컴포넌트 표의 `project_manifest` 서술도 조정 대상이다. ③ `documents/api-design.md`는 §4.1~4.3 구성이므로 '4.4 프로젝트 파일' 절 신설과 §3 에러 표 갱신이 자연스럽다. → **IMP-037**

부수적으로, `TRD.md:7` 머리말은 스택을 "React 19"로 적고 있으나 `package.json` 실측은 `react ^18.3.1`이다. 기능과는 무관하지만 이번에 TRD에 새 섹션을 덧붙이면서 같은 문서 안에 상충하는 스택 표기를 남기게 되므로 정정한다. → **IMP-038**

---

## 3. 개선 목표

1차 백로그 22개 항목(IMP-001 ~ IMP-022)을 6개 목표(A ~ F)로 재구성한다. 각 목표에 속한 항목 id를 병기한다. 2차 백로그 16개 항목(IMP-023 ~ IMP-038)은 목표 F 뒤의 **목표 G**로 재구성한다.

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

### 목표 G — 실행 화면에서 targetDir의 파일을 탐색·편집할 수 있게 한다
*(전부 신규 — 승계할 승인된 설계서가 없음. 미결 항목 8건은 §7 참조)*

**왜:** 사용자가 파이프라인 실행 결과를 확인하거나 손보려면 지금은 앱을 떠나 별도 파일 탐색기·에디터를 열어야 한다. 실행 화면은 claude가 무엇을 했는지(로그)와 다음에 무엇을 할지(프롬프트)는 보여주지만, **claude가 실제로 만들어낸 것**은 보여주지 않는다.

**누구를 위해:** 실행 중 진행 상황을 확인하려는 사용자, 체크포인트에서 승인 여부를 판단해야 하는 사용자, 실패한 run의 원인을 파악하려는 사용자. *(단, 이 세 시나리오 중 어느 것이 실제 요구인지에 대한 근거는 원자료에 없다 — §7 GAP-F8.)*

**무엇을:** `PipelineRun` 우측 컬럼을 탭 컨테이너로 만들고 '파일' 탭에서 `run.targetDir` 하위를 나열한다. 이때 **기존 상태 머신·보안 관례·최소 의존성 원칙을 깨뜨리지 않는다.**

> **✅ §7 GAP-F1 결정 (2026-09-09, 사용자): 이번 spec의 범위는 디렉토리 리스팅만이다.** 파일 내용 열람·편집·저장과 삭제·이름변경·새 파일 생성은 이번 범위에 포함되지 않으며, 사용자가 원하면 별도 후속 spec으로 연다. 이 결정이 아래 항목 형태를 좁히는 상세는 §7 GAP-F1을 참조 — 특히 IMP-028(bug)은 이번 spec에서 트리거되지 않고, IMP-035(에디터 라이브러리)는 적용 대상이 없다.

아래 5개 하위 묶음으로 16개 항목을 재구성한다.

#### G.1 — 화면 구조와 IPC 경로 신설 *(놓을 자리를 만든다)*

- **IMP-023** — `PipelineRun` 우측 컬럼 전체를 탭 컨테이너로 감싸고 기존 7개 블록(에러 alert / 상태 배지 / 라이브 로그 / 체크포인트 카드 / 실행 전 편집 패널 / 실패 안내 / 하단 버튼)을 '실행' 탭 콘텐츠로 이동시킨다. 탭은 사이드바가 아니라 실행 화면 내부에 둔다. 이 앱에 탭 UI 패턴이 존재한 적이 없으므로 신규 도입이며 참고할 내부 선례가 없다.
- **IMP-024** — 디렉토리 나열·파일 읽기(및 §7 GAP-F1에서 범위가 확정되면 쓰기) Tauri 커맨드를 신설하고 `lib.rs generate_handler!`에 등록한다. `commands.rs`의 기존 관례(`#[tauri::command]` + `State<Orchestrator>` + `Result<T, String>` + `map_err(|e| e.to_string())`, 본문에 비즈니스 로직 없음)를 따르며, 읽기 커맨드는 `get_run`의 "run lock을 잡지 않는 read-only" 선례를 재사용한다. `tokio`의 `fs` feature가 이미 켜져 있어 새 크레이트가 필요 없다.
- **IMP-035** — 트리 뷰·코드 에디터·신택스 하이라이터 의존성이 전무하므로 탐색·편집 UI를 자체 구현할지 라이브러리를 도입할지 결정한다. 이 결정은 기존 최소 의존성 관례(dependencies 4개, 단일 `styles.css`)를 바꾸는 결정이며, 도입 여부 자체는 §7 GAP-F6 미결이다.
- **IMP-036** — 탭 스트립 시각 요소는 기존 `.segmented` / `.segmented__option` / `.segmented__option--selected`를 전용하고, 탭 내부는 `.card`·`.badge`·`.field`/`.input`/`.textarea`·`.mono`·`.help-text`·`.alert`를 재사용해 **새 디자인 언어를 만들지 않는다.**

#### G.2 — 보안 경계 확정 *(이번 축의 최우선 묶음)*

- **IMP-025** — targetDir 하위 경로 containment 검증 정책을 새로 설계한다. 기존 두 장치 어느 쪽도 재사용할 수 없다: `validate_target_dir`은 canonical 형태를 UI 표시 목적으로 버려 하위 경로 containment 검사를 하지 않고, `is_valid_id`는 슬래시를 문자셋에서 배제해 상대 경로에 쓸 수 없다. 새 정책은 **`../` 상위 탈출, 심볼릭 링크 경유 탈출, 절대경로 주입 세 가지를 모두 차단**해야 하며 전용 단위 테스트로 고정한다.
- **IMP-026** — 앱의 파일 접근 권한 경계를 명시적으로 정한다. `tauri.conf.json`에 `app.security` 블록을 신설할지, capabilities에 fs 권한을 추가할지, 아니면 기존 관례대로 **모든 파일 I/O를 Rust 커맨드 안에 가두고 프론트에는 권한을 주지 않을지**를 결정하고 그 근거를 문서에 남긴다. `tauri-plugin-fs` 도입은 프론트에 임의 경로 접근 권한을 주게 되어 이 관례를 깨뜨린다는 점을 결정 근거에 포함한다.

#### G.3 — 기존 불변식 보존과 동시성 *(깨뜨리면 대가가 큰 묶음)*

- **IMP-027** — 파일 탐색·편집 커맨드는 TRD §3.2 상태 전이표(T1~T11)에 **새 전이를 만들지 않는 순수 부수 기능**으로 설계한다. 읽기 커맨드는 run lock 없는 read-only, 쓰기 커맨드도 상태 전이 없이 파일시스템만 만진다. `RunStatus`/`StageStatus` 값 집합은 변하지 않는다. 그렇게 하지 못할 이유가 생기면 그때 TRD 전이표를 정식으로 갱신한다.
- **IMP-028** *(이번 축의 유일한 bug 항목)* — `run.status === 'running'` 구간(claude 프로세스가 targetDir을 cwd로 점유, 최대 30분)에 편집 탭이 저장을 허용하면 두 주체가 같은 파일을 동시에 써 **한쪽 변경이 조용히 사라지는 데이터 손실**이 발생한다. §7 GAP-F4의 세 선택지(금지 / 경고 후 허용 / 낙관적 저장 후 충돌 감지) 중 하나를 반드시 명시한다 — 백로그 IMP-028이 최소선으로 든 "실행 중 저장 차단"과 "저장 직전 mtime/내용 재확인 후 충돌 경고"가 각각 첫째·셋째 선택지에 해당한다. 어느 것을 택할지는 §7 GAP-F4 미결이나, **"정책 없이 저장 기능을 여는 것"은 어느 경우에도 허용되지 않는다.**
- **IMP-034** — 낙관적 `pendingRun` 단계에서는 targetDir이 아직 `validate_target_dir` 검증·`create_dir_all` 생성을 거치지 않았다. 파일 탭은 백엔드 응답으로 레코드가 교체된 이후에만 나열을 시작하거나, 이 상태를 명시적 로딩/안내로 표시해 사용자가 원인 모를 빈 목록이나 에러를 보지 않게 한다.

#### G.4 — 기존 자산과의 통합 *(정보원·타입·에러·테스트)*

- **IMP-029** — 이미 추적 중인 `changedFiles`(스테이지 이벤트의 `Write`/`Edit` 도구에서 파싱, 체크포인트 카드에서만 표시, stage 전진 시 초기화)와 새 파일 탐색기를 연계한다. '변경된 파일 강조' 형태로 잇거나, 최소한 **두 목록의 의미 차이(이벤트 유래 vs 실측)를 UI에서 구분**해 사용자 혼란을 막는다.
- **IMP-030** — 파일 목록에 `.claude-pipeline-wizard/`를 노출할지 숨길지 토글로 둘지 결정한다. 이 디렉토리는 모든 targetDir에 예외 없이 존재하며, 기존 §6 GAP-5(디렉토리를 남기는 정책)를 재점화한다. 숨김 선택은 README의 "앱 없이 폴더를 들여다볼 수 있다"는 가치 서술과 긴장 관계다. 선택 자체는 §7 GAP-F5 미결.
- **IMP-031** — 새 파일 탐색 IPC 타입(`DirEntry` 등)을 추가하면서 `src/types.ts` ↔ Rust 모델의 **수작업 1:1 미러 부담**이 커진다. 최소한 명명 규약(Rust snake_case + `#[serde(rename_all = "camelCase")]`, TS camelCase)과 동기화 책임을 문서에 못 박는다.
- **IMP-032** — 새 커맨드의 구분 가능한 에러('파일이 사라짐', '경로가 targetDir 밖', '파일이 너무 큼', '바이너리라 표시 불가' 등)는 기존 `STALE_STAGE_INDEX_PREFIX` 규약을 확장한 **문자열 prefix 규약**을 따르고 Rust 테스트로 prefix를 고정한다. `documents/api-design.md` §3 에러 메시지 소스 표에 항목을 추가한다.
- **IMP-033** — 탭 도입으로 `PipelineRun`의 DOM 구조가 바뀌면 16.8KB 규모의 `PipelineRun.test.tsx`가 대규모로 깨진다. 테스트 개정을 **별도 작업 항목으로 명시**하고, 기존 실행 화면 동작(체크포인트 승인/수정요청/거부, 실행 전 편집, 실패 안내, 로그 표시)이 '실행' 탭 안에서 그대로 유지된다는 회귀 방어를 남긴다.

#### G.5 — 문서 정합성 *(IMP-011/IMP-022 선례 계승)*

- **IMP-037** — ① `README.md` Project layout 트리에 신규 컴포넌트·모듈 줄 추가, ② `documents/system-architecture.md` 보안 표에 **앱 자체의 파일 접근 범위** 항목 신설(현재는 프로세스 `permissionMode`만 다룸) 및 컴포넌트 표의 `project_manifest` 서술 조정, ③ `documents/api-design.md`에 '4.4 프로젝트 파일' 절 신설 및 §3 에러 표 갱신.
- **IMP-038** — `TRD.md:7` 머리말의 "React 19" 표기를 실측값 **React 18.3.1**로 정정한다. 이번에 TRD에 새 섹션을 덧붙이면서 같은 문서 안에 상충하는 스택 표기를 남기지 않기 위함이다.

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

1차 백로그(IMP-001 ~ IMP-022)에 `priority: "low"` 항목은 없다. 임의로 항목을 강등하거나 생성하지 않는다. *(IMP-023 이후의 low 3건은 §4.5.3 참조.)*

### 4.4 실행 순서에 대한 제약

우선순위와 별개로 다음 순서 제약이 있다. 1·2는 백로그에 명시된 제약이고, 3은 백로그 항목들로부터 이 PRD가 도출한 제약이므로 TRD 단계에서 재확인이 필요하다.

1. **IMP-021(기준선 커밋)은 모든 구현에 선행**한다 — 이 PRD·TRD가 인용하는 근거 문서가 버전 관리 밖에 있으면 안 된다. *(백로그 IMP-021 명시: "구현 착수 전에 두 문서를 커밋해 기준선으로 고정해야 한다")*
2. **IMP-018(마이그레이션)은 IMP-002(필드 추가)와 동시에 결정**되어야 한다 — 필드가 먼저 들어가면 기존 레코드가 즉시 로드 불가가 된다. *(백로그 IMP-018 및 gaps 6번이 이 동시 결정을 요구)*
3. **IMP-012/IMP-013(오염 경로)은 IMP-006(run 범위 격리)과 함께 판단**하는 것을 권고한다 — IMP-006만 구현하면 "run 안은 깨끗한데 진입할 때 오염된다"는 절반의 해결에 그친다. *(이 제약은 백로그에 명시된 것이 아니라 IMP-006·012·013의 내용에서 도출한 것이다.)*

### 4.5 우선순위 — IMP-023 ~ IMP-038 (파일 탐색·편집)

백로그의 priority 값을 그대로 유지한다. 각 항목에 백로그 `id`를 병기해 TRD에서 역참조할 수 있게 한다. §4.1~4.3은 IMP-001 ~ IMP-022에 대한 것이며 본 절이 그것을 대체하지 않는다.

#### 4.5.1 High (6건)

| id | 항목 | 카테고리 | 하위묶음 |
|---|---|---|---|
| IMP-023 | `PipelineRun` 우측 컬럼에 탭 컨테이너 신설 — 앱에 없던 UI 패턴 도입 | architecture | G.1 |
| IMP-024 | targetDir 나열·읽기(범위 확정 시 쓰기) Tauri 커맨드 신설 — 현재 그런 IPC가 0개 | architecture | G.1 |
| IMP-025 | targetDir 하위 경로 containment 검증 정책 신설 — 기존 두 검증 장치 재사용 불가 | architecture | G.2 |
| IMP-026 | 앱의 파일 접근 권한 경계 명시 — `app.security` 부재, capabilities 2줄 | architecture | G.2 |
| IMP-027 | 파일 커맨드를 상태 전이표(T1~T11)에 새 전이를 만들지 않는 순수 부수 기능으로 설계 | architecture | G.3 |
| IMP-028 | 실행 중 claude 프로세스와 편집 탭의 동일 파일 동시 쓰기 — 사용자 작업 유실 위험 | **bug** | G.3 |

#### 4.5.2 Medium (7건)

| id | 항목 | 카테고리 | 하위묶음 |
|---|---|---|---|
| IMP-029 | 이벤트 유래 `changedFiles`와 실측 파일 목록의 연계·구분 | ux | G.4 |
| IMP-030 | `.claude-pipeline-wizard/` 노출/숨김/토글 결정 — 기존 GAP-5 재점화 | ux | G.4 |
| IMP-031 | 새 IPC 타입이 Rust ↔ `types.ts` 수작업 1:1 미러 부담을 키움 | tech-debt | G.4 |
| IMP-032 | 새 커맨드 에러를 기존 문자열 prefix 규약으로 구분 가능하게 함 | tech-debt | G.4 |
| IMP-033 | 탭 도입으로 16.8KB `PipelineRun.test.tsx`가 대규모로 깨짐 — 개정 작업 별도 계상 | tech-debt | G.4 |
| IMP-034 | 낙관적 `pendingRun` 단계에서 파일 탭이 미검증 targetDir을 읽으면 실패 | ux | G.3 |
| IMP-035 | 트리 뷰·에디터·하이라이터 의존성 전무 — 자체 구현 범위 결정 필요 | architecture | G.1 |

#### 4.5.3 Low (3건)

> 기존 §4.3의 "Low (0건)"은 IMP-001 ~ IMP-022 기준 서술이다. 아래는 이번 축의 low 항목이며 **우선순위가 낮다는 이유로 생략하지 않는다.**

| id | 항목 | 카테고리 | 하위묶음 |
|---|---|---|---|
| IMP-036 | 탭 스트립에 기존 `.segmented` 계열 CSS 전용 — 새 디자인 언어 만들지 않음 | ux | G.1 |
| IMP-037 | README Project layout·system-architecture 보안 표·api-design 커맨드 절 갱신 | tech-debt | G.5 |
| IMP-038 | 문서의 'React 19' 표기를 실측 React 18.3.1로 정정 | tech-debt | G.5 |

#### 4.5.4 실행 순서에 대한 제약

우선순위와 별개로 다음 순서 제약이 있다. 1~3은 백로그 항목 내용에 직접 근거하며, 4는 이 PRD가 항목들로부터 도출한 것이므로 TRD 단계에서 재확인이 필요하다.

1. **§7 GAP-F1(기능 범위) 결정이 거의 모든 구현에 선행**한다 — 리스팅만인지, 읽기까지인지, 쓰기까지인지가 정해지지 않으면 IMP-024(커맨드 집합), IMP-026(권한 경계), IMP-028(충돌 정책), IMP-035(에디터 수준)의 구현 형태를 특정할 수 없다. *(§7 GAP-F1 = 2026-09-09 백로그 GAP-1이 "PRD가 임의로 확정하지 말고 명시적 선택지로 제시해야 한다"고 명시)*
2. **IMP-025(containment 검증)는 IMP-024(커맨드 신설)와 동시에** 들어가야 한다 — 검증 없는 커맨드가 먼저 머지되면 그 시점부터 임의 경로 읽기가 가능해진다. *(2026-09-09 백로그 IMP-025: "이것이 이번 기능의 최우선 보안 항목이다")*
3. **§7 GAP-F3(탐색 루트 고정 여부) 결정은 IMP-025 설계에 선행**한다 — 상위 이동을 허용하면 containment 검사의 의미 자체가 달라진다. *(§7 GAP-F3 = 2026-09-09 백로그 GAP-3 명시)*
4. **IMP-028(충돌 정책)은 쓰기 커맨드 구현보다 먼저 확정**되어야 한다 — 정책 없이 저장 기능이 먼저 머지되면 그 시점부터 데이터 손실 경로가 열린다. *(이 순서 제약은 2026-09-09 백로그에 명시된 것이 아니라 IMP-028의 내용에서 도출한 것이다.)*

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

### 5.7 목표 G — 실행 화면 파일 탐색·편집

각 기준마다 측정 가능한 완료 조건을 정의한다. 괄호 안은 해당 기준이 커버하는 백로그 id다.

> **전제:** §7 GAP-F1(기능 범위)이 확정되기 전에는 쓰기 관련 기준(G-9 ~ G-11)의 적용 여부가 확정되지 않는다. 각 기준에 조건부 적용 여부를 명시했다. 기존 §5.3 C-1이 같은 형식을 쓴 선례를 따른다.

#### 5.7.1 보안 — 최우선 기준 (IMP-025, IMP-026)

- **G-1 (path traversal 방어)** — 다음 세 유형의 입력에 대해 파일 나열·읽기·(있다면)쓰기 커맨드가 **targetDir 밖의 어떤 내용도 반환하거나 기록하지 않고**, 구분 가능한 에러를 돌려준다. 이를 고정하는 Rust 단위 테스트가 유형별로 존재한다. (IMP-025)
  - (a) `../` 상위 탈출 — 예: `../secret.txt`, `sub/../../secret.txt`
  - (b) 심볼릭 링크 경유 탈출 — targetDir 안의 링크가 밖을 가리키는 경우
  - (c) 절대경로 주입 — 예: `C:\Windows\win.ini`, `/etc/passwd`
- **G-2 (canonical prefix 비교의 실재)** — 코드에 **canonical 경로 기준의 prefix 포함 검사**가 존재하며, `validate_target_dir`이 canonical 형태를 버리는 것과 무관하게 하위 경로 검증이 독립적으로 성립한다(즉 `validate_target_dir` 재사용으로 대체되지 않았다). (IMP-025)
- **G-3 (권한 경계 결정의 문서화)** — 앱의 파일 접근 범위에 대한 결정이 `tauri.conf.json`·`capabilities/default.json`에 **실제로 반영**되어 있고, 그 근거가 `documents/system-architecture.md` 보안 표에 항목으로 기술되어 있다. 프론트에 임의 경로 fs 권한을 부여하지 않기로 결정한 경우, capabilities에 `fs:allow-read`/`fs:allow-write`가 **없음**이 그 결정의 확인 지표가 된다. (IMP-026, IMP-037②)

#### 5.7.2 기존 불변식 보존 (IMP-027, IMP-033)

- **G-4 (상태 머신 무변경)** — 파일 탐색·편집 기능 도입 전후로 `RunStatus`와 `StageStatus`의 **직렬화 값 집합이 동일**하다. 두 enum의 serde 값 목록을 고정하는 테스트(스냅샷 또는 명시적 assert)가 존재하며, 신규 값이 추가되면 실패한다. (IMP-027)
- **G-5 (전이표·통합 테스트 무변경)** — `TRD.md` §3.2 전이표 T1~T11이 **무수정**이고, `src-tauri/tests/orchestrator_test.rs`가 **무수정으로 전부 통과**한다. (IMP-027)
- **G-6 (부수 효과 없음)** — 파일 나열·읽기·(있다면)쓰기 커맨드 호출 전후로 `get_run` 결과의 `status`·`currentStageIndex`·`stages`·`resolvedStages`가 **바이트 단위로 동일**하다. 읽기 커맨드는 run lock을 잡지 않는다. (IMP-027)
- **G-7 (실행 화면 회귀 방어)** — 탭 도입 후에도 기존 실행 화면 동작 5종(체크포인트 승인 / 수정요청 / 거부 / 실행 전 단계 편집 / 실패 안내·로그 표시)이 '실행' 탭 안에서 그대로 동작함을 개정된 `PipelineRun.test.tsx`가 고정한다. 개정 전 테스트가 검증하던 동작 중 **의도적으로 삭제된 것이 0건**임이 확인된다. (IMP-033)

#### 5.7.3 동시성·데이터 보전 (IMP-028) — bug 항목

- **G-8 (실행 중 저장 정책의 발동)** — `run.status === 'running'`인 상태에서 편집 탭의 저장을 시도하면 §7 GAP-F4에서 결정된 정책이 **실제로 발동**한다. 정책별 확인 방법:
  - "차단" 선택 시 — 저장 UI가 비활성화되거나 저장 호출이 구분 가능한 에러로 거부되고, **파일 내용이 변경되지 않았음**이 확인된다.
  - "경고 후 허용" 선택 시 — 저장 전 사용자에게 실행 중임이 고지되며, 고지 없이 쓰이는 경로가 0개다.
  - "낙관적 저장 후 충돌 감지" 선택 시 — 아래 G-9가 적용된다.
  (IMP-028)
- **G-9 (조용한 손실 0)** — 편집 탭에서 파일을 연 뒤 **저장 전에 외부(claude 프로세스 또는 다른 프로세스)가 같은 파일을 변경한 상황**을 재현했을 때, 저장은 다음 둘 중 하나로 끝난다 — (i) 덮어쓰기가 일어나지 않고 충돌이 사용자에게 고지된다, 또는 (ii) 저장이 거부된다. **"고지 없이 외부 변경을 덮어쓴다"로 끝나는 경로가 0개**임을 자동화 테스트가 고정한다. (IMP-028)
- **G-10 (30분 창 전체에 적용)** — 위 G-8/G-9가 스테이지 시작 직후뿐 아니라 `STAGE_TIMEOUT`(30분) 구간 전체에 적용됨이 확인된다. 즉 "실행 중" 판정이 특정 시점 스냅샷이 아니라 저장 시점의 run 상태를 근거로 이루어진다. (IMP-028)
- **G-11 (쓰기 비범위 시의 대체 기준)** — §7 GAP-F1에서 **쓰기가 이번 범위 밖으로 결정되는 경우**, G-8 ~ G-10은 다음 단일 기준으로 대체된다: "파일을 기록하는 Tauri 커맨드가 `generate_handler!`에 **등록되어 있지 않고**, 프론트에 파일 쓰기 권한이 부여되어 있지 않다." 이 경우 IMP-028은 "범위 밖 결정에 의해 위험이 발생하지 않음"으로 종결되며 결정 근거가 문서에 남는다. (IMP-028)

#### 5.7.4 기능 성립 (IMP-023, IMP-024, IMP-034, IMP-035, IMP-036)

- **G-12 (탭 존재와 기존 블록 이전)** — `PipelineRun` 우측 컬럼에 탭 스트립이 존재하고, 기존 7개 블록이 '실행' 탭 안에 있으며, '파일' 탭 선택 시 targetDir 하위 목록이 표시된다. 좌측 240px 컬럼(템플릿명·targetDir·stage 타임라인)은 탭과 무관하게 항상 표시된다. (IMP-023)
- **G-13 (IPC 경로 신설)** — 디렉토리 나열 커맨드가 `lib.rs generate_handler!`에 등록되어 있고, `commands.rs` 관례(`#[tauri::command]` + `State<Orchestrator>` + `Result<T, String>`, 본문에 비즈니스 로직 없음)를 따른다. 새 크레이트가 `Cargo.toml`에 추가되지 않았거나(`tokio` `fs` feature로 충족), 추가된 경우 그 항목과 도입 근거가 문서에 기술되어 있다 — **결정 없이 조용히 추가된 크레이트가 0개**다. (IMP-024)
- **G-14 (마운트 타이밍)** — 낙관적 `pendingRun` 상태에서 파일 탭을 열어도 **원인 불명의 빈 목록이나 미가공 에러가 노출되지 않는다** — 나열이 백엔드 응답 이후로 지연되거나, 명시적 로딩/안내 문구가 표시된다. (IMP-034)
- **G-15 (의존성 결정의 실재)** — §7 GAP-F6 결정에 따라 `package.json` dependencies가 4개로 유지되거나, 추가된 경우 그 항목과 도입 근거가 문서에 기술되어 있다. **결정 없이 조용히 추가된 의존성이 0개**다. (IMP-035)
- **G-16 (시각 언어 재사용)** — 탭 스트립이 기존 `.segmented` / `.segmented__option` / `.segmented__option--selected`를 사용하고, 파일 탭 내부가 기존 `.card`/`.badge`/`.field`/`.input`/`.textarea`/`.mono`/`.help-text`/`.alert`를 재사용한다. **신규 CSS 컴포넌트 계열을 추가한 경우 그 항목과 근거가 문서에 기술되어 있다**(§7 GAP-F6에서 라이브러리 도입이 결정된 경우 그 라이브러리 스타일은 이 기준의 예외로 명시된다). (IMP-036)

#### 5.7.5 통합·정합성 (IMP-029 ~ IMP-032, IMP-037, IMP-038)

- **G-17 (두 정보원의 구분)** — 화면에서 `changedFiles`(스테이지 이벤트 유래)와 파일 탭 목록(파일시스템 실측)이 **의미상 구분되어 표시**된다 — 서로 다른 라벨·설명을 갖거나, 실측 목록 위에 '이번 단계에서 변경됨' 강조로 통합된다. 두 목록이 구분 없이 나란히 놓이는 화면이 존재하지 않는다. (IMP-029)
- **G-18 (`.claude-pipeline-wizard/` 처리)** — §7 GAP-F5에서 결정된 방식(노출/숨김/토글)이 실제로 적용되어 있고, 그 결정이 기존 §6 GAP-5와의 관계와 함께 문서에 기술되어 있다. (IMP-030)
- **G-19 (타입 미러 규약)** — 새 IPC 타입의 Rust 구조체가 `#[serde(rename_all = "camelCase")]`를 갖고 `src/types.ts`의 대응 타입과 필드가 1:1로 일치하며, 이 동기화 책임이 문서에 명시되어 있다. (IMP-031)
- **G-20 (에러 규약 확장)** — 프론트가 다르게 반응해야 하는 파일 에러들이 Rust 쪽 prefix 상수와 `api.ts` 헬퍼 쌍으로 구분 가능하고, **prefix를 고정하는 Rust 테스트가 존재**하며, `documents/api-design.md` §3 에러 표에 항목이 추가되어 있다. (IMP-032)
- **G-21 (문서 갱신)** — `README.md` Project layout에 신규 파일 줄이 추가되고, `documents/system-architecture.md` 보안 표에 앱 파일 접근 범위 항목이 존재하며, `documents/api-design.md`에 프로젝트 파일 커맨드 절이 신설되어 있다. (IMP-037)
- **G-22 (스택 표기 정정)** — `TRD.md` 머리말의 React 버전 표기가 `package.json` 실측값(18.3.1)과 일치한다. 문서 내에 상충하는 스택 표기가 남아 있지 않다. (IMP-038)

---

## 6. 범위 제외 I — 실행 게이트·매니페스트 (이번 spec에서 다루지 않음)

아래 7개 항목은 1차 백로그(2026-09-08)의 `gaps`를 그대로 옮긴 것이다. 파일 탐색·편집 축의 미결 항목 8건은 §7에 `GAP-F1` ~ `GAP-F8`로 따로 있다. **근거가 없어 판단을 보류한 지점이며, 이 PRD가 임의로 결론을 채우지 않는다.** TRD 작성자와 승인자는 이 항목들에 대해 별도 결정을 받아야 한다.

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

## 7. 범위 제외 II — 파일 탐색·편집 (이번 spec에서 결론 내리지 않음)

아래 8개 항목은 이번 백로그의 `gaps`를 그대로 옮긴 것이다. **근거가 없어 판단을 보류한 지점이며, 이 PRD가 임의로 결론을 채우지 않는다.** §6과 동일한 취급이며, TRD 작성자와 승인자는 이 항목들에 대해 별도 결정을 받아야 한다.

> **번호 체계 안내.** 이번 백로그의 gaps도 `GAP-1`~`GAP-8`로 번호가 붙어 있어 §6의 기존 `GAP-1`~`GAP-7`과 충돌한다. 따라서 본 장에서는 **`GAP-F` 접두사**(F = File browser)를 붙여 구분한다. §6의 GAP 번호는 변경하지 않는다.

| 본 장 id | 원 백로그 id | 요지 |
|---|---|---|
| GAP-F1 | 백로그 GAP-1 | 이번 기능의 범위(리스팅/읽기/쓰기/파일관리) |
| GAP-F2 | 백로그 GAP-2 | 바이너리·대용량 파일 처리 정책 |
| GAP-F3 | 백로그 GAP-3 | 탐색 루트를 targetDir로 고정할지 |
| GAP-F4 | 백로그 GAP-4 | 실행 중 편집·저장 허용 정책 |
| GAP-F5 | 백로그 GAP-5 | `.claude-pipeline-wizard/` 노출 여부 |
| GAP-F6 | 백로그 GAP-6 | 코드 에디터 라이브러리 도입 여부 |
| GAP-F7 | 백로그 GAP-7 | README "New folders only" 한계와의 긴장 |
| GAP-F8 | 백로그 GAP-8 | 실사용 시나리오와 성공 기준의 근거 부재 |

### ⚠️ GAP-F1 (최우선 확인 필요) — 이번 기능의 범위

> **거의 모든 후속 결정이 여기에 종속된다(§4.5.4 제약 1).**
>
> 디렉토리 리스팅만인지, 파일 내용 읽기까지인지, 쓰기/저장까지인지, 나아가 삭제·이름변경·새 파일 생성 같은 파일 관리 기능을 포함하는지가 미결이다.
>
> 요청 문구는 "서브디렉토리·파일 목록을 보고 편집할 수 있게 한다"까지이며, 원자료 어디에도 이 범위를 좁힐 근거가 없다. 코드·문서·git log 전수 조사에서 파일 탐색기·편집 시도 커밋이 0건이고 TODO·FIXME 주석도 0건이라 **과거 의도를 추론할 재료조차 없다.** 기능 범위는 사용자 결정 사항이므로 PRD가 임의로 확정하지 않고 명시적 선택지로 남긴다. **임의 확정 금지 — 사용자 결정 필요.**
>
> **✅ 결정 (2026-09-09, 사용자):** **디렉토리 리스팅만.** targetDir 하위 서브디렉토리·파일 목록(트리 또는 리스트)만 표시한다. **파일 내용 열람·편집·저장, 삭제·이름변경·새 파일 생성은 이번 spec의 범위에 포함되지 않는다.** 이 결정에 따라 목표 G의 항목 형태가 다음과 같이 좁혀진다 — TRD 작성 시 이 좁혀진 형태를 그대로 반영한다:
> - **IMP-024**는 디렉토리 나열(및 파일 메타데이터 — 이름/크기/종류/mtime) 커맨드만 신설한다. 파일 내용 읽기·쓰기 커맨드는 이번 범위에 없다.
> - **IMP-025**(containment 검증)는 나열 커맨드 하나에만 적용되며 범위는 줄지만 필요성은 그대로다 — 나열 대상 디렉토리 자체가 targetDir 밖으로 나가지 않아야 한다.
> - **IMP-026**(권한 경계)은 "읽기 전용 나열"이라는 더 좁은 권한만 결정하면 된다.
> - **IMP-028**(실행 중 쓰기 동시성 bug)은 **이번 spec에서 트리거되지 않는다** — 쓰기 기능 자체가 없으므로 데이터 손실 경로가 존재하지 않는다. 항목은 "이번 범위에서 발생하지 않음"으로 종결하되, 아래 GAP-F4 결정은 **향후 쓰기 기능이 추가될 때 적용할 정책**으로 기록해 둔다.
> - **IMP-032**(에러 규약 확장)의 '바이너리라 표시 불가' 유형은 파일 내용을 읽지 않으므로 불필요 — '경로가 targetDir 밖' 유형만 해당한다.
> - **IMP-035**(에디터 라이브러리 도입 여부)는 **적용 대상이 없어진다** — 편집 UI 자체를 만들지 않으므로 GAP-F6(에디터 라이브러리 결정)도 이번 spec에서는 다룰 필요가 없다.
> - **G-9~G-11**(§5.7.3, 쓰기 충돌 감지 기준)은 이번 spec에서는 "해당 없음 — 쓰기 커맨드가 `generate_handler!`에 등록되어 있지 않다"로 충족된다(기존 G-11이 정의한 대체 기준과 동일).
>
> 파일 내용 열람·편집은 **이번 spec이 명시적으로 다음 단계로 미루는 후속 확장**이며, 사용자가 원하면 별도 PRD 개정으로 다시 연다.

### ⚠️ GAP-F4 (안전상 최우선 확인 필요) — 실행 중 편집·저장 허용 정책

> **결정 없이 저장 기능을 여는 것은 데이터 손실을 의미한다(IMP-028).**
>
> `run.status === 'running'` 구간(claude 프로세스가 targetDir을 cwd로 점유, 최대 30분)에서 파일 편집·저장을 허용할지 — **금지 / 경고 후 허용 / 낙관적 저장 후 충돌 감지** 세 선택지 중 무엇을 택할지가 미결이다.
>
> 위험 자체는 `executor.rs:42-53`의 근거로 확정됐으나, 세 선택지 중 무엇을 택할지는 제품 결정이며 원자료에 선호를 시사하는 근거가 없다. `TRD.md:941` 이후의 "선택지 A/B/C + 구현 영향" 형식을 재사용해 제시하는 것이 적합하다. **임의 확정 금지 — 사용자 결정 필요.**
>
> **✅ 결정 (2026-09-09, 사용자):** **실행 중 저장 차단.** `run.status === 'running'`인 동안에는 저장(및 그 전제인 편집)을 허용하지 않는다. **GAP-F1 결정(디렉토리 리스팅만)에 의해 이번 spec에는 저장 기능 자체가 없으므로, 이 정책은 지금 당장 시행할 대상이 없다** — 향후 파일 읽기·쓰기가 추가되는 확장 spec에서 이 정책("실행 중 차단")을 그대로 적용한다는 선결정으로 기록해 둔다.

### GAP-F2 — 바이너리 파일과 대용량 파일 처리 정책

텍스트 여부 판별 방법, 읽기 크기 상한, 상한 초과 시 UI 동작이 미결이다. 판별·상한을 정할 근거가 원자료에 없다. Rust 측에 파일 타입 감지 크레이트가 없어 직접 구현해야 하고(`Cargo.toml:22-32`), 프론트에도 뷰어 라이브러리가 없다(`package.json:13-30`). 정책이 결정되지 않으면 구현 형태를 특정할 수 없다.

### GAP-F3 — 탐색 루트를 `run.targetDir`로 고정할지, 상위 디렉토리 이동을 허용할지

IMP-025가 보여주듯 `targetDir`은 canonicalize되지 않은 원본 문자열로 저장되므로(`project_manifest.rs:36-54`) 어떤 선택을 하든 canonical prefix 검사 설계가 선행되어야 한다. **상위 이동을 허용하면 containment 검사의 의미 자체가 달라지지만**, 어느 쪽이 사용자에게 필요한지에 대한 근거가 원자료에 없다.

### GAP-F5 — `.claude-pipeline-wizard/`를 파일 목록에 노출할지, 숨길지, 토글로 둘지

§6 GAP-5(이 디렉토리를 타겟 폴더에 남기는 정책)가 이미 미결 상태이고, `README.md:27-29`는 폴더를 앱 밖에서 들여다보는 것을 매니페스트의 가치로 서술해 숨김 선택과 긴장 관계에 있다. **기존 미결 항목의 하위 결정이므로 이번 spec이 단독으로 확정할 근거가 부족하다.**

### GAP-F6 — 편집 UI를 순수 textarea로 갈지 코드 에디터 라이브러리를 도입할지

현재 프론트 의존성이 4개뿐이고 에디터·트리 라이브러리가 전무하다(`package.json:13-30`). 의존성 추가는 이 프로젝트가 유지해 온 최소 의존성 관례를 바꾸는 결정인데, 어느 수준의 편집 경험이 요구되는지가 **GAP-F1에 종속되어 있어 지금 확정할 수 없다.**

### GAP-F7 — 파일 탐색기 도입과 README "New folders only" 한계 사이의 긴장

`README.md:100-119`는 `.claude-pipeline-wizard/run.json`이 이미 있는 폴더를 거부하고 기존 코드베이스 대상 실행을 지원하지 않는다고 명시하며, `orchestrator.rs:113-135`의 `TargetDirInUse` 차단이 이를 강제한다. 그런데 폴더 내용을 앱 안에서 보고 편집할 수 있게 되면 **사용자가 자연스럽게 기존 코드베이스를 대상으로 쓰고 싶어질 유인이 커진다.** 이 한계를 유지할지 완화할지는 이번 요청 범위를 크게 넘어서는 제품 결정이고 원자료에 사용자 요구 근거가 없어 확정하지 않는다.

> **다만 이 한 줄은 이번 spec이 명시한다:** 이번 기능은 README Known v1 limitations 1번("New folders only")을 **완화하지 않는다.** 즉 `TargetDirInUse` 차단을 완화하거나 우회하는 변경은 이번 범위에 포함되지 않는다. *(이 비범위 명시는 백로그 GAP-7이 "최소한 새 PRD가 이 비범위를 명시할 필요는 있다"고 요구한 것을 그대로 옮긴 것이며, 한계의 유지/완화 결정 자체는 여전히 미결이다. 탐색 루트를 `run.targetDir`로 고정할지 상위 이동을 허용할지는 이 문장이 아니라 GAP-F3에서 결정된다 — 이 비범위 명시가 GAP-F3의 선택지를 좁히지 않는다.)*

### GAP-F8 — 이 기능의 실사용 시나리오와 성공 기준의 근거 부재

§6 GAP-7이 이미 "우선순위 실사용 근거 부재"를 미결로 남겨 두었고, 이번 요청에 대해서도 **사용자가 언제·왜 파일을 열어보는지**(실행 중 진행 확인 / 체크포인트 심사 / 실패 후 원인 파악)를 시사하는 원자료가 없다. 인접한 근거는 `README.md:27-29`의 "앱 없이 폴더를 들여다볼 수 있다"는 서술과 목표 B·E뿐이며, 이는 정황일 뿐 성공 기준을 도출할 근거는 아니다.

> **이것이 §5.7에 미치는 영향:** §5.7의 기준들은 **"이 기능이 안전하고 기존 불변식을 깨지 않게 만들어졌는가"**를 측정할 뿐, **"이 기능이 사용자에게 실제로 유용한가"**는 측정하지 않는다. 후자의 기준은 GAP-F8이 해소된 뒤에야 세울 수 있다. §4.5의 High/Medium/Low 배치도 §6 GAP-7과 같은 이유로 코드 근거 기반의 잠정치다.

---

## 부록 A — 1차 백로그 id 커버리지 표 (IMP-001 ~ IMP-022)

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

**커버리지: 22 / 22 (100%).** 1차 백로그에 없는 개선 항목은 추가하지 않았다. *(IMP-023 ~ IMP-038의 커버리지는 부록 B 참조.)*

---

## 부록 B — IMP-023 ~ IMP-038 커버리지 표

| id | priority | 등장 섹션 |
|---|---|---|
| IMP-023 | high | 2.4(1), 3-목표G(G.1), 4.5.1, 5.7.4(G-12) |
| IMP-024 | high | 2.4(1), 3-목표G(G.1), 4.5.1, 4.5.4, 5.7.4(G-13) |
| IMP-025 | high | 2.4(2), 3-목표G(G.2), 4.5.1, 4.5.4, 5.7.1(G-1, G-2), 7-GAP-F3 |
| IMP-026 | high | 2.4(2), 3-목표G(G.2), 4.5.1, 5.7.1(G-3) |
| IMP-027 | high | 1.4.1, 2.4(3), 3-목표G(G.3), 4.5.1, 5.7.2(G-4 ~ G-6) |
| IMP-028 | high (bug) | 2.4(4), 3-목표G(G.3), 4.5.1, 4.5.4, 5.7.3(G-8 ~ G-11), 7-GAP-F4 |
| IMP-029 | medium | 2.4(5), 3-목표G(G.4), 4.5.2, 5.7.5(G-17) |
| IMP-030 | medium | 2.4(5), 3-목표G(G.4), 4.5.2, 5.7.5(G-18), 7-GAP-F5 |
| IMP-031 | medium | 2.4(6), 3-목표G(G.4), 4.5.2, 5.7.5(G-19) |
| IMP-032 | medium | 2.4(6), 3-목표G(G.4), 4.5.2, 5.7.5(G-20) |
| IMP-033 | medium | 2.4(6), 3-목표G(G.4), 4.5.2, 5.7.2(G-7) |
| IMP-034 | medium | 2.4(7), 3-목표G(G.3), 4.5.2, 5.7.4(G-14) |
| IMP-035 | medium | 2.4(8), 3-목표G(G.1), 4.5.2, 5.7.4(G-15), 7-GAP-F6 |
| IMP-036 | low | 2.4(8), 3-목표G(G.1), 4.5.3, 5.7.4(G-16) |
| IMP-037 | low | 2.4(9), 3-목표G(G.5), 4.5.3, 5.7.1(G-3), 5.7.5(G-21) |
| IMP-038 | low | 2.4(9), 3-목표G(G.5), 4.5.3, 5.7.5(G-22) |

**커버리지: 16 / 16 (100%).** 백로그에 없는 개선 항목은 추가하지 않았다.

### 부록 B-2 — GAP 커버리지

| 본 문서 id | 원 백로그 id | 등장 섹션 |
|---|---|---|
| GAP-F1 | GAP-1 | 4.5.4(제약 1), 5.7 전제, 5.7.3(G-11), 7 |
| GAP-F2 | GAP-2 | 7 |
| GAP-F3 | GAP-3 | 4.5.4(제약 3), 7 |
| GAP-F4 | GAP-4 | 2.4(4), 3-목표G(IMP-028), 4.5.4(제약 4), 5.7.3(G-8), 7 |
| GAP-F5 | GAP-5 | 2.4(5), 3-목표G(IMP-030), 5.7.5(G-18), 7 |
| GAP-F6 | GAP-6 | 2.4(8), 3-목표G(IMP-035), 5.7.4(G-15), 7 |
| GAP-F7 | GAP-7 | 7 (비범위 1줄 명시 포함) |
| GAP-F8 | GAP-8 | 7, 5.7 전제 |

**커버리지: 8 / 8 (100%).** 백로그가 GAP으로 분리한 항목에 대해 이 PRD가 임의로 결론을 내린 곳은 없다. 유일한 예외는 GAP-F7의 "이번 기능은 New folders only 한계를 완화하지 않는다"는 비범위 명시이며, 이는 백로그 GAP-7이 직접 요구한 문장이다.
