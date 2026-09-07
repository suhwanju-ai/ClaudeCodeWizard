@echo off
chcp 65001 > nul
setlocal enabledelayedexpansion
title Claude Pipeline Wizard - Build
color 0A

:: Always run from the repo root, regardless of the caller's cwd (e.g.
:: double-clicking this file from Explorer sets cwd to scripts\build\, which
:: silently breaks every relative path below -- npm/npx tools walk up to find
:: package.json/tsconfig.json so steps 1-3 mask the problem, but `cargo
:: --manifest-path` does not, so it's the first hard failure).
cd /d "%~dp0..\.."

echo.
echo  =============================================
echo   Claude Pipeline Wizard - Release Build
echo  =============================================
echo.

set "PKG_MGR=npm"
if exist "yarn.lock" set "PKG_MGR=yarn"
if exist "pnpm-lock.yaml" (
  where pnpm >nul 2>&1
  if not errorlevel 1 ( set "PKG_MGR=pnpm" ) else ( set "PKG_MGR=npx pnpm" )
)

echo [1/6] Checking dependencies...
if not exist "node_modules" (
  call %PKG_MGR% install
  if errorlevel 1 ( echo [ERROR] Dependency install failed! & pause & exit /b 1 )
)
echo        Dependencies OK

echo.
echo [2/6] TypeScript type check...
call npx tsc --noEmit
if errorlevel 1 ( echo [ERROR] Type check failed! & pause & exit /b 1 )
echo        Type check OK

echo.
echo [3/6] Frontend tests...
call %PKG_MGR% run test
if errorlevel 1 ( echo [ERROR] Frontend tests failed! & pause & exit /b 1 )
echo        Frontend tests OK

echo.
echo [4/6] Rust compile check...
call cargo check --manifest-path src-tauri\Cargo.toml
if errorlevel 1 ( echo [ERROR] Rust compile check failed! & pause & exit /b 1 )
echo        Rust compile OK

echo.
echo [5/6] Rust tests...
call cargo test --manifest-path src-tauri\Cargo.toml
if errorlevel 1 ( echo [ERROR] Rust tests failed! & pause & exit /b 1 )
echo        Rust tests OK

echo.
echo [6/6] Release build (no bundle)...
call %PKG_MGR% run tauri build -- --no-bundle
if errorlevel 1 ( echo [ERROR] Release build failed! & pause & exit /b 1 )

echo.
echo [OK] Build complete!
dir /b "src-tauri\target\release\claude-pipeline-wizard.exe" 2>nul
pause
