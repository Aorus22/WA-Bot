"""Backend discovery and ownership, a port of desktop-gpui `backend.rs` and
`backend_process.rs`. Stdlib only and blocking: callers run it off the UI thread.

Order of precedence (same as GPUI `detect_backend_url`):
  1. CLI `--url/--backend-url/--port/-p`   (never touches core.json)
  2. env `BACKEND_URL`, `BACKEND_PORT`     (never touches core.json)
  3. `<data>/run/core.json`, validated     (attach), else spawn the bundled backend
  4. `.env` `PORT=` as a last resort
  5. http://127.0.0.1:3090/api
"""
from __future__ import annotations

import json
import os
import re
import secrets
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path

CORE_NAME = "aorus22.wabot-core"
CORE_SCHEMA_VERSION = 1
BACKEND_PROCESS_NAME = "wa-bot-backend"
BACKEND_EXE_SUFFIX = ".exe" if sys.platform == "win32" else ""
DEFAULT_URL = "http://127.0.0.1:3090/api"


class BackendError(RuntimeError):
    pass


# --- core.json ---------------------------------------------------------------

@dataclass
class CoreRecord:
    schema_version: int
    name: str
    pid: int
    port: int
    instance_id: str
    started_at: str

    @classmethod
    def new(cls, pid: int, port: int) -> "CoreRecord":
        return cls(
            CORE_SCHEMA_VERSION, CORE_NAME, pid, port, new_instance_id(),
            datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        )

    def api_url(self) -> str:
        return f"http://127.0.0.1:{self.port}/api"


def new_instance_id() -> str:
    """UUID v4 shaped spawn marker (not a credential)."""
    b = bytearray(secrets.token_bytes(16))
    b[6] = (b[6] & 0x0F) | 0x40
    b[8] = (b[8] & 0x3F) | 0x80
    h = b.hex()
    return f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}"


def data_dir() -> Path:
    """`$WABOT_HOME`, else `~/.wa-bot` (fixed so the login survives any install path)."""
    override = os.environ.get("WABOT_HOME")
    return Path(override) if override else Path.home() / ".wa-bot"


def core_file() -> Path:
    return data_dir() / "run" / "core.json"


def read_core(path: Path) -> CoreRecord | None:
    try:
        raw = json.loads(path.read_bytes())
        rec = CoreRecord(
            int(raw["schema_version"]), str(raw["name"]), int(raw["pid"]),
            int(raw["port"]), str(raw["instance_id"]), str(raw["started_at"]),
        )
    except (OSError, ValueError, KeyError, TypeError):
        return None
    return rec if rec.schema_version == CORE_SCHEMA_VERSION and rec.name == CORE_NAME else None


def write_core(path: Path, rec: CoreRecord) -> None:
    """Atomic: temp file in the same folder, then rename."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.parent / f"core.json.{os.getpid()}.tmp"
    try:
        tmp.write_text(json.dumps(asdict(rec), indent=2))
        os.replace(tmp, path)
    finally:
        tmp.unlink(missing_ok=True)


def remove_core_if(path: Path, rec: CoreRecord) -> bool:
    """Delete only while the file still describes `rec`, so an old instance never
    removes a newer one's record."""
    cur = read_core(path)
    if cur and (cur.instance_id, cur.pid, cur.port) == (rec.instance_id, rec.pid, rec.port):
        try:
            path.unlink()
            return True
        except OSError:
            return False
    return False


# --- process inspection (Linux /proc; other platforms degrade to "unknown") ---

def listening_inode(table: str, port: int) -> int | None:
    for line in table.splitlines()[1:]:
        f = line.split()
        if len(f) < 10:
            continue
        try:
            local_port = int(f[1].rsplit(":", 1)[1], 16)
            if local_port == port and f[3] == "0A":
                return int(f[9])
        except (ValueError, IndexError):
            continue
    return None


