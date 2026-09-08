# Claude Pipeline Wizard UML 시퀀스 다이어그램

**참고**: 이 앱은 로그인/회원가입 같은 사용자 인증이 없는 로컬 단일 사용자 데스크톱 앱이다
(→ [API 설계 문서](api-design.md) 1절). 따라서 스킬 템플릿이 요구하는 "인증 플로우" 자리에는
이 앱에서 실제로 존재하는 가장 가까운 개념인 **앱 부팅 및 CLI 가용성 확인 플로우**를 담았다 —
"토큰 발급"을 대체하는 게 아니라, 이 프로젝트에 인증이 없다는 사실 자체를 문서화한 것이다.

---

## 1. 앱 부팅 및 CLI 가용성 확인 플로우

앱 시작 시 시드 템플릿을 생성하고, 갤러리 화면이 `claude` CLI 설치 여부를 확인하는 흐름.
(→ `src-tauri/src/lib.rs`의 `setup()`, `src/pages/TemplateGallery.tsx`)

```mermaid
sequenceDiagram
    participant OS as OS 런처
    participant Tauri as Tauri 런타임
    participant Setup as lib.rs setup
    participant Seed as seed_default_templates
    participant FS as 파일 시스템
    participant UI as TemplateGallery
    participant CLI as claude 바이너리

    OS->>Tauri: 앱 실행
    Tauri->>Setup: setup 훅 호출
    Setup->>FS: app_data_dir 확인
    Setup->>Seed: seed_default_templates
    loop 시드 템플릿 2개 각각
        Seed->>FS: templates/web-app-dev.json 존재 확인
        alt 이미 존재
            Seed-->>Seed: 건너뜀 (사용자 커스터마이즈 보존)
        else 존재하지 않음
            Seed->>FS: 기본 템플릿 저장
        end
    end
    Setup-->>Tauri: Orchestrator를 State로 등록
    Tauri-->>UI: 메인 윈도우 표시

    UI->>Tauri: invoke check_cli
    Tauri->>CLI: claude --version 실행
    alt 실행 성공
        CLI-->>Tauri: 버전 문자열
        Tauri-->>UI: available 콜론 버전
        Note over UI: 초록 배지 표시
    else 실행 파일 없음 또는 실패
        Tauri-->>UI: not-found
        Note over UI: 경고 배너 표시, 실행 기능은 계속 노출됨
    end
```

---

## 2. 핵심 비즈니스 로직 — run 생성부터 게이트, 단계 실행, 체크포인트 승인까지

이 앱에서 가장 중요한 흐름: 템플릿을 골라 run을 만들면(프로세스는 아직 뜨지 않음)
1단계의 실행 전 게이트에서 멈추고, 사용자가 **이 단계 실행**을 눌러야 비로소
`claude` 프로세스가 하나 뜬다. 단계가 끝나면(체크포인트 여부와 무관하게) 항상
다음 단계의 게이트로 돌아갈 뿐, 자동으로 다음 프로세스가 이어지지 않는다.
(→ `src-tauri/src/engine/orchestrator.rs`, `project_manifest.rs`, `executor.rs`, `commands.rs`)

