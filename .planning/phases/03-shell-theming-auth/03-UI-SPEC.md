# Phase 3: Shell + Theming + Auth — UI Design Contract (UI-SPEC)

**Created:** 2026-09-09
**Phase:** 3 — Shell + Theming + Auth
**Status:** Approved (1:1 web client parity specification)

---

## 1. Design System & Theme Tokens

### 1.1 Color Tokens (17 Semantic CSS Variables)
Aplikasi desktop GPUI mengadopsi 17 token semantic warna yang 100% identik dengan web client (`web/src/data/themes.ts` dan `web/src/components/AppThemeProvider.tsx`):

| Token Name | Deskripsi & Kegunaan | Contoh Nilai (Default Dark) |
|---|---|---|
| `background` | Warna dasar kanvas jendela dan container utama | `#0F172A` |
| `foreground` | Warna teks primer pada background | `#FFFFFF` |
| `card` | Background kartu, popover, dan panel | `#1E293B` |
| `card_foreground` | Teks di atas elemen card | `#FFFFFF` |
| `primary` | Aksen warna brand (tombol utama, active nav bar) | `#0E7490` |
| `primary_foreground`| Teks kontras di atas tombol primary | `#FFFFFF` |
| `secondary` | Background elemen sekunder / titlebar | `#1E293B` |
| `secondary_foreground` | Teks di atas elemen sekunder | `#FFFFFF` |
| `muted` | Background elemen subtle/hover non-aktif | `#21293c` |
| `muted_foreground` | Teks redup (sub-label, timestamp, placeholder) | `#94A3B8` |
| `accent` | Highlight aktif untuk item terpilih / badge | `#155E75` |
| `accent_foreground` | Teks kontras di atas aksen | `#FFFFFF` |
| `destructive` | Indikator error, bahaya, dan tombol logout/delete | `#7F1D1D` |
| `destructive_foreground` | Teks kontras di atas tombol destructive | `#FFFFFF` |
| `border` | Garis batas panel, separator, dan input field | `#334155` |
| `input` | Border atau background kontrol input formulir | `#334155` |
| `ring` | Outline focus ring untuk aksesibilitas keyboard | `#0E7490` |

### 1.2 Preset Catalog
- Katalog tema memuat seluruh 81 preset bawaan dari `web/src/data/themes.ts` (misal: `default-dark`, `default-light`, `ocean-dark`, `ocean-light`, `forest-dark`, `forest-light`, `lavender-dark`, `lavender-light`, `sunset-dark`, dsb.).
- Default preset: `default-dark`.
- Mode tema: `System`, `Light`, `Dark` (dikombinasikan dengan deteksi luminance `< 0.5`).
- Pergantian tema bersifat *hot-swappable* secara instan di GPUI tanpa memerlukan restart biner.

---

## 2. Layout & Shell Geometry

### 2.1 Window Anatomy
```
+-------------------------------------------------------------------------+
| [WA Bot]                              [-] [口] [X] (TitleBar: h=48px)   |
+----+--------------------------------------------------------------------+
| S  | [WS Connection Banner - Reconnecting...] (if offline)              |
| I  +--------------------------------------------------------------------+
| D  |                                                                    |
| E  |                                                                    |
| B  |                       Router Content Outlet                        |
| A  |                        (Login / Chat / etc)                        |
| R  |                                                                    |
|    |                                                                    |
|    |                                                                    |
|68px|                                                                    |
+----+--------------------------------------------------------------------+
```

- **Titlebar**: Tinggi tetap `48px` (`h-12`). Area drag window di Linux dan Windows. Sisi kanan memuat tombol jendela native/custom (`Minimize`, `Maximize/Restore`, `Close`) dengan ukuran hit target `44px x 36px`.
- **Navigation Sidebar**: Lebar tetap `68px` (`w-[68px]`). Letak di sisi kiri layar penuh, di bawah titlebar. Padding vertikal `py-6`.
- **Main Viewport**: Mengisi sisa area (`calc(100vh - 48px)`), `flex-1 flex overflow-hidden`.
- **Toast Layer**: Mengambang di atas konten (`top-center`, `z-50`).

---

## 3. Navigation Sidebar Specifications

Daftar item navigasi vertikal (`NavigationSidebar` 1:1 web):