def pid_listening_on(port: int) -> int | None:
    if not sys.platform.startswith("linux"):
        return None
    inode = None
    for p in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            inode = listening_inode(Path(p).read_text(), port)
        except OSError:
            continue
        if inode is not None:
            break
    if inode is None:
        return None
    needle = f"socket:[{inode}]"
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            for fd in (entry / "fd").iterdir():
                try:
                    if os.readlink(fd) == needle:
                        return int(entry.name)
                except OSError:
                    continue
        except OSError:
            continue
    return None


def process_name(pid: int) -> str | None:
    if not sys.platform.startswith("linux"):
        return None
    try:
        name = Path(f"/proc/{pid}/comm").read_text().strip()
    except OSError:
        return None
    return name.lower().removesuffix(".exe")


def backend_pids_using(directory: Path) -> list[int]:
    """Running backends whose cwd is `directory` (Linux only; else empty)."""
    if not sys.platform.startswith("linux"):
        return []
    try:
        target = directory.resolve(strict=True)
    except OSError:
        return []
    out = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        if process_name(pid) != BACKEND_PROCESS_NAME:
            continue
        try:
            if Path(os.readlink(entry / "cwd")) == target:
                out.append(pid)
        except OSError:
            continue
    return out


# --- probing ---------------------------------------------------------------

def looks_like_wabot_status(response: str) -> bool:
    return response.startswith("HTTP/1.1 200") and '"isLoggedIn"' in response


def is_wabot_backend(port: int) -> bool:
    """True when `port` answers GET /api/status like WA Bot does. An open port is
    not enough: a local Caddy answers every path with HTML."""
    try:
        s = socket.create_connection(("127.0.0.1", port), timeout=0.1)
    except OSError:
        return False
    with s:
        s.settimeout(0.4)
        try:
            s.sendall(
                f"GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n".encode()
            )
            data = b""
            while len(data) < 8192:
                chunk = s.recv(4096)
                if not chunk:
                    break
                data += chunk
        except OSError:
            pass
    return looks_like_wabot_status(data.decode("utf-8", "replace"))


def loopback_port(url: str) -> int | None:
    m = re.match(r"^http://(127\.0\.0\.1|localhost):(\d+)(/|$)", url)
    return int(m.group(2)) if m else None


@dataclass
class Probe:
    pid_name: str | None
    listener: tuple[int, str | None] | None
    responds: bool


REUSE, STALE, AMBIGUOUS = "reuse", "stale", "ambiguous"


def classify(rec: CoreRecord, probe: Probe) -> tuple[str, str]:
    """What to do about the backend core.json points at -> (verdict, reason)."""
    is_backend = lambda n: n == BACKEND_PROCESS_NAME  # noqa: E731
    if is_backend(probe.pid_name):
        if probe.listener and probe.listener[0] == rec.pid and probe.responds:
            return REUSE, ""
        return AMBIGUOUS, (
            f"backend pid {rec.pid} from {CORE_NAME} is running but does not serve port {rec.port} as expected"
        )
    if probe.listener and is_backend(probe.listener[1]):
        return AMBIGUOUS, (
            f"another {BACKEND_PROCESS_NAME} (pid {probe.listener[0]}) listens on port {rec.port}, "
            f"not the recorded pid {rec.pid}"
        )
    return STALE, ""  # gone, or its port belongs to an unrelated process: never adopted


def inspect(rec: CoreRecord) -> tuple[str, str]:
    pid = pid_listening_on(rec.port)
    listener = (pid, process_name(pid)) if pid else None
    probe = Probe(process_name(rec.pid), listener, bool(listener) and is_wabot_backend(rec.port))
    return classify(rec, probe)


# --- locating and spawning the bundled backend -------------------------------

def backend_binary(extra_dirs: list[Path] | None = None) -> Path | None:
    """`$WABOT_BACKEND`, then next to the app, `be/`, the repo root, and PATH-less
    dev locations (`<repo>/wa-bot-backend`)."""
    override = os.environ.get("WABOT_BACKEND")
    if override:
        # The child runs with the backend data directory as its cwd, so a
        # relative override must be frozen against the caller's cwd first.
        return Path(override).expanduser().resolve()
    name = f"wa-bot-backend{BACKEND_EXE_SUFFIX}"
    if getattr(sys, "frozen", False):
        # PyInstaller places the executable and bundled backend together in its
        # contents directory (`_internal` for this onedir build).
        here = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent.parent))
    else:
        here = Path(__file__).resolve().parent.parent  # desktop-qml/
    dirs = [here, here / "be", here.parent, *(extra_dirs or [])]
    return next((d / name for d in dirs if (d / name).is_file()), None)


