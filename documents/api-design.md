# Claude Pipeline Wizard API 설계 문서

**버전**: v0.1.0
**작성일**: 2026-09-07
**전송 계층**: Tauri v2 IPC (`invoke()` / `emit()` / `listen()`) — HTTP 서버 없음
**인증 방식**: 없음 (로컬 데스크톱 단일 사용자 앱, 네트워크 포트를 열지 않음)

---

## 1. 개요

Claude Pipeline Wizard는 Tauri v2 데스크톱 앱으로, 프론트엔드(React/TypeScript)와
백엔드(Rust)가 HTTP가 아닌 **Tauri IPC**로 통신한다. 이 문서에서 "엔드포인트"는
`src-tauri/src/commands.rs`에 정의된 `#[tauri::command]` 함수를 의미하며, 프론트엔드는
`@tauri-apps/api/core`의 `invoke("커맨드명", { 인자 })`로 호출한다.

| 항목 | 내용 |
|------|------|
| API 버전 | v0.1.0 (`tauri.conf.json`의 `version`) |
| 전송 방식 | Tauri IPC (`invoke`), Tauri 이벤트 (`emit`/`listen`) |
| 데이터 형식 | JSON (serde 직렬화, `camelCase`) |
| 인증 방식 | 없음 — 로컬 프로세스 간 통신만 존재 |
| Rate limiting | 없음 — 단일 사용자, 단일 세션 로컬 앱 |
| 동시성 정책 | 런(run) 1개당 `claude` CLI 자식 프로세스 1개. 동시에 여러 커맨드가 같은 `RunRecord`/`Template`을 수정하는 경우에 대한 락(lock)은 없음 — 실제 사용 패턴상 한 번에 하나의 화면(갤러리/에디터/실행)만 활성화되므로 발생하지 않는다. |

모든 커맨드는 등록된 `Orchestrator`(`src-tauri/src/engine/orchestrator.rs`)를 Tauri
`State`로 주입받아 사용한다. `Orchestrator`는 `TemplateStore`, `RunRecordStore`,
`ExecutorConfig`를 묶은 구조체다(→ [DB 설계 문서](database-design.md) 참고).

---

## 2. 공통 응답 형식

Rust 커맨드의 반환 타입은 대부분 `Result<T, String>`이며, Tauri는 이를 그대로
JS 쪽 Promise로 매핑한다.

### 성공 응답
`Ok(value)`는 JS에서 `invoke()`가 resolve하는 값 그 자체다 (별도의 `{success: true, data: ...}`
래퍼 없음).

```json
// list_templates() 성공 예 — Vec<Template> 그대로 반환
[
  { "id": "web-app-dev", "name": "웹 프로그램 개발", "description": "...", "stages": [...] }
]
```

### 에러 응답
`Err(String)`은 JS에서 `invoke()`가 reject하는 사유(에러 메시지 문자열) 그 자체다.
모든 백엔드 에러 타입(`StoreError`, `RunRecordStoreError`, `OrchestratorError`)은
각 커맨드 안에서 `.map_err(|e| e.to_string())`으로 문자열로 변환된다 — 별도의
에러 코드 필드는 없고, `thiserror`가 생성한 사람이 읽을 수 있는 메시지만 전달된다.

```json
// save_template() 실패 예 — 문자열 하나
"validation error: stage 's1' has an empty prompt"
```

프론트엔드는 이 문자열을 그대로 사용자에게 노출한다(`src/pages/*.tsx`의
`setError(String(e))` 패턴 참고).

---

## 3. 에러 메시지 소스 (에러 "코드" 대체표)

전용 에러 코드가 없으므로, 실제로 발생하는 에러 메시지의 출처를 정리한다.

