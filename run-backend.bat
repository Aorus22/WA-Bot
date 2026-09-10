@echo off
setlocal enabledelayedexpansion
echo ============================================
echo   Starting WA Bot Backend Server
echo ============================================
echo.

cd /d "%~dp0compiled\gpui-windows"

if not exist "database" mkdir "database"
if not exist "media" mkdir "media"

echo [*] Menjalankan Go backend di port 3000...
echo [*] QR code akan muncul di terminal ini atau buka http://localhost:3000 di browser.
echo.
wa-bot-backend.exe
