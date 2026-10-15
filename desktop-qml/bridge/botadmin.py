"""`BotAdmin` context property: triggers, cron jobs, webhooks (+ request logs), the Lua API docs and the
AI assistant. Ported from the React pages in `web/src/pages/bot` (the GPUI app only has placeholders here).

    BotAdmin.triggers / .crons / .webhooks      BotCollection: model, loading, error, query, reload(), save(), ...
    BotAdmin.logs                               WebhookLogs: paged request log (filter by webhook, clear all)
    BotAdmin.docs                               Docs: GET /docs markdown -> blocks + table of contents
    BotAdmin.assistant                          Assistant: POST /ai/assistant chat (+ Lua code proposals)
    BotAdmin.testTrigger / testCron / testWebhook(...)   -> testFinished(kind, entries)   debugger console lines
    BotAdmin.webhookUrl(path), copyText(text), luaHighlighter(textDocument)

REST (see backend-client `client.rs`): /triggers /cron /webhooks (+ /test, DELETE all), /webhooks/logs, /docs,
/ai/assistant. All calls are asynchronous (`Api.fetch`); failures surface through `Toast`.
"""
from __future__ import annotations

import difflib
import json
import re
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot
from PySide6.QtGui import QColor, QFont, QGuiApplication, QSyntaxHighlighter, QTextCharFormat

from . import mdblocks
from .models import DictListModel

WEBHOOK_PATH_RE = re.compile(r"^[A-Za-z0-9._-]+$")
AI_MODELS = [
    {"value": "gemma-3-27b-it", "label": "Gemma 3 27B"},
    {"value": "gemma-4-31b-it", "label": "Gemma 4 31B"},
    {"value": "gemma-4-26b-a4b-it", "label": "Gemma 4 26B A4B"},
    {"value": "gemini-3.1-flash-lite-preview", "label": "Gemini 3.1 Flash Lite"},
]

DEFAULT_TRIGGER_SCRIPT = (
    '-- Available: sender, content, matches, send_text(), fetch()...\nlocal param = matches[1]\n'
    'print("Simulating response for: " .. (param or "none"))\nsend_text(sender, "You said: " .. (param or "nothing"))'
)
DEFAULT_CRON_SCRIPT = (
    '-- Scheduled Task\nprint("Executing sequence: " .. os.date())\n'
    'send_text("628123456789@s.whatsapp.net", "Automatic scheduled broadcast")'
)
DEFAULT_WEBHOOK_SCRIPT = (
    '-- Webhook Handler\n-- Available: req.method, req.path, req.body, req.raw_body, req.headers, req.query_params\n'
    'local nama = req.body and req.body.nama or "unknown"\n'
    'send_text("628123456789@s.whatsapp.net", "Webhook triggered by: " .. nama)\n\n'
    'response = { status = 200, body = "ok" }'
)


def default_trigger_payload() -> str:
    return json.dumps({"event": "message.received",
                       "message": {"from": "6281234567890", "body": "hello", "timestamp": int(time.time())}}, indent=2)


# ---------------------------------------------------------------------------------------------
# helpers (pure, unit tested)
# ---------------------------------------------------------------------------------------------
def parse_time(value: Any) -> float | None:
    """ISO-8601 string (Go RFC3339, sqlite `YYYY-MM-DD HH:MM:SS`) or unix seconds/ms -> epoch seconds."""
    if value in (None, ""):
        return None
    if isinstance(value, (int, float)):
        return value / 1000 if value > 1e11 else float(value)
    s = str(value).strip().replace(" ", "T", 1)
    s = re.sub(r"(\.\d{6})\d+", r"\1", s)
    try:
        d = datetime.fromisoformat(s)
    except ValueError:
        return None
    if d.tzinfo is None:
        d = d.replace(tzinfo=timezone.utc)
    return d.timestamp()


def relative_time(value: Any, now: float | None = None) -> str:
    """List column "Last Edited": same buckets as the web `relativeTime`."""
    ts = parse_time(value)
    if ts is None:
        return "—"
    seconds = int((now if now is not None else time.time()) - ts)
    if seconds < 5:
        return "just now"
    if seconds < 60:
        return f"{seconds}s ago"
    minutes = seconds // 60
    if minutes < 60:
        return f"{minutes}m ago"
    hours = minutes // 60
    if hours < 24:
        return f"{hours}h ago"
    days = hours // 24
    if days < 7:
        return f"{days}d ago"
    if days < 30:
        return f"{days // 7}w ago"
    if days < 365:
        return f"{days // 30}mo ago"
    return f"{days // 365}y ago"


