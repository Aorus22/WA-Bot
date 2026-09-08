# Phase 6: Calls + Status + Channels - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning
**Mode:** Autonomous (1:1 Web Parity)

<domain>
## Phase Boundary

User bisa menelepon (suara/video, 1:1 dan grup), mem-post status, dan membaca channel seperti di web:
1. Calls (CALL-01 .. CALL-07):
   - Riwayat panggilan dengan filter (direction, type, status, target)
   - Panggilan suara/video 1:1 (preparing -> initiating -> ringing -> connected -> ended)
   - Group call (add participants, ringing participants)
   - Global Call Overlay (incoming call modal dan active call floating widget di atas rute apa pun)
   - Video upgrade flow (request/accept/reject/stop)
   - Auto refresh riwayat saat call berakhir
2. Status (STAT-01 .. STAT-04):
   - Story tray per pengirim (viewed/unviewed indicator, expiry 24h countdown)
   - Post status teks (background ARGB, typography)
   - Post status media (image/video dengan caption)
   - Story viewer modal dengan auto-advance progress timer dan read receipt dispatch
3. Channels (CHAN-01 .. CHAN-03):
   - Channel list (avatar, nama, followers/subscribers, verified badge, mute toggle)
   - Channel preview via invite link & follow/unfollow action
   - Channel message feed (views count, reactions map with live tally)

</domain>

<decisions>
## Implementation Decisions

### D-06-1: Call State Machine & Global Overlay
- Model `CallManager` in `desktop-gpui/crates/app/src/state/call.rs` tracking `active_call: Option<CallState>`, incoming call queue, audio device toggle (mute mic, mute speaker), and video upgrade requests.
- Overlay component renders conditionally over `AppShellView` on top of any active route.

### D-06-2: Status Tray & Viewer
- Model `StatusState` in `desktop-gpui/crates/app/src/state/status.rs` grouping statuses by sender with unviewed priority sorting.
- Status story viewer with progress bar timing (5 seconds per item), next/previous navigation, and mark-as-viewed trigger.

### D-06-3: Channels Feed & Follower
- Model `ChannelState` in `desktop-gpui/crates/app/src/state/channel.rs` maintaining followed channels, active channel feed, and reaction toggling.

</decisions>
