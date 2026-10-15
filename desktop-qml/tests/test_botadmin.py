"""BotAdmin bridge against the mock backend: CRUD payloads, optimistic toggles, errors, tests/debugger, logs, docs,
assistant, markdown blocks. No QML involved (see scripts via tests/shot_bot.py for screenshots)."""
import json
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest
from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from bridge import botadmin as ba  # noqa: E402
from bridge import mdblocks  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402
from mock_backend import MockBackend  # noqa: E402


@pytest.fixture(scope="module")
def app():
    return QCoreApplication.instance() or QCoreApplication([])


def wait_for(cond, ms=5000):
    loop, t = QEventLoop(), QTimer()
    t.setInterval(10)
    t.timeout.connect(lambda: loop.quit() if cond() else None)
    QTimer.singleShot(ms, loop.quit)
    t.start()
    loop.exec()
    t.stop()
    return cond()


class Env:
    def __init__(self, mb):
        self.mb = mb

        class B:
            apiUrl = mb.url

            def root_url(self):
                return mb.url.removesuffix("/api")

        self.toasts = []
        self.bus = ToastBus()
        self.bus.shown.connect(lambda i, m, k: self.toasts.append((k, m)))
        self.api = Api(B())
        self.svc = SimpleNamespace(api=self.api, toast=self.bus, backend=B())
        self.admin = ba.BotAdmin(self.svc)

    def calls(self, method=None, prefix=""):
        return [(m, p.removeprefix("/api"), b) for m, p, b in self.mb.requests
                if (method is None or m == method) and p.removeprefix("/api").startswith(prefix)]

    def body(self, method, path):
        return json.loads(next(b for m, p, b in reversed(self.mb.requests) if m == method and p.removeprefix("/api") == path))


@pytest.fixture
def env(app):
    mb = MockBackend(areas=["common", "bot"]).start()
    e = Env(mb)
    yield e
    mb.stop()


@pytest.fixture
def empty_env(app):
    mb = MockBackend()
    mb.state["bot_empty"] = True
    import mock_bot
    mock_bot.install(mb)
    mb.start()
    e = Env(mb)
    yield e
    mb.stop()


def loaded(col):
    col.reload()
    assert wait_for(lambda: col.loaded and not col.loading)
    return col


# ---- helpers -----------------------------------------------------------------------------------------------
def test_relative_time_buckets():
    now = 1_700_000_000
    assert ba.relative_time(None) == "—"
    assert ba.relative_time(now - 2, now) == "just now"
    assert ba.relative_time(now - 30, now) == "30s ago"
    assert ba.relative_time(now - 120, now) == "2m ago"
    assert ba.relative_time(now - 3600 * 5, now) == "5h ago"
    assert ba.relative_time(now - 86400 * 3, now) == "3d ago"
    assert ba.relative_time(now - 86400 * 14, now) == "2w ago"
    assert ba.relative_time(now - 86400 * 70, now) == "2mo ago"
    assert ba.relative_time(now - 86400 * 800, now) == "2y ago"
    assert ba.time_ago_long(now - 60 * 3, now) == "3 minutes ago"
    assert ba.time_ago_long(now - 3600, now) == "1 hour ago"
    # Go RFC3339 with nanoseconds, sqlite style, unix seconds / millis
    assert ba.parse_time("2024-01-02T03:04:05.123456789Z") == pytest.approx(1704164645.123456)
    assert ba.parse_time("2024-01-02 03:04:05") == pytest.approx(1704164645)
    assert ba.parse_time(1704164645) == ba.parse_time(1704164645000)


def test_trigger_message_extraction():
    assert ba.trigger_message('{"message": {"body": "hi"}}') == ("hi", "")
    assert ba.trigger_message('{"body": "yo"}') == ("yo", "")
    assert ba.trigger_message('{"message": "plain"}') == ("plain", "")
    assert ba.trigger_message("{nope")[1] == "Invalid JSON"
    assert ba.trigger_message("{}") == ("", "")


