# Claude Pipeline Wizard API 사용자 가이드

**대상 독자**: 이 앱의 프론트엔드 코드를 수정하거나, Tauri IPC 커맨드를 직접 호출하는
스크립트/테스트를 작성하려는 개발자.

> (→ [API 설계 문서](api-design.md) 참고) 이 앱은 HTTP API가 아니라 **Tauri IPC**로
> 동작한다. 따라서 curl 예제 대신 `@tauri-apps/api/core`의 `invoke()` 예제를 제공한다.

---

## 1. 시작하기

### 1.1 인증 — 해당 없음

로컬 데스크톱 프로세스 간 통신만 존재하므로 토큰 발급 절차가 없다. Tauri 런타임이
프론트엔드(WebView)와 백엔드(Rust) 사이의 IPC 채널 자체를 격리하므로, 외부 네트워크에서
이 앱의 커맨드를 호출할 방법이 없다.

### 1.2 첫 호출

```ts
import { invoke } from "@tauri-apps/api/core";

const templates = await invoke("list_templates");
console.log(templates); // Template[]
```

이 앱 안에서는 직접 `invoke`를 부르지 않고, 얇은 래퍼 모듈 `src/api.ts`를 통해 호출한다.
새 코드를 작성할 때도 이 패턴을 따르는 것을 권장한다.

```ts
// src/api.ts (실제 프로젝트 코드)
export function listTemplates(): Promise<Template[]> {
  return invoke("list_templates");
}
```

### 1.3 Python/셸에서 호출하기 — 해당 없음

`claude` CLI를 감싸는 이 앱 자체는 HTTP 서버를 띄우지 않으므로 `requests`/`curl` 예제가
성립하지 않는다. 백엔드 로직을 스크립트로 검증하고 싶다면 Rust 테스트
(`cargo test --manifest-path src-tauri/Cargo.toml`)를 사용하거나, `mock_claude` 바이너리로
`claude` CLI를 대체해 통합 테스트를 돌리는 방식을 쓴다(→ 5절 참고).

---

## 2. 주요 사용 시나리오

### 시나리오 1 — 템플릿 목록을 불러와 표시하기

```ts
import { listTemplates, checkCli } from "./api";

const templates = await listTemplates();
const cliStatus = await checkCli(); // "available:1.2.3" | "not-found"

if (cliStatus === "not-found") {
  console.warn("claude CLI가 설치되어 있지 않습니다.");
}
```

실제 구현: `src/pages/TemplateGallery.tsx`의 `refresh()` / `useEffect`.

### 시나리오 2 — 템플릿 생성 후 저장하기

```ts
import { saveTemplate } from "./api";
import type { Template } from "./types";

const draft: Template = {
  id: `template-${Date.now()}`,
  name: "새 파이프라인",
  description: "설명",
  stages: [
    {
      id: "s1",
      name: "1단계",
      prompt: "요구사항을 정리하라.",
      permissionMode: "acceptEdits",
      allowedTools: ["Read", "Write"],
      checkpoint: true,
    },
  ],
};

try {
  await saveTemplate(draft);
} catch (e) {
  // e는 Error 객체가 아니라 Rust에서 넘어온 에러 "문자열"이다.
  console.error(String(e)); // 예: "stage 's1' has an empty prompt"
}
```

> **주의**: 클라이언트 측 검증(`src/pages/TemplateEditor.tsx`의 `validate()`)과 백엔드
> 검증(`validate_template`, → [API 설계 문서](api-design.md) 3절)이 이중으로 존재한다.
> 프론트엔드 검증을 우회해도 백엔드가 최종 방어선 역할을 한다.

### 시나리오 3 — 파이프라인 실행 + 실시간 로그 구독 (End-to-End)

`start_pipeline_run`은 프로세스를 하나도 스폰하지 않는다 — run을 만들고 1단계의 실행 전
게이트에서 즉시 반환한다. 실제 실행은 게이트마다 `startStage`를 별도로 호출해야 하며,
그 호출은 `run.currentStageIndex`를 **세대 가드**로 그대로 넘겨야 한다. 이 두 호출을
반복하는 것이 v1의 실행 흐름 전체다.

