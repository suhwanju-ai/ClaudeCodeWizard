@echo off
chcp 65001 > nul
setlocal enabledelayedexpansion
title Claude Pipeline Wizard - Dev Server
color 0A

:: Always run from the project root, regardless of how this script was launched
:: (double-clicked in Explorer, or invoked from another directory) - otherwise
:: relative paths like src-tauri\Cargo.toml resolve against the wrong folder.
cd /d "%~dp0..\.."

echo.
echo  =============================================
echo   Claude Pipeline Wizard - Tauri Dev Server
echo  =============================================
echo.

:: Detect pnpm/yarn/npm
set "PKG_MGR=npm"
if exist "pnpm-lock.yaml" (
  where pnpm >nul 2>&1
  if not errorlevel 1 (
    set "PKG_MGR=pnpm"
  ) else (
    where npx >nul 2>&1
    if not errorlevel 1 ( set "PKG_MGR=npx pnpm" )
  )
)
if exist "yarn.lock" set "PKG_MGR=yarn"

:: Check Rust
where rustc >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Rust not found. Install from https://rustup.rs
  pause & exit /b 1
)
for /f "tokens=*" %%i in ('rustc --version') do echo [OK] %%i

:: Check Node.js
where node >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Node.js not found.
  pause & exit /b 1
)
for /f "tokens=*" %%i in ('node --version') do echo [OK] Node.js %%i

:: Check claude CLI (optional but recommended)
where claude >nul 2>&1
if errorlevel 1 (
  echo [WARN] claude CLI not found - pipeline execution will not work.
  echo        See https://claude.com/claude-code
) else (
  echo [OK] claude CLI found
)

:: Detect git (used below to self-heal a corrupted package.json/package-lock.json)
where git >nul 2>&1
set "HAS_GIT=1"
if errorlevel 1 set "HAS_GIT=0"

:: Install dependencies
if not exist "node_modules" (
  echo [INFO] Installing dependencies via %PKG_MGR%...
  call %PKG_MGR% install
  if errorlevel 1 (
    echo [WARN] Dependency install failed.
    if "%HAS_GIT%"=="1" (
      git diff --quiet -- package.json package-lock.json >nul 2>&1
      if errorlevel 1 (
        echo [WARN] package.json/package-lock.json differ from the last commit
        echo        ^(a known issue: something occasionally bumps vite/vitest to
        echo        versions that conflict with @vitejs/plugin-react^). Resetting
        echo        both files to the committed version and retrying once...
        call git checkout -- package.json package-lock.json
        call %PKG_MGR% install
        if errorlevel 1 ( echo [ERROR] Dependency install still failing after reset! & pause & exit /b 1 )
      ) else (
        echo [ERROR] Dependency install failed and package.json/package-lock.json
        echo         already match the last commit - this is not the known
        echo         version-drift issue. See the npm error above.
        pause & exit /b 1
      )
    ) else (
      echo [ERROR] Dependency install failed and git is not available to self-heal.
      pause & exit /b 1
    )
  )
)
echo [OK] Dependencies ready

:: Check for port conflict (Vite dev server: 1420)
netstat -ano | findstr ":1420" | findstr "LISTENING" >nul 2>&1
if not errorlevel 1 (
  echo [WARN] Port 1420 is already in use - a previous dev server may still be running.
)

echo.
echo [INFO] Starting Tauri dev mode...
call %PKG_MGR% run tauri dev
pause