def test_diff_rows():
    rows = ba.diff_rows("a\nb\nc", "a\nB\nc\nd")
    assert [(r["kind"], r["text"]) for r in rows] == [("ctx", "a"), ("del", "b"), ("add", "B"), ("ctx", "c"), ("add", "d")]


# ---- lists / search / toggle --------------------------------------------------------------------------------
def test_load_lists_and_search(env):
    t = loaded(env.admin.triggers)
    assert t.total == 3 and t.shown == 3 and not t.error
    m = t.model
    first = m.get(0)
    assert first["name"] == "Ping reply" and first["mainText"] == "^!ping$" and first["is_active"] is True
    assert first["updatedAgo"] in ("2m ago", "1m ago", "3m ago")
    assert m.get(1)["descShort"].endswith("…") and len(m.get(1)["descShort"]) == 81
    t.setQuery("CUACA")  # matches the pattern, case-insensitive
    assert t.shown == 1 and m.get(0)["id"] == "t2"
    t.setQuery("zzz")
    assert t.shown == 0 and t.total == 3
    t.setQuery("")
    assert t.shown == 3

    w = loaded(env.admin.webhooks)
    assert w.model.get(0)["mainText"] == "/webhook/github-deploy"
    w.setQuery("legacy")
    assert w.shown == 1
    c = loaded(env.admin.crons)
    assert c.model.get(1)["mainText"] == "*/5 * * * *"
    c.setQuery("weekly")
    assert c.shown == 1


def test_toggle_is_optimistic_and_sends_full_object(env):
    t = loaded(env.admin.triggers)
    t.toggle("t1")
    assert t.model.get(0)["is_active"] is False  # before the server answered
    assert wait_for(lambda: env.calls("PUT", "/triggers/t1"))
    body = env.body("PUT", "/triggers/t1")
    assert body["is_active"] is False and body["name"] == "Ping reply" and body["pattern"] == "^!ping$"
    assert wait_for(lambda: env.mb.state["bot"]["triggers"][0]["is_active"] is False)


def test_toggle_failure_reverts_and_toasts(env):
    env.mb.state["bot_fail"].add(("PUT", "/cron/c1"))
    c = loaded(env.admin.crons)
    c.toggle("c1")
    assert c.model.get(0)["is_active"] is False
    assert wait_for(lambda: ("error", "Failed to update status") in env.toasts)
    assert c.model.get(0)["is_active"] is True


def test_load_error_state(env):
    env.mb.state["bot_fail"].add(("GET", "/webhooks"))
    w = env.admin.webhooks
    w.reload()
    assert wait_for(lambda: w.error)
    assert w.error == "boom" and not w.loading and w.total == 0
    assert ("error", "Failed to load webhooks") in env.toasts
    env.mb.state["bot_fail"].clear()
    w.reload()
    assert wait_for(lambda: w.total == 3 and not w.error)


def test_empty_state(empty_env):
    t = loaded(empty_env.admin.triggers)
    assert t.total == 0 and t.shown == 0 and t.loaded


# ---- create / update / delete ---------------------------------------------------------------------------------
def test_create_trigger_payload(env):
    t = loaded(env.admin.triggers)
    done = []
    t.saveFinished.connect(lambda ok, i, m: done.append((ok, i, m)))
    data = env.admin.newItem("triggers")
    data.update(name="Echo", pattern=r"^!echo (.*)", priority="7", description="repeat")
    t.save(data)
    assert wait_for(lambda: done) and done[0][0] is True and done[0][1]
    sent = env.body("POST", "/triggers")
    assert sent["name"] == "Echo" and sent["pattern"] == "^!echo (.*)" and sent["priority"] == 7
    assert sent["is_active"] is True and sent["description"] == "repeat" and "id" not in sent
    assert ("success", "Trigger created successfully") in env.toasts
    assert wait_for(lambda: t.total == 4)  # reloaded after the write


def test_update_trigger_keeps_existing_fields(env):
    t = loaded(env.admin.triggers)
    done = []
    t.saveFinished.connect(lambda ok, i, m: done.append((ok, i)))
    row = t.get("t2")
    row["name"] = "Weather v2"
    t.save(row)
    assert wait_for(lambda: done) and done[0] == (True, "t2")
    sent = env.body("PUT", "/triggers/t2")
    assert sent["name"] == "Weather v2" and sent["id"] == "t2" and sent["priority"] == 5
    assert ("success", "Trigger updated successfully") in env.toasts


