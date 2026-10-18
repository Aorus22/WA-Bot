# Authentication parity

Status: **partial; QR and startup/login states render and bridge tests pass.** The QR and phone-tab views have matched GPUI/QML captures at 1100×720. Phone-code pairing remains unavailable because the current backend has no pairing endpoint, matching the GPUI/web “coming soon” state.

| GPUI source | QML | Endpoint / events | Evidence | Status |
|---|---|---|---|---|
| `views/auth/*`, `state/auth.rs` | `pages/auth/StartupGate.qml`, `LoginPage.qml`, `QrPanel.qml`, `PhoneTab.qml`; `bridge/auth.py` | `GET /status`, `GET /qr-code`, `POST /logout`; `qr_code`, `auth_success`, `logged_out` | `tests/test_auth.py`; [GPUI QR](gpui-baselines/login-qr-1100.png) / [QML QR](screenshots/login-qr-mock.png), [GPUI phone tab](gpui-baselines/login-phone-1100.png) / [QML phone tab](screenshots/login-phone-mock.png) | partial — mock state flow and QR generation verified; QR pairing with WhatsApp not attempted |

The QR screen encodes the mock backend code. It is not a real account QR.