| 발생 위치 | 메시지 형태 | 상황 |
|---|---|---|
| `TemplateValidationError` | `"template must have at least one stage"` | 단계 0개인 템플릿 저장 시도 |
| | `"stage '{id}' has an empty prompt"` | 프롬프트 빈 문자열/공백만 |
| | `"duplicate stage id '{id}'"` | 같은 템플릿 안에 단계 id 중복 |
| | `"invalid id '{id}': ids must be non-empty, contain only letters, digits, '.', '_', or '-', and not be '.' or '..'"` | 템플릿/단계 id에 `/`, 공백 등 포함 또는 `.`/`..` |
| `StoreError::NotFound` | `"template '{id}' not found"` | `load_template`/`delete_template` 대상 없음 |
| `StoreError::InvalidId` | 위와 동일 패턴 | path traversal 방지용 사전 검증 |
| `RunRecordStoreError::NotFound` | `"run '{id}' not found"` | 존재하지 않는 `run_id` 조회 |
| `OrchestratorError::NotAwaitingCheckpoint` | `"run '{id}' is not awaiting a checkpoint"` | 체크포인트 대기 중이 아닌 런에 승인/거부/수정요청 시도 |
| I/O 계열 | `"io error: ..."` | 디스크 읽기/쓰기 실패 |
| JSON 계열 | `"json error: ..."` | 저장된 JSON 파일 파싱 실패(손상) |
| `claude` 프로세스 실패 | `StageEvent::ProcessError { exitCode, stderr }` (이벤트로 전달, `Result` 반환값 아님) | CLI 비정상 종료, 타임아웃(30분), stdout 읽기 실패 |

---

## 4. 커맨드 (Commands)

### 4.1 템플릿 관리

#### `list_templates` — 템플릿 전체 목록 조회

**요청**: 인자 없음

```ts
invoke("list_templates")
```

**성공 응답**: `Template[]` (이름순 정렬)
```json
[
  {
    "id": "web-app-dev",
    "name": "웹 프로그램 개발",
    "description": "요구사항 정의부터 배포까지 웹 애플리케이션 개발 전체 파이프라인",
    "stages": [
      {
        "id": "requirements",
        "name": "요구사항 정의",
        "prompt": "PRD.md를 작성하라. project-architect 스킬을 사용해...",
        "permissionMode": "acceptEdits",
        "allowedTools": ["Read", "Write", "Glob", "Grep", "WebSearch"],
        "checkpoint": true
      }
    ]
  }
]
```

**에러**: 디렉터리 접근 실패 시 `"io error: ..."`. 저장 디렉터리(`templates/`)가 없으면
빈 배열 `[]`을 반환한다(에러 아님).

---

#### `load_template` — 템플릿 단건 조회

**요청**
```ts
invoke("load_template", { id: "web-app-dev" })
```

| 파라미터 | 타입 | 설명 |
|---|---|---|
| `id` | string | 템플릿 id (`[A-Za-z0-9._-]+`, `.`/`..` 불가) |

**성공 응답 (200 상당)**: `Template` 객체 1개 (형태는 4.1 참고)

**에러**
| 상황 | 메시지 |
|---|---|
| 존재하지 않는 id | `"template '{id}' not found"` |
| id에 `/` 등 불허 문자 포함 (path traversal 시도 포함) | `"invalid id '{id}': ..."` |

---

#### `save_template` — 템플릿 생성/수정 (upsert)

**요청**
```ts
invoke("save_template", { template: { id, name, description, stages } })
```

**Request Body 스키마** (`Template`)
| 필드 | 타입 | 설명 |
|---|---|---|
| `id` | string | 고유 id, 파일명(`templates/{id}.json`)으로 그대로 사용됨 |
| `name` | string | 표시 이름 |
| `description` | string | 설명 |
| `stages` | `Stage[]` | 최소 1개 필수 |

**`Stage` 스키마**
| 필드 | 타입 | 설명 |
|---|---|---|
| `id` | string | 단계 id, 템플릿 내에서 유일해야 함 |
| `name` | string | 표시 이름 |
| `prompt` | string | `claude -p`로 전달될 프롬프트, 공백만으로 채울 수 없음 |
| `permissionMode` | `"acceptEdits" or "bypassPermissions" or "default"` | `claude --permission-mode` 값 |
| `allowedTools` | string[] | `claude --allowedTools` 콤마 조인 값 (예: `["Read","Write"]`) |
| `checkpoint` | boolean | true면 단계 종료 후 사람 승인 대기 |

**성공 응답**: `null` (Rust `Result<(), String>`의 `Ok(())`가 JS에서 `undefined`)