def parse_handshake_line(line: str) -> int | None:
    m = re.search(r"BACKEND_PORT:(\d+)", line)
    if not m:
        return None
    port = int(m.group(1))
    return port if 0 < port <= 65535 else None


def spawn_env(base: Path, preferred_port: int | None = None) -> dict[str, str]:
    env = dict(os.environ)
    env.update(
        PORT=f":{preferred_port or 0}",
        ALLOWED_ORIGINS="*",
        DB_PATH=str(base / "database"),
        MEDIA_PATH=str(base / "media"),
        TZ="Asia/Jakarta",
    )
    return env


class OwnedBackend:
    """A backend process this app started (and only that one) and will stop."""

    def __init__(self, proc: subprocess.Popen, record: CoreRecord, use_registry: bool):
        self.proc, self.record, self.use_registry = proc, record, use_registry

    @property
    def api_url(self) -> str:
        return self.record.api_url()

    def stop(self, grace: float = 3.0) -> None:
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(grace)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        if self.use_registry:
            remove_core_if(core_file(), self.record)


def spawn_bundled(use_registry: bool, preferred_port: int | None = None,
                  handshake_timeout: float = 15.0, ready_timeout: float = 15.0) -> OwnedBackend:
    binary = backend_binary()
    if not binary:
        raise BackendError("wa-bot-backend not found next to the app (set WABOT_BACKEND to its path)")
    base = data_dir()
    (base / "database").mkdir(parents=True, exist_ok=True)
    (base / "media").mkdir(parents=True, exist_ok=True)
    # The Go backend opens database/*.db relative to its cwd.
    proc = subprocess.Popen(
        [str(binary)], cwd=base, env=spawn_env(base, preferred_port),
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
    )
    port: list[int] = []
    got = threading.Event()

    def drain() -> None:
        # Keep reading for the child's whole life: closing the pipe would SIGPIPE it.
        assert proc.stdout
        for line in proc.stdout:
            if not port and (p := parse_handshake_line(line)):
                port.append(p)
                got.set()
        got.set()

    threading.Thread(target=drain, name="backend-stdout", daemon=True).start()
    if not got.wait(handshake_timeout) or not port:
        proc.kill()
        raise BackendError("backend did not report BACKEND_PORT")
    deadline = time.monotonic() + ready_timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise BackendError(f"backend exited before readiness (status {proc.returncode})")
        if is_wabot_backend(port[0]):
            rec = CoreRecord.new(proc.pid, port[0])
            if use_registry:
                try:
                    write_core(core_file(), rec)
                except OSError as e:
                    print(f"[backend] could not write {core_file()}: {e}", file=sys.stderr)
            return OwnedBackend(proc, rec, use_registry)
        time.sleep(0.25)
    proc.kill()
    raise BackendError("backend readiness probe timed out")


def connect_or_start() -> tuple[str, OwnedBackend | None]:
    """Attach to the validated backend in core.json, or start the bundled one.
    Refuses when an unverifiable backend may already be using the data folder
    (two backends on one SQLite session would corrupt it)."""
    path = core_file()
    rec = read_core(path)
    if rec:
        verdict, why = inspect(rec)
        if verdict == REUSE:
            return rec.api_url(), None
        if verdict == AMBIGUOUS:
            raise BackendError(why)
        remove_core_if(path, rec)
    unknown = backend_pids_using(data_dir())
    if unknown:
        raise BackendError(
            f"wa-bot-backend (pid {', '.join(map(str, unknown))}) is already using {data_dir()} "
            f"but is not registered in {path}; stop it first"
        )
    owned = spawn_bundled(use_registry=True)
    return owned.api_url, owned


# --- restarting (GPUI `backend::restart` + `backend_process::stop_listener`) -----------------

