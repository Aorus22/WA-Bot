# desktop-gtk/environment.ps1
# Source this file in PowerShell before running `go build` for desktop-gtk:
#   . .\desktop-gtk\environment.ps1
#
# This adds MSYS2 mingw64 toolchain (gcc, pkg-config, GTK4, libadwaita) to PATH
# and sets the env vars Go needs to use it via CGO.

$msysRoot = "C:\msys64"
$mingwBin = "$msysRoot\mingw64\bin"

if (-not (Test-Path "$mingwBin\pkg-config.exe")) {
    Write-Error "MSYS2 mingw64 not found at $mingwBin. Install via: winget install -e --id MSYS2.MSYS2"
    return
}

$env:Path = "$mingwBin;$env:Path"
$env:PKG_CONFIG = "$mingwBin\pkg-config.exe"
$env:CGO_ENABLED = "1"

Write-Host "desktop-gtk environment ready:"
Write-Host "  gcc: $((Get-Command gcc -ErrorAction SilentlyContinue).Source)"
Write-Host "  pkg-config: $((Get-Command pkg-config -ErrorAction SilentlyContinue).Source)"
Write-Host "  GTK4: $(& pkg-config --modversion gtk4)"
Write-Host "  libadwaita: $(& pkg-config --modversion libadwaita-1)"
Write-Host "  CGO_ENABLED: $env:CGO_ENABLED"
