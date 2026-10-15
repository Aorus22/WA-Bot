"""Stateful mock of the bot administration endpoints (triggers, cron, webhooks, webhook logs, docs, AI assistant).
Shapes follow `desktop-gpui/crates/backend-client/src/dto/bot.rs` and the Go handlers (status codes included:
trigger DELETE answers 200 {"status":"ok"}, cron/webhook DELETE answer 204, POST 201).

    mb = MockBackend(areas=["common", "bot"])
    mb.state["bot"]["triggers"]          # live lists (dicts) — inspect after an action
    mb.state["bot_fail"] = {("PUT", "/triggers/t1")}   # make a call answer 500 {"error": "boom"}
    mb.state["bot_empty"] = True          # every list starts empty (empty-state screenshots)
Calls are recorded in `mb.requests` like every other mock request.
"""
from __future__ import annotations

import json
import re
import time
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

DOCS_FILE = Path(__file__).resolve().parents[2] / "internal" / "delivery" / "http" / "handlers" / "docs" / "api-reference.md"
FALLBACK_DOCS = """# Bot Lua API Documentation

Welcome.

## Global Variables

| Variable | Type | Description |
|:---|:---|:---|
| `msg` | `table` | **(Triggers Only)** Complete message state. |

## Messaging Functions

### `send_text(target, text)`

```lua
send_text(msg.chat_id, "Hello!")
```
"""


def iso(ago_s: int) -> str:
    return (datetime.now(timezone.utc) - timedelta(seconds=ago_s)).astimezone().isoformat(timespec="seconds")


def trigger(id_, name, pattern, active=True, ago=3600, priority=0, desc="", script=None):
    return {"id": id_, "name": name, "pattern": pattern, "script": script or 'send_text(sender, "pong")',
            "priority": priority, "is_active": active, "description": desc,
            "created_at": iso(ago + 600), "updated_at": iso(ago)}


def cron(id_, name, schedule, active=True, ago=7200, desc="", script=None):
    return {"id": id_, "name": name, "schedule": schedule, "script": script or 'print("tick")',
            "is_active": active, "description": desc, "created_at": iso(ago + 600), "updated_at": iso(ago)}


def webhook(id_, name, path, active=True, ago=86400 * 3, secret="", desc="", script=None):
    return {"id": id_, "name": name, "path": path, "script": script or 'response = { status = 200, body = "ok" }',
            "secret": secret, "is_active": active, "description": desc, "created_at": iso(ago + 600), "updated_at": iso(ago)}


def log(id_, wid, path, method, status, body, ago=60, ip="203.0.113.7", query=""):
    return {"id": id_, "webhook_id": wid, "webhook_path": path, "source_ip": ip, "method": method,
            "headers": json.dumps({"Content-Type": "application/json", "User-Agent": "curl/8.5.0", "X-Webhook-Secret": "s3cret"}),
            "body": body, "query_params": query, "status_code": status, "created_at": int(time.time()) - ago}


def fixtures() -> dict:
    return {
        "triggers": [
            trigger("t1", "Ping reply", "^!ping$", ago=120, desc="Answers pong whenever someone says !ping"),
            trigger("t2", "Weather lookup", r"^!cuaca\s+(.*)", ago=3600 * 5, priority=5,
                    desc="Fetches the forecast for a city from an external API and replies in the same chat with a short summary"),
            trigger("t3", "Greeting (disabled)", r"(?i)^(hi|halo|hello)\b", active=False, ago=86400 * 9),
        ],
        "crons": [
            cron("c1", "Morning broadcast", "0 7 * * *", ago=3600 * 20, desc="Good morning message to the team group"),
            cron("c2", "Heartbeat", "*/5 * * * *", ago=600),
            cron("c3", "Weekly report", "0 9 * * 1", active=False, ago=86400 * 40, desc="Monday report"),
        ],
        "webhooks": [
            webhook("w1", "GitHub deploys", "github-deploy", secret="s3cret", desc="Posts deploy status to the dev group"),
            webhook("w2", "Order notification", "orders", ago=3600 * 30),
            webhook("w3", "Legacy form", "legacy.form", active=False, ago=86400 * 80),
        ],
        "logs": [
            log("l1", "w1", "github-deploy", "POST", 200, json.dumps({"ref": "refs/heads/main", "pusher": {"name": "budi"}, "commits": 3}), ago=45),
            log("l2", "w2", "orders", "POST", 200, json.dumps({"order": 1042, "total": 250000}), ago=900, ip="198.51.100.4"),
            log("l3", "w1", "github-deploy", "POST", 401, "", ago=4000, query="secret=wrong"),
            log("l4", "w2", "orders", "GET", 500, "", ago=3600 * 6, ip="198.51.100.9", query="debug=1"),
            log("l5", "w3", "legacy.form", "PUT", 410, "name=ani&phone=0812", ago=86400 * 2),
        ],
    }


