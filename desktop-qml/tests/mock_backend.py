"""In-process mock of the Go backend for tests and offscreen screenshots — so nothing ever
touches a real WhatsApp account. REST under /api, JSON WebSocket at /ws, static media.

    mb = MockBackend(); mb.start()           # picks a free port
    mb.url                                    # "http://127.0.0.1:PORT/api"  (pass as --url)
    mb.broadcast("new_message", {...})        # push a WS event to every connected client
    @mb.route("GET", r"/chats/(?P<id>[^/]+)/messages")
    def messages(req): return 200, [...]      # req.params, req.query, req.json(), req.body, req.method

Handlers return (status, obj) with obj JSON-serialisable, (status, bytes, content_type) for raw
bodies, or None for 204. Add area-specific fixtures from tests/mock_<area>.py by calling
`install(mb)`; `MockBackend(areas=[...])` loads them. State lives in `mb.state` (a plain dict).
Ground truth for payload shapes: desktop-gpui/crates/backend-client/src/{client.rs,dto/}.
"""
from __future__ import annotations

import base64
import hashlib
import importlib
import json
import re
import socket
import struct
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Callable
from urllib.parse import parse_qs, unquote, urlparse

WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


class Request:
    def __init__(self, method: str, path: str, query: dict[str, list[str]], body: bytes, params: dict[str, str], headers):
        self.method, self.path, self.query, self.body, self.params, self.headers = method, path, query, body, params, headers

    def json(self) -> Any:
        return json.loads(self.body or b"null")

    def q(self, name: str, default: str | None = None) -> str | None:
        v = self.query.get(name)
        return v[0] if v else default


def jpeg_placeholder(rgb=(90, 120, 150), size=64) -> bytes:
    """A tiny valid PNG (named jpeg for brevity) so image bubbles have something to load."""
    import zlib
    raw = b"".join(b"\x00" + bytes(rgb) * size for _ in range(size))

    def chunk(t: bytes, d: bytes) -> bytes:
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


