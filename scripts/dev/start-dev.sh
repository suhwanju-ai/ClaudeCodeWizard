#!/bin/bash
# ============================================================
# 프로젝트명: Claude Pipeline Wizard
# 스크립트:  Tauri 개발 서버 시작
# 생성:      deploy-scripts-builder (자동 생성)
# ============================================================
set -e
trap 'echo -e "${RED}❌ 오류 발생! 라인 $LINENO${NC}"; exit 1' ERR

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; BLUE='\033[0;34m'; NC='\033[0m'

echo -e "${BLUE}🦀 Claude Pipeline Wizard Tauri 개발 서버 시작...${NC}"

# Rust 확인
rustc --version >/dev/null 2>&1 || { echo -e "${RED}❌ Rust 없음. https://rustup.rs${NC}"; exit 1; }
echo -e "${GREEN}✅ $(rustc --version)${NC}"

# Node.js 확인
node --version >/dev/null 2>&1 || { echo -e "${RED}❌ Node.js 없음${NC}"; exit 1; }
echo -e "${GREEN}✅ Node.js $(node --version)${NC}"

# claude CLI 확인 (없어도 개발 서버는 뜨지만, 파이프라인 실행은 안 됨)
if command -v claude >/dev/null 2>&1; then
  echo -e "${GREEN}✅ claude CLI 확인됨${NC}"
else
  echo -e "${YELLOW}⚠️  claude CLI를 찾을 수 없습니다 — 파이프라인 실행 기능은 동작하지 않습니다.${NC}"
  echo -e "${YELLOW}   https://claude.com/claude-code 참고${NC}"
fi

# 패키지 매니저 자동 감지
PKG_MGR="npm"
[ -f "pnpm-lock.yaml" ] && PKG_MGR="pnpm"
[ -f "yarn.lock" ] && PKG_MGR="yarn"

# 의존성 확인
if [ ! -d "node_modules" ]; then
  echo -e "${YELLOW}📦 의존성 설치 중 (${PKG_MGR})...${NC}"
  $PKG_MGR install
fi
echo -e "${GREEN}✅ 의존성 OK${NC}"

# Cargo 캐시 확인
echo -e "${YELLOW}🔍 Rust 의존성 확인...${NC}"
cargo check --manifest-path src-tauri/Cargo.toml 2>/dev/null || true

# 포트 충돌 확인 (Vite dev server: 1420)
if command -v lsof >/dev/null 2>&1 && lsof -i :1420 >/dev/null 2>&1; then
  echo -e "${YELLOW}⚠️  포트 1420이 이미 사용 중입니다 — 이전 dev 서버가 남아있는지 확인하세요.${NC}"
fi

echo -e "${GREEN}🚀 Tauri 개발 모드 시작...${NC}"
$PKG_MGR run tauri dev
