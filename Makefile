# WA Bot — perintah build (Windows dan Linux)
# Jalankan dari root repo:
#   make install      pasang dependensi FE + Go backend dan Go desktop-gtk
#   make install-electron pasang dependency Electron ke desktop/node_modules
#   make be           build backend -> wa-bot-backend[.exe] di root
#   make dist-fe      build frontend -> web/dist lalu disalin ke frontend/dist
#   make desktop-gtk  build aplikasi desktop GTK -> compiled/wa-bot-desktop[.exe]
#   make desktop-electron build aplikasi Electron -> compiled/electron
#
# Catatan:
#   - Backend memakai mattn/go-sqlite3 sehingga butuh gcc/CGO di semua OS.
#   - Linux: pasang dulu libgtk-4-dev + libadwaita-1-dev untuk target desktop-gtk.
#   - Windows: target desktop-gtk memakai MSYS2 via desktop-gtk/environment.ps1.

.DEFAULT_GOAL := help

ifeq ($(OS),Windows_NT)
    HOST_OS := windows
else
    UNAME_S := $(shell uname -s 2>/dev/null)
    ifeq ($(UNAME_S),Darwin)
        HOST_OS := darwin
    else
        HOST_OS := linux
    endif
endif

EXE_EXT :=
COPY_FRONTEND :=
ELECTRON_BUILD_SCRIPT := build:linux
NPM_INSTALL_WEB = cd web && npm install
NPM_INSTALL_DESKTOP = cd desktop && npm install
NPM_BUILD_WEB = cd web && npm run build
NPM_BUILD_ELECTRON = cd desktop && npm run $(ELECTRON_BUILD_SCRIPT) -- --config.directories.output=../compiled/electron
ifeq ($(HOST_OS),windows)
    SHELL := cmd.exe
    .SHELLFLAGS := /D /C
    EXE_EXT := .exe
    COPY_FRONTEND = powershell -NoProfile -ExecutionPolicy Bypass -Command "New-Item -ItemType Directory -Force -Path 'frontend' | Out-Null; if (Test-Path 'frontend/dist') { Remove-Item -Recurse -Force 'frontend/dist' }; Copy-Item -Recurse 'web/dist' 'frontend/dist'"
    ELECTRON_BUILD_SCRIPT := build:win
    NPM_INSTALL_WEB = cd /D web && npm install
    NPM_INSTALL_DESKTOP = cd /D desktop && npm install
    NPM_BUILD_WEB = cd /D web && npm run build
    NPM_BUILD_ELECTRON = cd /D desktop && npm run build:win -- --config.directories.output=../compiled/electron
else ifeq ($(HOST_OS),darwin)
    ELECTRON_BUILD_SCRIPT := build:mac
else
    COPY_FRONTEND = mkdir -p frontend && rm -rf frontend/dist && cp -r web/dist frontend/dist
endif

.PHONY: help install install-electron be dist-fe desktop-gtk desktop-electron

help:
	@echo Target yang tersedia:
	@echo   make install      - install dependency FE, Electron, backend, dan desktop-gtk
	@echo   make install-electron - install dependency ke desktop/node_modules
	@echo   make be           - build backend menjadi wa-bot-backend$(EXE_EXT)
	@echo   make dist-fe      - build frontend, hasil di web/dist dan frontend/dist
	@echo   make desktop-gtk  - build aplikasi desktop GTK ke folder compiled
	@echo   make desktop-electron - build aplikasi Electron ke compiled/electron
	@echo   Flag Electron eksternal BE: --no-backend --port 3090, default-port=8080

install:
	$(NPM_INSTALL_WEB)
	$(MAKE) install-electron
	go mod download
	cd desktop-gtk && go mod download

install-electron:
	$(NPM_INSTALL_DESKTOP)

be:
	go build -trimpath -ldflags "-s -w" -o wa-bot-backend$(EXE_EXT) ./cmd/api
	@echo Hasil: wa-bot-backend$(EXE_EXT)

dist-fe:
	$(NPM_BUILD_WEB)
	$(COPY_FRONTEND)
	@echo Hasil FE siap: web/dist dan frontend/dist

desktop-electron: be dist-fe
	$(NPM_BUILD_ELECTRON)
	@echo Hasil Electron siap: compiled/electron

ifeq ($(HOST_OS),windows)
desktop-gtk:
	powershell -NoProfile -ExecutionPolicy Bypass -File "desktop-gtk/scripts/build-gtk.ps1"
else
desktop-gtk:
	mkdir -p compiled
	cd desktop-gtk && go vet ./...
	cd desktop-gtk && go build -trimpath -ldflags "-s -w" -o ../compiled/wa-bot-desktop .
endif
