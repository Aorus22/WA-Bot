# Phase 5: Media + Rich Types + Group Admin - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning
**Mode:** Autonomous (1:1 Web Parity)

<domain>
## Phase Boundary

User bisa berkirim semua tipe pesan kaya web dan mengelola grup penuh dari info sheet:
1. Rich Bubbles & Message Types (MSGT-01 .. MSGT-10):
   - Voice note recording (PTT, durasi, waveform) & playback
   - Media attachments (image, video, audio, document, gif) dengan caption & flag view-once
   - Media viewer modal & dialog unduh native
   - Link-preview cards
   - Sticker picker (statis + animasi) & favorite shortcut
   - Polls: question, options, tally bar live, vote/retract
   - Lokasi: statis + live + caption, open maps
   - Kontak: displayName, phone, vcard
   - View-once tile gated
   - Reply/quote, Forwarded label, Deleted & Edited markers
2. Group Admin & Info Sheet (CONV-20, CONV-21):
   - Chat Info Sheet with tabs: Media, Docs, Links, and Group Members & Settings
   - Group administration: add/remove/promote/demote member, edit subject/description, toggle locked/announcement/join-approval, invite link copy/revoke, group photo, leave group.

</domain>

<decisions>
## Implementation Decisions

### D-05-1: Rich Message Bubbles & Meta Data
- Rich message bubbles implement visual parity for each type:
  - `PollBubble`: calculates option votes & percentage tally, supports single- and multi-select voting.
  - `LocationBubble`: renders map pin, coordinate display, and link action.
  - `ContactBubble`: renders avatar initials, contact display name, phone number, and vcard action.
  - `ViewOnceBubble`: gated tile displaying "1" badge, photo/video type label, and click-to-view guard.
  - `MediaViewerModal`: image/video/document inspection overlay with download button.

### D-05-2: Group Administration & Info Sheet
- `ChatInfoSheet`: sliding drawer / sheet containing:
  - Header: Avatar, subject, participant count
  - Tabs: Media, Documents, Links, Group Settings
  - Group Management: member list with roles (`admin`, `member`), role promotion/demotion, member kick, invite link management, and group parameter edits.

</decisions>
