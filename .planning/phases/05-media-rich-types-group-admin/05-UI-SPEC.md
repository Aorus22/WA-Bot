# Phase 5: Media + Rich Types + Group Admin - UI Design Contract

**Created:** 2026-09-09
**Status:** Approved

## 1. Visual Hierarchy & Dimensions

- **Poll Bubble:** Min-width `260px`, option card padding `8px 12px`, background progress fill `primary/15`, option checkbox `●` / `○`, option vote count on right.
- **Location Bubble:** Width `260px`, top map preview height `120px` with pin icon, address snippet `text_xs text_muted`.
- **Contact Bubble:** Width `240px`, avatar `36x36px`, phone number formatted, bottom "Message" or "Save" button.
- **View-Once Tile:** Capsule / box with circled "1" badge, text "Photo" or "Video", blurred/locked thumbnail before open.
- **Chat Info Sheet:** Width `380px`, sliding in from right side of the window, tabs header (Media, Docs, Links, Group).
- **Media Viewer Modal:** Full-window semi-transparent backdrop (`black/80`), centered media container with max 90vw/90vh, top-right close and download buttons.

## 2. Interaction Flows

1. **Poll Voting:** Clicking an option immediately updates tally visually and dispatches vote to backend.
2. **View-Once:** First click reveals media in memory-only viewer; once closed, status changes to viewed (`viewed: true`) and cannot be reopened.
3. **Group Admin Actions:** Clicking participant row menu triggers actions: "Make group admin", "Dismiss as admin", "Remove from group".
