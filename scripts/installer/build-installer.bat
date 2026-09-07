@echo off
chcp 65001 > nul
setlocal enabledelayedexpansion
title Claude Pipeline Wizard - Windows Installer Build
color 0E

:: Always run from the repo root, regardless of the caller's cwd (e.g.
:: double-clicking this file from Explorer sets cwd to scripts\installer\).
cd /d "%~dp0..\.."

echo.
echo  =================================================
echo   Claude Pipeline Wizard - Windows Installer Build
echo   Targets: MSI + NSIS (.exe)
echo  =================================================
echo.

:: Detect pnpm (direct or via corepack/npx), fallback to npm
set "PKG_MGR=npm"
if exist "yarn.lock" set "PKG_MGR=yarn"
if exist "pnpm-lock.yaml" (
  where pnpm >nul 2>&1
  if not errorlevel 1 (
    set "PKG_MGR=pnpm"
  ) else (
    where npx >nul 2>&1
    if not errorlevel 1 ( set "PKG_MGR=npx pnpm" )
  )
)

:: -- Prerequisites --------------------------------
echo [1/6] Checking prerequisites...

where rustc >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Rust not found!
  echo         Install from https://rustup.rs and retry.
  pause & exit /b 1
)
for /f "tokens=*" %%i in ('rustc --version') do echo        %%i

where node >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Node.js not found!
  pause & exit /b 1
)
for /f "tokens=*" %%i in ('node --version') do echo        Node.js %%i

:: claude CLI (optional but strongly recommended)
where claude >nul 2>&1
if errorlevel 1 (
  echo [WARN]  claude CLI not found - end users will need it installed too.
  echo         See https://claude.com/claude-code
) else (
  echo        claude CLI found
)

:: Visual Studio Build Tools check
where cl >nul 2>&1
if errorlevel 1 (
  echo [WARN]  Visual Studio Build Tools may be missing.
  echo         https://visualstudio.microsoft.com/visual-cpp-build-tools/
)

:: WebView2 runtime check
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" >nul 2>&1
if errorlevel 1 (
  echo [INFO]  WebView2 runtime not found - will be bundled in the installer.
) else (
  echo        WebView2 runtime detected
)

echo.
:: -- npm dependencies ------------------------------
echo [2/6] Checking dependencies (%PKG_MGR%)...
if not exist "node_modules" (
  echo        node_modules missing - installing...
  call %PKG_MGR% install
  if errorlevel 1 ( echo [ERROR] Dependency install failed! & pause & exit /b 1 )
)
echo        Dependencies OK

:: -- Rust compile check ----------------------------
echo.
echo [3/6] Rust compile check...
call cargo check --manifest-path src-tauri\Cargo.toml
if errorlevel 1 (
  echo [ERROR] Rust compile error!
  pause & exit /b 1
)
echo        Rust compile OK

:: -- Bump version (patch += 1) so every build produces a distinct installer --
echo.
echo [4/6] Bumping version (tauri.conf.json / package.json / Cargo.toml)...
call node scripts\installer\bump-version.cjs
if errorlevel 1 ( echo [ERROR] Version bump failed! & pause & exit /b 1 )

:: -- Build installer --------------------------------
echo.
echo [5/6] Building Windows installer... (5-15 min)
echo       Targets: MSI + NSIS
echo.

call %PKG_MGR% run tauri build -- --bundles msi,nsis
if errorlevel 1 (
  echo.
  echo [ERROR] Build failed!
  echo [HINT]  Check the following:
  echo         1. src-tauri/tauri.conf.json "identifier" setting
  echo         2. src-tauri/icons/icon.ico exists
  echo         3. Rust and Node.js versions
  pause & exit /b 1
)

:: -- Show results -----------------------------------
echo.
echo [6/6] Checking build output...
echo.
echo  =================================================
echo   Installer build succeeded!
echo  =================================================
echo.
echo  NSIS installer (.exe):
dir /b "src-tauri\target\release\bundle\nsis\*.exe" 2>nul
echo.
echo  MSI installer (.msi):
dir /b "src-tauri\target\release\bundle\msi\*.msi" 2>nul
echo.
echo  Pick NSIS (.exe) for general distribution, MSI for enterprise/GPO deployment.
echo.
pause