```mermaid
sequenceDiagram
    participant UI as PipelineRun 화면
    participant Cmd as commands.rs
    participant Orch as Orchestrator
    participant Manifest as ProjectManifest
    participant Exec as executor run_stage
    participant Claude as claude 프로세스
    participant RunFS as runs 저장소

    UI->>Cmd: invoke start_pipeline_run templateId targetDir runId
    Cmd->>Orch: start_run
    Orch->>Orch: template_store.load templateId
    Orch->>Orch: RunRecord::new (resolvedStages를 템플릿 stages에서 복사, 0번만 awaiting-start)
    Orch->>RunFS: run_store.save (status awaiting-stage-start, index 0)
    Orch->>Manifest: write run.json plus pipeline.json
    Manifest-->>Orch: ok (실패 시 save_with_rollback으로 직전 스냅샷 복원 후 에러 반환)
    Orch-->>Cmd: RunRecord (awaiting-stage-start, index 0)
    Cmd-->>UI: RunRecord 반환
    Note over UI: "다음 단계 - 실행 전 확인/수정" 게이트 패널 표시. 아직 아무 프로세스도 뜨지 않음

    UI->>Cmd: invoke start_stage runId expectedStageIndex stageOverride
    Cmd->>Orch: start_stage
    Orch->>Orch: 락 획득, status/세대(expectedStageIndex)/stageOverride.id 검증
    Orch->>Orch: stageOverride가 있으면 resolvedStages 해당 인덱스 교체
    Orch->>RunFS: run_store.save (status running)
    Orch->>Manifest: write run.json plus pipeline.json
    Orch->>Orch: 락 해제
    Orch->>Exec: run_stage resolvedStage target_dir resume_session_id
    Exec->>Claude: spawn claude --print --output-format stream-json --verbose --permission-mode mode -p prompt
    loop stdout 한 줄씩
        Claude-->>Exec: stream-json 라인
        Exec->>Exec: parse_line
        Exec-->>Orch: on_event StageEvent
        Orch-->>Cmd: emit_stage_event 콜백 전달
        Cmd-->>UI: emit pipeline colon stage-event (runId stageId event)
        Note over UI: 실시간 로그에 한 줄 추가
    end
    Claude-->>Exec: 프로세스 종료 exit_code 0
    Exec-->>Orch: exit_code 0 반환

    alt stage.checkpoint == true
        Orch->>Orch: mark_current_awaiting_checkpoint session_id
        Orch->>RunFS: run_store.save (status awaiting-checkpoint)
        Orch->>Manifest: write run.json plus pipeline.json
        Orch-->>Cmd: RunRecord 반환
        Cmd-->>UI: RunRecord (승인 대기)
        Note over UI: 승인 / 수정요청 / 거부 버튼 노출
    else stage.checkpoint == false
        Orch->>Orch: advance_to_gate (index+1, status awaiting-stage-start)
        Orch->>RunFS: run_store.save
        Orch->>Manifest: write run.json plus pipeline.json
        Orch-->>Cmd: RunRecord 반환
        Cmd-->>UI: RunRecord (다음 단계의 실행 전 게이트)
        Note over UI: 다음 stage의 게이트 패널 표시. 자동으로 프로세스가 뜨지 않음 - 이 단계 실행을 다시 눌러야 함
    end

    UI->>Cmd: invoke approve_checkpoint runId
    Cmd->>Orch: approve_checkpoint
    Orch->>RunFS: run_store.load runId
    Orch->>Orch: approve_current (다음 stage 게이트로 이동 또는 completed - 프로세스를 스폰하지 않음)
    Orch->>RunFS: run_store.save
    Orch->>Manifest: write run.json plus pipeline.json
    Orch-->>Cmd: 갱신된 RunRecord
    Cmd-->>UI: RunRecord (completed 이거나, 다음 단계의 실행 전 게이트 - awaiting-stage-start index+1)
    Note over UI: 마지막 단계가 아니면 항상 다음 게이트에서 멈춘다. start_stage를 다시 호출해야 다음 claude 프로세스가 뜬다
```

### 2.1 보조 흐름 — 수정 요청 보내기

체크포인트에서 "승인" 대신 "수정 요청 보내기"를 선택했을 때, 같은 단계를 사람이 입력한
피드백으로 재실행하는 흐름.

```mermaid
sequenceDiagram
    participant UI as PipelineRun 화면
    participant Cmd as commands.rs
    participant Orch as Orchestrator
    participant Exec as executor run_stage
    participant RunFS as runs 저장소

    UI->>Cmd: invoke request_changes runId feedback
    Cmd->>Orch: request_changes
    Orch->>RunFS: run_store.load runId
    Orch->>Orch: 상태 확인 (awaiting-checkpoint 아니면 에러)
    Orch->>Orch: feedback_stage 생성 (원본 stage 복제, prompt를 feedback으로 교체)
    Orch->>RunFS: 상태를 running으로 저장
    Orch->>Exec: run_stage feedback_stage resume_session_id=기존 session
    Exec-->>Orch: stream-json 이벤트 반복 emit (2절과 동일 패턴)
    alt 재실행 성공 exit_code 0
        Orch->>Orch: mark_current_awaiting_checkpoint 새 session_id
    else 재실행 실패
        Orch->>Orch: mark_current_failed
    end
    Orch->>RunFS: run_store.save
    Orch-->>Cmd: 갱신된 RunRecord
    Cmd-->>UI: 같은 단계가 다시 승인 대기 상태로 표시
```

