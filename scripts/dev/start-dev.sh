#!/bin/bash
# ============================================================
# 프로젝트명: Claude Pipeline Wizard
# 스크립트:  Tauri 개발 서버 시작
# 생성:      deploy-scripts-builder (자동 생성)
# ============================================================
set -e
trap 'echo -e "${RED}❌ 오류 발생! 라인 $LINENO${NC}"; exit 1' ERR

# 항상 프로젝트 루트에서 실행되도록 이동 (다른 디렉토리에서 호출돼도
# src-tauri/Cargo.toml 같은 상대경로가 깨지지 않게)
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

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
  if ! $PKG_MGR install; then
    echo -e "${YELLOW}⚠️  의존성 설치 실패.${NC}"
    if ! command -v git >/dev/null 2>&1; then
      echo -e "${RED}❌ git을 찾을 수 없어 자동 복구를 시도할 수 없습니다. 위 npm 오류를 확인하세요.${NC}"
      exit 1
    fi
    if git diff --quiet -- package.json package-lock.json 2>/dev/null; then
      echo -e "${RED}❌ package.json/package-lock.json는 이미 마지막 커밋과 동일합니다 —${NC}"
      echo -e "${RED}   알려진 버전 충돌 문제가 아닙니다. 위 npm 오류를 확인하세요.${NC}"
      exit 1
    fi
    echo -e "${YELLOW}⚠️  package.json/package-lock.json가 마지막 커밋과 다릅니다${NC}"
    echo -e "${YELLOW}   (가끔 vite/vitest가 @vitejs/plugin-react와 충돌하는 버전으로${NC}"
    echo -e "${YELLOW}   바뀌는 알려진 문제) — 커밋된 상태로 되돌리고 한 번 재시도합니다...${NC}"
    git checkout -- package.json package-lock.json
    if ! $PKG_MGR install; then
      echo -e "${RED}❌ 초기화 후에도 의존성 설치 실패${NC}"
      exit 1
    fi
  fi
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
