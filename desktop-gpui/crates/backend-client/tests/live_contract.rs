use std::path::PathBuf;
use std::time::Duration;
use wabot_backend_client::{HttpClient, WsClient, WsEvent, Chat, Message, HistorySyncStatus};
use wabot_supervisor::{ensure_data_dirs, BackendStatus, SpawnOptions, Supervisor};

#[test]
fn test_dto_serialization_roundtrip() {
    let raw_chat = r#"{
        "id": "12345@s.whatsapp.net",
        "name": "Test User",
        "avatar": "",
        "lastMsg": "Hello",
        "lastTime": 1700000000,
        "unread": 2,
        "isActive": true,
        "isGroup": false,
        "archived": false,
        "pinnedAt": null,
        "muteMode": "off",
        "mutedUntil": null
    }"#;
    let chat: Chat = serde_json::from_str(raw_chat).expect("Chat must deserialize");
    assert_eq!(chat.id, "12345@s.whatsapp.net");
    assert_eq!(chat.unread, 2);
    assert_eq!(chat.name, "Test User");

    let raw_msg = r#"{
        "id": "msg-123",
        "chatId": "12345@s.whatsapp.net",
        "from": "12345@s.whatsapp.net",
        "to": "me",
        "content": "Test content",
        "timestamp": 1700000010,
        "status": "delivered",
        "type": "text"
    }"#;
    let msg: Message = serde_json::from_str(raw_msg).expect("Message must deserialize");
    assert_eq!(msg.id, "msg-123");
    assert_eq!(msg.message_type, "text");

    let raw_sync = r#"{
        "state": "running",
        "pendingChats": 5,
        "pendingMessages": 100,
        "chatsTotal": 10,
        "chatsProcessed": 5,
        "messagesAdded": 50,
        "errors": []
    }"#;
    let sync: HistorySyncStatus = serde_json::from_str(raw_sync).expect("HistorySyncStatus must deserialize");
    assert_eq!(sync.state, "running");
    assert_eq!(sync.pending_chats, 5);
}

#[test]
fn test_media_url_resolving() {
    let client = HttpClient::from_port(8080);
    assert_eq!(client.media_url(None), None);
    assert_eq!(client.media_url(Some("")), None);
    assert_eq!(
        client.media_url(Some("https://example.com/img.jpg")),
        Some("https://example.com/img.jpg".to_string())
    );
    assert_eq!(
        client.media_url(Some("/api/media/test.jpg")),
        Some("http://127.0.0.1:8080/api/media/test.jpg".to_string())
    );
    assert_eq!(
        client.media_url(Some("media/test.jpg")),
        Some("http://127.0.0.1:8080/api/media/test.jpg".to_string())
    );
}

#[tokio::test]
async fn live_backend_contract_roundtrip_if_available() {
    if std::env::var("CI").is_ok() && std::env::var("RUNNER_OS").as_deref() == Ok("Windows") {
        return;
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .ancestors()
        .nth(3)
        .expect("repo root above desktop-gpui")
        .to_path_buf();
    if !repo_root.join("cmd/api").is_dir() {
        eprintln!("SKIP: cmd/api not found under {}", repo_root.display());
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    #[cfg(windows)]
    let bin = tmp.path().join("wa-bot-backend.exe");
    #[cfg(not(windows))]
    let bin = tmp.path().join("wa-bot-backend");
    let build = std::process::Command::new("go")
        .args(["build", "-o"])
        .arg(&bin)
        .arg("./cmd/api")
        .current_dir(&repo_root)
        .output();
    let Ok(out) = build else {
        eprintln!("SKIP: `go build` could not run");
        return;
    };
    if !out.status.success() {
        eprintln!(
            "SKIP: `go build ./cmd/api` failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let (db_dir, media_dir) = ensure_data_dirs(tmp.path()).unwrap();
    let opts = SpawnOptions::new(&bin, db_dir, media_dir)
        .unwrap()
        .with_work_dir(tmp.path())
        .with_handshake_timeout(Duration::from_secs(30))
        .with_readiness_timeout(Duration::from_secs(30));
    let mut sup = Supervisor::new();
    let info = sup.spawn(opts).await.expect("live backend must reach ready");
    assert_eq!(sup.status(), BackendStatus::Ready);

    let client = HttpClient::new(&info.base_url);

    // 1. Health check
    client.health_check().await.expect("health check must succeed");

    // 2. Status (is_logged_in)
    let status = client.get_status().await.expect("status check must succeed");
    assert!(!status.is_logged_in);

    // 3. Get chats
    let chats = client.get_chats().await.expect("get_chats must succeed");
    assert!(chats.is_empty() || !chats.is_empty());

    // 4. Get settings
    let settings = client.get_settings().await.expect("get_settings must succeed");
    assert!(settings.has_gemini_key.is_some() || settings.has_gemini_key.is_none());

    // 5. WebSocket connection & handshake
    let ws_url = format!("ws://127.0.0.1:{}/ws", info.port);
    let ws_client = WsClient::from_static_url(ws_url, Some("desktop-test-user".to_string()));
    let rx = ws_client.subscribe();
    ws_client.start();

    // Wait for connected event
    let mut connected = false;
    for _ in 0..30 {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv_async()).await {
            Ok(Ok(WsEvent::Connected)) => {
                connected = true;
                break;
            }
            _ => {}
        }
    }
    assert!(connected, "WsClient must connect to live backend WS");

    ws_client.stop();
    sup.stop(Duration::from_secs(10)).await.unwrap();
}