def test_validation_blocks_request(env):
    t = loaded(env.admin.triggers)
    done = []
    t.saveFinished.connect(lambda ok, i, m: done.append((ok, m)))
    t.save({"name": "x", "pattern": "", "script": "print(1)"})
    assert done == [(False, "Please fill in all required fields")]
    assert ("error", "Please fill in all required fields") in env.toasts
    assert not env.calls("POST", "/triggers")
    w = env.admin.webhooks
    assert w.validate({"name": "n", "path": "bad path", "script": "x"}).startswith("path must only")
    assert w.validate({"name": "n", "path": "ok-1.2_x", "script": "x"}) == ""
    assert env.admin.crons.validate({"name": "n", "schedule": "* * * * *", "script": ""}) != ""


def test_create_cron_and_webhook(env):
    c, w = loaded(env.admin.crons), loaded(env.admin.webhooks)
    res = []
    c.saveFinished.connect(lambda ok, i, m: res.append(("c", ok)))
    w.saveFinished.connect(lambda ok, i, m: res.append(("w", ok)))
    d = env.admin.newItem("crons")
    d.update(name="Nightly", schedule="0 0 * * *")
    c.save(d)
    d = env.admin.newItem("webhooks")
    d.update(name="Hook", path="my-hook", secret="abc")
    w.save(d)
    assert wait_for(lambda: len(res) == 2)
    assert env.body("POST", "/cron")["schedule"] == "0 0 * * *"
    hook = env.body("POST", "/webhooks")
    assert hook["path"] == "my-hook" and hook["secret"] == "abc" and hook["is_active"] is True
    assert ("success", "Cron job created successfully") in env.toasts
    assert ("success", "Webhook created successfully") in env.toasts


def test_save_failure_surfaces_server_message(env):
    w = loaded(env.admin.webhooks)
    res = []
    w.saveFinished.connect(lambda ok, i, m: res.append((ok, m)))
    env.mb.state["bot_fail"].add(("POST", "/webhooks"))
    d = env.admin.newItem("webhooks")
    d.update(name="Hook", path="x")
    w.save(d)
    assert wait_for(lambda: res)
    assert res[0] == (False, "boom") and w.saving is False
    assert ("error", "Failed to save webhook: boom") in env.toasts


def test_delete_one_and_all(env):
    t = loaded(env.admin.triggers)
    res = []
    t.deleteFinished.connect(lambda ok, i: res.append((ok, i)))
    t.remove("t3")
    assert wait_for(lambda: res)
    assert res[0] == (True, "t3") and not t.has("t3")
    assert env.calls("DELETE", "/triggers/t3")
    assert ("success", "Trigger deleted") in env.toasts
    c = loaded(env.admin.crons)
    c.removeAll()  # 204 No Content
    assert wait_for(lambda: c.total == 0 and not c.loading and env.calls("GET", "/cron")[-1])
    assert env.calls("DELETE", "/cron")[0][1] == "/cron"
    assert ("success", "All scheduled jobs purged") in env.toasts
    w = loaded(env.admin.webhooks)
    env.mb.state["bot_fail"].add(("DELETE", "/webhooks"))
    w.removeAll()
    assert wait_for(lambda: ("error", "Failed to purge webhooks") in env.toasts)
    assert w.total == 3 and not w.loading


def test_ensure_loads_when_cache_is_cold(env):
    ready, missing = [], []
    t = env.admin.triggers
    t.itemReady.connect(ready.append)
    t.itemMissing.connect(missing.append)
    t.ensure("t2")
    t.ensure("nope")
    assert wait_for(lambda: ready and missing)
    assert ready == ["t2"] and missing == ["nope"]
    n = len(env.calls("GET", "/triggers"))
    t.ensure("t1")  # cached: no new request
    assert ready == ["t2", "t1"] and len(env.calls("GET", "/triggers")) == n


