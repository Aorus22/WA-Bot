@echo off
setlocal enabledelayedexpansion
echo ============================================
echo   Starting WA Bot Desktop (GPUI)
echo ============================================
echo.

cd /d "%~dp0compiled\gpui-windows"

if not exist "database" mkdir "database"
if not exist "media" mkdir "media"

tasklist /fi "imagename eq wa-bot-backend.exe" 2>nul | findstr /i "wa-bot-backend.exe" >nul
if errorlevel 1 (
    echo [*] Menjalankan Go backend di background...
    start "WA-Bot-Backend" /min "wa-bot-backend.exe"
    ping 127.0.0.1 -n 3 >nul
) else (
    echo [*] Go backend sudah berjalan.
)

echo [*] Membuka WA Bot Desktop App...
start "" "wabot.exe"
echo.
echo Aplikasi telah diluncurkan!

