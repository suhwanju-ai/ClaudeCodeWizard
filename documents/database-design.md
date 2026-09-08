# Claude Pipeline Wizard 데이터 저장 설계 문서

**버전**: v0.1.0
**저장 방식**: 전통적 DBMS 없음 — **파일 시스템 기반 JSON 저장소** (1개 레코드 = 1개 파일)
**문자셋**: UTF-8
**작성일**: 2026-09-07

---

## 0. 왜 DBMS가 아닌가

이 프로젝트는 SQLite/PostgreSQL 등 어떤 DBMS도 사용하지 않는다. `src-tauri/src/template/store.rs`와
`src-tauri/src/engine/run_record.rs`가 각각 자체 파일 기반 저장소(`TemplateStore`, `RunRecordStore`)를
구현하며, 둘 다 같은 패턴을 따른다: **레코드 하나 = JSON 파일 하나**, id가 곧 파일명.
이 문서는 스킬의 표준 DB 문서 항목(테이블/컬럼/인덱스/마이그레이션)을 이 저장 방식에 맞게 재해석한다.

| 표준 DB 개념 | 이 프로젝트에서의 대응 |
|---|---|
| DBMS | 없음 — Rust `std::fs` + `serde_json` |
| 테이블 | 디렉터리 (`templates/`, `runs/`) |
| 로우(row) | 디렉터리 안의 `{id}.json` 파일 1개 |
| 기본키(PK) | 파일명에서 `.json`을 뗀 부분 (`Template.id`, `RunRecord.run_id`) |
| 스키마 | Rust 구조체 + `#[derive(Serialize, Deserialize)]` (→ [클래스 다이어그램](class-diagram.md) 참고) |
| 인덱스 | 없음 — `list()`는 디렉터리 전체를 순회(scan)하고 이름순 정렬 |
| 마이그레이션 도구 | 없음 (2.4절 참고) |
| 트랜잭션 | 없음 — 파일 하나를 통째로 `fs::write`로 덮어쓰는 원자성만 OS에 의존 |

---

## 1. 저장소(테이블) 목록

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

---

## 2. 저장소 상세 설계

### 2.1 templates/{id}.json — 템플릿

정의: `src-tauri/src/template/mod.rs`

**Template**
| 필드 | 타입 | NULL | 기본값 | 제약조건 | 설명 |
|---|---|---|---|---|---|
| `id` | string | NOT NULL | - | `is_valid_id()` — 영문/숫자/`.`/`_`/`-`만, `.`/`..` 금지. 파일명이므로 사실상 UNIQUE PK | 템플릿 고유 id, 파일 경로 `templates/{id}.json`에 직접 사용 |
| `name` | string | NOT NULL | - | 없음 | 표시 이름 (예: "웹 프로그램 개발") |
| `description` | string | NOT NULL | - | 없음 | 설명 |
| `stages` | `Stage[]` | NOT NULL | - | 최소 1개(`NoStages` 에러), 단계 id 중복 불가 | 파이프라인 단계 목록 (순서 = 배열 순서 그 자체) |

**Stage** (Template에 내장되는 값 객체 — 별도 파일로 존재하지 않음)
| 필드 | 타입 | NULL | 기본값 | 제약조건 | 설명 |
|---|---|---|---|---|---|
| `id` | string | NOT NULL | - | `is_valid_id()`, 같은 템플릿 내 UNIQUE | 단계 id |
| `name` | string | NOT NULL | - | - | 표시 이름 |
| `prompt` | string | NOT NULL | - | trim 후 비어있으면 안 됨(`EmptyPrompt`) | `claude -p`에 전달되는 프롬프트 |
| `permissionMode` | enum | NOT NULL | - | `acceptEdits` \| `bypassPermissions` \| `default` | `claude --permission-mode` 값 |
| `allowedTools` | string[] | NOT NULL | `[]` | - | `claude --allowedTools` 콤마 조인 값 |
| `checkpoint` | boolean | NOT NULL | - | - | true면 단계 종료 후 사람 승인 대기 |

**"인덱스" 대체 전략**: 파일 시스템 자체가 유일한 조회 경로다.
- **PK 조회** (`load(id)`): `is_valid_id(id)` 검증 후 `templates/{id}.json`을 직접 open — O(1) 파일 시스템 조회.
- **전체 목록** (`list()`): `templates/` 디렉터리를 `fs::read_dir`로 전체 스캔 후 `name`으로 정렬 — O(N),
  레코드 수가 수십~수백 개 수준으로 예상되므로 문제 없음.
- id 이외의 필드(예: `name`)로 검색하는 기능은 없다 — 필요해질 경우가 생기면 그때 실제 DBMS 도입을
  검토해야 한다(현재는 과설계 방지 차원에서 미도입).

### 2.2 runs/{run_id}.json — 실행 기록

정의: `src-tauri/src/engine/run_record.rs`

