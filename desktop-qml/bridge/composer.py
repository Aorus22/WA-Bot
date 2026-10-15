"""`Composer`: the message input and everything sending (GPUI `views/chat/composer/*` + `components/compose.rs`).

QML global `Composer`. The text editor itself lives in QML (`TextEdit`); it reports edits with `setText` and
Python pushes text back with the `textSetRequested` signal (draft restore, edit, mention completion, clear).

Optimistic rows go through `Messages.addPending(row)` / `ackPending(localId, serverId)` /
`failPending(localId, error)` (the chat-messages agent's contract); every call is guarded so the composer
still works (without the optimistic bubble) while that module is absent.

Row shape handed to `Messages.addPending` (same as the REST message, + local fields):
  id (local "temp-..."), chatId, from "me", to, content, timestamp (ms), status "pending", type
  (text|image|video|audio|document|gif|sticker|ptt), mediaUrl (None, or the sticker media_url), senderName "You",
  replyToId, extra {mentions}, and for attachments `localUrl` (file:// preview) + `localName`.
"""
from __future__ import annotations

import os
import tempfile
import time
from pathlib import Path
from typing import Any, Callable
from urllib.parse import quote

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QGuiApplication

from . import composer_util as cu
from .composer_upload import Uploader, file_name_of, url_to_path
from .composer_voice import MAX_SECONDS, MIN_SECONDS, VoiceRecorder, format_timer

LINK_DEBOUNCE_MS = 600
TYPING_RENEW_S = 5.0     # WhatsApp "composing" lapses after ~10 s; renew well before that
TYPING_IDLE_MS = 3000    # stop "typing…" after this long without edits
DRAFT_LIMIT = 200
IMAGE_EXT = {"png", "jpg", "jpeg", "gif", "webp", "bmp"}
TEMP_DIR = Path(tempfile.gettempdir()) / "wabot-qml-composer"


def _err(parsed: Any, status: int) -> str:
    if isinstance(parsed, dict):
        return str(parsed.get("error") or parsed.get("message") or f"HTTP {status}")
    return f"HTTP {status}" if status else "request interrupted"


def _kind_for(name: str) -> str:
    ext = cu.extension_lower(name)
    return "media" if (ext in IMAGE_EXT or cu.is_video_file(name) or cu.is_audio_file(name)) else "document"


def _ok(status: int) -> bool:
    return 200 <= status < 300


