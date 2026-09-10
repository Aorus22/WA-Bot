# Build release package for WA Bot Desktop GPUI on Windows (PowerShell).
#
# Requirements:
#   - Rust stable (MSVC or GNU toolchain)
#   - Go 1.22+
#   - Optional: WiX Toolset + cargo-wix for .msi installer

$ErrorActionPreference = "Stop"

$RootDir = (Get-Item "$PSScriptRoot\..\..").FullName
Set-Location $RootDir

$OutputDir = "$RootDir\compiled\gpui-windows"
New-Item -ItemType Directory -Force -Path "$OutputDir\be" | Out-Null
New-Item -ItemType Directory -Force -Path "$OutputDir\database" | Out-Null
New-Item -ItemType Directory -Force -Path "$OutputDir\media" | Out-Null

Write-Host "==> [1/3] Building Go backend (Windows release)..." -ForegroundColor Cyan
go build -trimpath -ldflags "-s -w" -o "$RootDir\wa-bot-backend.exe" ./cmd/api
Copy-Item -Force "$RootDir\wa-bot-backend.exe" "$OutputDir\be\wa-bot-backend.exe"
Copy-Item -Force "$RootDir\wa-bot-backend.exe" "$OutputDir\wa-bot-backend.exe"

# Ensure shader compiler helper is available for GPUI release build
$FxcTool = "$RootDir\desktop-gpui\tools\fxc\fxc.exe"
if ([string]::IsNullOrEmpty($env:GPUI_FXC_PATH) -or -not (Test-Path $env:GPUI_FXC_PATH -ErrorAction SilentlyContinue)) {
    if (-not (Test-Path $FxcTool)) {
        Write-Host "Compiling standalone FXC shader compiler helper..." -ForegroundColor Yellow
        rustc -O "$RootDir\desktop-gpui\tools\fxc\main.rs" -o $FxcTool
    }
    $env:GPUI_FXC_PATH = $FxcTool
}

Write-Host "==> [2/3] Building GPUI desktop shell (Windows release)..." -ForegroundColor Cyan
cargo build --release --locked --manifest-path "$RootDir\desktop-gpui\Cargo.toml" -p wabot
Copy-Item -Force "$RootDir\desktop-gpui\target\release\wabot.exe" "$OutputDir\wabot.exe"

Write-Host "==> [3/3] Checking optional packaging (cargo-wix)..." -ForegroundColor Cyan
if (Get-Command cargo-wix -ErrorAction SilentlyContinue) {
    Write-Host "cargo-wix found. Building MSI installer..." -ForegroundColor Green
    Set-Location "$RootDir\desktop-gpui"
    cargo wix -p wabot --no-build
    Set-Location $RootDir
    if (Test-Path "$RootDir\desktop-gpui\target\wix\*.msi") {
        Copy-Item -Force "$RootDir\desktop-gpui\target\wix\*.msi" "$OutputDir\"
        Write-Host "MSI installer copied to $OutputDir" -ForegroundColor Green
    }
} else {
    Write-Host "cargo-wix not installed (optional). Skipping .msi creation." -ForegroundColor Yellow
    Write-Host "Install via: cargo install cargo-wix"
}

Write-Host ""
Write-Host "=== GPUI Windows Release Build Complete ===" -ForegroundColor Green
Write-Host "Output Directory: $OutputDir"
Write-Host "  - $OutputDir\wabot.exe"
Write-Host "  - $OutputDir\wa-bot-backend.exe"
Write-Host "  - $OutputDir\be\wa-bot-backend.exe"
Write-Host "To run on Windows:"
Write-Host "  cd `"$OutputDir`"; .\wabot.exe"