def time_ago_long(value: Any, now: float | None = None) -> str:
    """Editor subtitle "Last edited 3 minutes ago" (web `formatTimeAgo`)."""
    ts = parse_time(value)
    if ts is None:
        return ""
    sec = int((now if now is not None else time.time()) - ts)
    mins, hours = sec // 60, sec // 3600
    days = hours // 24
    weeks, months = days // 7, days // 30
    if sec < 60:
        return "just now"
    plural = lambda n, w: f"{n} {w}{'s' if n > 1 else ''} ago"  # noqa: E731
    if mins < 60:
        return plural(mins, "minute")
    if hours < 24:
        return plural(hours, "hour")
    if days < 7:
        return plural(days, "day")
    if weeks < 5:
        return plural(weeks, "week")
    return plural(months, "month")


def error_text(status: int, body: Any, fallback: str = "") -> str:
    """Server message for a failed call: `{"error": ...}` / `{"message": ...}` / plain text / status."""
    if isinstance(body, dict):
        for k in ("error", "message"):
            if isinstance(body.get(k), str) and body[k]:
                return body[k]
    if isinstance(body, str) and body.strip():
        return body.strip()[:300]
    if status == 0:
        return fallback or "Backend not reachable"
    return fallback or f"HTTP {status}"


def trigger_message(payload_json: str) -> tuple[str, str]:
    """Pick the text the test sends out of the simulated payload (web `parseJsonMessage`):
    message.body ?? body ?? message. Returns (message, error)."""
    try:
        p = json.loads(payload_json)
    except ValueError:
        return "", "Invalid JSON"
    msg = p.get("message") if isinstance(p, dict) else None
    body = None
    if isinstance(msg, dict) and msg.get("body") is not None:
        body = msg["body"]
    elif isinstance(p, dict) and p.get("body") is not None:
        body = p["body"]
    elif msg is not None:
        body = msg
    return ("" if body is None else str(body)), ""


def pretty_json(text: str) -> str:
    try:
        return json.dumps(json.loads(text), indent=2, ensure_ascii=False)
    except (TypeError, ValueError):
        return text or ""


def trigger_test_entries(pattern: str, status: int, body: Any) -> list[dict]:
    """Debugger console lines for POST /triggers/test (web `TriggerEditorPage.handleTest`)."""
    out = [{"type": "invoke", "message": f"Starting execution for pattern: {pattern}"}]
    if status < 200 or status >= 300:
        return out + [{"type": "error", "message": error_text(status, body, "Execution failed")}]
    r = body if isinstance(body, dict) else {}
    out += [{"type": "print", "message": str(log)} for log in (r.get("logs") or [])]
    out += [{"type": "print", "message": str(a)} for a in (r.get("actions") or [])]
    if r.get("error"):
        out.append({"type": "error", "message": str(r["error"])})
    if r.get("matched"):
        out.append({"type": "success", "message": "Execution finished. Pattern matched. Returns: true"})
        if r.get("matches"):
            out.append({"type": "info", "message": "Captured groups: [" + ", ".join(str(m) for m in r["matches"]) + "]"})
    else:
        out.append({"type": "error", "message": "No match — pattern did not match the message."})
    return out


def cron_test_entries(status: int, body: Any) -> list[dict]:
    out = [{"type": "invoke", "message": "Starting cron simulation..."}]
    if status < 200 or status >= 300:
        return out + [{"type": "error", "message": error_text(status, body, "Execution failed")}]
    r = body if isinstance(body, dict) else {}
    out += [{"type": "print", "message": str(log)} for log in (r.get("logs") or [])]
    if r.get("error"):
        out.append({"type": "error", "message": str(r["error"])})
    else:
        out.append({"type": "success", "message": "Execution finished successfully."})
    return out


def webhook_test_entries(path: str, method: str, status: int, body: Any) -> list[dict]:
    out = [{"type": "invoke", "message": f"Starting execution for path: /webhook/{path} ({method})"}]
    r = body if isinstance(body, dict) else {}
    out += [{"type": "print", "message": str(log)} for log in (r.get("logs") or [])]
    if status == 0 or r.get("error"):
        return out + [{"type": "error", "message": error_text(status, body, "Execution failed")}]
    # A script may legitimately answer with a non-2xx `response.status`; the web page then shows the failure.
    if status >= 400:
        out.append({"type": "error", "message": f"HTTP {status}"})
    else:
        out.append({"type": "success", "message": "Execution finished successfully."})
    if body is not None and body != "":
        out.append({"type": "print", "message": "Response: " + json.dumps(body, indent=2, ensure_ascii=False)})
    return out


def diff_rows(old: str, new: str) -> list[dict]:
    """Line diff for the AI-suggestion preview: rows {kind: ctx|add|del, text, oldNo, newNo}."""
    a, b = old.split("\n"), new.split("\n")
    rows: list[dict] = []
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(a=a, b=b, autojunk=False).get_opcodes():
        if tag == "equal":
            rows += [{"kind": "ctx", "text": a[i], "oldNo": i + 1, "newNo": j1 + (i - i1) + 1} for i in range(i1, i2)]
        else:
            rows += [{"kind": "del", "text": a[i], "oldNo": i + 1, "newNo": 0} for i in range(i1, i2)]
            rows += [{"kind": "add", "text": b[j], "oldNo": 0, "newNo": j + 1} for j in range(j1, j2)]
    return rows


