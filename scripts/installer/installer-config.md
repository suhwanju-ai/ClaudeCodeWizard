# Tauri 인스톨러 설정 가이드

## 1. 현재 설정 (src-tauri/tauri.conf.json)

```json
{
  "productName": "claude-pipeline-wizard",
  "version": "0.1.0",
  "identifier": "com.suhwanju.claude-pipeline-wizard",
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.ico"]
  }
}
```

## 2. ⚠️ 아이콘 교체 필요

`src-tauri/icons/icon.ico`, `icon.png`는 개발 초기에 만든 **70~67바이트짜리 placeholder**입니다.
실제 배포 전 반드시 진짜 아이콘으로 교체하세요:

```bash
npm run tauri icon path/to/app-icon.png
# → src-tauri/icons/ 폴더에 32x32.png, 128x128.png, icon.icns, icon.ico 등 모든 크기 자동 생성
```

아이콘 생성 후 `tauri.conf.json`의 `bundle.icon` 배열을 전체 크기 세트로 갱신하세요:

```json
"icon": [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico"
]
```

## 3. 배포 전 권장 추가 설정

```json
{
  "bundle": {
    "copyright": "Copyright © 2026 Your Name/Company",
    "category": "DeveloperTool",
    "shortDescription": "Claude Code 파이프라인 템플릿 실행기",
    "longDescription": "여러 단계로 구성된 Claude Code 파이프라인 템플릿을 저장하고, 새 프로젝트 폴더에 대해 단계별 체크포인트 승인과 함께 실행하는 데스크톱 도구.",
    "windows": {
      "digestAlgorithm": "sha256",
      "webviewInstallMode": { "type": "embedBootstrapper" },
      "wix": { "language": "ko-KR" }
    }
  }
}
```

## 4. 자동 업데이트 설정 (선택, 미구현)

이 프로젝트는 현재 자동 업데이트를 사용하지 않습니다. 필요하면 `tauri-plugin-updater`를 추가하고
`tauri.conf.json`에 아래를 추가하세요:

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": ["https://yourserver.com/update/{{target}}/{{current_version}}"],
      "dialog": true,
      "pubkey": "YOUR_PUBLIC_KEY"
    }
  }
}
```

## 5. 빌드 후 파일 위치

| 플랫폼 | 형식 | 위치 |
|--------|------|------|
| Windows | .exe (NSIS) | `src-tauri/target/release/bundle/nsis/` |
| Windows | .msi | `src-tauri/target/release/bundle/msi/` |
| macOS | .dmg | `src-tauri/target/release/bundle/dmg/` |
| macOS | .app | `src-tauri/target/release/bundle/macos/` |
| Linux | .deb | `src-tauri/target/release/bundle/deb/` |
| Linux | .AppImage | `src-tauri/target/release/bundle/appimage/` |

## 6. 배포 시 유의사항

- 이 앱은 사용자 PC에 별도로 설치된 **`claude` CLI**에 의존합니다. 설치 안내 문서나
  설치 프로그램 첫 화면에 이를 명시하는 것을 권장합니다.
- 코드 서명(codesigning)은 아직 설정되어 있지 않습니다 — Windows SmartScreen 경고,
  macOS Gatekeeper 차단을 피하려면 배포 전 인증서를 준비하세요.
