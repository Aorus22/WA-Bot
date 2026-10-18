# Status and channels parity

Status: **partial; list, story viewer/composer, channel list/feed/discovery and reactions have mock-backed coverage.** Real account posting and media playback are not tested.

| GPUI source | QML | Endpoint / events | Evidence | Status |
|---|---|---|---|---|
| `views/status.rs` | `pages/status/StatusPage.qml`, `StatusStoryRow.qml`, `StatusViewer.qml`, `StatusComposer.qml` | `/statuses`, `/statuses/{id}/viewed`, `/statuses/{id}/reply` | `tests/test_status_channels.py`, `tests/test_qml_status.py`; [GPUI baseline](gpui-baselines/status-list-1100.png), [QML list](screenshots/status-list-mock.png), [viewer](screenshots/status-viewer-mock.png) | partial — Status list, text-only composer, image/text viewer, viewed receipt and reply follow GPUI; matched viewer visuals and real account behavior remain open |
| `views/channels.rs` | `pages/channels/ChannelsPage.qml`, `ChannelRow.qml`, `ChannelPost.qml`, `ChannelDiscoverDialog.qml` | channel list, posts, reactions, follow/mute/unfollow; channel WS events | `tests/test_status_channels.py`, `tests/test_qml_channels.py`; [GPUI baseline](gpui-baselines/channels-list-1100.png), [QML list](screenshots/channels-list-mock.png), [feed](screenshots/channel-feed-mock.png) | partial — QtTest mouse click opens a selected channel feed; channel cards now follow GPUI's 600 px text-card layout and Today/Yesterday/Earlier timestamp; matched feed visuals and live account behavior remain open |

No live account or user data was used.

The Status and Channels list pairs use the same local fixture at 1100×720. Remaining Status screens, channel feed/details, and other routes still need GPUI comparisons.
