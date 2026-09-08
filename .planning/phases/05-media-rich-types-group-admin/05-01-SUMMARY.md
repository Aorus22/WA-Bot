---
phase: 05-media-rich-types-group-admin
plan: 01
title: Rich Message Bubbles & Interactive Types (Poll, Location, Contact, View-Once)
status: passed
---

# Plan 05-01 Summary: Rich Message Bubbles & Interactive Types

## Accomplishments
- Implemented `PollHelper` in `desktop-gpui/crates/app/src/components/rich_bubbles.rs`:
  - Live vote tally calculation and percentages across voters (`compute_tallies`, `option_percentage`).
  - Single and multi-select vote toggling (`toggle_vote`).
- Implemented `LocationHelper`:
  - Human-readable location labels (`display_label`).
  - Google Maps web URL generation (`google_maps_url`).
- Implemented `ContactHelper`:
  - vCard telephone parsing and display extraction (`primary_phone`).
- Implemented `ViewOnceHelper`:
  - View status inspection and type-specific badge labeling (`is_viewed`, `label`).
- Unit test suite (`test_poll_tallies_and_toggle`, `test_location_and_maps_url`, `test_contact_vcard_phone_extraction`, `test_view_once_helper`) passing cleanly.

## Key Files
- `desktop-gpui/crates/app/src/components/rich_bubbles.rs`
- `desktop-gpui/crates/app/src/components/mod.rs`
