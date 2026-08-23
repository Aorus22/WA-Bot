# desktop-gtk — Native GTK4 frontend for WA-Bot

A native Go/GTK4 desktop app for the WA-Bot platform. Spawns the existing
`wa-bot-backend` as a subprocess (or attaches to an already-running one) and
provides a libadwaita WhatsApp-style chat UI.

> **v2 "Chat Inti" milestone.** The web frontend (`web/`) and the Electron
> wrapper (`desktop/`) continue to work in parallel. This is a *third*
> deployment surface, not a replacement of `web/`.

## Features

- Native GTK4 + libadwaita window (no Electron, no WebView, no JS runtime)
- Auto-spawns `wa-bot-backend`; discovers its bound port from stdout
- **Login/pairing screen**: live QR from the backend's `qr_code` WebSocket
  events, auto-jumps to chats on `auth_success`, logout button
- **Live chat list**: avatars (async + disk cache), last-message preview,
  unread badges, search filter, real-time reordering via `/ws`
- **Conversation view**: two-column WhatsApp-style layout
  - Bubbles for text, image, sticker, video, audio/PTT, document
  - Reply quotes, sender names in groups, date separators
  - Bidirectional pagination with scroll-position preservation
  - Optimistic sending with pending ✓ / sent ✓ / delivered ✓✓ / read ✓✓ /
    failed ⚠ status ticks
  - Typing presence pings while composing
  - Attachment menu: photo / video / document upload
- Media downloads cached under `<user-data>/cache`; images render inline,
  audio/video play inline when a GStreamer runtime is present and fall back
  to the system player otherwise
- Keyboard shortcuts: Ctrl+Q (quit), Ctrl+1 (dashboard), Ctrl+2 (chats),
  Ctrl+, (settings)

## Build

### Prerequisites

- **Go 1.25**
- **CGO** enabled
- **GTK 4.14+** development files
- **libadwaita 1.5+** development files
- **gobject-introspection** development files
- **pkg-config** and a C compiler

### Linux

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgirepository1.0-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel gobject-introspection-devel

# Build
./scripts/build-linux.sh
```

Output: `../compiled/wa-bot-desktop`

### Windows (MSYS2)

```sh
# Inside C:\msys64\mingw64.exe
pacman -S --needed mingw-w64-x86_64-toolchain mingw-w64-x86_64-pkg-config \
    mingw-w64-x86_64-gtk4 mingw-w64-x86_64-libadwaita \
    mingw-w64-x86_64-gobject-introspection

./scripts/build-windows.sh
```

Output: `..\compiled\wa-bot-desktop.exe` (built with `-H windowsgui`,
so no console window).

For a self-contained folder (exe + GTK DLLs + pixbuf loaders + optional
GStreamer runtime + backend):

```sh
# Build the backend first, then:
packaging/windows/make-portable.sh
```

#### Runtime on Windows (dev)

While developing you don't need to bundle anything — just make sure
`C:\msys64\mingw64\bin` is on PATH before launching the exe.

## Run

```sh
# Dev: spawns wa-bot-backend from next to the exe or ../
./wa-bot-desktop

# With custom backend path
./wa-bot-desktop -backend-path /opt/wa-bot/wa-bot-backend

# Without spawning a backend (attach to an externally-managed instance)
./wa-bot-desktop -no-backend -port 3090
```

User data lives at:

- **Linux:** `~/.local/share/wa-bot-desktop/`
- **macOS:** `~/Library/Application Support/wa-bot-desktop/`
- **Windows:** `%APPDATA%\wa-bot-desktop\` (media cache in `cache/`)

## Development

```sh
# Windows PowerShell: add MSYS2 mingw64 toolchain to PATH first
. .\environment.ps1

go build ./...
go build -o wa-bot-desktop-test.exe .   # console subsystem, logs visible
./wa-bot-desktop-test.exe -no-backend -port 3090
```

### Architecture

```
desktop-gtk/
├── main.go                          # entrypoint + flags
├── internal/
│   ├── api/                         # REST client (stdlib net/http)
│   │   ├── client.go                #   JSON envelope + multipart media
│   │   ├── chats.go                 #   chats/messages/read/typing/send
│   │   └── session.go               #   qr-code/logout/media/avatar URLs
│   ├── ws/ws.go                     # /ws client: reconnect + event bus
│   ├── store/store.go               # in-memory chat/message state + events
│   ├── media/
│   │   ├── cache.go                 # disk cache + GdkTexture helpers
│   │   └── probe.go                 # GStreamer runtime detection
│   ├── openx/openx.go               # OS default app launcher
│   ├── backend/                     # subprocess manager (unchanged v1.1)
│   ├── ui/
│   │   ├── app.go                   # lifecycle, WS wiring, auth gate
│   │   ├── window.go                # Adw shell + login overlay
│   │   ├── css.go                   # bubble/tick/badge stylesheet
│   │   └── dispatcher.go            # glib.IdleAdd wrapper
│   └── views/
│       ├── login.go                 # QR pairing screen
│       ├── chats_pane.go            # list + conversation split + toasts
│       ├── chat_list.go             # live sidebar
│       ├── avatar.go                # AdwAvatar async image loading
│       ├── conversation.go          # pane state, pagination, scroll sync
│       ├── conversation_rows.go     # bubbles per message type
│       └── conversation_composer.go # input, typing pings, attachments
└── scripts/                         # per-OS build scripts
```

Data flow: `ws.Client` → handlers on the read goroutine mutate `store.Store`
(mutex-guarded) → store emits change events → views hop to the GTK main
thread via `glib.IdleAdd` and reconcile their widgets. The store mirrors the
behaviour of the web UI's zustand `chatStore.ts` (temp-ID optimistic sends,
bidirectional cursor pages keyed on unix-milli timestamps).

### Why a separate Go module?

`desktop-gtk` is a *consumer* of the backend binary, not its code. Keeping
the two as separate modules means:

- The desktop binary doesn't pull in `whatsmeow`, `gorilla/mux`, etc.
- `gotk4` upgrades don't force a backend rebuild.
- Build caching is more effective (no `internal/` rebuild on UI changes).

## Status

- ✅ Backend subprocess spawn + port discovery (v1.1)
- ✅ Login/pairing screen with live QR
- ✅ Real-time chat list with avatars & unread badges
- ✅ Conversation view: all bubble types, pagination, optimistic send,
      typing presence, delivery ticks, attachments
- ✅ Media disk cache; inline playback behind GStreamer probe
- ✅ Windows portable bundling script; Linux/macOS build scripts
- ⏳ Post-v1 ideas: reply/edit/delete/react UI, media/docs/links tabs,
      bot automation pages (triggers/cron/webhooks), calls overlay
