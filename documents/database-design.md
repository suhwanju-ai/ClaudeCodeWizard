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

`<app_data_dir>`는 Tauri의 `app.path().app_data_dir()`이 반환하는 OS별 표준 앱 데이터
경로다(Windows: `%APPDATA%\com.suhwanju.claude-pipeline-wizard\`, macOS:
`~/Library/Application Support/com.suhwanju.claude-pipeline-wizard/`, Linux:
`~/.local/share/com.suhwanju.claude-pipeline-wizard/`). `identifier`는
`src-tauri/tauri.conf.json`에 정의되어 있다.

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
| `status` | enum | NOT NULL | `running` | `running` \| `awaiting-checkpoint` \| `completed` \| `failed` \| `cancelled` | 실행 전체 상태 |
| `currentStageIndex` | number | NOT NULL | `0` | `0 <= n < stages.length` | 현재 진행 중인 단계의 배열 인덱스 |
| `stages` | `StageRun[]` | NOT NULL | - | 생성 시 템플릿의 단계 수와 1:1 매핑 | 단계별 실행 상태 목록 |

**StageRun** (RunRecord에 내장되는 값 객체)
| 필드 | 타입 | NULL | 기본값 | 제약조건 | 설명 |
|---|---|---|---|---|---|
| `id` | string | NOT NULL | - | 소프트 FK → 원본 `Template.stages[].id` | 어느 단계인지 식별 (표시 이름은 실행 시점 `Template`을 다시 조회해서 얻음) |
| `status` | enum | NOT NULL | `pending` | `pending` \| `running` \| `awaiting-checkpoint` \| `approved` \| `failed` | 단계 상태 |
| `sessionId` | string \| null | NULL 허용 | `null` | `claude --resume`에 쓰이는 세션 id | 단계가 최소 1번 실행된 뒤에 채워짐 |
| `log` | JSON 배열 | NOT NULL | `[]` | 각 원소는 `StageEvent` 직렬화 값 | 해당 단계에서 발생한 모든 스트림 이벤트의 append-only 로그 |

**"인덱스" 대체 전략**: `templates`와 동일하게 PK(`runId`) 조회만 지원한다. 특정
`templateId`로 실행 이력을 검색하는 기능이나, 상태별 필터링 기능은 앱에 없다 —
필요 시 `runs/` 디렉터리를 전체 스캔해야 한다(현재 UI에는 이런 목록 화면 자체가 없다).

---

## 3. 관계 요약

| 관계 | 타입 | 강제 방식 | 설명 |
|---|---|---|---|
| Template → Stage | 1:N (내장) | Rust 타입 시스템 + `validate_template` | 별도 파일이 아니라 같은 JSON 안에 배열로 내장 |
| RunRecord → StageRun | 1:N (내장) | `RunRecord::new()`가 템플릿의 단계 수만큼 생성 | 마찬가지로 내장 배열 |
| RunRecord.templateId → Template.id | N:1 (소프트) | **강제 없음** | 참조 무결성 없음 — 템플릿이 삭제·수정된 뒤에도 과거 `RunRecord`는 원래 `templateId` 문자열만 들고 있다. `approve_checkpoint`/`request_changes`는 실행 시점에 `template_store.load(&record.template_id)`를 다시 호출하므로, 템플릿이 삭제된 상태에서 체크포인트를 승인하면 `"template '{id}' not found"` 에러가 발생한다 |
| RunRecord.stages[].id → Template.stages[].id | N:1 (소프트, 이름 매칭) | **강제 없음** | 프론트엔드가 `stageNameById` 맵으로 표시 이름을 붙일 때만 사용(`src/pages/PipelineRun.tsx`) |

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
  해야 한다(현재 코드베이스에는 아직 이런 필드 추가 이력이 없다 — 앞으로 필드를
  추가할 때 지켜야 할 원칙으로 문서화해 둔다).
- **파괴적 변경 시 절차**: 필드 이름 변경이나 필수 필드 추가처럼 기존 JSON과 호환되지
  않는 변경이 필요하면, 저장 디렉터리(`templates/`, `runs/`)의 기존 파일을 읽어 새
  스키마로 변환해 다시 쓰는 1회성 변환 스크립트를 별도로 작성해야 한다. 현재 코드베이스에는
  이런 스크립트가 없다.
- **손상 파일 대응**: `serde_json::from_str` 파싱 실패 시 `"json error: ..."`로 전체
  `list()` 호출 자체가 실패한다(→ [API 설계 문서](api-design.md) 3절) — 손상된 파일 하나가
  전체 목록 조회를 막을 수 있다는 뜻이므로, 운영 중 문제가 생기면 `templates/`/`runs/`
  디렉터리를 열어 손상된 `.json` 파일을 수동으로 찾아 제거/복구해야 한다.