**에러**: `id`/각 단계 `id` 검증 실패, 빈 프롬프트, 중복 단계 id → 3절 표 참고.
같은 `id`로 저장하면 기존 파일을 덮어쓴다(파일 경로가 곧 PK이므로 별도의 "이미 존재함"
409류 에러는 없다).

---

#### `delete_template` — 템플릿 삭제

**요청**
```ts
invoke("delete_template", { id: "web-app-dev" })
```

**성공 응답**: `null`

**에러**: `"template '{id}' not found"`, 또는 invalid id.

---

### 4.2 환경 확인

#### `check_cli` — `claude` CLI 설치 여부 확인

**요청**: 인자 없음

**성공 응답**: 문자열 하나 (이 커맨드만 `Result`가 아니라 `String`을 직접 반환 — 실패해도
reject하지 않음)
| 값 | 의미 |
|---|---|
| `"available:{version 출력}"` | `claude --version` 실행 성공, 예: `"available:1.2.3"` |
| `"not-found"` | 바이너리 실행 실패(미설치 또는 `PATH` 밖) |

프론트엔드는 이 문자열을 파싱해 `status.startsWith("available")`로 배지를 표시한다
(`src/App.tsx`의 `CliStatus` 컴포넌트, `src/pages/TemplateGallery.tsx`).

---

### 4.3 파이프라인 실행 (Orchestrator)

#### `start_pipeline_run` — 새 파이프라인 실행 시작 (비동기)

**요청**
```ts
invoke("start_pipeline_run", {
  templateId: "web-app-dev",
  targetDir: "/Users/me/projects/my-new-app",
  runId: crypto.randomUUID()
})
```

| 파라미터 | 타입 | 설명 |
|---|---|---|
| `templateId` | string | 실행할 템플릿 id |
| `targetDir` | string | 파이프라인이 실행될 대상 폴더(절대 경로). 존재하지 않으면 생성됨 |
| `runId` | string | 프론트엔드에서 미리 생성한 실행 id(`crypto.randomUUID()`) — `runs/{runId}.json`으로 저장 |

**동작**: 템플릿을 로드해 첫 번째 단계부터 `claude` CLI를 스폰하고, 단계에 `checkpoint: true`가
설정된 첫 지점(또는 실패, 또는 마지막 단계 완료)까지 자동으로 진행한다. 각 단계 이벤트는
`pipeline://stage-event`로 실시간 emit된다(5절 참고).

**성공 응답**: `RunRecord` (아래 구조, → [ER 다이어그램](er-diagram.md) 참고)
```json
{
  "runId": "b3f1...",
  "templateId": "web-app-dev",
  "targetDir": "/Users/me/projects/my-new-app",
  "status": "awaiting-checkpoint",
  "currentStageIndex": 0,
  "stages": [
    { "id": "requirements", "status": "awaiting-checkpoint", "sessionId": "sess-abc", "log": [ /* StageEvent[] */ ] },
    { "id": "design", "status": "pending", "sessionId": null, "log": [] }
  ]
}
```

**에러**: 템플릿을 찾을 수 없음, 대상 폴더 생성 실패(`io error`) 등.
> v1 제약: 이미 실행 기록이 있는 폴더에 대한 중복 실행 방지는 프론트엔드/백엔드
> 어디에도 구현돼 있지 않다(→ [사용 설명서](user-guide.md)의 알려진 제약 참고).

---

#### `approve_checkpoint` — 체크포인트 승인, 다음 단계로 진행 (비동기)

**요청**
```ts
invoke("approve_checkpoint", { runId: "b3f1..." })
```

**동작**: 현재 단계를 `approved`로 표시하고, 마지막 단계가 아니면 다음 단계를
`--resume {이전 session_id}`로 이어서 실행한다. 마지막 단계였다면 런 상태를
`completed`로 전환한다.

**성공 응답**: 갱신된 `RunRecord`

**에러**: 런이 `awaiting-checkpoint` 상태가 아니면 `"run '{id}' is not awaiting a checkpoint"`.
찾을 수 없는 `runId`면 `"run '{id}' not found"`.

