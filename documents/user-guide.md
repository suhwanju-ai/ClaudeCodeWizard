# Claude Pipeline Wizard 사용 설명서

**버전**: v0.1.0 | **작성일**: 2026-09-07

---

## 1. 시스템 요구사항

| 구분 | 요구사항 |
|------|---------|
| OS | Windows / macOS / Linux (Tauri v2 지원 플랫폼) |
| Node.js | 18+ (npm 포함) |
| Rust | stable 툴체인 (`rustup`) |
| Tauri v2 사전 요구사항 | OS별 WebView 런타임 등 — [Tauri v2 Prerequisites](https://v2.tauri.app/start/prerequisites/) 참고 |
| `claude` CLI | [Claude Code](https://claude.com/claude-code) 설치 후 `PATH`에 등록, **로그인(인증) 완료 상태** — 미설치 시 앱은 뜨지만 파이프라인 실행 기능이 동작하지 않음 |
| Windows 전용 (인스톨러 빌드 시) | Visual Studio Build Tools (C++), WebView2 런타임(없으면 인스톨러에 자동 번들) |
| Linux 전용 (인스톨러 빌드 시) | `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` |

---

## 2. 설치 가이드

### 2.1 저장소 클론

```bash
git clone <이 저장소 URL>
cd ClaudeCodeWizard
```

### 2.2 의존성 설치 및 개발 모드 실행 (가장 빠른 시작)

```bash
npm install
npx tauri dev
```

Vite 개발 서버(포트 1420)와 Tauri 데스크톱 창이 함께 뜬다. 최초 실행 시 앱 데이터
디렉터리에 시드 템플릿 2개("웹 프로그램 개발", "데스크톱 프로그램 개발 (Tauri)")가
자동 생성된다.

### 2.3 준비된 개발 서버 스크립트 사용 (권장 — 사전 점검 포함)

이 스크립트들은 Rust/Node.js/`claude` CLI 설치 여부, 포트 충돌, 의존성 버전 드리프트까지
자동으로 점검한다.

**macOS / Linux**
```bash
bash scripts/dev/start-dev.sh
```

**Windows**
```bat
scripts\dev\start-dev.bat
```

pnpm/yarn lockfile이 있으면 자동으로 해당 패키지 매니저를 사용한다. `node_modules`가
없으면 자동 설치하며, 설치가 실패하고 `package.json`/`package-lock.json`이 마지막 커밋과
다른 상태라면(알려진 vite/vitest 버전 드리프트 문제) 자동으로 `git checkout`으로 되돌린 뒤
한 번 더 설치를 재시도한다.

### 2.4 환경변수 설정 — 해당 없음

이 앱은 `.env` 파일이나 환경변수 기반 설정을 사용하지 않는다. 앱 설정은
`src-tauri/tauri.conf.json`(윈도우 크기, 앱 식별자 등 빌드 타임 설정)에 고정되어 있고,
런타임에 필요한 유일한 외부 의존성은 `PATH`에 있는 `claude` CLI 바이너리(로그인 상태
포함)뿐이다.

### 2.5 DB 마이그레이션 / 초기 데이터 — 해당 없음

DBMS를 사용하지 않으므로 마이그레이션 명령이 없다. "초기 데이터"에 해당하는 것은
2.2절에서 언급한 시드 템플릿 2개이며, 앱 최초 실행 시 `template::seed::seed_default_templates()`가
자동으로 생성한다(이미 같은 id의 템플릿이 있으면 덮어쓰지 않음, →
[DB 설계 문서](database-design.md)).

---

## 3. 실행 방법 (환경별)

### 3.1 개발 환경

```bash
npm run dev       # Vite 개발 서버만 (프론트엔드 단독, Tauri 창 없이 브라우저에서 UI만 확인)
npx tauri dev     # Vite 개발 서버 + Tauri 데스크톱 창 (실제 앱처럼 동작)
```

### 3.2 릴리스 바이너리 빌드 (번들 없이, 로컬 실행 파일만)

**macOS / Linux**
```bash
bash scripts/build/build.sh
```

**Windows**
```bat
scripts\build\build.bat
```

수행 단계: TypeScript 타입 체크(`tsc --noEmit`) → 프론트엔드 테스트(`npm run test`) →
Rust 컴파일 체크(`cargo check`) → Rust 테스트(`cargo test`) → `tauri build -- --no-bundle`.
결과물은 `src-tauri/target/release/claude-pipeline-wizard`(Windows는 `.exe`)에 생성된다.

### 3.3 배포용 설치 패키지(인스톨러) 빌드

**macOS / Linux** — OS를 자동 감지해 macOS는 dmg+app, Linux는 deb+AppImage를 빌드
```bash
bash scripts/installer/build-installer.sh
```

**Windows** — MSI + NSIS(.exe) 빌드
```bat
scripts\installer\build-installer.bat
```

두 스크립트 모두 빌드 전 `node scripts/installer/bump-version.cjs`로 patch 버전을
자동 증가시켜, 빌드할 때마다 서로 다른 버전의 설치 파일이 생성되게 한다. Windows
스크립트는 추가로 Visual Studio Build Tools(`cl` 명령)와 WebView2 런타임 설치 여부를
점검하고, 결과물 경로(`src-tauri/target/release/bundle/nsis/*.exe`,
`.../bundle/msi/*.msi`)를 출력한다.

### 3.4 프로덕션(설치된 앱) 실행

인스톨러로 설치한 뒤에는 OS 표준 방식으로 실행한다(Windows: 시작 메뉴, macOS:
Applications 폴더, Linux: 데스크톱 엔트리 또는 AppImage 직접 실행). 별도의 "프로덕션
서버 기동" 절차는 없다 — 로컬 데스크톱 앱이기 때문이다.

---

## 4. 주요 기능 사용법

### 4.1 템플릿 갤러리 (첫 화면)

```
+--------------------------------------------------+
| 템플릿 갤러리                        [+ 새 템플릿] |
| CLI 상태: (초록) available:1.2.3                  |
+--------------------------------------------------+
| [웹 프로그램 개발]        | [데스크톱 프로그램...]  |
| web-app-dev  6단계        | desktop-app-dev... 5단계|
| ●요구사항 ●설계 ●프론트... | ●요구사항 ●설계 ●구현...|
| [실행][편집]        [삭제]| [실행][편집]      [삭제]|
+--------------------------------------------------+
```

각 카드의 점(●)은 체크포인트(checkpoint: true) 여부를 색으로 표시한다. 좌측 하단에는
`claude` CLI 상태 배지와 다크/라이트 테마 전환 버튼이 있다.

### 4.2 템플릿 편집

편집 화면에서: 템플릿 이름/설명 수정 → 좌측 "단계 목록"에서 단계 선택/추가/삭제/순서
변경 → 우측 패널에서 선택한 단계의 이름/ID/프롬프트/권한 모드(`acceptEdits` /
`bypassPermissions` / `default`)/허용 도구(Read, Write, Edit, Glob, Grep, Bash,
WebSearch, WebFetch)/체크포인트 스위치를 편집한다. 상단의 `{ } JSON` 버튼으로 현재
템플릿의 원시 JSON을 바로 확인할 수 있다. 편집 화면에는 **실행 버튼이 없다** — 저장은
디스크에만 반영하며, 실행은 갤러리 카드의 **실행** 버튼에서 시작한다. 이렇게 분리해 둔
덕분에 "실행하려다 원본 템플릿이 덮어써지는" 일이 생기지 않는다. 이 run에서만 쓰고 싶은
수정은 저장하지 말고, 실행 화면의 실행 전 게이트에서 하면 된다.

### 4.3 파이프라인 실행 화면

좌측에 단계 타임라인(대기/실행 전 대기/진행/체크포인트 대기/승인됨/실패를 점 색으로
표시), 우측에 실시간 로그가 `pipeline://stage-event`를 통해 쌓인다.

**(1) 실행 전 게이트 — 모든 단계에 매번 나타난다**

run을 시작하면 아무것도 실행되지 않은 채 "다음 단계 — 실행 전 확인/수정" 패널이 먼저
뜬다. 여기에는 해당 단계의 이름·ID·프롬프트·권한 모드·허용 도구·체크포인트 스위치가
템플릿 값 그대로 채워져 있고, **단계 ID만 잠겨 있다**(바꾸면 실행 기록과 어긋난다).

| 버튼 | 동작 |
|---|---|
| **이 단계 실행** | 화면에 보이는 내용 그대로 이 단계 하나만 실행한다 |
| **이 run 취소** | run을 `cancelled`로 종료하고 갤러리로 돌아간다 |

> 여기서 고친 내용은 **이 run에만** 적용되며 저장된 템플릿은 바뀌지 않는다. 반대로 하단의
> **원본 템플릿 편집 (모든 향후 실행에 적용)** 버튼은 저장된 템플릿을 바꾸는 완전히 다른
> 경로이며, 진행 중인 run에는 반영되지 않는다 — 눌렀을 때 확인 창이 한 번 뜬다.

**(2) 체크포인트 — `checkpoint: true` 단계가 끝난 직후에만 나타난다**

| 버튼 | 동작 |
|---|---|
| **승인** | 현재 단계를 완료 처리하고 **다음 단계의 실행 전 게이트로 돌아간다**(마지막 단계면 파이프라인 완료) |
| **수정 요청 보내기** | 입력한 피드백 텍스트로 **같은 단계를 재실행**(원래 프롬프트를 완전히 대체). 이 피드백 텍스트는 어디에도 저장되지 않는다 |
| **거부** | 프로세스를 다시 실행하지 않고 파이프라인 자체를 취소(cancelled) 처리 |

변경된 파일 배지(Write/Edit 도구 호출에서 감지한 경로)가 체크포인트 카드 위에 같이
표시되어, 승인 전에 무엇이 바뀌었는지 훑어볼 수 있다.

**승인해도 다음 단계가 자동으로 시작되지 않는다** — 승인의 결과는 언제나 다음 단계의 실행 전
게이트다. 단계 실행이 실패하면 run은 `failed`로 종료되며, 그 run에서는 재시도할 수 없다.
프롬프트를 고쳐 다시 시도하려면 새 run을 시작해야 한다.

---

## 5. 관리자 기능

이 앱에는 별도의 "관리자" 역할이나 다중 사용자 권한 체계가 없다(로컬 단일 사용자
데스크톱 앱). 관리에 해당하는 작업은 아래와 같다.

- **템플릿 관리**: 갤러리 화면의 편집/삭제 버튼으로 직접 관리. 삭제는 확인
  다이얼로그(`window.confirm`)를 거친다.
- **로그 확인**: 진행 중인 실행의 로그는 실행 화면에서 실시간으로 보이며, 완료 후에도
  `runs/{run_id}.json`의 각 단계 `log` 배열에 전체 이벤트가 원문으로 남는다.
- **설정 변경**: 앱 자체 설정 화면은 없다. 유일한 사용자 설정은 다크/라이트 테마이며
  `localStorage`(`cpw-theme` 키)에 저장된다.

---

## 6. 운영 가이드

### 6.1 데이터 위치

| 데이터 | 경로 |
|---|---|
| 템플릿 | `<app_data_dir>/templates/{id}.json` |
| 실행 기록/로그 | `<app_data_dir>/runs/{run_id}.json` |
| 격리된 구 형식 실행 기록 | `<app_data_dir>/runs-legacy-v1/{run_id}.json` (앱이 읽지 않음) |
| 프로젝트 로컬 매니페스트 | `<targetDir>/.claude-pipeline-wizard/run.json`, `pipeline.json` |

`<app_data_dir>`(Tauri `app_data_dir()`, 식별자 `com.suhwanju.claude-pipeline-wizard`
기준):
- Windows: `%APPDATA%\com.suhwanju.claude-pipeline-wizard\`
- macOS: `~/Library/Application Support/com.suhwanju.claude-pipeline-wizard/`
- Linux: `~/.local/share/com.suhwanju.claude-pipeline-wizard/`

### 6.2 로그 확인

앱 전용 로그 파일(예: `logs/app.log`)은 없다. 실행 로그는 6.1절의 `runs/*.json`
파일이 곧 로그다. `claude` 프로세스가 표준에러(stderr)에 남긴 내용은 실패 시
`ProcessError` 이벤트의 `stderr` 필드로 캡처되어 같은 JSON 안에 포함된다.

### 6.3 백업

```bash
# 템플릿과 실행 기록을 통째로 백업 (예: macOS)
cp -r ~/Library/Application\ Support/com.suhwanju.claude-pipeline-wizard/templates ./backup-templates
cp -r ~/Library/Application\ Support/com.suhwanju.claude-pipeline-wizard/runs ./backup-runs
```

DBMS가 없으므로 `pg_dump` 같은 전용 백업 도구도 없다 — 위 디렉터리를 그대로
복사/복원하면 된다.

### 6.4 장애 대응

| 증상 | 원인 | 해결 방법 |
|------|------|---------|
| 갤러리에 "claude CLI가 설치되어 있지 않습니다" 경고 | `claude` 바이너리가 `PATH`에 없거나 `--version` 실행 실패 | `claude` CLI 설치 및 로그인 확인, 터미널에서 `claude --version` 직접 실행해 검증 |
| 파이프라인이 시작하자마자 실패(failed) | `claude` 프로세스가 비정상 종료 | 실행 화면 로그의 마지막 `error` 라인(stderr 내용) 확인 — 대부분 인증 만료 또는 잘못된 `--allowedTools` 값 |
| 특정 단계에서 30분 넘게 멈춰 있다가 자동 실패 | `STAGE_TIMEOUT`(30분) 도달 | 프롬프트가 무한정 응답을 기다리게 만드는 내용이 아닌지 확인, 네트워크/Anthropic API 상태 확인 |
| 앱을 재시작했더니 진행 중이던 런이 보이지 않음 | **v1 알려진 제약** — 재시작 후 실행 중이던 런을 다시 여는 화면이 없음 | `runs/{run_id}.json`을 직접 열어 마지막 `status`/`currentStageIndex`/`stages[].log`를 확인. 필요하면 해당 대상 폴더에서 같은 템플릿으로 새 런을 시작 |
| 템플릿/실행 목록 조회 자체가 에러(`json error`)로 실패 | `templates/` 또는 `runs/` 안의 특정 JSON 파일이 손상됨 | 6.1절 경로에서 손상된 `.json` 파일을 찾아 이름을 바꾸거나 삭제(→ [DB 설계 문서](database-design.md) 5절) |

---

## 7. 업데이트 / 마이그레이션

- **앱 자체 업데이트**: 자동 업데이트 기능은 없다. 새 버전은 3.3절의 인스톨러 빌드
  절차로 다시 패키징해 재설치해야 한다. `bump-version.cjs`가 `tauri.conf.json` /
  `package.json` / `Cargo.toml`의 버전을 함께 올린다.
- **데이터 마이그레이션**: 템플릿/실행 기록 스키마(Rust 구조체)가 바뀌면 기존
  `templates/*.json`, `runs/*.json`과의 호환성을 반드시 확인해야 한다(→
  [DB 설계 문서](database-design.md) 5절 "마이그레이션 전략" 참고) — 전용 마이그레이션
  도구는 없으므로 파괴적 변경 시 수동 변환이 필요하다.

---

## 8. 알려진 v1 제약 사항

(원문: 프로젝트 루트 `README.md` "Known v1 limitations")

- **새로 만든 폴더만 지원** — 템플릿은 사용자가 고른 폴더에 대해 실행되며, 이미
  `.claude-pipeline-wizard/run.json`이 있는 폴더는 **거부된다**(덮어쓰지 않는다).
  기존 코드베이스에 대해 실행하는 것은 아직 지원되지 않는다.
- **재시작 후 재개(resume) UI 없음** — 실행 상태는 전이마다 디스크에 저장되지만,
  **앱을 재시작한 뒤 진행 중이던 런에 다시 연결할 커맨드나 화면이 없다.** 오늘 기준
  복구 방법은 `runs/` 디렉터리 아래의 JSON 파일을 직접 열어 확인하는 것뿐이다
  (6.4절 참고).
- **프로젝트 로컬 매니페스트는 조회를 쉽게 할 뿐 resume을 해결하지 않는다** —
  `<targetDir>/.claude-pipeline-wizard/`가 매 전이마다 갱신되므로 폴더만 열어도 상태를
  읽을 수 있지만, 앱은 이 파일을 **다시 읽어들이지 않는다.** 위 제약은 그대로다.
- **구 형식 실행 기록은 열 수 없다** — 이 버전 최초 기동 시 `resolvedStages`가 없는 기존
  레코드는 `runs-legacy-v1/`로 이동된다. 삭제되지는 않지만 앱에서 열 수는 없다.
- **실패한 run은 종료 상태다** — 실패한 단계를 같은 run에서 다시 실행할 수 없다. 새 run을
  시작해야 한다.
- **순차 단계만 지원** — 템플릿 내에서 단계의 병렬 실행이나 조건 분기는 지원하지
  않는다.