class Composer(QObject):
    chatIdChanged = Signal()
    textChanged = Signal()
    replyChanged = Signal()
    editChanged = Signal()
    modeChanged = Signal()
    busyChanged = Signal()
    attachmentChanged = Signal()
    linkChanged = Signal()
    mentionChanged = Signal()
    voiceChanged = Signal()
    stickersChanged = Signal()
    # python -> QML
    textSetRequested = Signal(str)        # replace the editor text, caret at the end
    focusRequested = Signal()
    dialogResult = Signal(str, bool)      # "poll" | "location" | "contact", ok
    sent = Signal(str)                    # local id of an optimistic send, so lists can scroll to the bottom

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._s = services
        self._api = services.api
        self._uploader = Uploader(services.backend, self)
        self._chat = ""
        self._text = ""
        self._reply: dict = {}
        self._edit: dict = {}
        self._pre_edit_text = ""
        self._md = False
        self._uploads = 0
        self._dialog_busy = False
        self._att: dict = {}
        self._drafts: dict[str, str] = {}
        self._seq = 0
        self._id_n = 0
        # link preview
        self._link: dict = {}
        self._link_seq = 0
        self._link_cache: dict[str, dict | None] = {}
        self._link_url: str | None = None
        self._dismissed: set[str] = set()
        self._link_timer = QTimer(self)
        self._link_timer.setSingleShot(True)
        self._link_timer.setInterval(LINK_DEBOUNCE_MS)
        self._link_timer.timeout.connect(self._fetch_link)
        # mentions
        self._participants: list[dict] = []
        self._mention_q: str | None = None
        self._mention_i = 0
        self._mention_gen = 0
        # typing presence
        self._typing_on = False
        self._typing_sent = 0.0
        self._typing_idle = QTimer(self)
        self._typing_idle.setSingleShot(True)
        self._typing_idle.setInterval(TYPING_IDLE_MS)
        self._typing_idle.timeout.connect(self._stop_typing)
        # voice
        self._voice_state = "idle"
        self._voice_chat = ""
        self._voice = VoiceRecorder(self)
        self._voice.levelsChanged.connect(self.voiceChanged)
        # stickers
        self._stk_loading = False
        self._stk_loaded = False
        self._stk: dict[str, list] = {"recent": [], "starred": [], "custom": []}

        services.app.activeChatChanged.connect(self._on_chat_changed)
        self._on_chat_changed()

    # ------------------------------------------------------------------ plumbing
    def _messages(self):
        return self._s.exposed.get("Messages")

    def _call_messages(self, name: str, *args) -> None:
        m = self._messages()
        fn = getattr(m, name, None) if m is not None else None
        if callable(fn):
            try:
                fn(*args)
            except Exception as e:  # never let a list bug break sending
                print(f"[composer] Messages.{name} failed: {e}")

    def _row(self, message_id: str) -> dict | None:
        m = self._messages()
        model = getattr(m, "model", None) if m is not None else None
        model = model() if callable(model) else model
        if model is None:
            return None
        try:
            if hasattr(model, "get_row"):
                row = model.get_row(message_id)
            else:
                row = model.find(message_id)
        except Exception:
            return None
        return dict(row) if row else None

    def _toast(self, msg: str, kind: str = "info") -> None:
        t = self._s.toast
        if t is not None:
            t.show(msg, kind)

    def _new_id(self, kind: str) -> str:
        self._id_n += 1
        return f"temp-{kind}-{int(time.time() * 1000)}-{self._id_n}"

    def _path(self, tail: str) -> str:
        return f"/chats/{quote(self._chat, safe='')}{tail}"

    def _post(self, path: str, body: dict | None, cb: Callable[[int, Any], None]) -> None:
        self._api.fetch("POST", path, body if body is not None else {}, cb)

    # ------------------------------------------------------------------ properties
    @Property(str, notify=chatIdChanged)
    def chatId(self) -> str:
        return self._chat

    @Property(bool, notify=chatIdChanged)
    def isGroup(self) -> bool:
        return self._chat.endswith("@g.us")

    @Property(str, notify=textChanged)
    def text(self) -> str:
        return self._text

    @Property("QVariant", notify=replyChanged)
    def replyTo(self) -> dict:
        return self._reply

    @Property("QVariant", notify=editChanged)
    def editing(self) -> dict:
        return self._edit

    @Property(str, notify=modeChanged)
    def placeholder(self) -> str:
        if self._edit:
            return "Edit message..."
        if self._reply:
            return "Type reply..."
        if self._md:
            return "Write markdown..."
        return "Type a message"

    @Property(bool, notify=modeChanged)
    def markdownMode(self) -> bool:
        return self._md

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        """An attachment / poll / location / contact send is in flight (GPUI `is_sending_attachment`)."""
        return self._uploads > 0 or self._dialog_busy

    @Property(bool, notify=busyChanged)
    def dialogBusy(self) -> bool:
        return self._dialog_busy

    @Property("QVariant", notify=attachmentChanged)
    def pendingAttachment(self) -> dict:
        return self._att

    @Property(bool, notify=textChanged)
    def canSend(self) -> bool:
        return bool(self._text.strip()) or bool(self._att)

    @Property("QVariant", notify=linkChanged)
    def linkPreview(self) -> dict:
        return self._link

    @Property("QVariant", notify=mentionChanged)
    def mentionCandidates(self) -> list:
        return cu.mention_candidates(self._mention_q, self._participants)

    @Property(int, notify=mentionChanged)
    def mentionIndex(self) -> int:
        n = len(self.mentionCandidates)
        return min(self._mention_i, n - 1) if n else 0

    @Property(str, notify=voiceChanged)
    def voiceState(self) -> str:
        """idle | starting | recording"""
        return self._voice_state

    @Property(float, notify=voiceChanged)
    def voiceSeconds(self) -> float:
        return min(self._voice.seconds(), float(MAX_SECONDS))

    @Property(str, notify=voiceChanged)
    def voiceTimer(self) -> str:
        return format_timer(self.voiceSeconds)

    @Property("QVariant", notify=voiceChanged)
    def voiceLevels(self) -> list:
        return self._voice.levels()

    @Property(bool, notify=stickersChanged)
    def stickersLoading(self) -> bool:
        return self._stk_loading

    @Property("QVariant", notify=stickersChanged)
    def stickersRecent(self) -> list:
        return self._stk["recent"]

    @Property("QVariant", notify=stickersChanged)
    def stickersStarred(self) -> list:
        return self._stk["starred"]

    @Property("QVariant", notify=stickersChanged)
    def stickersCustom(self) -> list:
        return self._stk["custom"]

    @Property("QVariant", constant=True)
    def emojiCategories(self) -> list:
        return cu.EMOJI_CATEGORIES

    # ------------------------------------------------------------------ chat switching + drafts
    def _on_chat_changed(self) -> None:
        new = self._s.app.activeChatId or ""
        if new == self._chat and self._seq:
            return
        self._seq += 1
        old = self._chat
        self._stop_typing()
        if old:
            if self._text.strip() and not self._edit:
                self._drafts[old] = self._text
                while len(self._drafts) > DRAFT_LIMIT:
                    self._drafts.pop(next(iter(self._drafts)))
            else:
                self._drafts.pop(old, None)
        self._cancel_voice_silently()
        self._clear_attachment(delete_temp=True)
        self._chat = new
        self._reply, self._edit, self._pre_edit_text = {}, {}, ""
        self._link, self._mention_q, self._mention_i = {}, None, 0
        self._dismissed.clear()
        self._link_timer.stop()
        self._link_seq += 1
        self._participants = []
        self._mention_gen += 1
        self.chatIdChanged.emit()
        self.replyChanged.emit()
        self.editChanged.emit()
        self.modeChanged.emit()
        self.linkChanged.emit()
        self.mentionChanged.emit()
        draft = self._drafts.get(new, "")
        self._text = draft
        self.textChanged.emit()
        self.textSetRequested.emit(draft)
        if new:
            self._after_select(new)

    def _after_select(self, chat: str) -> None:
        if chat.endswith("@g.us"):
            gen = self._mention_gen

            def got(status: int, parsed: Any) -> None:
                if gen == self._mention_gen and _ok(status) and isinstance(parsed, dict):
                    self._participants = list(parsed.get("participants") or [])
                    self._refresh_mentions()

            self._api.fetch("GET", f"/groups/{quote(chat, safe='')}", None, got)
        else:
            # one-to-one typing indicators only arrive for chats we subscribed to (GPUI select_chat)
            self._api.fetch("POST", f"/chats/{quote(chat, safe='')}/presence-subscribe", {}, None)

    # ------------------------------------------------------------------ text from the editor
    @Slot(str)
    def setText(self, text: str) -> None:
        if text == self._text:
            return
        self._text = text
        self.textChanged.emit()
        self._refresh_mentions()
        self._on_text_changed()

    def _clear_input(self) -> None:
        self._text = ""
        self.textChanged.emit()
        self.textSetRequested.emit("")
        self._reset_link()
        self._refresh_mentions()
        self._stop_typing()

    # ------------------------------------------------------------------ typing presence
    def _typing_enabled(self) -> bool:
        return self._s.settings.get_py("send_typing", True) is not False

    def _on_text_changed(self) -> None:
        if not self._text.strip():
            self._stop_typing()
            self._reset_link()
            return
        self._link_check()
        if self._edit or not self._chat or not self._typing_enabled():
            return
        now = time.monotonic()
        if not self._typing_on or now - self._typing_sent >= TYPING_RENEW_S:
            self._typing_on = True
            self._typing_sent = now
            self._post(self._path("/typing"), {"isTyping": True}, lambda *_: None)
        self._typing_idle.start()

    def _stop_typing(self) -> None:
        self._typing_idle.stop()
        if self._typing_on:
            self._typing_on = False
            if self._chat:
                self._post(self._path("/typing"), {"isTyping": False}, lambda *_: None)

    # ------------------------------------------------------------------ mentions
    def _refresh_mentions(self) -> None:
        q = cu.mention_query(self._text, self.isGroup, bool(self._edit))
        if q != self._mention_q:
            if q is not None and self._mention_q is not None and q != self._mention_q:
                self._mention_i = 0
            self._mention_q = q
            if q is None:
                self._mention_i = 0
            self.mentionChanged.emit()

    @Slot(int)
    def moveMention(self, delta: int) -> None:
        n = len(self.mentionCandidates)
        if n:
            self._mention_i = (self.mentionIndex + delta) % n
            self.mentionChanged.emit()

    @Slot()
    def cancelMention(self) -> None:
        if self._mention_q is not None:
            self._mention_q = None
            self.mentionChanged.emit()

    @Slot(result=bool)
    def mentionOpen(self) -> bool:
        return bool(self.mentionCandidates)

    @Slot(str)
    def insertMention(self, jid: str) -> None:
        member = next((p for p in self._participants if p.get("jid") == jid), None)
        at = self._text.rfind("@")
        if not member or at < 0:
            return
        text = self._text[:at] + f"@{cu.participant_label(member)} "
        self._mention_q = None
        self.mentionChanged.emit()
        self._text = text
        self.textChanged.emit()
        self.textSetRequested.emit(text)

    @Slot()
    def insertHighlightedMention(self) -> None:
        c = self.mentionCandidates
        if c:
            self.insertMention(c[self.mentionIndex]["jid"])

    # ------------------------------------------------------------------ reply / edit
    def _banner_preview(self, row: dict | None) -> tuple[str, list]:
        if not row:
            return "", []
        text = cu.first_line(cu.decode_content(str(row.get("content") or "")))
        return cu.render_mentions(text, row.get("extra") if isinstance(row.get("extra"), dict) else None,
                                  self._participants)

    @staticmethod
    def _is_mine(row: dict) -> bool:
        return row.get("from") == "me" or bool(row.get("fromMe") or row.get("isFromMe") or row.get("outgoing"))

    @Slot(str)
    def startReply(self, message_id: str) -> None:
        row = self._row(message_id)
        self.startReplyRow(row or {"id": message_id})

    @Slot("QVariant")
    def startReplyRow(self, row) -> None:
        row = dict(row or {})
        mid = str(row.get("id") or "")
        if not mid:
            return
        preview, spans = self._banner_preview(row)
        sender = "You" if self._is_mine(row) else (row.get("senderName") or "Sender")
        self._leave_edit_state()
        self._reply = {"id": mid, "sender": sender, "preview": preview, "mentionSpans": spans}
        self.replyChanged.emit()
        self.editChanged.emit()
        self.modeChanged.emit()
        self._refresh_mentions()
        self.focusRequested.emit()

    @Slot(str)
    def startEdit(self, message_id: str) -> None:
        row = self._row(message_id)
        if row is None:
            return
        self.startEditRow(row)

    @Slot("QVariant")
    def startEditRow(self, row) -> None:
        row = dict(row or {})
        mid = str(row.get("id") or "")
        if not mid:
            return
        if not self._edit:
            self._pre_edit_text = self._text
        content = cu.decode_content(str(row.get("content") or ""))
        first = cu.first_line(content)
        self._edit = {"id": mid, "preview": first, "original": row.get("content") or ""}
        self._reply = {}
        self._reset_link()
        self.replyChanged.emit()
        self.editChanged.emit()
        self.modeChanged.emit()
        self._text = content
        self.textChanged.emit()
        self.textSetRequested.emit(content)
        self._refresh_mentions()
        self._stop_typing()
        self.focusRequested.emit()

    def _leave_edit_state(self) -> None:
        if self._edit:
            self._edit = {}
            self._restore_pre_edit()

    def _restore_pre_edit(self) -> None:
        self._text = self._pre_edit_text
        self._pre_edit_text = ""
        self.textChanged.emit()
        self.textSetRequested.emit(self._text)

    @Slot()
    def cancelReply(self) -> None:
        if self._reply:
            self._reply = {}
            self.replyChanged.emit()
            self.modeChanged.emit()

    @Slot()
    def cancelEdit(self) -> None:
        if self._edit:
            self._edit = {}
            self.editChanged.emit()
            self.modeChanged.emit()
            self._restore_pre_edit()

    @Slot(result=bool)
    def cancel(self) -> bool:
        """Esc: drop the reply or edit banner. Returns whether there was anything to cancel."""
        if self._reply:
            self.cancelReply()
            return True
        if self._edit:
            self.cancelEdit()
            return True
        return False

    @Slot()
    def toggleMarkdown(self) -> None:
        self._md = not self._md
        self.modeChanged.emit()
        self._toast("Markdown aktif untuk pesan berikutnya" if self._md else "Markdown nonaktif", "info")
        self.focusRequested.emit()

    @Slot(bool, bool, result=bool)
    def enterShouldSend(self, shift: bool, ctrl: bool) -> bool:
        """GPUI: plain Enter sends / Shift+Enter newline; markdown mode flips it (Ctrl+Enter sends)."""
        return ctrl if self._md else not shift

    # ------------------------------------------------------------------ optimistic rows
    def _row_for(self, local_id: str, mtype: str, content: str, *, reply_id: str | None = None,
                 media_url: str | None = None, extra: dict | None = None, **more) -> dict:
        row = {"id": local_id, "chatId": self._chat, "from": "me", "to": self._chat, "content": content,
               "timestamp": int(time.time() * 1000), "status": "pending", "type": mtype, "mediaUrl": media_url,
               "senderName": "You", "replyToId": reply_id, "extra": extra, "starred": False}
        row.update(more)
        return row

    def _finish(self, local_id: str, status: int, parsed: Any, fail_toast: str | None, ok_toast: str | None = None,
                have_row: bool = True) -> bool:
        ok = _ok(status)
        if ok:
            sid = parsed.get("id") if isinstance(parsed, dict) else ""
            if have_row:
                self._call_messages("ackPending", local_id, str(sid or ""))
            if ok_toast:
                self._toast(ok_toast, "success")
        else:
            if have_row:
                self._call_messages("failPending", local_id, _err(parsed, status))
            if fail_toast:
                self._toast(fail_toast, "error")
        return ok

    # ------------------------------------------------------------------ send text / reply / edit
    @Slot()
    def send(self) -> None:
        text = self._text.strip()
        if self._att and not self._edit:
            self._send_pending_attachment()
            return
        if not text or not self._chat:
            return
        chat = self._chat
        no_preview = (not self._edit and cu.first_url(text) in self._dismissed and cu.first_url(text) is not None)
        self._clear_input()
        wire = cu.wire_text(text, self._md)

        if self._edit:
            edit = self._edit
            self._edit, self._pre_edit_text = {}, ""
            self.editChanged.emit()
            self.modeChanged.emit()
            self._send_edit(chat, edit, wire)
            return

        mention_text, mentions = cu.prepare_mention_text(text, self._participants)
        wire = cu.wire_text(mention_text, self._md)
        reply_id = self._reply.get("id") if self._reply else None
        if self._reply:
            self._reply = {}
            self.replyChanged.emit()
            self.modeChanged.emit()
        local = self._new_id("text")
        extra = {"mentions": mentions} if mentions else None
        self._call_messages("addPending", self._row_for(local, "text", wire, reply_id=reply_id, extra=extra))
        self.sent.emit(local)

        def done(status: int, parsed: Any) -> None:
            self._finish(local, status, parsed, "Failed to send message")

        if reply_id:
            def rdone(status: int, parsed: Any) -> None:
                # the reply endpoint returns no message id: ack with "" and let the WS event reconcile
                self._finish(local, status, parsed if isinstance(parsed, dict) and parsed.get("id") else {}, "Failed to send message")
            self._api.fetch("POST", f"/chats/{quote(chat, safe='')}/messages/{quote(str(reply_id), safe='')}/reply",
                            {"content": wire, "mentions": mentions}, rdone)
        else:
            body = {"secret": "default-secret", "target": chat, "message": wire, "mentions": mentions}
            if no_preview:
                body["noLinkPreview"] = True
            self._api.fetch("POST", "/send-message", body, done)

    def _send_edit(self, chat: str, edit: dict, wire: str) -> None:
        mid = edit["id"]
        model = getattr(self._messages(), "model", None) if self._messages() is not None else None
        patched = False
        if model is not None and hasattr(model, "patch"):
            try:
                patched = bool(model.patch(mid, {"content": wire}))
            except Exception:
                patched = False

        def done(status: int, parsed: Any) -> None:
            if _ok(status):
                self._toast("Pesan berhasil diedit", "success")
                return
            if patched and model is not None:
                try:
                    model.patch(mid, {"content": edit.get("original", "")})
                except Exception:
                    pass
            self._toast("Gagal mengedit pesan", "error")

        self._api.fetch("POST", f"/chats/{quote(chat, safe='')}/messages/{quote(mid, safe='')}/edit",
                        {"content": wire}, done)

    # ------------------------------------------------------------------ attachments
    def _stage(self, path: str, *, temp: bool = False, name: str | None = None) -> None:
        name = name or file_name_of(path)
        ext = cu.extension_lower(name)
        kind = _kind_for(name)
        self._clear_attachment(delete_temp=True)
        self._att = {"path": path, "fileName": name, "kind": kind, "temp": temp, "isImage": ext in IMAGE_EXT,
                     "url": QUrl.fromLocalFile(path).toString()}
        self.attachmentChanged.emit()
        self.textChanged.emit()  # canSend
        self.focusRequested.emit()

    def _clear_attachment(self, delete_temp: bool = False) -> None:
        att = self._att
        if not att:
            return
        self._att = {}
        if delete_temp and att.get("temp"):
            try:
                os.unlink(att["path"])
            except OSError:
                pass
        self.attachmentChanged.emit()
        self.textChanged.emit()

    @Slot()
    def clearPendingAttachment(self) -> None:
        self._clear_attachment(delete_temp=True)

    def _send_pending_attachment(self) -> None:
        att, chat = self._att, self._chat
        if not att or not chat:
            return
        caption = self._text.strip()
        self._att = {}
        self.attachmentChanged.emit()
        self._clear_input()
        self._send_file(att["path"], att["kind"], caption=caption, temp=att.get("temp", False), name=att["fileName"])

    @Slot(str, str)
    def sendFile(self, url: str, kind: str) -> None:
        """Send a picked file straight away (file dialog result). kind: media | document | audio | gif."""
        path = url_to_path(url)
        if self._chat and path:
            self._send_file(path, kind or "media")

    @Slot("QVariantList", str)
    def sendFiles(self, urls, kind: str) -> None:
        for u in urls or []:
            self.sendFile(str(u), kind)

    @Slot("QVariantList")
    def handleDrop(self, urls) -> None:
        """Files dropped on the chat: one file is staged (caption friendly), several go out as is."""
        paths = [url_to_path(str(u)) for u in (urls or [])]
        paths = [p for p in paths if p and os.path.isfile(p)]
        if not paths or not self._chat:
            return
        if len(paths) == 1 and not self._edit:
            self._stage(paths[0])
        else:
            for p in paths:
                self._send_file(p, _kind_for(p))

    def _send_file(self, path: str, kind: str, *, caption: str = "", temp: bool = False, name: str | None = None,
                   mime: str | None = None) -> None:
        chat = self._chat
        name = name or file_name_of(path)
        if not os.path.isfile(path):
            self._toast("Gagal membaca file", "error")
            return
        mtype = cu.media_type(kind, name)
        content = caption or cu.optimistic_content(name, mtype)
        local = self._new_id(mtype)
        self._call_messages("addPending", self._row_for(local, cu.optimistic_type(mtype), content,
                                                        localUrl=QUrl.fromLocalFile(path).toString(), localName=name))
        self.sent.emit(local)
        self._uploads += 1
        self.busyChanged.emit()

        def done(status: int, parsed: Any) -> None:
            self._uploads -= 1
            self.busyChanged.emit()
            self._finish(local, status, parsed, "Gagal mengirim lampiran", "Lampiran terkirim")
            if temp:
                try:
                    os.unlink(path)
                except OSError:
                    pass

        if not self._uploader.send_media(chat, mtype, name, path=path, caption=caption, mime=mime, cb=done):
            self._uploads -= 1
            self.busyChanged.emit()
            self._call_messages("failPending", local, "unreadable file")
            self._toast("Gagal membaca file", "error")

    # ------------------------------------------------------------------ clipboard / paste
    @Slot(result=bool)
    def handlePaste(self) -> bool:
        """Ctrl+V hook (GPUI `stage_clipboard_image`): returns True when the paste was consumed (an image or
        file(s) on the clipboard and no text). Text falls through to the editor's normal paste."""
        if not self._chat or self._edit:
            return False
        md = QGuiApplication.clipboard().mimeData()
        if md is None:
            return False
        if md.hasText() and md.text().strip() and not md.hasUrls():
            return False
        if md.hasUrls():
            paths = [u.toLocalFile() for u in md.urls() if u.isLocalFile() and os.path.isfile(u.toLocalFile())]
            if paths:
                self.handleDrop(paths)
                return True
        if md.hasText() and md.text().strip():
            return False
        if md.hasImage():
            img = QGuiApplication.clipboard().image()
            if not img.isNull():
                TEMP_DIR.mkdir(parents=True, exist_ok=True)
                name = f"pasted-image-{int(time.time())}.png"
                path = str(TEMP_DIR / f"{int(time.time() * 1000)}-{name}")
                if img.save(path, "PNG"):
                    self._stage(path, temp=True, name=name)
                    return True
        return False

    @Slot(result=bool)
    def clipboardHasFiles(self) -> bool:
        md = QGuiApplication.clipboard().mimeData()
        return bool(md and (md.hasImage() or md.hasUrls()))

    # ------------------------------------------------------------------ stickers
    @Slot()
    def loadStickers(self) -> None:
        if self._stk_loaded or self._stk_loading:
            return
        self._stk_loading = True
        self.stickersChanged.emit()

        def fav(status: int, parsed: Any) -> None:
            if not _ok(status) or not isinstance(parsed, list):
                self._stk_loading, self._stk_loaded = False, True
                self.stickersChanged.emit()
                self._toast("Gagal memuat stiker", "error")
                return
            favorites = [self._sticker(s) for s in parsed]

            def rec(st2: int, p2: Any) -> None:
                recent = [self._sticker(s) for s in p2] if _ok(st2) and isinstance(p2, list) else []
                self._stk = {"recent": recent,
                             "starred": [s for s in favorites if s["source"] == "wa"],
                             "custom": [s for s in favorites if s["source"] != "wa"]}
                self._stk_loading, self._stk_loaded = False, True
                self.stickersChanged.emit()

            self._api.fetch("GET", "/stickers/recent", None, rec)

        self._api.fetch("GET", "/stickers/favorites", None, fav)

    def _sticker(self, s: dict) -> dict:
        media = str(s.get("mediaUrl") or "")
        return {"id": str(s.get("id") or ""), "mediaUrl": media, "isAnimated": bool(s.get("isAnimated")),
                "source": s.get("source"), "url": self._s.api.mediaUrl(media)}

    @Slot(str, bool)
    def sendSticker(self, media_url: str, animated: bool) -> None:
        chat = self._chat
        if not chat:
            return
        local = self._new_id("sticker")
        self._call_messages("addPending", self._row_for(local, "sticker", "[Sticker]", media_url=media_url))
        self.sent.emit(local)

        def done(status: int, parsed: Any) -> None:
            if not self._finish(local, status, parsed, None):
                self._toast(f"Gagal mengirim stiker: {_err(parsed, status)}", "error")

        self._api.fetch("POST", "/send-sticker", {"secret": "default-secret", "target": chat,
                                                  "mediaUrl": media_url, "isAnimated": bool(animated)}, done)

    # ------------------------------------------------------------------ poll / location / contact
    def _dialog_send(self, kind: str, path: str, body: dict, ok_toast: str, fail_toast: str) -> str:
        if not self._chat:
            return "No chat selected"
        if self._dialog_busy:
            return ""
        self._dialog_busy = True
        self.busyChanged.emit()

        def done(status: int, parsed: Any) -> None:
            self._dialog_busy = False
            self.busyChanged.emit()
            ok = _ok(status)
            self._toast(ok_toast if ok else fail_toast, "success" if ok else "error")
            self.dialogResult.emit(kind, ok)

        self._post(path, body, done)
        return ""

    @Slot(str, "QVariantList", bool, result=str)
    def sendPoll(self, question: str, options, multi: bool) -> str:
        """Validate and send; returns an error message (toast it) or "" when the request was sent / ignored."""
        err, parts = cu.validate_poll(question, [str(o) for o in (options or [])], multi)
        if err:
            self._toast(err, "error")
            return err
        q, opts, m = parts
        return self._dialog_send("poll", self._path("/poll"), {"question": q, "options": opts, "multiSelect": m},
                                 "Poll terkirim", "Gagal mengirim poll")

    @Slot(str, str, str, str, bool, result=str)
    def sendLocation(self, lat: str, lon: str, name: str, address: str, live: bool) -> str:
        err, parts = cu.validate_location(lat, lon, name, address, live)
        if err:
            self._toast(err, "error")
            return err
        la, lo, n, a, lv = parts
        return self._dialog_send("location", self._path("/location"),
                                 {"latitude": la, "longitude": lo, "name": n, "address": a, "live": lv},
                                 "Lokasi terkirim", "Gagal mengirim lokasi")

    @Slot(str, str, result=str)
    def sendContact(self, name: str, phone: str) -> str:
        err, parts = cu.validate_contact(name, phone)
        if err:
            self._toast(err, "error")
            return err
        n, p = parts
        return self._dialog_send("contact", self._path("/contact"), {"displayName": n, "phone": p, "vcard": ""},
                                 "Kontak terkirim", "Gagal mengirim kontak")

    # ------------------------------------------------------------------ link preview card
    def _reset_link(self) -> None:
        self._link_timer.stop()
        self._dismissed.clear()
        self._link_seq += 1
        if self._link:
            self._link = {}
            self.linkChanged.emit()

    def _link_check(self) -> None:
        url = None if self._edit else cu.first_url(self._text)
        if url is None:
            self._link_timer.stop()
            self._link_seq += 1
            if self._link:
                self._link = {}
                self.linkChanged.emit()
            return
        if self._link.get("url") == url:
            return
        if self._link:
            self._link = {}
            self.linkChanged.emit()
        self._link_seq += 1
        if url in self._dismissed:
            return
        if url in self._link_cache:
            card = cu.link_card(url, self._link_cache[url])
            if card:
                self._show_link(card)
            return
        self._link_url = url
        self._link_timer.start()

    def _show_link(self, card: dict) -> None:
        card = dict(card)
        card["thumbnailUrl"] = self._s.api.mediaUrl(card["thumbnailUrl"]) if card.get("thumbnailUrl") else ""
        self._link = card
        self.linkChanged.emit()

    def _fetch_link(self) -> None:
        url, seq = self._link_url, self._link_seq
        if not url:
            return

        def got(status: int, parsed: Any) -> None:
            if _ok(status) and isinstance(parsed, dict):
                self._link_cache[url] = parsed
            elif status == 404:
                self._link_cache[url] = None  # nothing to scrape: remember
            else:
                return  # offline / backend restarting: not worth remembering
            if self._link_seq == seq:
                card = cu.link_card(url, self._link_cache[url])
                if card:
                    self._show_link(card)

        self._api.fetch("GET", f"/link-preview?url={quote(url, safe='')}", None, got)

    @Slot()
    def dismissLinkPreview(self) -> None:
        if self._link:
            self._dismissed.add(self._link["url"])
            self._link = {}
            self.linkChanged.emit()

    # ------------------------------------------------------------------ voice notes
    @Slot()
    def startVoice(self) -> None:
        if not self._chat or self._voice_state != "idle":
            return
        self._voice_state, self._voice_chat = "starting", self._chat
        self.voiceChanged.emit()
        QTimer.singleShot(0, lambda seq=self._seq: self._open_mic(seq))

    def _open_mic(self, seq: int) -> None:
        if self._voice_state != "starting" or seq != self._seq:
            return
        err = self._voice.start()
        if err:
            self._voice_state = "idle"
            self.voiceChanged.emit()
            self._toast(err, "error")
            return
        self._voice_state = "recording"
        self.voiceChanged.emit()

    def _cancel_voice_silently(self) -> None:
        if self._voice_state != "idle":
            self._voice.stop()
            self._voice_state = "idle"
            self.voiceChanged.emit()

    @Slot()
    def cancelVoice(self) -> None:
        self._cancel_voice_silently()

    @Slot()
    def sendVoice(self) -> None:
        if self._voice_state != "recording":
            return  # still opening the microphone: nothing to send yet
        chat = self._voice_chat
        rec = self._voice.finish()
        self._voice_state = "idle"
        self.voiceChanged.emit()
        self.sendVoiceData(chat, rec["wav"], rec["seconds"], rec["peak"])

    def sendVoiceData(self, chat: str, wav: bytes, seconds: float, peak: int) -> None:
        """Upload a finished recording as a push-to-talk message (also the test seam)."""
        if seconds < MIN_SECONDS:
            self._toast("Rekaman terlalu pendek", "info")
            return
        if peak == 0:
            self._toast("Mikrofon tidak menangkap suara. Periksa perangkat dan izin mikrofon.", "error")
            return
        show = chat == self._chat
        local = self._new_id("ptt")
        if show:
            self._call_messages("addPending", self._row_for(local, "ptt", "[Audio]"))
            self.sent.emit(local)

        def done(status: int, parsed: Any) -> None:
            self._uploads -= 1
            self.busyChanged.emit()
            self._finish(local, status, parsed, "Gagal mengirim pesan suara", have_row=show)

        self._uploads += 1
        self.busyChanged.emit()
        self._uploader.send_media(chat, "ptt", "voice-note.wav", data=wav, mime="audio/wav", ptt=True,
                                  seconds=seconds, cb=done)

    # ------------------------------------------------------------------ helpers for QML
    @Slot(str, result=str)
    def fileName(self, url: str) -> str:
        return file_name_of(url_to_path(url))


def register(services) -> None:
    services.expose("Composer", Composer(services))