```ts
import { open } from "@tauri-apps/plugin-dialog";
import { startPipelineRun, startStage, onStageEvent, isStaleStageIndexError, getRun } from "./api";
import type { RunRecord } from "./types";

const targetDir = await open({ directory: true });
if (!targetDir || Array.isArray(targetDir)) throw new Error("폴더를 선택하세요");

const runId = crypto.randomUUID();

// 1) 이벤트 구독을 먼저 건다
const unlisten = await onStageEvent((payload) => {
  if (payload.runId !== runId) return;
  console.log(`[${payload.stageId}]`, payload.event);
});

// 2) run 생성 — 아무 프로세스도 뜨지 않는다. 1단계의 실행 전 게이트에서 멈춘다.
let run: RunRecord = await startPipelineRun("web-app-dev", targetDir, runId);
console.log(run.status); // "awaiting-stage-start"

// 3) 게이트마다 startStage를 호출하며 종료 상태(completed/failed/cancelled)까지 반복한다.
const TERMINAL = new Set(["completed", "failed", "cancelled"]);
while (!TERMINAL.has(run.status)) {
  if (run.status === "awaiting-stage-start") {
    try {
      // editedStage는 사용자가 게이트 패널에서 고친 값(또는 undefined = 수정 없음)
      run = await startStage(run.runId, run.currentStageIndex, undefined);
    } catch (e) {
      if (isStaleStageIndexError(e)) {
        // 화면이 보던 게이트가 이미 지나갔다 — 재동기화만 하고 사용자에게 다시 확인받는다.
        run = await getRun(run.runId);
        continue;
      }
      throw e;
    }
  } else if (run.status === "awaiting-checkpoint") {
    // 시나리오 4 참고 — 승인/수정요청/거부 중 하나를 호출해야 여기서 벗어난다.
    break;
  }
}

// 화면을 벗어날 때 반드시 해제
unlisten();
```

실제 구현: `src/App.tsx`의 `handleRun`, `src/pages/PipelineRun.tsx`의 게이트/체크포인트 렌더 분기.

### 시나리오 4 — 체크포인트 승인 / 수정 요청 / 거부

```ts
import { approveCheckpoint, requestChanges, rejectCheckpoint } from "./api";

// 승인하고 다음 단계로 진행
const afterApprove = await approveCheckpoint(runId);

// 또는: 같은 단계를 피드백과 함께 재실행 ("수정 요청 보내기" 버튼)
const afterFeedback = await requestChanges(runId, "버튼 색상을 파란색으로 바꿔줘");

// 또는: 런 자체를 취소 (프로세스를 다시 실행하지 않는 유일한 케이스)
const afterReject = await rejectCheckpoint(runId);
```

세 커맨드 모두 런이 `"awaiting-checkpoint"` 상태일 때만 성공한다. 이미 완료되었거나
실행 중인 런에 호출하면 `"run '{id}' is not awaiting a checkpoint"` 에러가 reject된다.

### 시나리오 5 — CLI 미설치 상태 처리

```ts
import { checkCli } from "./api";

const status = await checkCli();
if (status === "not-found") {
  // 갤러리 화면 상단에 경고 배너를 표시 (실제 구현: TemplateGallery.tsx)
}
```

`check_cli`는 다른 커맨드와 달리 실패해도 reject하지 않고 `"not-found"` 문자열을
반환하도록 설계되어 있다 — try/catch 없이도 안전하게 다룰 수 있다.

---

## 3. 에러 코드 전체 목록 및 대처 방법