def test_cron_run_now_posts_script(env):
    c = loaded(env.admin.crons)
    c.runNow("c1")
    assert wait_for(lambda: ("success", "Job 'Morning broadcast' started manually") in env.toasts)
    assert env.body("POST", "/cron/test") == {"script": 'print("tick")'}


# ---- import / export ----------------------------------------------------------------------------------------------
def test_export_and_import_roundtrip(env, tmp_path):
    t = loaded(env.admin.triggers)
    out = tmp_path / "t.json"
    assert t.exportJson(str(out))
    data = json.loads(out.read_text())
    assert [d["id"] for d in data] == ["t1", "t2", "t3"]
    assert ("success", "Triggers exported successfully") in env.toasts
    done = []
    t.importFinished.connect(lambda ok, fail: done.append((ok, fail)))
    t.importJson(str(out))
    assert wait_for(lambda: done) and done[0] == (3, 0)
    posts = [json.loads(b) for m, p, b in env.mb.requests if m == "POST" and p.endswith("/triggers")]
    assert len(posts) == 3 and all("id" not in p and "created_at" not in p for p in posts)
    assert ("success", "Imported 3 triggers successfully") in env.toasts
    assert wait_for(lambda: t.total == 6)
    bad = tmp_path / "bad.json"
    bad.write_text("{not json")
    t.importJson(str(bad))
    assert ("error", "Failed to parse import file") in env.toasts


def test_export_empty_toasts(empty_env):
    c = loaded(empty_env.admin.crons)
    assert c.exportJson("/nonexistent/x.json") is False
    assert ("error", "No jobs to export") in empty_env.toasts


# ---- debugger / tests ---------------------------------------------------------------------------------------------
def collect(admin):
    out = []
    admin.testFinished.connect(lambda k, e: out.append((k, e)))
    return out


def test_trigger_test_match_and_miss(env):
    out = collect(env.admin)
    env.admin.testTrigger(r"^!echo (.*)", "print(1)", json.dumps({"message": {"body": "!echo halo"}}))
    assert wait_for(lambda: out)
    assert env.body("POST", "/triggers/test") == {"pattern": r"^!echo (.*)", "script": "print(1)", "message": "!echo halo"}
    kind, entries = out[0]
    types = [e["type"] for e in entries]
    assert kind == "trigger" and types[0] == "invoke" and "success" in types
    assert {"type": "info", "message": "Captured groups: [!echo halo, halo]"} in entries
    assert {"type": "print", "message": "Simulating response for: halo"} in entries
    out.clear()
    env.admin.testTrigger(r"^!echo (.*)", "print(1)", json.dumps({"body": "nothing"}))
    assert wait_for(lambda: out)
    assert out[0][1][-1] == {"type": "error", "message": "No match — pattern did not match the message."}


def test_trigger_test_validation_and_errors(env):
    out = collect(env.admin)
    env.admin.testTrigger("x", "y", "{bad json")
    assert out[0][1][0]["message"].startswith("Failed to parse JSON payload")
    assert ("error", "Invalid JSON payload") in env.toasts
    env.admin.testTrigger("", "y", '{"body": "m"}')
    assert ("error", "Pattern, script, and test message are required") in env.toasts
    assert not env.calls("POST", "/triggers/test")
    out.clear()
    env.admin.testTrigger("(", "y", '{"body": "m"}')  # bad regex -> 500 {"error": ...}
    assert wait_for(lambda: out)
    assert out[0][1][-1]["type"] == "error" and "invalid regex" in out[0][1][-1]["message"]


def test_cron_and_webhook_tests(env):
    out = collect(env.admin)
    env.admin.testCron('print("x")')
    assert wait_for(lambda: out)
    assert out[0] == ("cron", [{"type": "invoke", "message": "Starting cron simulation..."},
                               {"type": "success", "message": "Execution finished successfully."}])
    assert env.body("POST", "/cron/test") == {"script": 'print("x")'}
    out.clear()
    env.admin.testWebhook("orders", "response={}", "PUT", '{"a": 1}')
    assert wait_for(lambda: out)
    assert env.body("POST", "/webhooks/test") == {"path": "orders", "script": "response={}", "method": "PUT", "body": '{"a": 1}'}
    kind, entries = out[0]
    assert kind == "webhook" and entries[0]["message"] == "Starting execution for path: /webhook/orders (PUT)"
    assert entries[1]["type"] == "success" and entries[2]["message"].startswith("Response: {")
    out.clear()
    env.admin.testWebhook("orders", "error('x')", "POST", "{}")
    assert wait_for(lambda: out)
    assert out[0][1][-1] == {"type": "error", "message": "runtime error: boom"}
    env.admin.testWebhook("orders", "x", "POST", "oops")
    assert ("error", "Invalid JSON payload") in env.toasts