class MockBackend:
    def __init__(self, areas: list[str] | None = None, port: int = 0):
        self.routes: list[tuple[str, re.Pattern, Callable[[Request], Any]]] = []
        self.clients: list[socket.socket] = []
        self.requests: list[tuple[str, str, bytes]] = []  # (method, path, body) log for assertions
        self.state: dict[str, Any] = {"logged_in": True, "phone": "628123456789"}
        self._lock = threading.Lock()
        self._port = port
        self._server: ThreadingHTTPServer | None = None
        self._install_core()
        for a in areas or []:
            importlib.import_module(f"mock_{a}").install(self)

    # -- routing -------------------------------------------------------------
    def route(self, method: str, pattern: str):
        rx = re.compile(r"^/api" + pattern + r"/?$")

        def deco(fn):
            self.routes.insert(0, (method.upper(), rx, fn))  # later registrations win
            return fn
        return deco

    def _dispatch(self, method: str, raw_path: str, body: bytes, headers) -> tuple[int, bytes, str]:
        u = urlparse(raw_path)
        path = unquote(u.path)
        self.requests.append((method, raw_path, body))
        if method == "OPTIONS":
            return 204, b"", "text/plain"
        for m, rx, fn in self.routes:
            mt = rx.match(path)
            if m == method and mt:
                out = fn(Request(method, path, parse_qs(u.query), body, mt.groupdict(), headers))
                if out is None:
                    return 204, b"", "text/plain"
                if len(out) == 3:
                    return out[0], out[1], out[2]
                return out[0], json.dumps(out[1]).encode(), "application/json"
        return 404, json.dumps({"error": f"mock: no route {method} {path}"}).encode(), "application/json"

    # -- websocket -------------------------------------------------------------
    def broadcast(self, type_: str, payload: dict | None = None) -> None:
        frame = self._frame(json.dumps({"type": type_, "payload": payload or {}}).encode())
        with self._lock:
            for c in list(self.clients):
                try:
                    c.sendall(frame)
                except OSError:
                    self.clients.remove(c)

    @staticmethod
    def _frame(data: bytes, opcode: int = 0x1) -> bytes:
        n = len(data)
        head = bytes([0x80 | opcode]) + (bytes([n]) if n < 126 else b"\x7e" + struct.pack(">H", n) if n < 65536 else b"\x7f" + struct.pack(">Q", n))
        return head + data

    def close_clients(self) -> None:
        with self._lock:
            for c in self.clients:
                try:
                    c.sendall(self._frame(struct.pack(">H", 1000), 0x8))
                    c.close()
                except OSError:
                    pass
            self.clients.clear()

    def wait_for_client(self, timeout: float = 5.0) -> bool:
        end = time.time() + timeout
        while time.time() < end:
            if self.clients:
                return True
            time.sleep(0.02)
        return False

    # -- lifecycle ---------------------------------------------------------------
    def start(self) -> "MockBackend":
        outer = self

        class H(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *a):  # quiet
                pass

            def _handle(self):
                n = int(self.headers.get("Content-Length") or 0)
                body = self.rfile.read(n) if n else b""
                if self.path.split("?")[0] == "/ws" and self.headers.get("Upgrade", "").lower() == "websocket":
                    return self._ws()
                status, data, ctype = outer._dispatch(self.command, self.path, body, self.headers)
                self.send_response(status)
                self.send_header("Content-Type", ctype)
                self.send_header("Content-Length", str(len(data)))
                self.send_header("Access-Control-Allow-Origin", "*")
                self.end_headers()
                self.wfile.write(data)

            do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = do_OPTIONS = _handle

            def _ws(self):
                key = self.headers["Sec-WebSocket-Key"]
                accept = base64.b64encode(hashlib.sha1((key + WS_GUID).encode()).digest()).decode()
                self.send_response(101, "Switching Protocols")
                self.send_header("Upgrade", "websocket")
                self.send_header("Connection", "Upgrade")
                self.send_header("Sec-WebSocket-Accept", accept)
                self.end_headers()
                sock = self.connection
                with outer._lock:
                    outer.clients.append(sock)
                try:  # read and ignore client frames until it goes away
                    while True:
                        hdr = sock.recv(2)
                        if len(hdr) < 2:
                            break
                        op, ln = hdr[0] & 0x0F, hdr[1] & 0x7F
                        if ln == 126:
                            ln = struct.unpack(">H", sock.recv(2))[0]
                        elif ln == 127:
                            ln = struct.unpack(">Q", sock.recv(8))[0]
                        if hdr[1] & 0x80:
                            sock.recv(4)
                        left = ln
                        while left:
                            left -= len(sock.recv(left))
                        if op == 0x8:
                            break
                except OSError:
                    pass
                finally:
                    with outer._lock:
                        if sock in outer.clients:
                            outer.clients.remove(sock)
                    self.close_connection = True

        self._server = ThreadingHTTPServer(("127.0.0.1", self._port), H)
        self._server.daemon_threads = True
        self._port = self._server.server_address[1]
        threading.Thread(target=self._server.serve_forever, daemon=True, name="mock-backend").start()
        return self

    def stop(self) -> None:
        if self._server:
            self.close_clients()
            self._server.shutdown()
            self._server.server_close()
            self._server = None

    @property
    def port(self) -> int:
        return self._port

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self._port}/api"

    # -- core fixtures -------------------------------------------------------------
    def _install_core(self) -> None:
        mb = self

        @self.route("GET", "/status")
        def status(req):
            return 200, {"isLoggedIn": mb.state["logged_in"], "phone": mb.state["phone"] if mb.state["logged_in"] else ""}

        @self.route("GET", "/qr-code")
        def qr(req):
            return 200, {"code": mb.state.get("qr", "2@MOCKQRCODEPAYLOAD,abc,def,ghi")}

        @self.route("GET", "/health")
        def health(req):
            return 200, {"status": "ok"}

        @self.route("GET", "/settings")
        def settings(req):
            return 200, mb.state.setdefault("settings", {})

        @self.route("POST", "/logout")
        def logout(req):
            mb.state["logged_in"] = False
            mb.broadcast("logged_out")
            return 200, {"status": "ok"}

        @self.route("GET", "/chats")
        def chats(req):
            return 200, mb.state.get("chats", [])

        @self.route("GET", "/chats/(?P<id>[^/]+)/messages")
        def messages(req):
            return 200, mb.state.get("messages", {}).get(req.params["id"], [])

        @self.route("GET", r"/media/(?P<name>.+)")
        def media(req):
            return 200, jpeg_placeholder(), "image/png"


if __name__ == "__main__":
    mb = MockBackend().start()
    print(mb.url)
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        mb.stop()
