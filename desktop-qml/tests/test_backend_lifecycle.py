"""Spawn/attach/ownership against the real Go backend in a scratch data dir.
Skipped when no backend binary is available (set WABOT_BACKEND)."""
import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from bridge import discovery as d  # noqa: E402

pytestmark = pytest.mark.skipif(d.backend_binary() is None, reason="wa-bot-backend not found")


@pytest.fixture
def scratch_home(tmp_path, monkeypatch):
    monkeypatch.setenv("WABOT_HOME", str(tmp_path))
    return tmp_path


def test_spawn_registers_attach_reuses_owner_stops(scratch_home):
    url, owned = d.connect_or_start()
    try:
        assert owned is not None and url == owned.api_url
        rec = d.read_core(d.core_file())
        assert rec and rec.pid == owned.proc.pid and rec.port == d.loopback_port(url)
        assert d.process_name(rec.pid) == d.BACKEND_PROCESS_NAME

        # a second client attaches to the same backend and does not own it
        url2, owned2 = d.connect_or_start()
        assert url2 == url and owned2 is None
        assert d.is_wabot_backend(rec.port)
    finally:
        owned.stop()
    assert not d.core_file().exists()
    assert owned.proc.poll() is not None


def test_stale_record_is_cleared_then_respawned(scratch_home):
    d.write_core(d.core_file(), d.CoreRecord.new(2**22 - 3, 1))  # dead pid, nothing on port 1
    url, owned = d.connect_or_start()
    try:
        assert owned is not None
        assert d.read_core(d.core_file()).pid == owned.proc.pid
    finally:
        owned.stop()


def test_unregistered_backend_on_our_data_dir_is_refused(scratch_home):
    url, owned = d.connect_or_start()
    try:
        d.core_file().unlink()  # backend alive, registry gone
        with pytest.raises(d.BackendError, match="not registered"):
            d.connect_or_start()
    finally:
        owned.stop()


def test_explicit_url_never_touches_registry(scratch_home):
    url, owned, err = d.resolve(["--port", "4321"], {})
    assert (url, owned, err) == ("http://127.0.0.1:4321/api", None, None)
    assert not d.core_file().exists()
