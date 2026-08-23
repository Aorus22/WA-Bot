; NSIS installer script for WA-Bot Desktop (Windows)
;
; Build with: makensis installer.nsi  (run inside the MSYS2 mingw64 shell)
; Or:        makensis /DVERSION=1.1.0 installer.nsi
;
; Bundles the wa-bot-desktop.exe binary + GTK4/libadwaita runtime DLLs +
; wa-bot-backend.exe into a self-contained NSIS installer.

!define APPNAME "WA Bot"
!define EXENAME "wa-bot-desktop.exe"
!define BACKENDEXE "wa-bot-backend.exe"

!ifndef VERSION
  !define VERSION "1.1.0"
!endif

Name "${APPNAME} ${VERSION}"
OutFile "wa-bot-desktop-setup-${VERSION}.exe"
InstallDir "$PROGRAMFILES64\WA-Bot"
RequestExecutionLevel admin
ShowInstDetails show
Compression lzma
SolidCompression

; ─── MUI ───
!include "MUI2.nsh"
!define MUI_ABORTWARNING
!define MUI_ICON "..\..\assets\icon.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "..\..\assets\installer-header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "..\..\assets\installer-sidebar.bmp"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

; ─── Installer ───
Section "WA Bot Desktop (required)"
  SectionIn RO

  SetOutPath "$INSTDIR"

  ; Desktop binary
  File "..\..\..\compiled\wa-bot-desktop.exe"

  ; Backend binary (same name as desktop-gtk/internal/backend/manager.go expects)
  File "..\..\..\compiled\wa-bot-backend.exe"

  ; Bundled GTK4 / libadwaita runtime DLLs (from MSYS2 mingw64/bin)
  ; This list is the set required at runtime; update via `pacman -Ql` if needed.
  File "C:\msys64\mingw64\bin\libatk-1.0-0.dll"
  File "C:\msys64\mingw64\bin\libatk-bridge-2.0-0.dll"
  File "C:\msys64\mingw64\bin\libcairo-2.dll"
  File "C:\msys64\mingw64\bin\libcairo-gobject-2.dll"
  File "C:\msys64\mingw64\bin\libdatrie-1.dll"
  File "C:\msys64\mingw64\bin\libepoxy-0.dll"
  File "C:\msys64\mingw64\bin\libfontconfig-1.dll"
  File "C:\msys64\mingw64\bin\libfreetype-6.dll"
  File "C:\msys64\mingw64\bin\libfribidi-0.dll"
  File "C:\msys64\mingw64\bin\libgdk_pixbuf-2.0-0.dll"
  File "C:\msys64\mingw64\bin\libgdk-4-1.dll"
  File "C:\msys64\mingw64\bin\libgio-2.0-0.dll"
  File "C:\msys64\mingw64\bin\libglib-2.0-0.dll"
  File "C:\msys64\mingw64\bin\libgmodule-2.0-0.dll"
  File "C:\msys64\mingw64\bin\libgobject-2.0-0.dll"
  File "C:\msys64\mingw64\bin\libgraphene-1.0-0.dll"
  File "C:\msys64\mingw64\bin\libgtk-4-1.dll"
  File "C:\msys64\mingw64\bin\libharfbuzz-0.dll"
  File "C:\msys64\mingw64\bin\libintl-8.dll"
  File "C:\msys64\mingw64\bin\libpango-1.0-0.dll"
  File "C:\msys64\mingw64\bin\libpangocairo-1.0-0.dll"
  File "C:\msys64\mingw64\bin\libpangoft2-1.0-0.dll"
  File "C:\msys64\mingw64\bin\libpangowin32-1.0-0.dll"
  File "C:\msys64\mingw64\bin\libpixman-1-0.dll"
  File "C:\msys64\mingw64\bin\libpng16-16.dll"
  File "C:\msys64\mingw64\bin\libthai-0.dll"
  File "C:\msys64\mingw64\bin\libwinpthread-1.dll"
  File "C:\msys64\mingw64\bin\libxml2-2.dll"
  File "C:\msys64\mingw64\bin\libadwaita-1-0.dll"
  File "C:\msys64\mingw64\bin\libgcc_s_seh-1.dll"
  File "C:\msys64\mingw64\bin\libstdc++-6.dll"
  File "C:\msys64\mingw64\bin\libffi-8.dll"
  File "C:\msys64\mingw64\bin\libbrotlicommon.dll"
  File "C:\msys64\mingw64\bin\libbrotlidec.dll"
  File "C:\msys64\mingw64\bin\libbrotlienc.dll"
  File "C:\msys64\mingw64\bin\libgraphite2.dll"
  File "C:\msys64\mingw64\bin\zlib1.dll"

  ; Write uninstaller
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Start menu shortcuts
  CreateDirectory "$SMPROGRAMS64\WA-Bot"
  CreateShortCut "$SMPROGRAMS64\WA-Bot\WA Bot.lnk" "$INSTDIR\${EXENAME}" "" "$INSTDIR\${EXENAME}" 0
  CreateShortCut "$SMPROGRAMS64\WA-Bot\Uninstall.lnk" "$INSTDIR\Uninstall.exe"

  ; Desktop shortcut
  CreateShortCut "$DESKTOP\WA Bot.lnk" "$INSTDIR\${EXENAME}" "" "$INSTDIR\${EXENAME}" 0

  ; Registry: AddUninstallEntry
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "Publisher" "WA-Bot"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot" "NoRepair" 1
SectionEnd

; ─── Uninstaller ───
Section "Uninstall"
  RMDir /r "$INSTDIR"
  RMDir /r "$SMPROGRAMS64\WA-Bot"
  Delete "$DESKTOP\WA Bot.lnk"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WABot"
SectionEnd