# ---- webhook logs ----------------------------------------------------------------------------------------------------
def test_logs_paging_filter_clear(env):
    lg = env.admin.logs
    lg.reload()
    assert wait_for(lambda: lg.total == 5 and not lg.loading)
    assert lg.model.count == 5 and not lg.hasMore
    row = lg.model.get(0)
    assert row["method"] == "POST" and row["status_code"] == 200 and row["hasBody"]
    assert '"ref": "refs/heads/main"' in row["bodyPretty"]  # pretty printed
    assert '"User-Agent": "curl/8.5.0"' in row["headersPretty"]
    assert lg.model.get(2)["hasBody"] is False
    lg.setWebhook("w1")
    assert wait_for(lambda: lg.model.count == 2 and lg.total == 2)
    assert "webhook_id=w1" in env.mb.requests[-1][1]
    # paging uses limit/offset
    WL = ba.WebhookLogs
    old = WL.PAGE
    try:
        WL.PAGE = 1
        lg.setWebhook("")
        assert wait_for(lambda: lg.model.count == 1 and lg.total == 5)
        assert lg.hasMore
        lg.loadMore()
        assert wait_for(lambda: lg.model.count == 2)
        assert "limit=1&offset=1" in env.mb.requests[-1][1]
    finally:
        WL.PAGE = old
    lg.clearAll()
    assert wait_for(lambda: lg.model.count == 0 and lg.total == 0)
    assert ("success", "All logs cleared") in env.toasts
    assert env.calls("DELETE", "/webhooks/logs")


def test_logs_error(env):
    env.mb.state["bot_fail"].add(("GET", "/webhooks/logs"))
    env.admin.logs.reload()
    assert wait_for(lambda: env.admin.logs.error)
    assert ("error", "Failed to load logs") in env.toasts


# ---- docs ----------------------------------------------------------------------------------------------------------------
def test_docs_load_and_toc(env):
    d = env.admin.docs
    d.ensure()
    assert wait_for(lambda: d.blocks)
    assert d.markdown.startswith("# Bot Lua API Documentation")
    labels = [t["label"] for t in d.toc]
    assert "Global Variables" in labels and "Messaging Functions" in labels
    assert d.indexOfSlug("global-variables") > 0 and d.indexOfSlug("#global-variables") > 0
    assert d.indexOfSlug("nope") == -1
    n = len(env.calls("GET", "/docs"))
    d.ensure()
    assert len(env.calls("GET", "/docs")) == n  # cached


def test_docs_error(env):
    env.mb.state["bot_fail"].add(("GET", "/docs"))
    d = env.admin.docs
    d.reload()
    assert wait_for(lambda: d.error)
    assert d.error == "boom" and not d.blocks


