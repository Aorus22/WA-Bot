import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from bridge import discovery as d  # noqa: E402

TABLE = (
    "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n"
    "   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 424242 1 0 100 0 0 10 0\n"
    "   1: 0100007F:0BB9 0100007F:C350 01 00000000:00000000 00:00000000 00000000  1000        0 515151 1 0 100 0 0 10 0\n"
)


def test_listening_inode():
    assert d.listening_inode(TABLE, 3000) == 424242
    assert d.listening_inode(TABLE, 3001) is None  # established, not listening
    assert d.listening_inode(TABLE, 9999) is None


def test_handshake():
    assert d.parse_handshake_line("2026 Starting BACKEND_PORT:8080 (ready)") == 8080
    assert d.parse_handshake_line("BACKEND_PORT:0") is None
    assert d.parse_handshake_line("BACKEND_PORT:") is None
    assert d.parse_handshake_line("log line") is None


def test_status_probe_rejects_other_servers():
    ok = 'HTTP/1.1 200 OK\r\n\r\n{"isLoggedIn":false}'
    assert d.looks_like_wabot_status(ok)
    assert not d.looks_like_wabot_status("HTTP/1.1 200 OK\r\nServer: Caddy\r\n\r\n<!doctype html>")
    assert not d.looks_like_wabot_status('HTTP/1.1 404 Not Found\r\n\r\n{"isLoggedIn":false}')
    assert not d.looks_like_wabot_status("")


def test_loopback_port():
    assert d.loopback_port("http://127.0.0.1:51234/api") == 51234
    assert d.loopback_port("http://localhost:3090/api") == 3090
    assert d.loopback_port("https://example.com:443/api") is None
    assert d.loopback_port("http://10.0.0.5:3090/api") is None
    assert d.loopback_port("http://127.0.0.1/api") is None


def rec(pid=10, port=3000):
    return d.CoreRecord.new(pid, port)


def test_classify():
    B = d.BACKEND_PROCESS_NAME
    assert d.classify(rec(), d.Probe(B, (10, B), True))[0] == d.REUSE
    assert d.classify(rec(), d.Probe(None, None, False))[0] == d.STALE
    assert d.classify(rec(), d.Probe("firefox", None, False))[0] == d.STALE  # recycled pid
    assert d.classify(rec(), d.Probe(None, (77, "caddy"), True))[0] == d.STALE  # never adopt
    assert d.classify(rec(), d.Probe(B, None, False))[0] == d.AMBIGUOUS
    assert d.classify(rec(), d.Probe(B, (10, B), False))[0] == d.AMBIGUOUS
    assert d.classify(rec(), d.Probe(None, (99, B), True))[0] == d.AMBIGUOUS


def test_core_record_roundtrip_and_guarded_remove(tmp_path):
    path = tmp_path / "run" / "core.json"
    r = rec(4242, 45321)
    d.write_core(path, r)
    assert d.read_core(path) == r
    assert len(list(path.parent.iterdir())) == 1  # no temp file left
    # same pid/port but another instance id must not delete it
    assert not d.remove_core_if(path, rec(4242, 45321))
    assert d.remove_core_if(path, r) and not path.exists()


def test_corrupt_or_foreign_records_read_as_none(tmp_path):
    path = tmp_path / "core.json"
    assert d.read_core(path) is None
    path.write_text("{ not json")
    assert d.read_core(path) is None
    other = rec()
    other.schema_version = 99
    path.write_text(json.dumps(other.__dict__))
    assert d.read_core(path) is None


def test_explicit_overrides_priority():
    assert d.explicit_url(["--url", "http://x:1/api"], {"BACKEND_PORT": "9"}) == "http://x:1/api"
    assert d.explicit_url(["--url=http://x:2/api"], {}) == "http://x:2/api"
    assert d.explicit_url(["-p", "4040"], {}) == "http://127.0.0.1:4040/api"
    assert d.explicit_url(["--port=5050"], {}) == "http://127.0.0.1:5050/api"
    assert d.explicit_url([], {"BACKEND_URL": " http://y/api "}) == "http://y/api"
    assert d.explicit_url([], {"BACKEND_PORT": ":7070"}) == "http://127.0.0.1:7070/api"
    assert d.explicit_url([], {}) is None


def test_ws_url_follows_api_url():
    assert d.ws_url("http://127.0.0.1:45321/api") == "ws://127.0.0.1:45321/ws"
    assert d.ws_url("https://bot.example.com/api") == "wss://bot.example.com/ws"


def test_data_dir_override(monkeypatch):
    monkeypatch.setenv("WABOT_HOME", "/tmp/wabot-x")
    assert d.data_dir() == Path("/tmp/wabot-x")
    monkeypatch.delenv("WABOT_HOME")
    assert d.data_dir().name == ".wa-bot"


def test_spawn_env_uses_env_only_no_secrets():
    env = d.spawn_env(Path("/tmp/b"), 3090)
    assert env["PORT"] == ":3090" and env["DB_PATH"] == "/tmp/b/database"
    assert env["TZ"] == "Asia/Jakarta" and env["ALLOWED_ORIGINS"] == "*"
    assert d.spawn_env(Path("/tmp/b"))["PORT"] == ":0"