> (→ [API 설계 문서](api-design.md) 3절에 원문 메시지 표가 있다. 여기서는 "발생 시
> 무엇을 해야 하는가"에 집중한다.)

| 에러 메시지(요약) | 원인 | 대처 |
|---|---|---|
| `template must have at least one stage` | 단계 0개 템플릿 저장 시도 | 최소 1개 단계 추가 후 재시도 |
| `stage '{id}' has an empty prompt` | 프롬프트가 빈 문자열/공백 | 해당 단계 프롬프트 입력 |
| `duplicate stage id '{id}'` | 같은 템플릿에 동일 id 두 번 | 단계 id를 유일하게 수정 |
| `invalid id '{id}': ...` | id에 `/`, 공백, `..` 등 불허 문자 | 영문/숫자/`.`/`_`/`-`만 사용 |
| `template '{id}' not found` | 삭제/조회 대상 없음 | id 오타 확인, `list_templates`로 목록 재확인 |
| `run '{id}' not found` | 존재하지 않는 runId | `runId`는 `start_pipeline_run` 호출 시 직접 생성한 값과 일치해야 함 |
| `run '{id}' is not awaiting a checkpoint` | 이미 완료/실패/취소된 런에 승인·거부·수정요청 시도 | 최신 `RunRecord.status`를 먼저 확인 |
| `io error: ...` | 디스크 접근 실패(권한, 디스크 풀 등) | OS 권한/디스크 공간 확인 |
| `json error: ...` | 저장된 템플릿/런 JSON 파일이 손상됨 | 앱 데이터 디렉터리의 해당 파일을 수동 점검(→ [사용 설명서](user-guide.md)) |
| `processError` 이벤트, exitCode 있음 | `claude` CLI가 비정상 종료 | 이벤트의 `stderr` 필드 확인 — 대부분 인증 만료, 잘못된 플래그, 네트워크 오류 |
| `processError` 이벤트, exitCode 없음 + "30분 동안 응답이 없어..." | 단계가 30분간 stdout 출력 없음(타임아웃) | 프롬프트가 무한 대기를 유발하는지 점검, 네트워크/API 상태 확인 |

---

## 4. SDK / 클라이언트 라이브러리

별도의 SDK는 없다. 클라이언트 쪽 유일한 통합 지점은 `src/api.ts`이며, 이 파일 자체가
"SDK" 역할을 한다.

### 4.1 래퍼 함수 ↔ 커맨드 대응표

| `src/api.ts` 함수 | Tauri 커맨드 | 설명 |
|---|---|---|
| `listTemplates()` | `list_templates` | 템플릿 전체 목록 |
| `loadTemplate(id)` | `load_template` | 템플릿 단건 조회 |
| `saveTemplate(template)` | `save_template` | 템플릿 생성/수정(upsert) |
| `deleteTemplate(id)` | `delete_template` | 템플릿 삭제 |
| `checkCli()` | `check_cli` | `claude` CLI 설치 여부 |
| `startPipelineRun(templateId, targetDir, runId)` | `start_pipeline_run` | run 생성. **프로세스를 스폰하지 않는다** — 1단계 실행 전 게이트에서 반환 |
| `startStage(runId, expectedStageIndex, stageOverride?)` | `start_stage` | 게이트에 멈춘 단계 하나를 실행. 두 번째 인자는 선택이 아니라 **세대 가드**이므로 화면이 렌더한 인덱스를 그대로 넘겨야 한다 |
| `cancelRun(runId)` | `cancel_run` | 실행을 `cancelled`로 종료 |
| `getRun(runId)` | `get_run` | 읽기 전용 재조회 |
| `isStaleStageIndexError(error)` | — | `startStage` 거부가 세대 불일치인지 판별하는 유일한 지점. 참이면 에러를 띄우지 말고 `getRun`으로 화면을 갱신한다 |
| `approveCheckpoint(runId)` | `approve_checkpoint` | 체크포인트 승인 — 다음 단계의 실행 전 게이트로 이동(프로세스는 뜨지 않음) |
| `requestChanges(runId, feedback)` | `request_changes` | 피드백 텍스트로 같은 단계 재실행 |
| `rejectCheckpoint(runId)` | `reject_checkpoint` | 체크포인트에서 런 취소 |
| `onStageEvent(handler)` | `pipeline://stage-event`(구독) | 실시간 스테이지 이벤트 리스너 등록 |

새로운 커맨드를 백엔드에 추가했다면:

1. `src-tauri/src/commands.rs`에 `#[tauri::command]` 함수 추가
2. `src-tauri/src/lib.rs`의 `tauri::generate_handler![...]` 목록에 등록
3. `src/api.ts`에 대응하는 `invoke()` 래퍼 함수 추가
4. `src/types.ts`에 요청/응답 타입 추가

이 4단계를 누락하면(특히 2번) 프론트엔드에서 `invoke()` 호출 시 "command not found" 런타임
에러가 발생한다.

---

## 5. FAQ / 트러블슈팅

**Q. `invoke()`가 계속 `"command not found"`로 실패합니다.**
A. `src-tauri/src/lib.rs`의 `tauri::generate_handler![...]`에 해당 커맨드가 등록되어
있는지 확인하세요. 컴파일은 되지만 런타임에만 실패하는 흔한 실수입니다.

**Q. `check_cli`는 `"available"`인데 `start_pipeline_run`이 계속 실패합니다.**
A. `check_cli`는 `claude --version`만 확인합니다. 실제 파이프라인 실행은 로그인 상태,
API 크레딧, `--permission-mode`/`--allowedTools` 조합이 유효한지까지 확인하지 않습니다.
`pipeline://stage-event`의 `processError` 이벤트에 담긴 `stderr`를 확인하세요.

**Q. 앱을 재시작했더니 진행 중이던 런을 다시 열 수 없습니다.**
A. v1의 알려진 제약입니다. 런 상태는 `runs/{runId}.json`에 매 전이(transition)마다
저장되지만, 이를 다시 불러와 재개하는 프론트엔드 화면이 아직 없습니다. 해당 JSON 파일을
직접 열어 마지막 상태를 확인하는 것이 현재 유일한 방법입니다(→ [사용 설명서](user-guide.md)).

**Q. 같은 폴더에 파이프라인을 두 번 실행하면 어떻게 되나요?**
A. v1은 이를 막지 않습니다 — 새 `RunRecord`가 별도로 생성되어 같은 폴더에 대해 독립적으로
동작하며, 두 런이 같은 파일을 동시에 수정할 경우의 충돌은 사용자 책임입니다.

**Q. `permissionMode`는 어떤 값을 받나요?**
A. `"acceptEdits" | "bypassPermissions" | "default"` 세 가지뿐입니다. 이 값은 그대로
`claude --permission-mode <값>`으로 전달됩니다. 헤드리스 실행 특성상 단계 도중에는
권한 프롬프트에 대화형으로 응답할 수 없으므로, 파일을 수정해야 하는 단계는 보통
`acceptEdits`를 사용합니다.

**Q. 여러 개의 `pipeline://stage-event` 리스너가 겹쳐서 로그가 중복 출력됩니다.**
A. `listen()`은 호출할 때마다 새 구독을 추가합니다. 화면 전환 시 반드시
`onStageEvent()`가 반환한 `unlisten` 함수를 `useEffect`의 cleanup에서 호출하세요
(`src/pages/PipelineRun.tsx` 참고).
