# Build wa-bot-desktop.exe after wiring up the MSYS2 mingw64 toolchain via
# environment.ps1. Run from anywhere:
#   powershell -NoProfile -ExecutionPolicy Bypass -File desktop-gtk/scripts/build-gtk.ps1
#
# Optional:
#   -Output ..\some\dir\app.exe   destination binary (default ..\compiled\wa-bot-desktop.exe)
#   -Console                      keep the console subsystem (logs visible)
param(
    [string]$Output = "..\compiled\wa-bot-desktop.exe",
    [switch]$Console
)

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

. .\environment.ps1

$ldflags = "-s -w"
if (-not $Console) {
    $ldflags = "$ldflags -H windowsgui"
}

Write-Host "--- go vet ---"
go vet ./...
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "--- go build ---"
go build -trimpath -ldflags "$ldflags" -o $Output .
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "BUILD OK -> $Output"