---

## 3. 에러 처리 플로우 — claude 프로세스 실패 / 타임아웃

`claude` CLI가 비정상 종료하거나 30분간 응답이 없을 때의 처리 흐름.
(→ `src-tauri/src/engine/executor.rs`)

```mermaid
sequenceDiagram
    participant Orch as Orchestrator start_stage
    participant Exec as executor run_stage
    participant Claude as claude 프로세스
    participant UI as PipelineRun 화면

    Orch->>Exec: run_stage stage target_dir resume_session_id
    Exec->>Claude: spawn 자식 프로세스

    alt 정상 종료지만 exit_code != 0
        Claude-->>Exec: 프로세스 종료 exit_code 1
        Exec->>Exec: stderr 버퍼 읽기
        Exec-->>Orch: on_event ProcessError exitCode 1 stderr 메시지
        Orch-->>UI: emit pipeline colon stage-event (kind processError)
        Orch->>Orch: mark_current_failed (stage 및 run status를 failed로)
        Note over UI: 로그에 종료 코드와 stderr 표시, 체크포인트 버튼 대신 실패 표시
    else stdout 읽기 도중 30분 동안 새 줄이 없음 (STAGE_TIMEOUT)
        Note over Exec: tokio timeout 만료
        Exec->>Claude: child.kill (강제 종료)
        Exec->>Exec: stderr 드레인 수거
        Exec-->>Orch: on_event ProcessError exitCode None stderr 타임아웃 메시지
        Orch-->>UI: emit pipeline colon stage-event (kind processError)
        Orch->>Orch: run_stage가 Err TimedOut 반환
        Note over Orch: start_stage 호출의 exit_code 분기에서 즉시 mark_current_failed
    else stdout을 읽는 도중 IO 에러 발생
        Exec->>Exec: stdout_error에 에러 저장, 루프 종료
        Exec-->>Orch: on_event ProcessError exitCode 있음 stderr에 IO 에러 문구 포함
        Orch->>Orch: mark_current_failed
    end

    Note over UI: 세 경우 모두 최종적으로 run.status는 failed. 체크포인트 승인 없이 실행이 종료된다. 이 run은 재시도할 수 없다 - 새 run이 필요하다.
```

**다음 단계로 자동 진행되지 않는다**는 이 절의 실패/타임아웃 경우만의 이야기가 아니다 —
2절에서 보듯 정상 종료(체크포인트 없는 단계 포함)조차 다음 단계의 게이트로 돌아갈 뿐,
`claude` 프로세스를 자동으로 다시 스폰하지 않는다. 이 절은 그 규칙이 실패 경로에서도
예외 없이 적용된다는 것을 보여준다.

### 3.1 잘못된 상태에서의 커맨드 호출 (검증 에러)

이미 완료되었거나 취소된 런에 대해 승인/거부/수정 요청을 시도할 때의 방어 로직.

```mermaid
sequenceDiagram
    participant UI as 프론트엔드
    participant Cmd as commands.rs
    participant Orch as Orchestrator
    participant RunFS as runs 저장소

    UI->>Cmd: invoke approve_checkpoint runId
    Cmd->>Orch: approve_checkpoint
    Orch->>RunFS: run_store.load runId
    RunFS-->>Orch: RunRecord (status completed 이미 종료됨)
    Orch->>Orch: status != AwaitingCheckpoint 확인
    Orch-->>Cmd: Err NotAwaitingCheckpoint runId
    Cmd-->>UI: Promise reject 문자열 "run runId is not awaiting a checkpoint"
    Note over UI: setError로 화면에 에러 메시지 표시, run 상태는 변경되지 않음
```