| Order | Route | Icon (Lucide/GPUI) | Label Tooltip | Posisi |
|---|---|---|---|---|
| 1 | `/chat` | `MessageSquare` | Chats | Atas |
| 2 | `/status` | `CircleDashed` | Status | Atas |
| 3 | `/channels` | `Megaphone` | Channels | Atas |
| 4 | `/calls` | `Phone` | Calls | Atas |
| 5 | `/triggers` | `Bot` | Triggers | Atas |
| 6 | `/cron` | `Clock` | Cron Jobs | Atas |
| 7 | `/webhooks` | `Globe` | Webhooks | Atas |
| 8 | `/documentation` | `FileText` | Documentation | Atas |
| 9 | `/settings` | `Settings` | Settings | Bawah (`mt-auto`) |

### Interaksi NavButton:
- Ukuran tombol: `p-3.5 rounded-2xl`.
- State Aktif: Background `bg-primary`, icon `text-primary-foreground`, shadow `shadow-primary/20`, indikator garis aktif di tepi kiri: `w-1 h-6 bg-primary rounded-r-full`.
- State Normal: `text-muted-foreground`, hover `hover:bg-muted hover:text-foreground`.
- Tooltip: Muncul di sebelah kanan tombol (`side="right" sideOffset={10}`).

---

## 4. Auth Gate & Views Specification

### 4.1 Gate Lifecycle
- **State 1: `Checking`**: Menampilkan loader spinner terpusat (`Loader2 animate-spin`, `h-10 w-10 text-primary`) dengan background `bg-background`.
- **State 2: `Unauthenticated`**: Menampilkan `LoginPage`.
- **State 3: `Authenticated`**: Membuka `AppLayout` dengan rute aktif default `/chat`.

### 4.2 Login View Anatomy
- Card utama terpusat, lebar maksimal `max-w-lg` (`~512px`).
- **Header**: Icon badge `QrCode` (`w-14 h-14 rounded-2xl bg-primary/10 text-primary`), judul "Link your device", subjudul "Scan the QR code with your WhatsApp to connect".
- **Tabs Switcher**: Bersebelahan ("QR Code" vs "Phone"). Tinggi ~36px, `p-1 rounded-xl bg-muted`. Tab aktif mendapat `bg-background shadow-sm`.
- **Tab 1: QR Code**:
  - Container QR: Kotak terpusat `min-w-[280px] min-h-[280px] bg-muted/50 rounded-2xl border border-border/40`.
  - State Loading: Spinner `Loader2` + "Loading QR code...".
  - State Error: Teks error + tombol outline rounded "Retry".
  - State QR Ready: Gambar QR hitam-putih kontras tinggi (`240x240px` dengan padding `12px` rounded-xl), level error correction 'M'.
  - Tombol Refresh QR: Tombol outline rounded dengan icon `RefreshCw`, auto-refresh setiap 30 detik.
  - Teks Instruksi: 3 langkah bernomor (1: Open WhatsApp on your phone, 2: Tap Menu/Settings > Linked Devices, 3: Point your phone to scan).
- **Tab 2: Phone Link**:
  - Icon `Smartphone`, judul "Link with phone number", teks informasi "This feature is coming soon to the bot interface.", tombol "Back to QR code".
- **Footer**: Badge "End-to-end encrypted" dengan icon `ShieldCheck`.

### 4.3 Logout Confirmation Dialog
- Dipicu dari klik Logout (di sidebar atau settings).
- Modal alert dialog dengan overlay backdrop `bg-black/50`.
- Judul: "Log out of WhatsApp?"
- Deskripsi: "You will need to scan the QR code again to reconnect."
- Aksi: Tombol "Cancel" (`variant=outline`) dan tombol "Log Out" (`variant=destructive`).

---

## 5. Feedback & Connection Indicator

### 5.1 Connection Status Banner
- Ketika WebSocket dalam kondisi disconnect atau reconnecting:
  - Muncul banner di bagian atas main area: `h-9 bg-warning/15 text-warning border-b border-warning/30 flex items-center justify-center gap-2 text-xs font-medium`.
  - Teks: "Connecting to WhatsApp... Attempting to reconnect".
  - Indikator icon spinner kecil.
- Ketika terhubung (`Connected`): Banner menghilang mulus.

### 5.2 Toast System
- Toast muncul di posisi `top-center` dengan auto-dismiss ~4000ms.
- Tipe toast: `Info`, `Success`, `Warning`, `Error`.
- Setiap aksi mutasi (misal: logout, refresh QR, perubahan tema) menghasilkan toast yang jelas.

---

## 6. Verification Criteria
1. Seluruh 81 preset tema dapat di-render tanpa crash.
2. Pergantian tema langsung merubah warna seluruh elemen UI yang aktif.
3. Transisi login -> spinner -> shell -> logout dialog -> login berjalan mulus.
4. Layout frameless window dan sidebar responsif terhadap perubahan ukuran jendela.
