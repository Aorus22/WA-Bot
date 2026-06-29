@echo off
setlocal enabledelayedexpansion

echo ============================================
echo   WA Bot - Desktop Build Script (Windows)
echo ============================================
echo.

set "COMPILED_DIR=compiled"
set "BIN_DIR=%COMPILED_DIR%\bin"

:: Clean previous build (force close any running instances)
echo [1/5] Cleaning previous build...
taskkill /f /im "WA Bot.exe" >nul 2>&1
taskkill /f /im "wa-bot-backend.exe" >nul 2>&1
timeout /t 2 /nobreak >nul
if exist "%COMPILED_DIR%" rmdir /s /q "%COMPILED_DIR%"
if exist "desktop\dist" rmdir /s /q "desktop\dist"
mkdir "%BIN_DIR%"

:: ── Build Backend (Go) ──────────────────────
echo.
echo [2/5] Building backend (Go)...
cd /d "%~dp0"
go build -ldflags="-s -w" -o "wa-bot-backend.exe" ./cmd/api/main.go
if errorlevel 1 (
    echo ERROR: Backend build failed!
    exit /b 1
)
echo       Backend done: wa-bot-backend.exe

:: ── Build Frontend (Vite) ───────────────────
echo.
echo [3/5] Building frontend (Vite + React)...
cd /d "%~dp0web"
call npm run build
if errorlevel 1 (
    echo ERROR: Frontend build failed!
    cd /d "%~dp0"
    exit /b 1
)
cd /d "%~dp0"

echo       Copying frontend to %COMPILED_DIR%\fe...
mkdir "%COMPILED_DIR%\fe" 2>nul
xcopy /e /i /q /y "web\dist" "%COMPILED_DIR%\fe\dist"

:: Copy backend binary
copy /y "wa-bot-backend.exe" "%BIN_DIR%\" >nul
echo       Backend binary copied to %BIN_DIR%

:: ── Build Electron Desktop App ──────────────
echo.
echo [4/5] Building Electron desktop app...
cd /d "%~dp0desktop"
call npx electron-builder --win
if errorlevel 1 (
    echo ERROR: Electron build failed!
    cd /d "%~dp0"
    exit /b 1
)
cd /d "%~dp0"

:: ── Copy outputs ────────────────────────────
echo.
echo [5/5] Copying desktop builds to %COMPILED_DIR%...

:: Windows unpacked
if exist "desktop\dist\win-unpacked" (
    mkdir "%COMPILED_DIR%\electron-win" 2>nul
    xcopy /e /i /q /y "desktop\dist\win-unpacked" "%COMPILED_DIR%\electron-win"
    echo       Copied win-unpacked
)

:: Windows installer
if exist "desktop\dist\WA Bot Setup 1.0.0.exe" (
    copy /y "desktop\dist\WA Bot Setup 1.0.0.exe" "%COMPILED_DIR%\" >nul
    echo       Copied installer
)

:: Linux unpacked (if exists)
if exist "desktop\dist\linux-unpacked" (
    mkdir "%COMPILED_DIR%\electron-linux" 2>nul
    xcopy /e /i /q /y "desktop\dist\linux-unpacked" "%COMPILED_DIR%\electron-linux"
    echo       Copied linux-unpacked
)

:: ── Done ─────────────────────────────────────
echo.
echo ============================================
echo   All builds complete!
echo   Output in %COMPILED_DIR%:
echo ============================================
echo.
dir /b "%COMPILED_DIR%"
echo.
echo Run the app:    desktop\npx electron .
echo Or portable:    %COMPILED_DIR%\electron-win\WA Bot.exe
echo.

endlocal