# ---------------------------------------------------------------------------------------------
# collections: triggers / cron jobs / webhooks
# ---------------------------------------------------------------------------------------------
class Kind:
    def __init__(self, key, path, noun, plural, required, search, roles, export_name, msgs, import_style):
        self.key, self.path, self.noun, self.plural = key, path, noun, plural
        self.required, self.search, self.roles = required, search, roles
        self.export_name, self.msgs, self.import_style = export_name, msgs, import_style

    @property
    def Noun(self) -> str:
        return self.noun[0].upper() + self.noun[1:]


_BASE_ROLES = ["id", "name", "description", "is_active", "script", "created_at", "updated_at",
               "descShort", "updatedAgo", "updatedTs"]

TRIGGERS = Kind(
    "triggers", "/triggers", "trigger", "triggers", ("name", "pattern", "script"), ("name", "pattern"),
    _BASE_ROLES + ["pattern", "priority", "mainText"], "wa-bot-triggers",
    dict(load="Failed to load triggers", save_ok="Trigger {verb} successfully", save_fail="Failed to save trigger",
         del_ok="Trigger deleted", del_fail="Failed to delete trigger", all_ok="All triggers deleted",
         all_fail="Failed to delete all triggers", export_none="No triggers to export",
         export_ok="Triggers exported successfully", import_fail="Failed to parse import file"), "triggers")
CRONS = Kind(
    "crons", "/cron", "cron job", "cron jobs", ("name", "schedule", "script"), ("name", "schedule"),
    _BASE_ROLES + ["schedule", "mainText"], "wa-bot-cron-jobs",
    dict(load="Failed to load cron jobs", save_ok="Cron job {verb} successfully", save_fail="Failed to save cron job",
         del_ok="Cron job deleted", del_fail="Failed to delete cron job", all_ok="All scheduled jobs purged",
         all_fail="Failed to purge jobs", export_none="No jobs to export", export_ok="Export successful",
         import_fail="Import failed"), "plain")
WEBHOOKS = Kind(
    "webhooks", "/webhooks", "webhook", "webhooks", ("name", "path", "script"), ("name", "path"),
    _BASE_ROLES + ["path", "secret", "mainText"], "wa-bot-webhooks",
    dict(load="Failed to load webhooks", save_ok="Webhook {verb} successfully", save_fail="Failed to save webhook",
         del_ok="Webhook deleted", del_fail="Failed to delete webhook", all_ok="All webhooks purged",
         all_fail="Failed to purge webhooks", export_none="No webhooks to export", export_ok="Export successful",
         import_fail="Import failed"), "plain")