# ---- markdown ---------------------------------------------------------------------------------------------------------------
def test_markdown_blocks():
    md = ("# Title\n\nSome **bold** and `code` text with [a link](https://x.y/z).\n\n- one\n- two\n  - nested\n\n"
          "1. first\n2. second\n\n| A | B |\n|:--|--:|\n| 1 | 2 |\n| `x|y` | z |\n\n> quote\n\n---\n\n"
          "```lua\n-- hi\nlocal x = 'a'\nprint(x)\n```\n\n## Sec Two\n")
    b = mdblocks.parse(md)
    assert [x["type"] for x in b] == ["heading", "paragraph", "list", "list", "table", "quote", "hr", "code", "heading"]
    assert b[0]["slug"] == "title" and b[8]["slug"] == "sec-two"
    assert "<b>bold</b>" in b[1]["html"] and 'href="https://x.y/z"' in b[1]["html"] and "%CODE_BG%" in b[1]["html"]
    assert [(r["marker"], r["depth"]) for r in b[2]["rows"]] == [("•", 0), ("•", 0), ("◦", 1)]
    assert [r["marker"] for r in b[3]["rows"]] == ["1.", "2."]
    assert b[4]["align"] == ["left", "right"] and len(b[4]["rows"]) == 2 and len(b[4]["rows"][1][0]) > 5
    assert b[7]["lang"] == "lua" and b[7]["code"].startswith("-- hi")
    assert "%TK_COMMENT%" in b[7]["html"] and "%TK_KEYWORD%" in b[7]["html"] and "%TK_STRING%" in b[7]["html"]
    assert mdblocks.headings(b)[0]["label"] == "Sec Two"


def test_markdown_real_docs_parse():
    p = Path(__file__).resolve().parents[2] / "internal/delivery/http/handlers/docs/api-reference.md"
    if not p.exists():
        pytest.skip("docs file not in this checkout")
    b = mdblocks.parse(p.read_text())
    kinds = {x["type"] for x in b}
    assert {"heading", "paragraph", "code", "table", "list"} <= kinds
    assert len(mdblocks.headings(b)) >= 6


def test_lua_scanner_multiline_state():
    spans, st = mdblocks.scan_lua_line("x = [[abc", 0)
    assert st == ("s", 0) and spans[-1][2] == "string"
    spans, st = mdblocks.scan_lua_line("def]] -- tail", st)
    assert st == 0 and [k for *_, k in spans] == ["string", "comment"]
    spans, st = mdblocks.scan_lua_line("--[==[ open", 0)
    assert st == ("c", 2)
    spans, st = mdblocks.scan_lua_line("still ]=] no ]==] done", st)
    assert st == 0 and spans[0][1] == len("still ]=] no ]==]")
    kinds = [k for *_, k in mdblocks.scan_lua_line('local n = foo(12, "s", true) -- c', 0)[0]]
    assert kinds == ["keyword", "function", "number", "string", "constant", "comment"]


# ---- assistant --------------------------------------------------------------------------------------------------------------------
def test_assistant_chat_and_code_proposal(env):
    a = env.admin.assistant
    proposed = []
    a.codeProposed.connect(proposed.append)
    a.selectedModel = "gemma-4-31b-it"
    assert a.send("make it greet", "print('old')") is True
    assert a.loading and a.send("again") is False  # one request at a time
    assert wait_for(lambda: not a.loading and a.model.count == 2)
    sent = env.body("POST", "/ai/assistant")
    assert sent == {"prompt": "make it greet", "model": "gemma-4-31b-it", "currentCode": "print('old')"}
    assert a.model.get(0)["role"] == "user" and a.model.get(1)["role"] == "assistant"
    assert proposed == ['local who = matches[1] or "world"\nsend_text(sender, "Hello, " .. who .. "!")']
    assert any(b["type"] == "code" for b in a.model.get(1)["blocks"])
    a.reset()
    assert a.model.count == 0
    a.send("no code")
    assert wait_for(lambda: a.model.count == 2)
    assert "currentCode" not in env.body("POST", "/ai/assistant")


def test_assistant_error_message(env):
    env.mb.state["bot_fail"].add(("POST", "/ai/assistant"))
    a = env.admin.assistant
    a.send("hello")
    assert wait_for(lambda: a.model.count == 2 and not a.loading)
    text = a.model.get(1)["content"]
    assert text.startswith("Sorry, an error occurred while connecting to the AI assistant.")
    assert "AI Service not available" in text


def test_webhook_url_and_defaults(env):
    assert env.admin.webhookUrl("orders") == env.mb.url.removesuffix("/api") + "/webhook/orders"
    d = env.admin.newItem("crons")
    assert d["schedule"] == "0 * * * *" and d["is_active"] is True
    assert "handler" not in d and "send_text" in d["script"]
    assert json.loads(env.admin.defaultTriggerPayload())["message"]["body"] == "hello"