---

#### `request_changes` — 수정 요청 보내기 (같은 단계 재실행, 비동기)

**요청**
```ts
invoke("request_changes", { runId: "b3f1...", feedback: "버튼 색상을 변경해줘" })
```

| 파라미터 | 타입 | 설명 |
|---|---|---|
| `runId` | string | 대상 실행 id |
| `feedback` | string | 사람이 입력한 수정 요청 텍스트. 이 텍스트가 **원래 단계 프롬프트를 완전히 대체**해 같은 `claude --resume` 세션으로 재실행됨 |

**성공 응답**: 갱신된 `RunRecord` (같은 단계가 다시 `awaiting-checkpoint`로 돌아옴, 실패 시 `failed`)

**에러**: `approve_checkpoint`와 동일한 상태 검증 규칙.

---

#### `reject_checkpoint` — 체크포인트 거부, 런 취소 (동기)

**요청**
```ts
invoke("reject_checkpoint", { runId: "b3f1..." })
```

**동작**: 다른 승인/수정요청 커맨드와 달리 `claude` 프로세스를 실행하지 않는 유일한 실행계
커맨드다 — 단순히 런 상태를 `cancelled`로 표시하고 저장한다.

**성공 응답**: 갱신된 `RunRecord` (`status: "cancelled"`)

**에러**: `approve_checkpoint`와 동일한 상태 검증 규칙.

---

## 5. 실시간 스트리밍 채널 — `pipeline://stage-event`

HTTP 기반 웹소켓/SSE가 없는 대신, Tauri의 전역 이벤트 버스를 사용한다.
백엔드는 `app.emit("pipeline://stage-event", payload)`로 브로드캐스트하고,
프론트엔드는 `listen("pipeline://stage-event", handler)`로 구독한다
(`src/api.ts`의 `onStageEvent`).

**구독 예시**
```ts
import { onStageEvent } from "./api";

const unlisten = await onStageEvent((payload) => {
  if (payload.runId !== currentRunId) return; // 여러 런이 동시에 열려 있을 수 있으므로 runId로 필터링
  console.log(payload.stageId, payload.event);
});
// 화면을 벗어날 때
unlisten();
```

**페이로드 형태**
```json
{
  "runId": "b3f1...",
  "stageId": "requirements",
  "event": { "kind": "assistantText", "text": "PRD.md를 작성하겠습니다..." }
}
```

**`event` (`StageEvent`) 종류** — `claude --print --output-format=stream-json --verbose`의
stdout 한 줄 한 줄을 파싱한 결과(`src-tauri/src/engine/stream_json.rs`):

| `kind` | 발생 시점 | 필드 |
|---|---|---|
| `init` | 세션 시작 (stream-json `type: "system"`) | `sessionId: string` |
| `assistantText` | 어시스턴트 텍스트 응답 조각 | `text: string` |
| `toolUse` | 어시스턴트가 도구를 호출 | `name: string`, `input: unknown` |
| `toolResult` | 도구 실행 결과 | `content: unknown` |
| `result` | 단계(턴) 종료 | `sessionId: string`, `success: boolean`, `result: string or null` |
| `processError` | `claude` 프로세스가 비정상 종료/타임아웃/stdout 읽기 실패 (stream-json 라인이 아니라 executor가 직접 합성) | `exitCode: number or null`, `stderr: string` |
| `unknown` | 인식하지 못한 이벤트 타입 또는 JSON 파싱 실패 라인 | `raw: unknown` |

이 이벤트들은 `RunRecord.stages[i].log`에도 순서대로 누적 저장되므로, 실시간 스트림을
놓쳐도(예: 창을 나중에 열었을 때) `load_template`/실행 조회를 통해 전체 로그를 다시 볼 수 있다.
단, v1에는 진행 중인 런을 다시 여는 프론트엔드 화면이 없다(→ [사용 설명서](user-guide.md)).

**타임아웃 정책**: 한 단계가 30분(`STAGE_TIMEOUT`, `src-tauri/src/engine/executor.rs`)
동안 stdout 출력이 없으면 프로세스를 강제 종료하고 `processError` 이벤트를 emit한다.