**RunRecord**
| 필드 | 타입 | NULL | 기본값 | 제약조건 | 설명 |
|---|---|---|---|---|---|
| `runId` | string | NOT NULL | - | `is_valid_id()`, 파일명 → 사실상 UNIQUE PK | 실행 고유 id (프론트엔드에서 `crypto.randomUUID()`로 생성) |
| `templateId` | string | NOT NULL | - | 소프트 FK → `templates/{templateId}.json` (DB 레벨 강제 없음, 애플리케이션 로직으로만 참조) | 이 실행이 시작된 템플릿 id |
| `targetDir` | string | NOT NULL | - | - | 파이프라인이 실행되는 절대 경로 |
| `status` | enum | NOT NULL | `awaiting-stage-start` | `running` \| `awaiting-stage-start` \| `awaiting-checkpoint` \| `completed` \| `failed` \| `cancelled` | 실행 전체 상태. `awaiting-stage-start`는 **아무것도 실행되고 있지 않은 실행 전 게이트**를 뜻한다 |
| `currentStageIndex` | number | NOT NULL | `0` | `0 <= n < stages.length` | 현재 게이트에 있거나 실행 중인 단계의 배열 인덱스 |
| `stages` | `StageRun[]` | NOT NULL | - | 생성 시 템플릿의 단계 수와 1:1 매핑 | 단계별 실행 상태 목록 |
| `resolvedStages` | `Stage[]` | NOT NULL | 생성 시 템플릿 `stages`의 복사본 | `stages`와 같은 길이·같은 id 순서. **`#[serde(default)]` 없음 — 필수 필드** | 이 run이 실제로 실행할 단계 정의. 실행 전 게이트에서 수정한 내용이 여기 들어가며, 원본 템플릿 파일은 절대 쓰지 않는다 |

**StageRun** (RunRecord에 내장되는 값 객체)
| 필드 | 타입 | NULL | 기본값 | 제약조건 | 설명 |
|---|---|---|---|---|---|
| `id` | string | NOT NULL | - | 소프트 FK → 원본 `Template.stages[].id` | 어느 단계인지 식별 (표시 이름은 실행 시점 `Template`을 다시 조회해서 얻음) |
| `status` | enum | NOT NULL | `pending` (0번 단계만 생성 시 `awaiting-start`) | `pending` \| `awaiting-start` \| `running` \| `awaiting-checkpoint` \| `approved` \| `failed` | 단계 상태. `awaiting-start`는 "이 단계 차례가 됐지만 아직 실행하지 않았다" |
| `sessionId` | string \| null | NULL 허용 | `null` | `claude --resume`에 쓰이는 세션 id | 단계가 최소 1번 실행된 뒤에 채워짐 |
| `log` | JSON 배열 | NOT NULL | `[]` | 각 원소는 `StageEvent` 직렬화 값 | 해당 단계에서 발생한 모든 스트림 이벤트의 append-only 로그 |

**"인덱스" 대체 전략**: `templates`와 동일하게 PK(`runId`) 조회만 지원한다. 특정
`templateId`로 실행 이력을 검색하는 기능이나, 상태별 필터링 기능은 앱에 없다 —
필요 시 `runs/` 디렉터리를 전체 스캔해야 한다(현재 UI에는 이런 목록 화면 자체가 없다).

### 2.3 <targetDir>/.claude-pipeline-wizard/ — 프로젝트 로컬 매니페스트

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

---

## 3. 관계 요약

| 관계 | 타입 | 강제 방식 | 설명 |
|---|---|---|---|
| Template → Stage | 1:N (내장) | Rust 타입 시스템 + `validate_template` | 별도 파일이 아니라 같은 JSON 안에 배열로 내장 |
| RunRecord → StageRun | 1:N (내장) | `RunRecord::new()`가 템플릿의 단계 수만큼 생성 | 마찬가지로 내장 배열 |
| RunRecord.templateId → Template.id | N:1 (소프트) | **강제 없음** | 참조 무결성 없음. 다만 **실행 중 단계 정의는 `resolvedStages`에서만 읽으므로**, 템플릿이 삭제·수정된 뒤에 체크포인트를 승인해도 실행이 실패하지 않는다. `templateId`는 표시·추적용 문자열로만 남는다 |
| RunRecord.stages[].id → Template.stages[].id | N:1 (소프트, 이름 매칭) | **강제 없음** | 프론트엔드가 `stageNameById` 맵으로 표시 이름을 붙일 때만 사용(`src/pages/PipelineRun.tsx`) |
| RunRecord → Stage (`resolvedStages`) | 1:N (내장) | `RunRecord::new()`가 템플릿 `stages`를 복사 | 이 run 전용 파이프라인 사본. `apply_stage_override`가 현재 인덱스 1개만 교체하며, 원본 `templates/{id}.json`은 어떤 경로로도 쓰지 않는다 |
| RunRecord ↔ 프로젝트 로컬 매니페스트 | 1:1 (미러) | `Orchestrator::save()`가 두 곳에 연속으로 쓴다 | `runs/{run_id}.json`이 원본, `<targetDir>/.claude-pipeline-wizard/run.json`이 사본. 사본 쓰기 실패는 원본 쓰기를 롤백시킨다 |

자세한 엔티티 구조는 → [ER 다이어그램](er-diagram.md) 참고.

---

## 4. 동시성 및 원자성

- 파일 쓰기는 `fs::write()` 한 번으로 전체 내용을 교체한다 — 부분 쓰기로 인한 파일 손상
  가능성은 OS/파일시스템 수준의 원자성에 의존하며, 애플리케이션 레벨의 락(lock)이나
  WAL(write-ahead log)은 없다.
- 동일 `run_id`/`template_id`에 대한 동시 쓰기 경합을 막는 뮤텍스나 트랜잭션이 없다.
  실제로는 Tauri 커맨드가 한 번에 하나씩 순차 호출되는 UI 흐름(승인 버튼 클릭 → 완료까지
  버튼 비활성화)으로 경합을 회피하고 있다.

---

## 5. "마이그레이션" 전략

Alembic/Django migrations 같은 스키마 마이그레이션 도구는 없다. 대신:

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
