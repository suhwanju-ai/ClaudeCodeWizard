# 스크립트 사용 가이드

Claude Pipeline Wizard의 개발/빌드/인스톨러 스크립트 모음입니다.

## 빠른 시작

| 목적 | Linux/Mac | Windows |
|------|-----------|---------|
| 개발 서버 시작 | `./scripts/dev/start-dev.sh` | `scripts\dev\start-dev.bat` |
| Release 빌드 (번들 없이) | `./scripts/build/build.sh` | `scripts\build\build.bat` |
| 인스톨러 빌드 | `./scripts/installer/build-installer.sh` | `scripts\installer\build-installer.bat` |

## 실행 권한 부여 (Linux/Mac)

```bash
chmod +x scripts/**/*.sh
```

## 사전 요구사항

- Node.js 18+ / npm
- Rust (stable) + [Tauri v2 사전 요구사항](https://v2.tauri.app/start/prerequisites/)
- `claude` CLI (PATH에 등록, 로그인 완료) — 파이프라인 실행 기능에 필요
- Windows 인스톨러 빌드 시: Visual Studio Build Tools (C++ 워크로드), WebView2 런타임(없으면 자동 번들)

## 각 스크립트 설명

### `dev/start-dev.sh` / `.bat`
Rust/Node/claude CLI 설치 여부 확인 → 의존성 자동 설치 → `npm run tauri dev` 실행.

### `build/build.sh` / `.bat`
TypeScript 타입 체크 → 프론트엔드 테스트 → Rust 컴파일 체크 → Rust 테스트 → `tauri build --no-bundle`로
번들 없이 실행 바이너리만 생성. CI나 코드 검증 목적으로 적합합니다.

### `installer/build-installer.sh` / `.bat`
실제 배포용 인스톨러(Windows: MSI+NSIS, macOS: dmg+app, Linux: deb+AppImage)를 빌드합니다.
빌드 직전에 `installer/bump-version.cjs`를 호출해 `tauri.conf.json`/`package.json`/`Cargo.toml`의
patch 버전을 자동으로 1씩 올리므로, 빌드할 때마다 서로 다른 버전의 설치 파일이 생성됩니다.

인스톨러 세부 설정(아이콘 교체, 서명 등)은 `installer/installer-config.md`를 참고하세요.

## ⚠️ 배포 전 확인할 것

- 코드 서명이 설정되어 있지 않습니다.
- 이 앱은 최종 사용자 PC에도 `claude` CLI가 설치되어 있어야 동작합니다.