class BotCollection(QObject):
    """One list screen's store (triggers, cron jobs or webhooks) with web-app semantics:
    load on entry, optimistic enable toggles (reverted on failure), reload after writes."""

    stateChanged = Signal()
    # save: ok, id, message — QML leaves the editor on success
    saveFinished = Signal(bool, str, str)
    deleteFinished = Signal(bool, str)
    # ensure(id): the item is available (cache or fetched) / does not exist
    itemReady = Signal(str)
    itemMissing = Signal(str)
    importFinished = Signal(int, int)

    def __init__(self, kind: Kind, services, parent: QObject | None = None):
        super().__init__(parent)
        self.kind = kind
        self._svc = services
        self._items: list[dict] = []
        self._query = ""
        self._loading = False
        self._saving = False
        self._error = ""
        self._loaded = False
        self._gen = 0
        self._pending_ensure: list[str] = []
        self._model = DictListModel(kind.roles, key="id", parent=self)
        self._ago = QTimer(self)
        self._ago.setInterval(30_000)
        self._ago.timeout.connect(self._refresh_ago)
        self._ago.start()

    # -- properties ---------------------------------------------------------------------
    @Property(QObject, constant=True)
    def model(self) -> QObject:
        return self._model

    @Property(bool, notify=stateChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(bool, notify=stateChanged)
    def saving(self) -> bool:
        return self._saving

    @Property(bool, notify=stateChanged)
    def loaded(self) -> bool:
        return self._loaded

    @Property(str, notify=stateChanged)
    def error(self) -> str:
        return self._error

    @Property(str, notify=stateChanged)
    def query(self) -> str:
        return self._query

    @Property(int, notify=stateChanged)
    def total(self) -> int:
        return len(self._items)

    @Property(int, notify=stateChanged)
    def shown(self) -> int:
        return self._model.count

    # -- rows -----------------------------------------------------------------------------
    def _row(self, item: dict) -> dict:
        k = self.kind
        row = {r: item.get(r) for r in k.roles}
        row["is_active"] = bool(item.get("is_active"))
        desc = item.get("description") or ""
        row["description"] = desc
        row["descShort"] = desc if len(desc) <= 80 else desc[:80] + "…"
        row["updatedTs"] = parse_time(item.get("updated_at")) or 0
        row["updatedAgo"] = relative_time(item.get("updated_at"))
        if k.key == "webhooks":
            row["mainText"] = f"/webhook/{item.get('path', '')}"
        else:
            row["mainText"] = str(item.get("pattern" if k.key == "triggers" else "schedule", ""))
        return row

    def _matches(self, item: dict) -> bool:
        q = self._query.strip().lower()
        return not q or any(q in str(item.get(f) or "").lower() for f in self.kind.search)

    def _rebuild(self) -> None:
        self._model.reset([self._row(i) for i in self._items if self._matches(i)])
        self.stateChanged.emit()

    def _refresh_ago(self) -> None:
        for r in list(self._model.rows()):
            self._model.patch(r["id"], {"updatedAgo": relative_time(r.get("updated_at"))})

    def items(self) -> list[dict]:
        return self._items

    # -- queries ----------------------------------------------------------------------------
    @Slot(str)
    def setQuery(self, q: str) -> None:
        if q != self._query:
            self._query = q
            self._rebuild()

    @Slot(str, result="QVariantMap")
    def get(self, item_id: str) -> dict:
        return next((dict(i) for i in self._items if i.get("id") == item_id), {})

    @Slot(str, result=bool)
    def has(self, item_id: str) -> bool:
        return any(i.get("id") == item_id for i in self._items)

    # -- loading ------------------------------------------------------------------------------
    @Slot()
    def reload(self) -> None:
        self._gen += 1
        gen = self._gen
        self._loading = True
        self._error = ""
        self.stateChanged.emit()

        def done(status: int, body: Any) -> None:
            if gen != self._gen:
                return
            self._loading = False
            if 200 <= status < 300 and (isinstance(body, list) or body is None):
                self._items = [i for i in (body or []) if isinstance(i, dict)]
                self._loaded = True
                self._error = ""
                self._rebuild()
            else:
                self._error = error_text(status, body)
                self._svc.toast.error(self.kind.msgs["load"])
                self.stateChanged.emit()
            pend, self._pending_ensure = self._pending_ensure, []
            for i in pend:
                (self.itemReady if self.has(i) else self.itemMissing).emit(i)

        self._svc.api.fetch("GET", self.kind.path, None, done)

    @Slot(str)
    def ensure(self, item_id: str) -> None:
        """Editor entry: use the cached item or load the list once (there is no GET /<id> endpoint)."""
        if self.has(item_id):
            self.itemReady.emit(item_id)
            return
        self._pending_ensure.append(item_id)
        if not self._loading:
            self.reload()

    # -- writes -----------------------------------------------------------------------------------
    @Slot("QVariantMap", result=str)
    def validate(self, data: dict) -> str:
        """"" when the form may be saved, else the message (web: "Please fill in all required fields")."""
        for f in self.kind.required:
            if not str(data.get(f) or "").strip():
                return "Please fill in all required fields"
        if self.kind.key == "webhooks" and not WEBHOOK_PATH_RE.match(str(data.get("path"))):
            return "path must only contain alphanumeric characters, dots, hyphens, and underscores"
        return ""

    def _payload(self, data: dict) -> dict:
        d = {k: v for k, v in dict(data).items() if v is not None}
        if self.kind.key == "triggers":
            try:
                d["priority"] = int(d.get("priority") or 0)
            except (TypeError, ValueError):
                d["priority"] = 0
        d.setdefault("is_active", True)
        d["is_active"] = bool(d["is_active"])
        return d

    @Slot("QVariantMap")
    def save(self, data: dict) -> None:
        """Create (no id) or update (id) — mirrors the editors' `handleSave`."""
        k = self.kind
        msg = self.validate(data)
        if msg:
            self._svc.toast.error(msg)
            self.saveFinished.emit(False, "", msg)
            return
        item_id = str(data.get("id") or "")
        body = self._payload({**self.get(item_id), **data} if item_id else data)
        if not item_id:
            body.pop("id", None)
            body.pop("created_at", None)
            body.pop("updated_at", None)
        self._saving = True
        self.stateChanged.emit()

        def done(status: int, resp: Any) -> None:
            self._saving = False
            self.stateChanged.emit()
            if 200 <= status < 300:
                rid = str((resp or {}).get("id") or item_id) if isinstance(resp, dict) else item_id
                if isinstance(resp, dict) and resp.get("id"):
                    self._upsert(resp)
                self._svc.toast.success(k.msgs["save_ok"].format(verb="updated" if item_id else "created"))
                self.saveFinished.emit(True, rid, "")
                self.reload()
            else:
                detail = error_text(status, resp)
                self._svc.toast.error(f"{k.msgs['save_fail']}: {detail}" if status else k.msgs["save_fail"])
                self.saveFinished.emit(False, item_id, detail)

        if item_id:
            self._svc.api.fetch("PUT", f"{k.path}/{self._svc.api.enc(item_id)}", body, done)
        else:
            self._svc.api.fetch("POST", k.path, body, done)

    def _upsert(self, item: dict) -> None:
        for n, i in enumerate(self._items):
            if i.get("id") == item.get("id"):
                self._items[n] = {**i, **item}
                break
        else:
            self._items.append(item)
        self._rebuild()

    @Slot(str)
    def toggle(self, item_id: str) -> None:
        """Flip `is_active`: optimistic, reverted (with a toast) when the server refuses."""
        cur = next((i for i in self._items if i.get("id") == item_id), None)
        if cur is None:
            return
        new = not bool(cur.get("is_active"))
        body = {**cur, "is_active": new}
        self._set_active(item_id, new)

        def done(status: int, resp: Any) -> None:
            if not 200 <= status < 300:
                self._set_active(item_id, not new)
                self._svc.toast.error("Failed to update status")

        self._svc.api.fetch("PUT", f"{self.kind.path}/{self._svc.api.enc(item_id)}", body, done)

    def _set_active(self, item_id: str, value: bool) -> None:
        for i in self._items:
            if i.get("id") == item_id:
                i["is_active"] = value
        self._model.patch(item_id, {"is_active": value})

    @Slot(str)
    def remove(self, item_id: str) -> None:
        k = self.kind

        def done(status: int, resp: Any) -> None:
            if 200 <= status < 300:
                self._items = [i for i in self._items if i.get("id") != item_id]
                self._model.remove(item_id)
                self.stateChanged.emit()
                self._svc.toast.success(k.msgs["del_ok"])
                self.deleteFinished.emit(True, item_id)
                self.reload()
            else:
                self._svc.toast.error(k.msgs["del_fail"])
                self.deleteFinished.emit(False, item_id)

        self._svc.api.fetch("DELETE", f"{k.path}/{self._svc.api.enc(item_id)}", None, done)

    @Slot()
    def removeAll(self) -> None:
        k = self.kind
        self._loading = True
        self.stateChanged.emit()

        def done(status: int, resp: Any) -> None:
            self._loading = False
            if 200 <= status < 300:
                self._items = []
                self._rebuild()
                self._svc.toast.success(k.msgs["all_ok"])
                self.deleteFinished.emit(True, "")
                self.reload()
            else:
                self.stateChanged.emit()
                self._svc.toast.error(k.msgs["all_fail"])
                self.deleteFinished.emit(False, "")

        self._svc.api.fetch("DELETE", k.path, None, done)

    # -- cron: "Run Now" (the web page calls POST /cron/test with the job's script) ---------------------
    @Slot(str)
    def runNow(self, item_id: str) -> None:
        job = self.get(item_id)
        if not job:
            return

        def done(status: int, resp: Any) -> None:
            if 200 <= status < 300:
                self._svc.toast.success(f"Job '{job.get('name')}' started manually")
            else:
                self._svc.toast.error("Manual execution failed")

        self._svc.api.fetch("POST", "/cron/test", {"script": job.get("script") or ""}, done)

    # -- import / export ------------------------------------------------------------------------------
    @Slot(result=str)
    def exportName(self) -> str:
        return f"{self.kind.export_name}-{datetime.now().strftime('%Y-%m-%d')}.json"

    @Slot(str, result=bool)
    def exportJson(self, path: str) -> bool:
        k = self.kind
        if not self._items:
            self._svc.toast.error(k.msgs["export_none"])
            return False
        path = path[7:] if path.startswith("file://") else path
        try:
            Path(path).write_text(json.dumps(self._items, indent=2, ensure_ascii=False), encoding="utf-8")
        except OSError as e:
            self._svc.toast.error(f"Export failed: {e.strerror or e}")
            return False
        self._svc.toast.success(k.msgs["export_ok"])
        return True

    @Slot(str)
    def importJson(self, path: str) -> None:
        """Create every entry of a previously exported file (ids/timestamps dropped), one after the other."""
        k = self.kind
        path = path[7:] if path.startswith("file://") else path
        try:
            data = json.loads(Path(path).read_text(encoding="utf-8"))
            if not isinstance(data, list):
                raise ValueError("Invalid format: Expected an array")
        except (OSError, ValueError):
            self._svc.toast.error(k.msgs["import_fail"])
            return
        queue = [d for d in data if isinstance(d, dict)]
        self._loading = True
        self.stateChanged.emit()
        ok = fail = 0

        def step() -> None:
            nonlocal ok, fail
            if not queue:
                self._loading = False
                if k.import_style == "triggers":
                    self._svc.toast.success(f"Imported {ok} triggers successfully" + (f". {fail} failed." if fail else ""))
                else:
                    self._svc.toast.success(f"Imported {ok} {k.plural}")
                self.importFinished.emit(ok, fail)
                self.reload()
                return
            item = {k2: v for k2, v in queue.pop(0).items() if k2 not in ("id", "created_at", "updated_at")}

            def done(status: int, resp: Any) -> None:
                nonlocal ok, fail
                if 200 <= status < 300:
                    ok += 1
                else:
                    fail += 1
                step()

            self._svc.api.fetch("POST", k.path, item, done)

        step()


# ---------------------------------------------------------------------------------------------
# webhook request log
# ---------------------------------------------------------------------------------------------
class WebhookLogs(QObject):
    stateChanged = Signal()
    PAGE = 50
    ROLES = ["id", "webhook_id", "webhook_path", "source_ip", "method", "status_code", "created_at",
             "timeText", "headersPretty", "bodyPretty", "query_params", "hasBody"]

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._model = DictListModel(self.ROLES, key="id", parent=self)
        self._webhook_id = ""
        self._total = 0
        self._loading = False
        self._more = False
        self._error = ""
        self._gen = 0

    @Property(QObject, constant=True)
    def model(self) -> QObject:
        return self._model

    @Property(bool, notify=stateChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(bool, notify=stateChanged)
    def loadingMore(self) -> bool:
        return self._more

    @Property(str, notify=stateChanged)
    def error(self) -> str:
        return self._error

    @Property(int, notify=stateChanged)
    def total(self) -> int:
        return self._total

    @Property(bool, notify=stateChanged)
    def hasMore(self) -> bool:
        return self._model.count < self._total

    @Property(str, notify=stateChanged)
    def webhookId(self) -> str:
        return self._webhook_id

    @staticmethod
    def _row(log: dict) -> dict:
        ts = parse_time(log.get("created_at")) or 0
        body = log.get("body") or ""
        return {
            "id": log.get("id"), "webhook_id": log.get("webhook_id"), "webhook_path": log.get("webhook_path"),
            "source_ip": log.get("source_ip"), "method": (log.get("method") or "").upper(),
            "status_code": int(log.get("status_code") or 0), "created_at": log.get("created_at"),
            "timeText": datetime.fromtimestamp(ts).strftime("%d %b %Y, %H:%M:%S") if ts else "",
            "headersPretty": pretty_json(log.get("headers") or ""), "bodyPretty": pretty_json(body),
            "query_params": log.get("query_params") or "", "hasBody": bool(body),
        }

    @Slot()
    def reload(self) -> None:
        self._fetch(0, False)

    @Slot()
    def loadMore(self) -> None:
        if not self._loading and not self._more and self.hasMore:
            self._fetch(self._model.count, True)

    @Slot(str)
    def setWebhook(self, webhook_id: str) -> None:
        if webhook_id != self._webhook_id:
            self._webhook_id = webhook_id
            self.stateChanged.emit()
            self.reload()

    def _fetch(self, offset: int, append: bool) -> None:
        self._gen += 1
        gen = self._gen
        if append:
            self._more = True
        else:
            self._loading = True
            self._error = ""
        self.stateChanged.emit()
        q = f"?limit={self.PAGE}&offset={offset}"
        if self._webhook_id:
            q += f"&webhook_id={self._svc.api.enc(self._webhook_id)}"

        def done(status: int, body: Any) -> None:
            if gen != self._gen:
                return
            self._loading = self._more = False
            if 200 <= status < 300 and isinstance(body, dict):
                rows = [self._row(x) for x in (body.get("logs") or []) if isinstance(x, dict)]
                self._total = int(body.get("total") or 0)
                if append:
                    self._model.extend(rows)
                else:
                    self._model.reset(rows)
            else:
                self._error = error_text(status, body)
                self._svc.toast.error("Failed to load logs")
            self.stateChanged.emit()

        self._svc.api.fetch("GET", "/webhooks/logs" + q, None, done)

    @Slot()
    def clearAll(self) -> None:
        def done(status: int, body: Any) -> None:
            if 200 <= status < 300:
                self._model.clear()
                self._total = 0
                self.stateChanged.emit()
                self._svc.toast.success("All logs cleared")
            else:
                self._svc.toast.error("Failed to clear logs")

        self._svc.api.fetch("DELETE", "/webhooks/logs", None, done)


# ---------------------------------------------------------------------------------------------
# documentation
# ---------------------------------------------------------------------------------------------
class Docs(QObject):
    stateChanged = Signal()

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._markdown = ""
        self._blocks: list[dict] = []
        self._toc: list[dict] = []
        self._loading = False
        self._error = ""

    @Property(bool, notify=stateChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(str, notify=stateChanged)
    def error(self) -> str:
        return self._error

    @Property(str, notify=stateChanged)
    def markdown(self) -> str:
        return self._markdown

    @Property("QVariantList", notify=stateChanged)
    def blocks(self) -> list:
        return self._blocks

    @Property("QVariantList", notify=stateChanged)
    def toc(self) -> list:
        return self._toc

    @Slot(str, result=int)
    def indexOfSlug(self, slug: str) -> int:
        return next((i for i, b in enumerate(self._blocks) if b["type"] == "heading" and b["slug"] == slug.lstrip("#")), -1)

    @Slot()
    def reload(self) -> None:
        self._loading = True
        self._error = ""
        self.stateChanged.emit()

        def done(status: int, body: Any) -> None:
            self._loading = False
            if 200 <= status < 300 and body is not None:
                self._markdown = body if isinstance(body, str) else json.dumps(body, indent=2)
                self._blocks = mdblocks.parse(self._markdown)
                self._toc = mdblocks.headings(self._blocks)
            else:
                self._error = error_text(status, body, "Failed to load documentation file.")
            self.stateChanged.emit()

        self._svc.api.fetch("GET", "/docs", None, done)

    @Slot()
    def ensure(self) -> None:
        if not self._blocks and not self._loading:
            self.reload()


# ---------------------------------------------------------------------------------------------
# AI assistant
# ---------------------------------------------------------------------------------------------
class Assistant(QObject):
    stateChanged = Signal()
    codeProposed = Signal(str)  # Lua extracted from a reply: the editor shows it as a diff to accept

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._model = DictListModel(["id", "role", "content", "blocks"], key="id", parent=self)
        self._seq = 0
        self._epoch = 0
        self._loading = False
        self._llm = AI_MODELS[0]["value"]

    @Property(QObject, constant=True)
    def model(self) -> QObject:
        return self._model

    @Property(bool, notify=stateChanged)
    def loading(self) -> bool:
        return self._loading

    @Property("QVariantList", constant=True)
    def models(self) -> list:
        return AI_MODELS

    @Property(str, notify=stateChanged)
    def selectedModel(self) -> str:
        return self._llm

    @selectedModel.setter
    def selectedModel(self, v: str) -> None:
        if v != self._llm:
            self._llm = v
            self.stateChanged.emit()

    @Slot()
    def reset(self) -> None:
        """A fresh chat per editor, like the web component unmounting with its page."""
        self._epoch += 1  # drops an answer still in flight
        self._model.clear()
        self._loading = False
        self.stateChanged.emit()

    def _add(self, role: str, content: str) -> None:
        self._seq += 1
        self._model.append({"id": self._seq, "role": role, "content": content,
                            "blocks": mdblocks.parse(content) if role == "assistant" else []})

    @Slot(str, str, result=bool)
    def send(self, prompt: str, current_code: str = "") -> bool:
        prompt = prompt.strip()
        if not prompt or self._loading:
            return False
        self._add("user", prompt)
        self._loading = True
        self.stateChanged.emit()
        epoch = self._epoch
        body: dict[str, Any] = {"prompt": prompt, "model": self._llm}
        if current_code:
            body["currentCode"] = current_code

        def done(status: int, resp: Any) -> None:
            if epoch != self._epoch:
                return  # reset() while waiting
            self._loading = False
            if 200 <= status < 300 and isinstance(resp, dict) and isinstance(resp.get("answer"), str):
                answer = resp["answer"]
                self._add("assistant", answer)
                m = re.search(r"```lua\n([\s\S]*?)```", answer)
                if m and m.group(1).strip():
                    self.codeProposed.emit(m.group(1).strip())
            else:
                detail = error_text(status, resp)
                self._add("assistant", "Sorry, an error occurred while connecting to the AI assistant."
                          + (f"\n\n> {detail}" if status else ""))
            self.stateChanged.emit()

        self._svc.api.fetch("POST", "/ai/assistant", body, done)
        return True


# ---------------------------------------------------------------------------------------------
# Lua syntax highlighting inside TextEdit documents
# ---------------------------------------------------------------------------------------------
class LuaHighlighter(QSyntaxHighlighter):
    COLORS = {"keyword": "#569cd6", "string": "#ce9178", "comment": "#6a9955", "number": "#b5cea8",
              "function": "#dcdcaa", "constant": "#569cd6"}

    def __init__(self, doc):
        super().__init__(doc)
        self._fmt = {}
        for kind, color in self.COLORS.items():
            f = QTextCharFormat()
            f.setForeground(QColor(color))
            if kind == "comment":
                f.setFontItalic(True)
            self._fmt[kind] = f

    def highlightBlock(self, text: str) -> None:
        prev = self.previousBlockState()
        state: Any = 0
        if prev >= 1:  # 1 + level*2 (+1 for strings)
            lvl, is_string = divmod(prev - 1, 2)
            state = ("s" if is_string else "c", lvl)
        spans, end = mdblocks.scan_lua_line(text, state)
        for a, b, kind in spans:
            self.setFormat(a, b - a, self._fmt[kind])
        if isinstance(end, tuple):
            self.setCurrentBlockState(1 + end[1] * 2 + (1 if end[0] == "s" else 0))
        else:
            self.setCurrentBlockState(0)


# ---------------------------------------------------------------------------------------------
# facade
# ---------------------------------------------------------------------------------------------
class BotAdmin(QObject):
    testFinished = Signal(str, "QVariantList")  # kind (trigger|cron|webhook), console entries

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._triggers = BotCollection(TRIGGERS, services, self)
        self._crons = BotCollection(CRONS, services, self)
        self._webhooks = BotCollection(WEBHOOKS, services, self)
        self._logs = WebhookLogs(services, self)
        self._docs = Docs(services, self)
        self._assistant = Assistant(services, self)
        self._highlighters: list[LuaHighlighter] = []

    triggers = Property(QObject, lambda s: s._triggers, constant=True)
    crons = Property(QObject, lambda s: s._crons, constant=True)
    webhooks = Property(QObject, lambda s: s._webhooks, constant=True)
    logs = Property(QObject, lambda s: s._logs, constant=True)
    docs = Property(QObject, lambda s: s._docs, constant=True)
    assistant = Property(QObject, lambda s: s._assistant, constant=True)

    # -- defaults for the "new" forms ------------------------------------------------------------------
    @Slot(str, result="QVariantMap")
    def newItem(self, kind: str) -> dict:
        if kind == "triggers":
            return {"name": "", "pattern": "", "script": DEFAULT_TRIGGER_SCRIPT, "priority": 0, "description": "", "is_active": True}
        if kind == "crons":
            return {"name": "", "schedule": "0 * * * *", "script": DEFAULT_CRON_SCRIPT, "description": "", "is_active": True}
        return {"name": "", "path": "", "script": DEFAULT_WEBHOOK_SCRIPT, "secret": "", "description": "", "is_active": True}

    @Slot(result=str)
    def defaultTriggerPayload(self) -> str:
        return default_trigger_payload()

    # -- debugger ----------------------------------------------------------------------------------------
    @Slot(str, str, str)
    def testTrigger(self, pattern: str, script: str, payload_json: str) -> None:
        message, err = trigger_message(payload_json)
        if err:
            self._svc.toast.error("Invalid JSON payload")
            self.testFinished.emit("trigger", [{"type": "error", "message": "Failed to parse JSON payload. Please check syntax."}])
            return
        if not pattern or not script or not message:
            self._svc.toast.error("Pattern, script, and test message are required")
            self.testFinished.emit("trigger", [])
            return
        self._svc.api.fetch("POST", "/triggers/test", {"pattern": pattern, "script": script, "message": message},
                            lambda st, body: self.testFinished.emit("trigger", trigger_test_entries(pattern, st, body)))

    @Slot(str)
    def testCron(self, script: str) -> None:
        self._svc.api.fetch("POST", "/cron/test", {"script": script or ""},
                            lambda st, body: self.testFinished.emit("cron", cron_test_entries(st, body)))

    @Slot(str, str, str, str)
    def testWebhook(self, path: str, script: str, method: str, body_text: str) -> None:
        try:
            json.loads(body_text)
        except ValueError:
            self._svc.toast.error("Invalid JSON payload")
            self.testFinished.emit("webhook", [{"type": "error", "message": "Invalid JSON payload. Please check syntax."}])
            return
        if not path or not script:
            self._svc.toast.error("Webhook path and script are required")
            self.testFinished.emit("webhook", [])
            return
        self._svc.api.fetch("POST", "/webhooks/test", {"path": path or "test", "script": script, "method": method, "body": body_text},
                            lambda st, body: self.testFinished.emit("webhook", webhook_test_entries(path, method, st, body)))

    # -- utilities for QML ---------------------------------------------------------------------------------
    @Slot(str, result=str)
    def webhookUrl(self, path: str) -> str:
        root = self._svc.backend.root_url() if hasattr(self._svc.backend, "root_url") else ""
        return f"{root}/webhook/{path}"

    @Slot(str)
    def copyText(self, text: str) -> None:
        cb = QGuiApplication.clipboard()
        if cb is not None:
            cb.setText(text)

    @Slot(str, result=str)
    def timeAgoLong(self, value: str) -> str:
        return time_ago_long(value)

    @Slot(str, str, result="QVariantList")
    def diff(self, old: str, new: str) -> list:
        return diff_rows(old, new)

    @Slot(str, result="QVariantList")
    def parseMarkdown(self, md: str) -> list:
        return mdblocks.parse(md)

    @Slot(str, result="QVariantMap")
    def jsonCheck(self, text: str) -> dict:
        """{ok, error} for the JSON payload editors."""
        try:
            json.loads(text)
            return {"ok": True, "error": ""}
        except ValueError as e:
            return {"ok": False, "error": str(e)}

    @Slot("QVariant")
    def luaHighlighter(self, quick_doc) -> None:
        """Attach Lua colouring to a TextEdit's `textDocument` (kept alive with the document)."""
        doc = quick_doc.textDocument() if quick_doc is not None else None
        if doc is None or any(h.document() is doc for h in self._highlighters):
            return
        self._highlighters.append(LuaHighlighter(doc))


def register(services) -> None:
    admin = BotAdmin(services)
    services.expose("BotAdmin", admin)
    services.botadmin = admin  # reachable from Python (tests, other bridges)
