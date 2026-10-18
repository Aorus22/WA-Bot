# Settings parity

Status: **partial; settings behavior has mock coverage and a matched screenshot pair.** At 1100×720 the QML content column is 672 px, the title/section header and appearance-row positions align, and primary controls precede the theme grid. The transparency/scale sliders, theme-mode button and switch styling now closely match GPUI; a few row baselines still differ by about 1–2 px.

| GPUI source | QML | Backend / state | Evidence | Status |
|---|---|---|---|---|
| `views/settings.rs`, settings state | `pages/settings/SettingsPage.qml`; `bridge/system.py` | persisted theme/layout/notification values; settings, history sync/media, TTS, account, restart actions | [GPUI baseline](gpui-baselines/settings-1100.png), [QML render](screenshots/settings-mock.png), both at default-dark/100% scale/28% transparency; `tests/test_settings.py` covers temporary preference persistence, AI/TTS saves, read receipts/contact rollback, history jobs, logout and restart callbacks | partial — callback contracts pass with test-owned state and the main appearance controls now align closely; backend restart, GTK/blur behavior and real history jobs remain unverified |

No real WhatsApp account was used. No settings were written to the user's configuration path by the screenshot run.
