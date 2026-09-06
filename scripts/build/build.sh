#!/bin/bash
# ============================================================
# 프로젝트명: Claude Pipeline Wizard
# 스크립트:  Release 빌드 (번들 없이 바이너리만)
# 생성:      deploy-scripts-builder (자동 생성)
# ============================================================
set -e
trap 'echo -e "${RED}❌ 오류 발생! 라인 $LINENO${NC}"; exit 1' ERR

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

echo -e "${YELLOW}🔨 Release 빌드 시작...${NC}"

PKG_MGR="npm"
[ -f "pnpm-lock.yaml" ] && PKG_MGR="pnpm"
[ -f "yarn.lock" ] && PKG_MGR="yarn"

# 의존성 확인
[ ! -d "node_modules" ] && { echo -e "${YELLOW}📦 의존성 설치 중...${NC}"; $PKG_MGR install; }

# TypeScript 타입 체크
echo -e "${YELLOW}🔎 TypeScript 타입 검사...${NC}"
npx tsc --noEmit
echo -e "${GREEN}✅ 타입 검사 통과${NC}"

# 프론트엔드 테스트
echo -e "${YELLOW}🧪 프론트엔드 테스트...${NC}"
$PKG_MGR run test
echo -e "${GREEN}✅ 프론트엔드 테스트 통과${NC}"

# Rust 컴파일 + 테스트
echo -e "${YELLOW}🦀 Rust 컴파일 검사...${NC}"
cargo check --manifest-path src-tauri/Cargo.toml
echo -e "${GREEN}✅ Rust 컴파일 검사 통과${NC}"

echo -e "${YELLOW}🧪 Rust 테스트...${NC}"
cargo test --manifest-path src-tauri/Cargo.toml
echo -e "${GREEN}✅ Rust 테스트 통과${NC}"

# Release 빌드 (번들 없이)
echo -e "${YELLOW}📦 Release 빌드 중... (시간이 걸립니다)${NC}"
$PKG_MGR run tauri build -- --no-bundle

# 결과 확인
BINARY="src-tauri/target/release/claude-pipeline-wizard"
if [ -f "$BINARY" ]; then
  echo -e "${GREEN}✅ 바이너리 생성: ${BINARY}${NC}"
else
  echo -e "${RED}❌ 바이너리를 찾을 수 없습니다${NC}"
  exit 1
fi