def stop_listener(port: int, grace: float = 5.0) -> None:
    """Stop the process listening on `port`, but only when it is `wa-bot-backend`: SIGTERM, then
    SIGKILL after `grace` seconds. Raises BackendError when it is some other program."""
    pid = pid_listening_on(port)
    if pid is None:
        raise BackendError(f"could not tell which process is listening on port {port}")
    name = process_name(pid) or ""
    if name != BACKEND_PROCESS_NAME:
        raise BackendError(
            f"port {port} is held by '{name}' (pid {pid}), not by {BACKEND_PROCESS_NAME}; leaving it alone")

    def gone(within: float) -> bool:
        end = time.monotonic() + within
        while time.monotonic() < end:
            if not is_wabot_backend(port):
                return True
            time.sleep(0.15)
        return False

    os.kill(pid, signal.SIGTERM)
    if gone(grace):
        return
    os.kill(pid, signal.SIGKILL if hasattr(signal, "SIGKILL") else signal.SIGTERM)
    if not gone(3.0):
        raise BackendError(f"pid {pid} would not stop")


def restart_backend(owned: OwnedBackend | None, port: int | None, use_registry: bool = True
                    ) -> tuple[str, OwnedBackend]:
    """Stop the backend (the one we own through its handle, else an adopted wa-bot-backend found by
    its port) and spawn a fresh one, on the same port when it can be reused. Returns (url, owned):
    the caller then owns the new backend."""
    old_record = owned.record if owned else None
    if owned:
        owned.stop(5.0)
    if port and is_wabot_backend(port):
        if old_record is None:
            rec = read_core(core_file())
            old_record = rec if rec and rec.port == port else None
        stop_listener(port, 5.0)
    if use_registry and old_record:
        remove_core_if(core_file(), old_record)
    try:
        new = spawn_bundled(use_registry, preferred_port=port)
    except BackendError:
        if port is None:
            raise
        new = spawn_bundled(use_registry)  # the port can linger for a moment; any free one beats staying down
    return new.api_url, new


# --- entry point ----------------------------------------------------------------

def explicit_url(argv: list[str], env: dict[str, str]) -> str | None:
    """CLI flags then env overrides. Returns None when neither is given."""
    for i, a in enumerate(argv):
        nxt = argv[i + 1] if i + 1 < len(argv) else None
        if a in ("--url", "--backend-url") and nxt:
            return nxt
        if a.startswith("--url="):
            return a[len("--url="):]
        if a in ("--port", "-p") and nxt:
            return f"http://127.0.0.1:{nxt}/api"
        if a.startswith("--port="):
            return f"http://127.0.0.1:{a[len('--port='):]}/api"
    if (url := env.get("BACKEND_URL", "").strip()):
        return url
    if (port := env.get("BACKEND_PORT", "").strip().lstrip(":")):
        return f"http://127.0.0.1:{port}/api"
    return None


def dotenv_port(search: list[Path]) -> str | None:
    for p in search:
        try:
            for line in p.read_text().splitlines():
                if line.startswith("PORT="):
                    v = line[5:].strip().strip("\"'").lstrip(":")
                    if v:
                        return f"http://127.0.0.1:{v}/api"
        except OSError:
            continue
    return None


def resolve(argv: list[str], env: dict[str, str] | None = None) -> tuple[str, OwnedBackend | None, str | None]:
    """-> (api_url, owned_backend_or_None, error_message_or_None)."""
    env = os.environ if env is None else env
    if (url := explicit_url(argv, dict(env))):
        return url, None, None
    err = None
    try:
        url, owned = connect_or_start()
        return url, owned, None
    except BackendError as e:
        err = str(e)
        print(f"[backend] no backend available: {e}", file=sys.stderr)
    cwd = Path.cwd()
    fallback = dotenv_port([cwd / ".env", cwd.parent / ".env", cwd.parent.parent / ".env"])
    return fallback or DEFAULT_URL, None, err


def ws_url(api_url: str) -> str:
    base = api_url.rstrip("/").removesuffix("/api")
    if base.startswith("https://"):
        return "wss://" + base[len("https://"):] + "/ws"
    return base.replace("http://", "ws://", 1) + "/ws"
