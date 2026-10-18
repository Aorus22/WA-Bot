# Composer parity

Status: **partial; the QML composer exposes the GPUI text, attachment, emoji, sticker, poll, location, contact and voice-message flows.** File/device interactions are not fully verified on a native desktop.

| GPUI source | QML | Endpoint / state | Evidence | Status |
|---|---|---|---|---|
| `views/chat/composer/composer_bar.rs`, `composer/input.rs` | `composer/ComposerBar.qml` | `Composer.setText/send`; typing and optimistic message rows | [composer mock render](screenshots/chat-composer-mock.png); [compact chat](screenshots/chat-budi-800.png); `tests/test_composer_runtime.py`, `tests/test_qml_composer_keys.py` | partial — the attach, emoji/sticker, field, and mic/send controls share the GPUI pill layout; QTest verifies plain Enter/Ctrl+Enter send, Shift+Enter newline, and Markdown Enter/Shift+Enter newline with Ctrl+Enter send; IME composition, caret/selection and other focus contexts remain unverified |
| `views/chat/composer/attach_panel.rs`, `composer/send.rs` | plus menu, Qt `FileDialog`, pending attachment preview | `Composer.sendFile`; multipart `/send-media` | previous [attachment menu capture](screenshots/composer-attach-menu-mock.png) predates the latest source-alignment edit | partial — menu choices and single-file selection now follow GPUI; native dialog and real image clipboard behavior remain unverified |
| emoji and sticker pickers | `EmojiPicker.qml`, `StickerPicker.qml` | emoji data; `/stickers/favorites`, `/stickers/recent`, `/send-sticker` | [emoji picker](screenshots/composer-emoji-picker-mock.png), [sticker picker](screenshots/composer-sticker-picker-mock.png) | partial — category tabs, 30 px emoji cells, 96 px sticker tiles, empty states, and the shared Emoji/Sticker switch follow GPUI; native sticker send and animated sticker playback remain unverified; QML-only emoji search and keyboard grid navigation are omitted |
| poll/location/contact send dialogs | `ComposeDialogs.qml` | `/chats/{id}/poll`, `/location`, `/contact` | [poll dialog](screenshots/composer-poll-dialog-mock.png); `tests/test_composer_runtime.py` | labels, dimensions, checkbox controls, input focus, draft retention on cancel, and close/reset after successful response now follow GPUI; live-account behavior remains unverified |
| voice-note recorder | `ComposerBar.qml` waveform and controls; `bridge/composer_voice.py` | `Composer.startVoice/cancelVoice/sendVoice` | none | partial — no microphone permission/device test was run |

All tests and screenshots use the local mock backend and no real WhatsApp account.
