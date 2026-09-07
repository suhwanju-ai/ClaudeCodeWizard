#!/bin/bash
# ============================================================
# 프로젝트명: Claude Pipeline Wizard
# 스크립트:  인스톨러 패키지 빌드 (Linux/Mac)
# 생성:      deploy-scripts-builder (자동 생성)
# ============================================================
set -e
trap 'echo -e "${RED}❌ 오류 발생! 라인 $LINENO${NC}"; exit 1' ERR

# 항상 저장소 루트에서 실행 (호출 시점 cwd가 어디든 무관하게)
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../.."

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'

OS=$(uname -s)
echo -e "${CYAN}==============================================${NC}"
echo -e "${CYAN} Claude Pipeline Wizard 인스톨러 빌드 시작${NC}"
echo -e "${CYAN} OS: ${OS}${NC}"
echo -e "${CYAN}==============================================${NC}"

# 사전 요구사항 확인
rustc --version >/dev/null 2>&1 || { echo -e "${RED}❌ Rust 없음${NC}"; exit 1; }
node --version >/dev/null 2>&1  || { echo -e "${RED}❌ Node.js 없음${NC}"; exit 1; }

PKG_MGR="npm"
[ -f "pnpm-lock.yaml" ] && PKG_MGR="pnpm"
[ -f "yarn.lock" ] && PKG_MGR="yarn"

# 의존성 설치
[ ! -d "node_modules" ] && $PKG_MGR install

# Rust 컴파일 검사
echo -e "${YELLOW}🦀 Rust 컴파일 검사...${NC}"
cargo check --manifest-path src-tauri/Cargo.toml

# 버전 자동 증가 (patch += 1) — 빌드마다 다른 버전의 설치 파일이 나오게 한다
echo -e "${YELLOW}🔢 버전 자동 증가 중...${NC}"
node scripts/installer/bump-version.cjs

# macOS 전용 설정
if [ "$OS" = "Darwin" ]; then
  echo -e "${YELLOW}🍎 macOS 인스톨러 빌드 (dmg + app)...${NC}"
  $PKG_MGR run tauri build -- --bundles dmg,app

  echo -e "${GREEN}✅ macOS 빌드 완료!${NC}"
  echo -e "${GREEN}📦 인스톨러 위치:${NC}"
  find src-tauri/target/release/bundle -name "*.dmg" -o -name "*.app" 2>/dev/null | head -5

# Linux 전용 설정
elif [ "$OS" = "Linux" ]; then
  # 필수 패키지 확인
  if command -v dpkg >/dev/null 2>&1; then
    dpkg -l | grep -q "libwebkit2gtk" || {
      echo -e "${YELLOW}📦 libwebkit2gtk 설치 중...${NC}"
      sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
    }
  fi

  echo -e "${YELLOW}🐧 Linux 인스톨러 빌드 (deb + AppImage)...${NC}"
  $PKG_MGR run tauri build -- --bundles deb,appimage

  echo -e "${GREEN}✅ Linux 빌드 완료!${NC}"
  echo -e "${GREEN}📦 인스톨러 위치:${NC}"
  find src-tauri/target/release/bundle -name "*.deb" -o -name "*.AppImage" 2>/dev/null | head -5

else
  echo -e "${RED}❌ 지원하지 않는 OS입니다: ${OS} (Windows는 build-installer.bat 사용)${NC}"
  exit 1
fi