def install(mb) -> None:
    st = mb.state.setdefault("bot", {})
    data = {"triggers": [], "crons": [], "webhooks": [], "logs": []} if mb.state.get("bot_empty") else fixtures()
    st.update(data)
    mb.state.setdefault("bot_fail", set())

    def fails(req) -> bool:
        return (req.method, re.sub(r"^/api", "", req.path)) in mb.state["bot_fail"]

    def boom(_req=None):
        return 500, {"error": "boom"}

    def body(req):
        try:
            return req.json() or {}
        except ValueError:
            return {}

    def crud(kind, path, noun, strict_delete_body, extra_create=None):
        items = lambda: st[kind]  # noqa: E731

        @mb.route("GET", path)
        def list_(req):
            return boom() if fails(req) else (200, items())

        @mb.route("POST", path)
        def create(req):
            if fails(req):
                return boom()
            d = body(req)
            if extra_create and (err := extra_create(d)):
                return 400, {"error": err}
            d = {**d, "id": uuid.uuid4().hex[:12], "created_at": iso(0), "updated_at": iso(0)}
            items().append(d)
            return 201, d

        @mb.route("PUT", path + r"/(?P<id>[^/]+)")
        def update(req):
            if fails(req):
                return boom()
            d = body(req)
            if extra_create and kind == "webhooks" and d.get("path") and (err := extra_create(d)):
                return 400, {"error": err}
            for i, it in enumerate(items()):
                if it["id"] == req.params["id"]:
                    items()[i] = {**it, **d, "id": it["id"], "updated_at": iso(0)}
                    return 200, items()[i]
            return 404, {"error": f"{noun} not found"}

        @mb.route("DELETE", path + r"/(?P<id>[^/]+)")
        def delete(req):
            if fails(req):
                return boom()
            items()[:] = [i for i in items() if i["id"] != req.params["id"]]
            return (200, {"status": "ok"}) if strict_delete_body else None

        @mb.route("DELETE", path)
        def delete_all(req):
            if fails(req):
                return boom()
            items().clear()
            return (200, {"status": "ok"}) if strict_delete_body else None

    def webhook_path_ok(d):
        if not d.get("path"):
            return "path is required"
        if not re.match(r"^[A-Za-z0-9._-]+$", d["path"]):
            return "path must only contain alphanumeric characters, dots, hyphens, and underscores"
        return None

    crud("triggers", "/triggers", "trigger", True)
    crud("crons", "/cron", "cron job", False)
    crud("webhooks", "/webhooks", "webhook", False, webhook_path_ok)

    # --- tests / debugger ------------------------------------------------------------------------------
    @mb.route("POST", "/triggers/test")
    def trigger_test(req):
        if fails(req):
            return boom()
        d = body(req)
        try:
            rx = re.compile(d.get("pattern", ""))
        except re.error as e:
            return 500, {"error": f"invalid regex: {e}"}
        m = rx.search(d.get("message", ""))
        if not m:
            return 200, {"matched": False}
        logs = [f'Simulating response for: {m.group(1) if m.groups() else "none"}']
        return 200, {"matched": True, "matches": [m.group(0), *m.groups()], "logs": logs,
                     "actions": [f"Action: send_text(to: 628123456789@s.whatsapp.net, msg: You said: {m.group(1) if m.groups() else ''})"]}

    @mb.route("POST", "/cron/test")
    def cron_test(req):
        return boom() if fails(req) else (200, {"status": "started"})

    @mb.route("POST", "/webhooks/test")
    def webhook_test(req):
        if fails(req):
            return boom()
        d = body(req)
        if "error(" in d.get("script", ""):
            return 500, {"error": "runtime error: boom"}
        return 200, {"status": "200", "body": "ok"}

    # --- webhook request log (registered after the id routes so /webhooks/logs wins) ---------------------
    @mb.route("GET", "/webhooks/logs")
    def logs(req):
        if fails(req):
            return boom()
        wid = req.q("webhook_id") or ""
        limit, offset = int(req.q("limit", "50")), int(req.q("offset", "0"))
        rows = [lg for lg in st["logs"] if not wid or lg["webhook_id"] == wid]
        return 200, {"logs": rows[offset:offset + limit], "total": len(rows), "limit": limit, "offset": offset}

    @mb.route("DELETE", "/webhooks/logs")
    def clear_logs(req):
        if fails(req):
            return boom()
        st["logs"].clear()
        return None

    # --- docs & assistant -------------------------------------------------------------------------------------
    @mb.route("GET", "/docs")
    def docs(req):
        if fails(req):
            return boom()
        text = DOCS_FILE.read_text() if DOCS_FILE.exists() else FALLBACK_DOCS
        return 200, text.encode(), "text/markdown"

    @mb.route("POST", "/ai/assistant")
    def assistant(req):
        if fails(req):
            return 503, {"error": "AI Service not available"}
        d = body(req)
        st.setdefault("assistant_calls", []).append(d)
        code = 'local who = matches[1] or "world"\nsend_text(sender, "Hello, " .. who .. "!")'
        return 200, {"answer": f"Here is the updated script:\n\n```lua\n{code}\n```\n\nIt greets the **captured name** (model `{d.get('model')}`)."}
