"""Render the app offscreen and save a PNG.
    QT_QPA_PLATFORM=offscreen python scripts/shot.py out.png --url http://127.0.0.1:PORT/api
        [--route chat] [--select-chat JID] [--preset default-dark] [--theme-mode system|dark|light]
        [--size 1100x720] [--scale 1.0] [--window-transparency 28] [--wait 2500]
        [--message-context] [--click-sidebar-toggle] [--calls-filter incoming]
        [--click-status SENDER] [--click-channel JID]
        [--composer-text TEXT] [--composer-key Enter|ShiftEnter|CtrlEnter] [--composer-markdown]
Everything after the known options is passed to the app (e.g. --port N). Uses the same startup as main.py."""
import os
import json
import sys
import time
from pathlib import Path
from urllib.parse import urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import QByteArray, QCoreApplication, QEvent, QMetaObject, QMimeData, QObject, Property, QPoint, QPointF, QTimer, QUrl, Qt, Signal, Slot  # noqa: E402
from PySide6.QtGui import QAccessible, QColor, QFontDatabase, QGuiApplication, QImage, QPainter  # noqa: E402
from PySide6.QtQuickControls2 import QQuickStyle  # noqa: E402
from PySide6.QtTest import QTest  # noqa: E402

import main as app_main  # noqa: E402


def opt(args, name, default=None, flag=False):
    if name in args:
        i = args.index(name)
        if flag:
            args.pop(i)
            return True
        v = args[i + 1]
        del args[i:i + 2]
        return v
    return False if flag else default


class ShotTrayPlatform(QObject):
    showWindowRequested = Signal()
    toggleWindowRequested = Signal()

    def __init__(self):
        super().__init__()
        self.window = None
        self.hide_count = 0

    @Property(bool, constant=True)
    def trayAvailable(self) -> bool:
        return True

    @Property(bool, constant=True)
    def quitting(self) -> bool:
        return False

    @Property(bool, constant=True)
    def popoverMode(self) -> bool:
        return False

    @Slot()
    def hideToTray(self) -> None:
        self.hide_count += 1
        if self.window is not None:
            self.window.hide()


def run(argv: list[str]) -> int:
    args = argv[1:]
    out = args.pop(0)
    api_url = opt(args, "--url") or opt(args, "--backend-url")
    if not api_url:
        print("[shot] refusing to discover a backend; pass --url for tests/mock_backend.py", file=sys.stderr)
        return 2
    if urlsplit(api_url).hostname not in {"127.0.0.1", "localhost", "::1"}:
        print("[shot] only loopback mock URLs are allowed", file=sys.stderr)
        return 2
    args.extend(["--url", api_url])
    route = opt(args, "--route", "chat")
    status_sender = opt(args, "--open-status")
    click_status_sender = opt(args, "--click-status")
    status_new_key = opt(args, "--status-new-key", flag=True)
    status_post_text = opt(args, "--status-post-text")
    status_post_key = opt(args, "--status-post-key")
    status_reply_text = opt(args, "--status-reply-text")
    status_reply_sender = opt(args, "--status-reply-sender")
    if status_post_text is not None and (not status_post_text.strip() or route != "status"):
        print("[shot] --status-post-text requires non-empty text and --route status", file=sys.stderr)
        return 2
    if status_post_key is not None and (not status_post_key.strip() or route != "status"):
        print("[shot] --status-post-key requires non-empty text and --route status", file=sys.stderr)
        return 2
    if status_post_key is not None and status_post_text is not None:
        print("[shot] choose either mouse or keyboard status-post input", file=sys.stderr)
        return 2
    if status_post_key is not None:
        status_post_text = status_post_key
    if status_reply_text is not None and (not status_reply_text.strip() or route != "status"):
        print("[shot] --status-reply-text requires non-empty text and --route status", file=sys.stderr)
        return 2
    if status_reply_text is not None and not status_reply_sender:
        print("[shot] --status-reply-text requires --status-reply-sender", file=sys.stderr)
        return 2
    if status_reply_text is not None and status_post_text is not None:
        print("[shot] choose one status action", file=sys.stderr)
        return 2
    if status_new_key and route != "status":
        print("[shot] --status-new-key requires --route status", file=sys.stderr)
        return 2
    channel_jid = opt(args, "--select-channel")
    click_channel_jid = opt(args, "--click-channel")
    channel_mute = opt(args, "--channel-mute", flag=True)
    channel_reaction = opt(args, "--channel-reaction")
    channel_reaction_post = opt(args, "--channel-reaction-post", "p1")
    channel_follow_url = opt(args, "--channel-follow-url")
    if channel_mute and (route != "channels" or not click_channel_jid):
        print("[shot] --channel-mute requires --route channels and --click-channel", file=sys.stderr)
        return 2
    if channel_reaction and (route != "channels" or not click_channel_jid):
        print("[shot] --channel-reaction requires --route channels and --click-channel", file=sys.stderr)
        return 2
    if channel_follow_url is not None and (route != "channels" or not channel_follow_url.strip()):
        print("[shot] --channel-follow-url requires a non-empty URL and --route channels", file=sys.stderr)
        return 2
    call_id = opt(args, "--select-call")
    calls_filter = opt(args, "--calls-filter")
    if calls_filter and calls_filter not in {"all", "incoming", "outgoing", "missed"}:
        print("[shot] --calls-filter must be all, incoming, outgoing or missed", file=sys.stderr)
        return 2
    incoming_call = opt(args, "--incoming-call", flag=True)
    call_action = opt(args, "--call-action")
    call_participant_jid = opt(args, "--call-participant-jid")
    active_call = opt(args, "--active-call", flag=True)
    group_call = opt(args, "--group-call", flag=True)
    if call_action and call_action not in {"answer", "reject", "hangup", "add-participant", "video-start", "video-stop", "mic-toggle", "speaker-toggle"}:
        print("[shot] --call-action must name a supported call control", file=sys.stderr)
        return 2
    if call_action in {"answer", "reject"} and not incoming_call:
        print("[shot] answer/reject --call-action requires --incoming-call", file=sys.stderr)
        return 2
    if call_action in {"hangup", "video-start", "video-stop", "mic-toggle", "speaker-toggle"} and not active_call:
        print("[shot] active-call --call-action requires --active-call", file=sys.stderr)
        return 2
    if call_action == "add-participant" and (not group_call or not call_participant_jid):
        print("[shot] add-participant --call-action requires --group-call and --call-participant-jid", file=sys.stderr)
        return 2
    if call_participant_jid and call_action != "add-participant":
        print("[shot] --call-participant-jid requires --call-action add-participant", file=sys.stderr)
        return 2
    media_id = opt(args, "--open-media")
    click_media_message = opt(args, "--click-media")
    chat_info_media_message = opt(args, "--chat-info-media")
    keyboard_image_message = opt(args, "--keyboard-image")
    save_document_message = opt(args, "--save-document")
    save_document_path = opt(args, "--save-path")
    jump_message = opt(args, "--jump-message")
    vote_poll = opt(args, "--vote-poll")
    vote_poll_key = opt(args, "--vote-poll-key")
    open_contact = opt(args, "--open-contact")
    open_location = opt(args, "--open-location")
    open_link_key = opt(args, "--open-link-key")
    keyboard_archive = opt(args, "--keyboard-archive", flag=True)
    tab_chat_list = opt(args, "--tab-chat-list", flag=True)
    chat_row_arrow = opt(args, "--chat-row-arrow")
    keyboard_chat_id = opt(args, "--keyboard-chat")
    chat_row_start, chat_row_key = chat_row_arrow.split(":", 1) if chat_row_arrow and ":" in chat_row_arrow else ("", "")
    if chat_row_arrow and (not chat_row_start or chat_row_key not in {"Up", "Down"}):
        print("[shot] --chat-row-arrow must be CHAT_ID:Up or CHAT_ID:Down", file=sys.stderr)
        return 2
    contact_message, contact_jid = open_contact.split(":", 1) if open_contact and ":" in open_contact else ("", "")
    if open_contact and (not contact_message or not contact_jid):
        print("[shot] --open-contact must be MESSAGE_ID:JID", file=sys.stderr)
        return 2
    if open_location and route != "chat":
        print("[shot] --open-location requires --route chat", file=sys.stderr)
        return 2
    if open_link_key and route != "chat":
        print("[shot] --open-link-key requires --route chat", file=sys.stderr)
        return 2
    if keyboard_chat_id and route != "chat":
        print("[shot] --keyboard-chat requires --route chat", file=sys.stderr)
        return 2
    if keyboard_archive and route != "chat":
        print("[shot] --keyboard-archive requires --route chat", file=sys.stderr)
        return 2
    if tab_chat_list and route != "chat":
        print("[shot] --tab-chat-list requires --route chat", file=sys.stderr)
        return 2
    poll_request = vote_poll_key or vote_poll
    poll_vote_message, poll_vote_option = poll_request.split(":", 1) if poll_request and ":" in poll_request else ("", "")
    if poll_request and (not poll_vote_message or not poll_vote_option):
        print("[shot] --vote-poll and --vote-poll-key must be MESSAGE_ID:OPTION", file=sys.stderr)
        return 2
    if vote_poll and vote_poll_key:
        print("[shot] choose mouse or keyboard poll voting", file=sys.stderr)
        return 2
    message_context = opt(args, "--message-context", flag=True)
    scroll_key = opt(args, "--scroll-key")
    focus_composer = opt(args, "--focus-composer", flag=True)
    focus_chat_search = opt(args, "--focus-chat-search", flag=True)
    expect_no_scroll = opt(args, "--expect-no-scroll", flag=True)
    caret_start = opt(args, "--caret-start", flag=True)
    scroll_keys = {"PageUp": Qt.Key_PageUp, "PageDown": Qt.Key_PageDown, "Home": Qt.Key_Home, "End": Qt.Key_End}
    if scroll_key and (scroll_key not in scroll_keys or not message_context):
        print("[shot] --scroll-key requires --message-context and one of PageUp, PageDown, Home or End", file=sys.stderr)
        return 2
    if focus_composer and not scroll_key:
        print("[shot] --focus-composer requires --scroll-key", file=sys.stderr)
        return 2
    if focus_chat_search and not scroll_key:
        print("[shot] --focus-chat-search requires --scroll-key", file=sys.stderr)
        return 2
    if focus_composer and focus_chat_search:
        print("[shot] choose one keyboard focus target", file=sys.stderr)
        return 2
    if expect_no_scroll and not scroll_key:
        print("[shot] --expect-no-scroll requires --scroll-key", file=sys.stderr)
        return 2
    if caret_start and not focus_composer:
        print("[shot] --caret-start requires --focus-composer", file=sys.stderr)
        return 2
    select_message_id = opt(args, "--select-message")
    forward_message_id = opt(args, "--forward-message")
    forward_key = opt(args, "--forward-key")
    multi_forward_key = opt(args, "--multi-forward-key")
    multi_forward_ids, multi_forward_target = multi_forward_key.rsplit(":", 1) if multi_forward_key and ":" in multi_forward_key else ("", "")
    multi_forward_ids = [message_id for message_id in multi_forward_ids.split(",") if message_id]
    reply_message_key = opt(args, "--reply-message-key")
    edit_message_key = opt(args, "--edit-message-key")
    react_message_key = opt(args, "--react-message-key")
    react_message_id, react_emoji = react_message_key.rsplit(":", 1) if react_message_key and ":" in react_message_key else ("", "")
    react_chip_key = opt(args, "--react-chip-key")
    react_chip_id, react_chip_emoji = react_chip_key.rsplit(":", 1) if react_chip_key and ":" in react_chip_key else ("", "")
    disappearing_message_key = opt(args, "--disappearing-message-key")
    message_info_key = opt(args, "--message-info-key")
    star_message_key = opt(args, "--star-message-key")
    delete_message_key = opt(args, "--delete-message-key")
    copy_message_key = opt(args, "--copy-message-key")
    context_forward_key = opt(args, "--context-forward-key")
    context_forward_id, context_forward_target = context_forward_key.rsplit(":", 1) if context_forward_key and ":" in context_forward_key else ("", "")
    pin_message_key = opt(args, "--pin-message-key")
    selection_copy_key = opt(args, "--selection-copy-key", flag=True)
    selection_delete_key = opt(args, "--selection-delete-key", flag=True)
    compose_dialog = opt(args, "--compose-dialog")
    composer_text = opt(args, "--composer-text")
    composer_key = opt(args, "--composer-key")
    composer_markdown = opt(args, "--composer-markdown", flag=True)
    clipboard_paste = opt(args, "--clipboard-paste")
    file_dialog_path = opt(args, "--file-dialog-file")
    if clipboard_paste and clipboard_paste not in {"image", "text"}:
        print("[shot] --clipboard-paste must be image or text", file=sys.stderr)
        return 2
    if file_dialog_path and not os.path.isfile(file_dialog_path):
        print("[shot] --file-dialog-file must point to a local file", file=sys.stderr)
        return 2
    if composer_key and composer_key not in {"Enter", "ShiftEnter", "CtrlEnter"}:
        print("[shot] --composer-key must be Enter, ShiftEnter or CtrlEnter", file=sys.stderr)
        return 2
    if composer_markdown and not composer_key:
        print("[shot] --composer-markdown requires --composer-key", file=sys.stderr)
        return 2
    if composer_key and composer_text is None:
        composer_text = "keyboard sample"
    emoji_picker = opt(args, "--emoji-picker", flag=True)
    sticker_picker = opt(args, "--sticker-picker", flag=True)
    if emoji_picker and sticker_picker:
        print("[shot] choose either the emoji or sticker picker", file=sys.stderr)
        return 2
    attach_menu = opt(args, "--attach-menu", flag=True)
    chat_sheet = opt(args, "--chat-sheet")
    chat_info_page = opt(args, "--chat-info-page")
    leave_info_page_open = opt(args, "--leave-info-page-open", flag=True)
    group_member_action = opt(args, "--group-member-action")
    group_member = opt(args, "--group-member")
    info_mute_mode = opt(args, "--info-mute-mode")
    info_disappearing_arg = opt(args, "--info-disappearing")
    voice_no_device = opt(args, "--voice-no-device", flag=True)
    voice_send_synthetic = opt(args, "--voice-send-synthetic", flag=True)
    disappearing_seconds_arg = opt(args, "--disappearing-seconds")
    try:
        disappearing_seconds = int(disappearing_seconds_arg) if disappearing_seconds_arg is not None else None
    except ValueError:
        print("[shot] --disappearing-seconds must be an integer", file=sys.stderr)
        return 2
    try:
        info_disappearing = int(info_disappearing_arg) if info_disappearing_arg is not None else None
    except ValueError:
        print("[shot] --info-disappearing must be an integer", file=sys.stderr)
        return 2
    chat_id = opt(args, "--select-chat")
    if chat_info_page and chat_info_page not in {"media", "docs", "links"}:
        print("[shot] --chat-info-page must be media, docs or links", file=sys.stderr)
        return 2
    if chat_info_page and (route != "chat" or not chat_id or chat_sheet):
        print("[shot] --chat-info-page requires --route chat and --select-chat, without --chat-sheet", file=sys.stderr)
        return 2
    if leave_info_page_open and not chat_info_page:
        print("[shot] --leave-info-page-open requires --chat-info-page", file=sys.stderr)
        return 2
    if disappearing_seconds is not None and (route != "chat" or not chat_id
                                             or disappearing_seconds not in {0, 86400, 604800, 7776000}):
        print("[shot] --disappearing-seconds requires --route chat, --select-chat, and 0/86400/604800/7776000", file=sys.stderr)
        return 2
    if (select_message_id or forward_message_id) and (route != "chat" or not chat_id):
        print("[shot] --select-message and --forward-message require --route chat and --select-chat", file=sys.stderr)
        return 2
    if multi_forward_key and (route != "chat" or not chat_id):
        print("[shot] --multi-forward-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if multi_forward_key and (not multi_forward_target or len(multi_forward_ids) < 2
                              or len(set(multi_forward_ids)) != len(multi_forward_ids)):
        print("[shot] --multi-forward-key must be MESSAGE_ID,MESSAGE_ID:TARGET_CHAT_ID with distinct IDs", file=sys.stderr)
        return 2
    if forward_key and not forward_message_id:
        print("[shot] --forward-key requires --forward-message", file=sys.stderr)
        return 2
    if reply_message_key and (route != "chat" or not chat_id):
        print("[shot] --reply-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if edit_message_key and (route != "chat" or not chat_id):
        print("[shot] --edit-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if react_message_key and (route != "chat" or not chat_id or not react_message_id or not react_emoji):
        print("[shot] --react-message-key must be MESSAGE_ID:EMOJI and requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if react_chip_key and (route != "chat" or not chat_id or not react_chip_id or not react_chip_emoji):
        print("[shot] --react-chip-key must be MESSAGE_ID:EMOJI and requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if star_message_key and (route != "chat" or not chat_id):
        print("[shot] --star-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if pin_message_key and (route != "chat" or not chat_id):
        print("[shot] --pin-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if delete_message_key and (route != "chat" or not chat_id):
        print("[shot] --delete-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if disappearing_message_key and (route != "chat" or not chat_id):
        print("[shot] --disappearing-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if message_info_key and (route != "chat" or not chat_id):
        print("[shot] --message-info-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if copy_message_key and (route != "chat" or not chat_id):
        print("[shot] --copy-message-key requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if context_forward_key and (route != "chat" or not chat_id or not context_forward_id or not context_forward_target):
        print("[shot] --context-forward-key must be MESSAGE_ID:TARGET_CHAT_ID and requires --route chat and --select-chat", file=sys.stderr)
        return 2
    if sum(bool(value) for value in (reply_message_key, edit_message_key, react_message_key,
                                     star_message_key, pin_message_key, delete_message_key,
                                     copy_message_key, context_forward_key, message_info_key)) > 1:
        print("[shot] choose one message context action", file=sys.stderr)
        return 2
    if selection_copy_key and not select_message_id:
        print("[shot] --selection-copy-key requires --select-message", file=sys.stderr)
        return 2
    if selection_delete_key and not select_message_id:
        print("[shot] --selection-delete-key requires --select-message", file=sys.stderr)
        return 2
    if selection_copy_key and selection_delete_key:
        print("[shot] choose either --selection-copy-key or --selection-delete-key", file=sys.stderr)
        return 2
    if sum(bool(value) for value in (select_message_id, forward_message_id, multi_forward_key)) > 1:
        print("[shot] choose one message selection or forwarding action", file=sys.stderr)
        return 2
    if composer_key and not chat_id:
        print("[shot] composer keyboard actions require --select-chat", file=sys.stderr)
        return 2
    if click_media_message and not chat_id:
        print("[shot] --click-media requires --select-chat", file=sys.stderr)
        return 2
    if chat_info_media_message and not chat_id:
        print("[shot] --chat-info-media requires --select-chat", file=sys.stderr)
        return 2
    if keyboard_image_message and not chat_id:
        print("[shot] --keyboard-image requires --select-chat", file=sys.stderr)
        return 2
    if bool(save_document_message) != bool(save_document_path):
        print("[shot] --save-document and --save-path must be supplied together", file=sys.stderr)
        return 2
    if save_document_message and not chat_id:
        print("[shot] --save-document requires --select-chat", file=sys.stderr)
        return 2
    if save_document_path and (os.path.exists(save_document_path) or not os.path.isdir(os.path.dirname(os.path.abspath(save_document_path)))):
        print("[shot] --save-path must be a new file in an existing directory", file=sys.stderr)
        return 2
    if jump_message and not chat_id:
        print("[shot] --jump-message requires --select-chat", file=sys.stderr)
        return 2
    if vote_poll and not chat_id:
        print("[shot] --vote-poll requires --select-chat", file=sys.stderr)
        return 2
    if open_contact and not chat_id:
        print("[shot] --open-contact requires --select-chat", file=sys.stderr)
        return 2
    if open_location and not chat_id:
        print("[shot] --open-location requires --select-chat", file=sys.stderr)
        return 2
    if open_link_key and not chat_id:
        print("[shot] --open-link-key requires --select-chat", file=sys.stderr)
        return 2
    if keyboard_chat_id and chat_id:
        print("[shot] use --keyboard-chat without --select-chat", file=sys.stderr)
        return 2
    if chat_row_arrow and keyboard_chat_id:
        print("[shot] choose a chat-row focus move or a chat-row activation", file=sys.stderr)
        return 2
    if voice_no_device and not chat_id:
        print("[shot] --voice-no-device requires --select-chat", file=sys.stderr)
        return 2
    if voice_send_synthetic and not chat_id:
        print("[shot] --voice-send-synthetic requires --select-chat", file=sys.stderr)
        return 2
    if voice_no_device and voice_send_synthetic:
        print("[shot] choose unavailable-microphone or synthetic voice-send scenario", file=sys.stderr)
        return 2
    if clipboard_paste and not chat_id:
        print("[shot] --clipboard-paste requires --select-chat", file=sys.stderr)
        return 2
    if file_dialog_path and not chat_id:
        print("[shot] --file-dialog-file requires --select-chat", file=sys.stderr)
        return 2
    if info_mute_mode and info_mute_mode not in {"off", "8h", "1w", "forever"}:
        print("[shot] --info-mute-mode must be off, 8h, 1w or forever", file=sys.stderr)
        return 2
    if info_disappearing is not None and info_disappearing not in {0, 86400, 604800, 7776000}:
        print("[shot] --info-disappearing must be 0, 86400, 604800 or 7776000", file=sys.stderr)
        return 2
    if info_mute_mode and info_disappearing is not None:
        print("[shot] choose either an info mute or disappearing setting", file=sys.stderr)
        return 2
    if (info_mute_mode or info_disappearing is not None) and (route != "chat" or not chat_id):
        print("[shot] info settings require --route chat and --select-chat", file=sys.stderr)
        return 2
    if group_member_action and group_member_action != "add":
        print("[shot] --group-member-action must be add", file=sys.stderr)
        return 2
    if group_member_action and (not chat_id or not chat_id.endswith("@g.us")):
        print("[shot] group actions require a group --select-chat", file=sys.stderr)
        return 2
    if group_member_action and not group_member:
        print("[shot] --group-member-action requires --group-member", file=sys.stderr)
        return 2
    collapse_sidebar = opt(args, "--collapse-sidebar", flag=True)
    tray_close_cycle = opt(args, "--tray-close-cycle", flag=True)
    click_sidebar_toggle = opt(args, "--click-sidebar-toggle", flag=True)
    click_chat_profile = opt(args, "--click-chat-profile", flag=True)
    if click_sidebar_toggle and not chat_id:
        print("[shot] --click-sidebar-toggle requires --select-chat", file=sys.stderr)
        return 2
    if click_chat_profile and not chat_id:
        print("[shot] --click-chat-profile requires --select-chat", file=sys.stderr)
        return 2
    chat_new_action = opt(args, "--chat-new-action")
    auth_tab = opt(args, "--auth-tab")
    assistant_panel = opt(args, "--assistant-panel", flag=True)
    webhook_logs_id = opt(args, "--webhook-logs")
    if webhook_logs_id is not None and (route != "webhooks" or not webhook_logs_id):
        print("[shot] --webhook-logs requires a webhook ID and --route webhooks", file=sys.stderr)
        return 2
    bot_test_trigger = opt(args, "--bot-test-trigger")
    if bot_test_trigger is not None and (route != "triggers" or not bot_test_trigger):
        print("[shot] --bot-test-trigger requires a trigger ID and --route triggers", file=sys.stderr)
        return 2
    bot_test_webhook = opt(args, "--bot-test-webhook")
    if bot_test_webhook is not None and (route != "webhooks" or not bot_test_webhook):
        print("[shot] --bot-test-webhook requires a webhook ID and --route webhooks", file=sys.stderr)
        return 2
    if bot_test_trigger is not None and bot_test_webhook is not None:
        print("[shot] choose one bot test target", file=sys.stderr)
        return 2
    bot_test_kind = "triggers" if bot_test_trigger is not None else "webhooks" if bot_test_webhook is not None else None
    bot_test_id = bot_test_trigger if bot_test_trigger is not None else bot_test_webhook
    cron_run_now = opt(args, "--cron-run-now")
    if cron_run_now is not None and (route != "cron" or not cron_run_now):
        print("[shot] --cron-run-now requires a cron ID and --route cron", file=sys.stderr)
        return 2
    bot_create_trigger = opt(args, "--bot-create-trigger")
    if bot_create_trigger is not None and (route != "triggers" or not bot_create_trigger.strip()):
        print("[shot] --bot-create-trigger requires a non-empty name and --route triggers", file=sys.stderr)
        return 2
    bot_create_webhook = opt(args, "--bot-create-webhook")
    if bot_create_webhook is not None and (route != "webhooks" or not bot_create_webhook.strip()):
        print("[shot] --bot-create-webhook requires a non-empty name and --route webhooks", file=sys.stderr)
        return 2
    if bot_create_trigger is not None and bot_create_webhook is not None:
        print("[shot] choose one bot collection to create", file=sys.stderr)
        return 2
    bot_assistant_prompt = opt(args, "--bot-assistant-prompt")
    if bot_assistant_prompt is not None and (route != "triggers" or not bot_assistant_prompt.strip()):
        print("[shot] --bot-assistant-prompt requires non-empty text and --route triggers", file=sys.stderr)
        return 2
    docs_toc_index = opt(args, "--docs-toc-index")
    if docs_toc_index is not None:
        try:
            docs_toc_index = int(docs_toc_index)
        except ValueError:
            print("[shot] --docs-toc-index must be an integer", file=sys.stderr)
            return 2
        if route != "documentation" or docs_toc_index < 0:
            print("[shot] --docs-toc-index requires --route documentation and a non-negative index", file=sys.stderr)
            return 2
    bot_toggle_trigger = opt(args, "--bot-toggle-trigger")
    if bot_toggle_trigger is not None and (route != "triggers" or not bot_toggle_trigger):
        print("[shot] --bot-toggle-trigger requires an ID and --route triggers", file=sys.stderr)
        return 2
    bot_delete_trigger = opt(args, "--bot-delete-trigger")
    if bot_delete_trigger is not None and (route != "triggers" or not bot_delete_trigger):
        print("[shot] --bot-delete-trigger requires an ID and --route triggers", file=sys.stderr)
        return 2
    bot_delete_all = opt(args, "--bot-delete-all", flag=True)
    if bot_delete_all and route != "triggers":
        print("[shot] --bot-delete-all requires --route triggers", file=sys.stderr)
        return 2
    bot_export_file = opt(args, "--bot-export-file")
    bot_import_file = opt(args, "--bot-import-file")
    if (bot_export_file is not None or bot_import_file is not None) and route != "triggers":
        print("[shot] bot import/export file actions require --route triggers", file=sys.stderr)
        return 2
    if bot_export_file is not None and bot_import_file is not None:
        print("[shot] choose either bot export or import", file=sys.stderr)
        return 2
    if bot_import_file is not None and not os.path.isfile(bot_import_file):
        print("[shot] --bot-import-file must point to an existing file", file=sys.stderr)
        return 2
    preset = opt(args, "--preset")
    theme_mode = opt(args, "--theme-mode")
    size = opt(args, "--size", "1100x720")
    wait = int(opt(args, "--wait", "2500"))
    scale = opt(args, "--scale")
    chat_layout = opt(args, "--chat-layout")
    window_transparency = opt(args, "--window-transparency")
    QQuickStyle.setStyle("Basic")
    app = QGuiApplication(["shot", *args])
    for f in (app_main.QML / "assets" / "fonts").glob("*.ttf"):
        QFontDatabase.addApplicationFont(str(f))
    from bridge.settings import Settings
    import tempfile
    settings = Settings(Path(tempfile.mkdtemp()) / "settings.json")  # never touch the user's settings
    if preset:
        settings.set_transient("theme_preset", preset)
    if theme_mode:
        settings.set_transient("theme_mode", theme_mode)
    if scale:
        settings.set_transient("ui_font_scale", int(float(scale) * 100))
    if chat_layout:
        settings.set_transient("chat_layout", chat_layout)
    settings.set_transient("window_transparency", int(window_transparency) if window_transparency is not None else 0)
    services, engine = app_main.build(["shot", *args], app, settings)
    shot_tray = None
    if tray_close_cycle:
        shot_tray = ShotTrayPlatform()
        engine.rootContext().setContextProperty("Platform", shot_tray)
    engine.load(QUrl.fromLocalFile(str(app_main.QML / "Main.qml")))
    if not engine.rootObjects():
        return 1
    w, h = (int(x) for x in size.split("x"))
    win = engine.rootObjects()[0]
    if shot_tray is not None:
        shot_tray.window = win
    win.setWidth(w)
    win.setHeight(h)
    scenario_ready = not status_new_key and status_post_text is None and status_reply_text is None and channel_follow_url is None and not status_sender and not click_status_sender and not channel_jid and not click_channel_jid and not call_id and not calls_filter and not incoming_call and not active_call \
        and not media_id and not click_media_message and not chat_info_media_message and not keyboard_image_message and not save_document_message and not select_message_id and not forward_message_id and not compose_dialog and not composer_key \
        and not emoji_picker and not sticker_picker and not attach_menu and not chat_sheet and not chat_info_page and not auth_tab \
        and not assistant_panel and docs_toc_index is None and bot_assistant_prompt is None and webhook_logs_id is None and bot_test_kind is None and cron_run_now is None and bot_create_trigger is None and bot_create_webhook is None and bot_toggle_trigger is None and bot_delete_trigger is None and not bot_delete_all and bot_export_file is None and bot_import_file is None and not group_call and not chat_new_action and not message_context and not chat_id and not keyboard_archive and not tab_chat_list and not chat_row_arrow and not keyboard_chat_id \
        and not group_member_action and not info_mute_mode and info_disappearing is None and not channel_mute and not channel_reaction \
        and not click_sidebar_toggle and not click_chat_profile and not jump_message and not vote_poll and not vote_poll_key and not open_contact and not open_location and not open_link_key and not file_dialog_path and not voice_no_device \
        and not collapse_sidebar \
        and not tray_close_cycle
    scenario_started = False
    scenario_error = False

    def find_item(name: str):
        try:
            pending = [win.contentItem()]
            while pending:
                item = pending.pop()
                if item is None:
                    continue
                if item.objectName() == name:
                    return item
                pending.extend(item.childItems())
        except RuntimeError:
            pass
        return None

    def run_scenario() -> None:
        nonlocal scenario_ready, scenario_started, scenario_error
        if scenario_started:
            return
        scenario_started = True

        def show_compose_overlay() -> None:
            nonlocal scenario_error, scenario_ready
            target_name, method = None, "open"
            if compose_dialog:
                target_name = "composeDialogs"
                method = {"poll": "openPoll", "location": "openLocation", "contact": "openContact"}.get(compose_dialog, "")
            elif emoji_picker:
                target_name, method = "composeDialogs", "openEmoji"
            elif sticker_picker:
                target_name, method = "composeDialogs", "openStickers"
            elif attach_menu:
                target_name, method = "composeDialogs", "openAttachMenu"
            target = find_item(target_name) if target_name else None
            if target is not None and method:
                QMetaObject.invokeMethod(target, method)
            else:
                names = []
                pending = [win.contentItem()]
                while pending:
                    item = pending.pop()
                    if item is None:
                        continue
                    if item.objectName():
                        names.append(str(item.objectName()))
                    pending.extend(item.childItems())
                print(f"[shot] compose overlay unavailable: {target_name or compose_dialog}; named={names[:30]}")
            scenario_ready = True
            return

        def copy_selected_message(messages) -> None:
            nonlocal scenario_error, scenario_ready
            control = find_item("selectionCopyButton")
            row = messages.model.find(select_message_id) or {}
            expected = str(row.get("copyText") or "")
            if control is None or not control.isVisible() or not control.isEnabled() or not expected:
                print("[shot] selected-message Copy control is unavailable")
                scenario_error = True
                scenario_ready = True
                return
            accessible = QAccessible.queryAccessibleInterface(control)
            name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
            control.forceActiveFocus()
            if not bool(control.property("activeFocus")):
                print("[shot] selected-message Copy control did not receive keyboard focus")
                scenario_error = True
                scenario_ready = True
                return
            QTest.keyClick(win, Qt.Key.Key_Space)
            copied = QGuiApplication.clipboard().text()
            if expected in copied and not messages.selectMode:
                print(f"[shot] selection Copy accessible name={name!r}; Space copied the selected message and exited selection mode")
                scenario_ready = True
            else:
                print(f"[shot] selection Copy did not complete: name={name!r}, copied={copied!r}, selectMode={messages.selectMode}")
                scenario_error = True
                scenario_ready = True

        def delete_selected_message(messages) -> None:
            nonlocal scenario_error, scenario_ready
            control = find_item("selectionDeleteButton")
            if control is None or not control.isVisible() or not control.isEnabled():
                print("[shot] selected-message Delete control is unavailable")
                scenario_error = True
                scenario_ready = True
                return
            accessible = QAccessible.queryAccessibleInterface(control)
            initial_name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
            control.forceActiveFocus()
            if not bool(control.property("activeFocus")):
                print("[shot] selected-message Delete control did not receive keyboard focus")
                scenario_error = True
                scenario_ready = True
                return
            QTest.keyClick(win, Qt.Key.Key_Space)
            if not messages.selectConfirmDelete:
                print("[shot] first Delete Space did not arm confirmation")
                scenario_error = True
                scenario_ready = True
                return
            confirm_name = "the same Delete action"
            QTest.keyClick(win, Qt.Key.Key_Space)
            deadline = time.monotonic() + 5

            def confirm_delete() -> None:
                nonlocal scenario_error, scenario_ready
                row = messages.model.find(select_message_id) or {}
                if int(row.get("revokedAt") or 0) > 0 and not messages.selectMode:
                    print(f"[shot] selection Delete accessible name={initial_name!r}; second Space confirmed {confirm_name!r} and exited selection mode")
                    scenario_ready = True
                elif time.monotonic() < deadline:
                    QTimer.singleShot(50, confirm_delete)
                else:
                    print(f"[shot] selection Delete did not finish: revokedAt={row.get('revokedAt')!r}, selectMode={messages.selectMode}")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(150, confirm_delete)

        def forward_targets_by_keyboard(chats, target_id: str, message_ids: list[str]) -> None:
            nonlocal scenario_error, scenario_ready
            deadline = time.monotonic() + 8
            target_index = next((i for i in range(int(chats.model.count))
                                 if str((chats.model.get(i) or {}).get("id") or "") == target_id), -1)
            if target_index < 0:
                print(f"[shot] forward target chat does not exist: {target_id}")
                scenario_error = True
                scenario_ready = True
                return
            target_chat = chats.model.get(target_index) or {}
            target_name = str(target_chat.get("name") or target_id)

            def choose_and_send() -> None:
                nonlocal scenario_error, scenario_ready
                picker = find_item("forwardDialog")
                search = find_item("forwardChatSearch")
                target_list = find_item("forwardTargetsList")
                forward_button = find_item("forwardSubmitButton")
                if (picker is None or search is None or target_list is None or forward_button is None
                        or not bool(search.property("focused"))):
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, choose_and_send)
                    else:
                        print("[shot] keyboard-forward dialog or initial search focus did not become ready:",
                              {"picker": picker is not None,
                               "search": search is not None,
                               "searchFocused": bool(search.property("focused")) if search is not None else None,
                               "list": target_list is not None,
                               "button": forward_button is not None})
                        scenario_error = True
                        scenario_ready = True
                    return
                search.setProperty("text", target_name)
                target_list.setProperty("currentIndex", target_index)
                QTest.keyClick(win, Qt.Key.Key_Tab)
                if not bool(target_list.property("activeFocus")):
                    print("[shot] Tab from forward search did not focus target list")
                    scenario_error = True
                    scenario_ready = True
                    return
                target_row = find_item("forwardTargetRow_" + target_id)
                accessible = QAccessible.queryAccessibleInterface(target_row) if target_row is not None else None
                name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                if (target_row is None or not target_row.isVisible()
                        or int(target_list.property("currentIndex")) != target_index):
                    print(f"[shot] keyboard-forward target row is unavailable: {target_id} (index={target_list.property('currentIndex')})")
                    scenario_error = True
                    scenario_ready = True
                    return
                finished = []
                chats.forwardFinished.connect(lambda ok: finished.append(bool(ok)))
                QTest.keyClick(win, Qt.Key.Key_Space)
                selected_value = picker.property("selectedTargets")
                selected = selected_value.toVariant() if hasattr(selected_value, "toVariant") else selected_value
                if target_id not in selected:
                    print(f"[shot] Space did not select forward target {target_id}: {selected!r}")
                    scenario_error = True
                    scenario_ready = True
                    return
                QTest.keyClick(win, Qt.Key.Key_Tab)
                if not bool(forward_button.property("activeFocus")):
                    print("[shot] Tab from forward target list did not focus the Forward button")
                    scenario_error = True
                    scenario_ready = True
                    return
                QTest.keyClick(win, Qt.Key.Key_Space)

                def confirm_forward() -> None:
                    nonlocal scenario_error, scenario_ready
                    if finished and finished[-1]:
                        print(f"[shot] forward target accessible name={name!r}; Tab/Space forwarded {','.join(message_ids)} to {target_name}")
                        scenario_ready = True
                    elif finished and not finished[-1]:
                        print("[shot] QML Forward button reported a failed forward")
                        scenario_error = True
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, confirm_forward)
                    else:
                        print(f"[shot] keyboard-forward did not finish: {finished!r}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(100, confirm_forward)

            QTimer.singleShot(120, choose_and_send)

        def multi_select_and_forward(messages, chats, message_ids: list[str], target_id: str) -> None:
            nonlocal scenario_error, scenario_ready
            deadline = time.monotonic() + 8
            if any(not messages.model.contains(message_id) for message_id in message_ids):
                print(f"[shot] multi-forward message is not loaded: {message_ids!r}")
                scenario_error = True
                scenario_ready = True
                return

            def select_first_message() -> None:
                nonlocal scenario_error, scenario_ready
                message_id = message_ids[0]
                bubble = find_item("messageBubble_" + message_id)
                bubble_root = find_item("messageBubbleRoot_" + message_id)
                if bubble is None or not bubble.isVisible() or bubble_root is None:
                    messages.jumpTo(message_id)
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, select_first_message)
                    else:
                        print(f"[shot] first multi-forward bubble did not become visible: {message_id}")
                        scenario_error = True
                        scenario_ready = True
                    return
                center = bubble.mapToScene(QPointF(bubble.width() / 2, bubble.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.RightButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def activate_select() -> None:
                    nonlocal scenario_error, scenario_ready
                    select_action = find_item("messageSelectAction_" + message_id)
                    if not bool(bubble_root.property("contextMenuOpen")) or select_action is None or not bool(select_action.property("visible")):
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, activate_select)
                        else:
                            print(f"[shot] Select context action did not open for {message_id}")
                            scenario_error = True
                            scenario_ready = True
                        return
                    select_action.forceActiveFocus()
                    QTest.keyClick(win, Qt.Key.Key_Return)
                    if not messages.selectMode or int(messages.selectedCount) != 1:
                        print(f"[shot] Select context action did not select the first message: count={messages.selectedCount}")
                        scenario_error = True
                        scenario_ready = True
                        return
                    select_next_message(1)

                QTimer.singleShot(60, activate_select)

            def select_next_message(position: int) -> None:
                nonlocal scenario_error, scenario_ready
                if position >= len(message_ids):
                    if int(messages.selectedCount) != len(message_ids):
                        print(f"[shot] keyboard selection count mismatch: {messages.selectedCount} != {len(message_ids)}")
                        scenario_error = True
                        scenario_ready = True
                        return
                    button = find_item("selectionForwardButton")
                    if button is None or not button.isVisible() or not button.isEnabled():
                        print("[shot] selection-toolbar Forward button is unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    button.forceActiveFocus()
                    QTest.keyClick(win, Qt.Key.Key_Space)
                    QTimer.singleShot(150, lambda: forward_targets_by_keyboard(chats, target_id, message_ids))
                    return

                message_id = message_ids[position]
                messages.jumpTo(message_id)

                def activate_message() -> None:
                    nonlocal scenario_error, scenario_ready
                    bubble_root = find_item("messageBubbleRoot_" + message_id)
                    if bubble_root is None or not bubble_root.isVisible():
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, activate_message)
                        else:
                            print(f"[shot] message bubble did not become visible for multi-forward selection: {message_id}")
                            scenario_error = True
                            scenario_ready = True
                        return
                    bubble_root.forceActiveFocus()
                    if not bool(bubble_root.property("activeFocus")):
                        print(f"[shot] message bubble could not receive selection focus: {message_id}")
                        scenario_error = True
                        scenario_ready = True
                        return
                    QTest.keyClick(win, Qt.Key.Key_Space)
                    row = messages.model.find(message_id) or {}
                    if not bool(row.get("selected")):
                        print(f"[shot] Space did not select message {message_id}")
                        scenario_error = True
                        scenario_ready = True
                        return
                    select_next_message(position + 1)

                QTimer.singleShot(80, activate_message)

            QTimer.singleShot(100, select_first_message)

        def message_context_action_by_keyboard(messages, message_id: str, action: str) -> None:
            nonlocal scenario_error, scenario_ready
            deadline = time.monotonic() + 5

            def open_reply_context() -> None:
                nonlocal scenario_error, scenario_ready
                bubble = find_item("messageBubble_" + message_id)
                bubble_root = find_item("messageBubbleRoot_" + message_id)
                if bubble is None or not bubble.isVisible():
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_reply_context)
                    else:
                        print(f"[shot] message bubble did not become visible for keyboard {action}: {message_id}")
                        scenario_error = True
                        scenario_ready = True
                    return
                if bubble_root is None:
                    print("[shot] message context-menu Reply control is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = bubble.mapToScene(QPointF(bubble.width() / 2, bubble.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.RightButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def activate_reply() -> None:
                    nonlocal scenario_error, scenario_ready
                    if action.startswith("Reaction:"):
                        emoji = action.split(":", 1)[1]
                        control = find_item("messageReactionAction_" + message_id + "_" + emoji)
                    elif action == "Star":
                        emoji = ""
                        control = find_item("messageStarAction_" + message_id)
                    elif action == "Pin":
                        emoji = ""
                        control = find_item("messagePinAction_" + message_id)
                    elif action == "Delete":
                        emoji = ""
                        control = find_item("messageDeleteAction_" + message_id)
                    elif action == "Copy":
                        emoji = ""
                        control = find_item("messageCopyAction_" + message_id)
                    elif action == "Forward":
                        emoji = ""
                        control = find_item("messageForwardAction_" + message_id)
                    else:
                        emoji = ""
                        control = find_item(f"message{action}Action_" + message_id)
                    if (not bool(bubble_root.property("contextMenuOpen")) or control is None
                            or not bool(control.property("visible"))):
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, activate_reply)
                        else:
                            print(f"[shot] message context menu action did not open: {action} ({message_id})")
                            scenario_error = True
                            scenario_ready = True
                        return
                    accessible = QAccessible.queryAccessibleInterface(control)
                    name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                    if action.startswith("Reaction:"):
                        center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))
                        reaction_deadline = time.monotonic() + 5

                        def verify_quick_reaction() -> None:
                            nonlocal scenario_error, scenario_ready
                            row = messages.model.find(message_id) or {}
                            reacted = any(str(reaction.get("emoji") or "") == emoji
                                          and "me" in (reaction.get("senders") or [])
                                          for reaction in (row.get("reactions") or []))
                            if reacted:
                                print(f"[shot] message context quick reaction accessible name={name!r}; mouse applied {emoji} to {message_id}")
                                scenario_ready = True
                            elif time.monotonic() < reaction_deadline:
                                QTimer.singleShot(50, verify_quick_reaction)
                            else:
                                print(f"[shot] context quick reaction did not apply {emoji} to {message_id}")
                                scenario_error = True
                                scenario_ready = True

                        QTimer.singleShot(100, verify_quick_reaction)
                        return
                    control.forceActiveFocus()
                    if not bool(control.property("activeFocus")):
                        print(f"[shot] message context-menu {action} did not receive keyboard focus")
                        scenario_error = True
                        scenario_ready = True
                        return
                    QTest.keyClick(win, Qt.Key.Key_Return)
                    if action == "Copy":
                        expected = str((messages.model.find(message_id) or {}).get("copyText") or "")
                        copied = QGuiApplication.clipboard().text()
                        if expected and expected in copied:
                            print(f"[shot] message context action accessible name={name!r}; Enter copied {message_id}")
                            scenario_ready = True
                        else:
                            print(f"[shot] message context Copy did not update clipboard: {copied!r}")
                            scenario_error = True
                            scenario_ready = True
                        return
                    if action == "Forward":
                        chats = services.exposed.get("Chats")
                        QTimer.singleShot(120, lambda: forward_targets_by_keyboard(chats, context_forward_target, [message_id]))
                        return
                    if action == "Info":
                        info_deadline = time.monotonic() + 6

                        def verify_message_info() -> None:
                            nonlocal scenario_error, scenario_ready
                            info = messages.info
                            if (messages.infoOpen and not info.get("loading")
                                    and str(info.get("messageId") or "") == message_id):
                                sheet = find_item("messageInfoSheet")
                                print(f"[shot] message context action accessible name={name!r}; Enter opened receipts: read={len(info.get('read') or [])} delivered={len(info.get('delivered') or [])} remaining={len(info.get('remaining') or [])}; sheet_visible={bool(sheet and sheet.isVisible())}")
                                scenario_ready = True
                            elif time.monotonic() < info_deadline:
                                QTimer.singleShot(40, verify_message_info)
                            else:
                                print(f"[shot] Message info did not load for {message_id}: {info!r}")
                                scenario_error = True
                                scenario_ready = True

                        QTimer.singleShot(40, verify_message_info)
                        return
                    if action == "Pin":
                        pin_deadline = time.monotonic() + 5

                        def choose_default_pin_duration() -> None:
                            nonlocal scenario_error, scenario_ready
                            duration = find_item("menuItem_pin-604800")
                            if duration is None or not bool(duration.property("visible")):
                                if time.monotonic() < pin_deadline:
                                    QTimer.singleShot(40, choose_default_pin_duration)
                                else:
                                    print(f"[shot] Pin duration menu did not open for {message_id}")
                                    scenario_error = True
                                    scenario_ready = True
                                return
                            duration.forceActiveFocus()
                            QTest.keyClick(win, Qt.Key.Key_Return)

                            def verify_pin() -> None:
                                nonlocal scenario_error, scenario_ready
                                row = messages.model.find(message_id) or {}
                                if bool(row.get("pinned")):
                                    print(f"[shot] message context action accessible name={name!r}; Enter pinned {message_id}")
                                    scenario_ready = True
                                elif time.monotonic() < pin_deadline:
                                    QTimer.singleShot(50, verify_pin)
                                else:
                                    print(f"[shot] context Pin did not update message {message_id}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(120, verify_pin)

                        QTimer.singleShot(60, choose_default_pin_duration)
                        return
                    if action == "Delete":
                        delete_deadline = time.monotonic() + 5

                        def verify_deleted() -> None:
                            nonlocal scenario_error, scenario_ready
                            row = messages.model.find(message_id) or {}
                            if int(row.get("revokedAt") or 0) > 0:
                                print(f"[shot] message context action accessible name={name!r}; Enter deleted {message_id}")
                                scenario_ready = True
                            elif time.monotonic() < delete_deadline:
                                QTimer.singleShot(50, verify_deleted)
                            else:
                                print(f"[shot] context Delete did not revoke message {message_id}: {row.get('revokedAt')!r}")
                                scenario_error = True
                                scenario_ready = True

                        QTimer.singleShot(100, verify_deleted)
                        return
                    if action == "Reply":
                        state = services.exposed.get("Composer").replyTo
                        completed = str(state.get("id") or "") == message_id
                        description = f"started reply for {message_id}"
                    elif action == "Edit":
                        state = services.exposed.get("Composer").editing
                        completed = str(state.get("id") or "") == message_id
                        description = f"started edit for {message_id}"
                    elif action == "Star":
                        row = messages.model.find(message_id) or {}
                        completed = bool(row.get("starred"))
                        description = f"starred {message_id}"
                    elif action == "Pin":
                        row = messages.model.find(message_id) or {}
                        completed = bool(row.get("pinned"))
                        description = f"pinned {message_id}"
                    elif action == "Delete":
                        row = messages.model.find(message_id) or {}
                        completed = int(row.get("revokedAt") or 0) > 0
                        description = f"deleted {message_id}"
                    else:
                        row = messages.model.find(message_id) or {}
                        completed = any(
                            str(reaction.get("emoji") or "") == emoji
                            and "me" in (reaction.get("senders") or [])
                            for reaction in (row.get("reactions") or [])
                        )
                        description = f"applied reaction {emoji} to {message_id}"
                    if completed:
                        print(f"[shot] message context action accessible name={name!r}; Enter {description}")
                        scenario_ready = True
                    else:
                        print(f"[shot] Enter did not start message {action.lower()}: {state!r}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(60, activate_reply)

            messages.jumpTo(message_id)
            QTimer.singleShot(80, open_reply_context)

        def react_chip_by_keyboard(messages, message_id: str, emoji: str) -> None:
            nonlocal scenario_error, scenario_ready
            deadline = time.monotonic() + 6
            messages.jumpTo(message_id)

            def activate_reaction_chip() -> None:
                nonlocal scenario_error, scenario_ready
                chip = find_item("messageReactionChip_" + message_id + "_" + emoji)
                if chip is None or not chip.isVisible():
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, activate_reaction_chip)
                    else:
                        print(f"[shot] reaction chip did not become visible: {message_id} {emoji}")
                        scenario_error = True
                        scenario_ready = True
                    return
                accessible = QAccessible.queryAccessibleInterface(chip)
                name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                chip.forceActiveFocus()
                if not bool(chip.property("activeFocus")):
                    print("[shot] reaction chip did not receive keyboard focus")
                    scenario_error = True
                    scenario_ready = True
                    return
                QTest.keyClick(win, Qt.Key.Key_Space)

                def confirm_reaction() -> None:
                    nonlocal scenario_error, scenario_ready
                    row = messages.model.find(message_id) or {}
                    reacted = any(str(item.get("emoji") or "") == emoji and "me" in (item.get("senders") or [])
                                  for item in (row.get("reactions") or []))
                    if reacted:
                        print(f"[shot] reaction chip accessible name={name!r}; Space applied {emoji} to {message_id}")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, confirm_reaction)
                    else:
                        print(f"[shot] Space did not apply reaction {emoji} to {message_id}: {row.get('reactions')!r}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(80, confirm_reaction)

            QTimer.singleShot(80, activate_reaction_chip)

        def set_disappearing_by_keyboard(chats, seconds: int) -> None:
            nonlocal scenario_error, scenario_ready
            menu_button = find_item("chatHeaderMenuButton")
            if menu_button is None or not menu_button.isVisible() or not menu_button.isEnabled():
                print("[shot] chat-header menu button is unavailable")
                scenario_error = True
                scenario_ready = True
                return
            center = menu_button.mapToScene(QPointF(menu_button.width() / 2, menu_button.height() / 2))
            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                             QPoint(round(center.x()), round(center.y())))
            deadline = time.monotonic() + 6

            def open_timer_options() -> None:
                nonlocal scenario_error, scenario_ready
                current = find_item("menuItem_pin")
                target = find_item("menuItem_disappearing")
                if current is None or target is None or not bool(current.property("activeFocus")):
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_timer_options)
                    else:
                        print("[shot] chat-header menu did not receive its initial keyboard focus")
                        scenario_error = True
                        scenario_ready = True
                    return
                for _ in range(6):
                    QTest.keyClick(win, Qt.Key.Key_Down)
                accessible = QAccessible.queryAccessibleInterface(target)
                name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                if not bool(target.property("activeFocus")):
                    print(f"[shot] ArrowDown navigation did not reach disappearing menu item: {name!r}")
                    scenario_error = True
                    scenario_ready = True
                    return
                QTest.keyClick(win, Qt.Key.Key_Space)

                def choose_duration() -> None:
                    nonlocal scenario_error, scenario_ready
                    option = find_item("disappearingOption_" + str(seconds))
                    if option is None or not option.isVisible():
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, choose_duration)
                        else:
                            print(f"[shot] disappearing-duration option did not open: {seconds}")
                            scenario_error = True
                            scenario_ready = True
                        return
                    option.forceActiveFocus()
                    QTest.keyClick(win, Qt.Key.Key_Space)
                    duration_deadline = time.monotonic() + 5

                    def confirm_duration() -> None:
                        nonlocal scenario_error, scenario_ready
                        if int(chats.activeChat.get("disappearingSecs") or 0) == seconds:
                            indicator = find_item("chatHeaderDisappearing")
                            if seconds > 0 and (indicator is None or not indicator.isVisible()):
                                print("[shot] disappearing timer updated but the header indicator is not visible")
                                scenario_error = True
                            else:
                                print(f"[shot] menu item accessible name={name!r}; ArrowDown/Space opened options; Space set {chats.disappearingLabel(seconds)}")
                            scenario_ready = True
                        elif time.monotonic() < duration_deadline:
                            QTimer.singleShot(50, confirm_duration)
                        else:
                            print(f"[shot] disappearing duration did not update: expected={seconds}, actual={chats.activeChat.get('disappearingSecs')!r}")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(120, confirm_duration)

                QTimer.singleShot(100, choose_duration)

            QTimer.singleShot(100, open_timer_options)

        def verify_disappearing_message(messages, message_id: str) -> None:
            nonlocal scenario_error, scenario_ready
            messages.jumpTo(message_id)
            deadline = time.monotonic() + 6

            def check_note() -> None:
                nonlocal scenario_error, scenario_ready
                note = find_item("messageDisappearing_" + message_id)
                if note is not None and note.isVisible():
                    accessible = QAccessible.queryAccessibleInterface(note)
                    name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                    if name:
                        print(f"[shot] disappearing message accessible name={name!r}")
                        scenario_ready = True
                    else:
                        print(f"[shot] disappearing message note has no accessible name: {message_id}")
                        scenario_error = True
                        scenario_ready = True
                elif time.monotonic() < deadline:
                    QTimer.singleShot(50, check_note)
                else:
                    print(f"[shot] disappearing message note did not become visible: {message_id}")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(100, check_note)

        if tray_close_cycle:
            if shot_tray is None:
                scenario_error = True
                scenario_ready = True
                return

            def check_hidden() -> None:
                nonlocal scenario_ready, scenario_error
                if shot_tray.hide_count != 1 or win.isVisible():
                    print(f"[shot] close-to-tray failed: hides={shot_tray.hide_count} visible={win.isVisible()}")
                    scenario_error = True
                    scenario_ready = True
                    return
                shot_tray.showWindowRequested.emit()
                QTimer.singleShot(120, check_raised)

            def check_raised() -> None:
                nonlocal scenario_ready, scenario_error
                if win.isVisible():
                    shot_tray.toggleWindowRequested.emit()
                    QTimer.singleShot(100, check_toggled_hidden)
                else:
                    print("[shot] Platform show request did not restore the tray-hidden window")
                    scenario_error = True
                    scenario_ready = True

            def check_toggled_hidden() -> None:
                nonlocal scenario_ready, scenario_error
                if not win.isVisible():
                    shot_tray.toggleWindowRequested.emit()
                    QTimer.singleShot(120, check_toggled_visible)
                else:
                    print("[shot] Platform toggle did not hide the visible window")
                    scenario_error = True
                    scenario_ready = True

            def check_toggled_visible() -> None:
                nonlocal scenario_ready, scenario_error
                if win.isVisible():
                    print("[shot] close-to-tray QML close was intercepted and show/toggle restored the window")
                    scenario_ready = True
                else:
                    print("[shot] Platform toggle did not restore the hidden window")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(200, lambda: (win.close(), QTimer.singleShot(80, check_hidden)))
            return
        if tab_chat_list:
            chat_list = services.exposed.get("ChatList")
            search = find_item("chatSearchField")
            archive = find_item("archiveChatsButton")
            if chat_list is None or search is None or archive is None:
                print("[shot] chat list Tab controls are unavailable")
                scenario_error = True
                scenario_ready = True
                return
            deadline = time.monotonic() + 8

            def verify_tab_order() -> None:
                nonlocal scenario_error, scenario_ready
                first_chat = chat_list.get(0) if int(chat_list.count) > 0 else None
                first_id = str((first_chat or {}).get("id") or "")
                row_button = find_item("chatRowOpen_" + first_id) if first_id else None
                if not services.hub.connected or row_button is None or not row_button.isVisible():
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, verify_tab_order)
                    else:
                        print("[shot] first chat row did not load for Tab traversal")
                        scenario_error = True
                        scenario_ready = True
                    return
                if not QMetaObject.invokeMethod(search, "forceActiveFocus"):
                    print("[shot] chat search input could not receive focus")
                    scenario_error = True
                    scenario_ready = True
                    return
                QTest.keyClick(win, Qt.Key.Key_Tab)
                if not bool(archive.property("activeFocus")):
                    print(f"[shot] Tab from chat search input did not focus Archived (search={search.property('focused')}, archive={archive.property('activeFocus')})")
                    scenario_error = True
                    scenario_ready = True
                    return
                QTest.keyClick(win, Qt.Key.Key_Tab)
                if bool(row_button.property("activeFocus")):
                    archive_accessible = QAccessible.queryAccessibleInterface(archive)
                    row_accessible = QAccessible.queryAccessibleInterface(row_button)
                    archive_name = archive_accessible.text(QAccessible.Text.Name) if archive_accessible else ""
                    row_name = row_accessible.text(QAccessible.Text.Name) if row_accessible else ""
                    print(f"[shot] Tab order: search → {archive_name!r} → {row_name!r}")
                    scenario_ready = True
                else:
                    print(f"[shot] second Tab did not focus first chat row {first_id}")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(200, verify_tab_order)
            return
        if keyboard_archive:
            chat_list = services.exposed.get("ChatList")
            control = find_item("archiveChatsButton")
            if chat_list is None or control is None or not control.isVisible() or not control.isEnabled():
                print("[shot] Archived chat button is unavailable")
                scenario_error = True
                scenario_ready = True
                return
            accessible = QAccessible.queryAccessibleInterface(control)
            name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
            control.forceActiveFocus()
            QTest.keyClick(win, Qt.Key.Key_Space)
            if chat_list.archivedMode:
                print(f"[shot] Archived button accessible name={name!r}; Space opened archive")
                scenario_ready = True
            else:
                print(f"[shot] Archived Space did not open archive (name={name!r})")
                scenario_error = True
                scenario_ready = True
            return
        if chat_row_arrow:
            chat_list = services.exposed.get("ChatList")
            if chat_list is None:
                print("[shot] chat list model is unavailable for keyboard navigation")
                scenario_error = True
                scenario_ready = True
                return
            deadline = time.monotonic() + 8
            key = Qt.Key.Key_Up if chat_row_key == "Up" else Qt.Key.Key_Down

            def move_chat_row_focus() -> None:
                nonlocal scenario_error, scenario_ready
                rows = [chat_list.get(i) or {} for i in range(int(chat_list.count))]
                start_index = next((i for i, row in enumerate(rows)
                                    if str(row.get("id") or "") == chat_row_start), -1)
                if not services.hub.connected or start_index < 0:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, move_chat_row_focus)
                    else:
                        print(f"[shot] chat row did not load for keyboard navigation: {chat_row_start}")
                        scenario_error = True
                        scenario_ready = True
                    return
                start_control = find_item("chatRowOpen_" + chat_row_start)
                if start_control is None or not start_control.isVisible():
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, move_chat_row_focus)
                    else:
                        print(f"[shot] starting chat row is unavailable: {chat_row_start}")
                        scenario_error = True
                        scenario_ready = True
                    return
                target_index = max(0, min(len(rows) - 1, start_index + (1 if chat_row_key == "Down" else -1)))
                target_id = str(rows[target_index].get("id") or "")
                start_control.forceActiveFocus()
                QTest.keyClick(win, key)

                def confirm_focus_move() -> None:
                    nonlocal scenario_error, scenario_ready
                    target = find_item("chatRowOpen_" + target_id)
                    if target is not None and bool(target.property("activeFocus")):
                        accessible = QAccessible.queryAccessibleInterface(target)
                        name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                        print(f"[shot] chat row {chat_row_key} moved focus to accessible row {name!r}")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, confirm_focus_move)
                    else:
                        print(f"[shot] chat row {chat_row_key} did not move focus to {target_id}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(100, confirm_focus_move)

            QTimer.singleShot(150, move_chat_row_focus)
            return
        if keyboard_chat_id:
            chat_list = services.exposed.get("ChatList")
            if chat_list is None:
                print("[shot] chat list model is unavailable for keyboard selection")
                scenario_error = True
                scenario_ready = True
                return
            deadline = time.monotonic() + 8

            def activate_chat_row() -> None:
                nonlocal scenario_ready, scenario_error
                has_chat = any(
                    str((chat_list.get(i) or {}).get("id") or "") == keyboard_chat_id
                    for i in range(int(chat_list.count))
                )
                if not services.hub.connected or not has_chat:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, activate_chat_row)
                    else:
                        print(f"[shot] chat row did not load for keyboard selection: {keyboard_chat_id}")
                        scenario_error = True
                        scenario_ready = True
                    return
                control = find_item("chatRowOpen_" + keyboard_chat_id)
                if control is None or not control.isVisible() or not control.isEnabled():
                    if time.monotonic() < deadline:
                        QTimer.singleShot(50, activate_chat_row)
                    else:
                        print(f"[shot] chat row button is unavailable: {keyboard_chat_id}")
                        scenario_error = True
                        scenario_ready = True
                    return
                accessible = QAccessible.queryAccessibleInterface(control)
                name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                control.forceActiveFocus()
                QTest.keyClick(win, Qt.Key.Key_Space)
                if services.app.activeChatId == keyboard_chat_id:
                    print(f"[shot] chat row accessible name={name!r}; Space opened chat")
                    scenario_ready = True
                else:
                    print(f"[shot] chat row Space did not open chat: {keyboard_chat_id} (name={name!r})")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(200, activate_chat_row)
            return
        if docs_toc_index is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            docs = bot.docs
            deadline = time.monotonic() + 8

            def click_docs_toc() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or docs.loading:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, click_docs_toc)
                    else:
                        print("[shot] API documentation did not load")
                        scenario_error = True
                        scenario_ready = True
                    return
                toc = list(docs.toc)
                if docs_toc_index >= len(toc):
                    print(f"[shot] documentation TOC index unavailable: {docs_toc_index}")
                    scenario_error = True
                    scenario_ready = True
                    return
                row = find_item(f"docsTocRow_{docs_toc_index}")
                view = find_item("docsBlocksList")
                if row is None or view is None or not row.isVisible():
                    print("[shot] documentation TOC row is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                slug = str(toc[docs_toc_index].get("slug") or "")
                block_index = int(docs.indexOfSlug(slug))
                before = float(view.property("contentY"))
                center = row.mapToScene(QPointF(row.width() / 2, row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def confirm_docs_scroll() -> None:
                    nonlocal scenario_ready, scenario_error
                    after = float(view.property("contentY"))
                    if block_index >= 0 and after != before:
                        print(f"[shot] documentation TOC mouse click: {slug} → block {block_index}")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, confirm_docs_scroll)
                    else:
                        print(f"[shot] documentation TOC did not scroll: {slug} (contentY {before} → {after})")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(120, confirm_docs_scroll)

            QTimer.singleShot(250, click_docs_toc)
        elif bot_assistant_prompt is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers
            deadline = time.monotonic() + 8

            def open_assistant_prompt() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_assistant_prompt)
                    else:
                        print("[shot] trigger list did not load before assistant prompt")
                        scenario_error = True
                        scenario_ready = True
                    return
                row = find_item("botRow_t1")
                if row is None or not row.isVisible():
                    print("[shot] assistant trigger row is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = row.mapToScene(QPointF(min(180, row.width() / 2), row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def open_panel() -> None:
                    nonlocal scenario_ready, scenario_error
                    page = find_item("botCollectionPage")
                    if page is None or str(page.property("currentId") or "") != "t1":
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, open_panel)
                        else:
                            print("[shot] assistant trigger editor did not open")
                            scenario_error = True
                            scenario_ready = True
                        return
                    button = find_item("botAssistantOpen")
                    if button is None or not button.isVisible():
                        print("[shot] AI assistant button is unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))

                    def submit_prompt() -> None:
                        nonlocal scenario_ready, scenario_error
                        input_field = find_item("botAssistantInput")
                        send_button = find_item("botAssistantSend")
                        if input_field is None or send_button is None:
                            print("[shot] AI assistant prompt controls are unavailable")
                            scenario_error = True
                            scenario_ready = True
                            return
                        input_field.setProperty("text", bot_assistant_prompt)
                        center = send_button.mapToScene(QPointF(send_button.width() / 2, send_button.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))

                        def confirm_answer() -> None:
                            nonlocal scenario_ready, scenario_error
                            if bot.assistant.model.count >= 2 and not bot.assistant.loading:
                                print("[shot] AI assistant prompt answered through QML controls")
                                scenario_ready = True
                            elif time.monotonic() < deadline:
                                QTimer.singleShot(50, confirm_answer)
                            else:
                                print("[shot] AI assistant did not return a response")
                                scenario_error = True
                                scenario_ready = True

                        QTimer.singleShot(120, confirm_answer)

                    QTimer.singleShot(180, submit_prompt)

                QTimer.singleShot(180, open_panel)

            QTimer.singleShot(250, open_assistant_prompt)
        elif cron_run_now is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.crons
            deadline = time.monotonic() + 8

            def open_cron_editor() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_cron_editor)
                    else:
                        print("[shot] cron list did not load")
                        scenario_error = True
                        scenario_ready = True
                    return
                row = find_item("botRow_" + cron_run_now)
                if row is None or not row.isVisible():
                    print(f"[shot] cron row is unavailable: {cron_run_now}")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = row.mapToScene(QPointF(min(180, row.width() / 2), row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def click_run_now() -> None:
                    nonlocal scenario_ready, scenario_error
                    page = find_item("botCollectionPage")
                    if page is None or str(page.property("currentId") or "") != cron_run_now:
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, click_run_now)
                        else:
                            print("[shot] cron editor did not open")
                            scenario_error = True
                            scenario_ready = True
                        return
                    flickable = find_item("botEditorFlickable")
                    if flickable is not None:
                        max_y = max(0, float(flickable.property("contentHeight")) - float(flickable.property("height")))
                        flickable.setProperty("contentY", max_y)
                    button = find_item("botRunTest")
                    if button is None or not button.isVisible():
                        print("[shot] cron Run now button is unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))
                    print(f"[shot] cron Run now clicked through QML controls: {cron_run_now}")
                    scenario_ready = True

                QTimer.singleShot(180, click_run_now)

            QTimer.singleShot(250, open_cron_editor)
        elif bot_test_kind is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers if bot_test_kind == "triggers" else bot.webhooks
            deadline = time.monotonic() + 8
            editor_scrolled = [False]

            def open_trigger_test() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_trigger_test)
                    else:
                        print("[shot] trigger list did not load before test")
                        scenario_error = True
                        scenario_ready = True
                    return
                row = find_item("botRow_" + bot_test_id)
                if row is None or not row.isVisible():
                    print(f"[shot] {bot_test_kind[:-1]} row is unavailable: {bot_test_id}")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = row.mapToScene(QPointF(min(180, row.width() / 2), row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def click_run_test() -> None:
                    nonlocal scenario_ready, scenario_error
                    page = find_item("botCollectionPage")
                    button = find_item("botRunTest")
                    if page is None or str(page.property("currentId") or "") != bot_test_id:
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, click_run_test)
                        else:
                            print(f"[shot] {bot_test_kind[:-1]} editor did not open")
                            scenario_error = True
                            scenario_ready = True
                        return
                    if not editor_scrolled[0]:
                        flickable = find_item("botEditorFlickable")
                        if flickable is not None:
                            max_y = max(0, float(flickable.property("contentHeight")) - float(flickable.property("height")))
                            flickable.setProperty("contentY", max_y)
                        editor_scrolled[0] = True
                        QTimer.singleShot(120, click_run_test)
                        return
                    if button is None or not button.isVisible():
                        print("[shot] trigger Run test button is unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    draft = page.property("draft")
                    center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))

                    def confirm_test_result() -> None:
                        nonlocal scenario_ready, scenario_error
                        entries = page.property("testEntries")
                        if hasattr(entries, "property"):
                            entry_count = entries.property("length").toInt()
                        else:
                            entry_count = len(entries or [])
                        if entry_count > 0:
                            print(f"[shot] {bot_test_kind[:-1]} test executed through QML controls: {entry_count} entries")
                            scenario_ready = True
                        elif time.monotonic() < deadline:
                            QTimer.singleShot(50, confirm_test_result)
                        else:
                            print(f"[shot] {bot_test_kind[:-1]} test produced no debugger output")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(120, confirm_test_result)

                QTimer.singleShot(180, click_run_test)

            QTimer.singleShot(250, open_trigger_test)
        elif webhook_logs_id is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.webhooks
            deadline = time.monotonic() + 8

            def open_webhook_logs() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_webhook_logs)
                    else:
                        print("[shot] webhook list did not load")
                        scenario_error = True
                        scenario_ready = True
                    return
                row = find_item("botRow_" + webhook_logs_id)
                if row is None or not row.isVisible():
                    print(f"[shot] webhook row is unavailable: {webhook_logs_id}")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = row.mapToScene(QPointF(min(180, row.width() / 2), row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def click_logs_button() -> None:
                    nonlocal scenario_ready, scenario_error
                    page = find_item("botCollectionPage")
                    button = find_item("webhookViewLogs")
                    if page is None or str(page.property("currentId") or "") != webhook_logs_id:
                        if time.monotonic() < deadline:
                            QTimer.singleShot(40, click_logs_button)
                        else:
                            print("[shot] webhook editor did not open")
                            scenario_error = True
                            scenario_ready = True
                        return
                    if button is None or not button.isVisible():
                        print("[shot] webhook View logs button is unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))

                    def confirm_logs() -> None:
                        nonlocal scenario_ready, scenario_error
                        logs = bot.logs
                        if str(logs.webhookId or "") == webhook_logs_id and not logs.loading:
                            print(f"[shot] webhook logs opened through QML: {webhook_logs_id} ({logs.model.count} rows)")
                            scenario_ready = True
                        elif time.monotonic() < deadline:
                            QTimer.singleShot(50, confirm_logs)
                        else:
                            print(f"[shot] webhook logs did not load: {webhook_logs_id}")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(120, confirm_logs)

                QTimer.singleShot(180, click_logs_button)

            QTimer.singleShot(250, open_webhook_logs)
        elif bot_export_file is not None or bot_import_file is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers
            importing = bot_import_file is not None
            path = bot_import_file if importing else bot_export_file
            deadline = time.monotonic() + 8

            def open_bot_file_dialog() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_bot_file_dialog)
                    else:
                        print("[shot] trigger list did not load before file dialog")
                        scenario_error = True
                        scenario_ready = True
                    return
                page = find_item("botCollectionPage")
                button = find_item("botImportButton" if importing else "botExportButton")
                if page is None or button is None or not button.isVisible():
                    print("[shot] trigger import/export control is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                page.setProperty("fileDialogsForceFallback", True)
                page.setProperty("importFileDialogSelectedFile" if importing else "exportFileDialogCurrentFile",
                                 QUrl.fromLocalFile(path))
                center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))
                QTimer.singleShot(140, accept_bot_file_dialog)

            def accept_bot_file_dialog() -> None:
                nonlocal scenario_ready, scenario_error
                method = "acceptImportFileDialog" if importing else "acceptExportFileDialog"
                page = find_item("botCollectionPage")
                if page is None or not QMetaObject.invokeMethod(page, method):
                    print("[shot] trigger import/export dialog could not accept selection")
                    scenario_error = True
                    scenario_ready = True
                    return
                if importing:
                    QTimer.singleShot(100, confirm_bot_import)
                else:
                    try:
                        data = json.loads(Path(path).read_text(encoding="utf-8"))
                    except (OSError, ValueError):
                        data = None
                    if isinstance(data, list) and data:
                        print(f"[shot] trigger export dialog wrote {len(data)} entries")
                        scenario_ready = True
                    else:
                        print("[shot] trigger export dialog did not write a non-empty JSON array")
                        scenario_error = True
                        scenario_ready = True

            def confirm_bot_import() -> None:
                nonlocal scenario_ready, scenario_error
                imported = [item for item in store.items() if str(item.get("name") or "").startswith("file import ")]
                if imported and not store.loading:
                    print(f"[shot] trigger import dialog created {len(imported)} entries")
                    scenario_ready = True
                elif time.monotonic() < deadline:
                    QTimer.singleShot(50, confirm_bot_import)
                else:
                    print("[shot] trigger import dialog did not create entries")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(250, open_bot_file_dialog)
        elif bot_delete_all:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers
            deadline = time.monotonic() + 8

            def click_trigger_delete_all() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, click_trigger_delete_all)
                    else:
                        print("[shot] trigger list did not load before delete all")
                        scenario_error = True
                        scenario_ready = True
                    return
                button = find_item("botDeleteAll")
                if button is None or not button.isVisible():
                    print("[shot] delete-all control is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def confirm_delete_all() -> None:
                    nonlocal scenario_ready, scenario_error
                    confirm = find_item("botConfirmDeleteAll")
                    if confirm is not None and confirm.isVisible():
                        center = confirm.mapToScene(QPointF(confirm.width() / 2, confirm.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))
                        QTimer.singleShot(100, verify_delete_all)
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(40, confirm_delete_all)
                    else:
                        print("[shot] delete-all confirmation is unavailable")
                        scenario_error = True
                        scenario_ready = True

                def verify_delete_all() -> None:
                    nonlocal scenario_ready, scenario_error
                    if store.model.count == 0 and not store.loading:
                        print("[shot] all triggers deleted through QML confirmation")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, verify_delete_all)
                    else:
                        print(f"[shot] trigger delete all did not complete: remaining={store.model.count}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(80, confirm_delete_all)

            QTimer.singleShot(250, click_trigger_delete_all)
        elif bot_delete_trigger is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers
            deadline = time.monotonic() + 8

            def click_trigger_delete() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, click_trigger_delete)
                    else:
                        print("[shot] trigger list did not load before delete")
                        scenario_error = True
                        scenario_ready = True
                    return
                if store.get(bot_delete_trigger) is None:
                    print(f"[shot] trigger not found: {bot_delete_trigger}")
                    scenario_error = True
                    scenario_ready = True
                    return
                delete_button = find_item("botDelete_" + bot_delete_trigger)
                if delete_button is None or not delete_button.isVisible():
                    print("[shot] trigger delete control is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = delete_button.mapToScene(QPointF(delete_button.width() / 2, delete_button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def confirm_delete() -> None:
                    nonlocal scenario_ready, scenario_error
                    confirm = find_item("botConfirmDelete")
                    if confirm is not None and confirm.isVisible():
                        center = confirm.mapToScene(QPointF(confirm.width() / 2, confirm.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))
                        QTimer.singleShot(100, verify_delete)
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(40, confirm_delete)
                    else:
                        print("[shot] trigger delete confirmation is unavailable")
                        scenario_error = True
                        scenario_ready = True

                def verify_delete() -> None:
                    nonlocal scenario_ready, scenario_error
                    if not store.has(bot_delete_trigger) and not store.loading:
                        print(f"[shot] trigger deleted through QML confirmation: {bot_delete_trigger}")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, verify_delete)
                    else:
                        print(f"[shot] trigger was not deleted: {bot_delete_trigger}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(80, confirm_delete)

            QTimer.singleShot(250, click_trigger_delete)
        elif bot_toggle_trigger is not None:
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers
            deadline = time.monotonic() + 8

            def click_trigger_toggle() -> None:
                nonlocal scenario_ready, scenario_error
                row = store.get(bot_toggle_trigger)
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, click_trigger_toggle)
                    else:
                        print("[shot] trigger list did not load before toggle")
                        scenario_error = True
                        scenario_ready = True
                    return
                if not row:
                    print(f"[shot] trigger not found: {bot_toggle_trigger}")
                    scenario_error = True
                    scenario_ready = True
                    return
                expected = not bool(row.get("is_active"))
                button = find_item("botToggle_" + bot_toggle_trigger)
                if button is None or not button.isVisible():
                    print("[shot] trigger toggle control is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def confirm_toggle() -> None:
                    nonlocal scenario_ready, scenario_error
                    updated = store.get(bot_toggle_trigger) or {}
                    if bool(updated.get("is_active")) == expected:
                        print(f"[shot] trigger toggle applied through QML: {bot_toggle_trigger} → {expected}")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(40, confirm_toggle)
                    else:
                        print(f"[shot] trigger toggle did not apply: {bot_toggle_trigger}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(100, confirm_toggle)

            QTimer.singleShot(250, click_trigger_toggle)
        elif bot_create_trigger is not None or bot_create_webhook is not None:
            create_kind = "triggers" if bot_create_trigger is not None else "webhooks"
            create_name = bot_create_trigger if bot_create_trigger is not None else bot_create_webhook
            bot = services.exposed.get("BotAdmin")
            if bot is None:
                scenario_error = True
                scenario_ready = True
                return
            store = bot.triggers if create_kind == "triggers" else bot.webhooks
            baseline_count = [None]
            deadline = time.monotonic() + 8

            def open_bot_editor() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or not store.loaded:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_bot_editor)
                    else:
                        print(f"[shot] {create_kind} list did not load")
                        scenario_error = True
                        scenario_ready = True
                    return
                baseline_count[0] = int(store.total)
                button = find_item("botCreateNew")
                if button is None or not button.isVisible():
                    print("[shot] new-item button is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def fill_and_save() -> None:
                    nonlocal scenario_ready, scenario_error
                    page = find_item("botCollectionPage")
                    save_button = find_item("botEditorSave")
                    if page is None or save_button is None:
                        print(f"[shot] {create_kind} editor controls are unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    page.setField("name", create_name.lower())
                    if create_kind == "triggers":
                        page.setField("pattern", "hello")
                    else:
                        page.setField("path", "qml-hook")
                    center = save_button.mapToScene(QPointF(save_button.width() / 2, save_button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))

                    def confirm_trigger_save() -> None:
                        nonlocal scenario_ready, scenario_error
                        found = any(str(store.model.get(i).get("name") or "") == create_name.lower()
                                    for i in range(store.model.count))
                        if found and not store.saving and int(store.total) > baseline_count[0]:
                            print(f"[shot] {create_kind[:-1]} created through QML mouse controls: {create_name.lower()}")
                            scenario_ready = True
                        elif time.monotonic() < deadline:
                            QTimer.singleShot(50, confirm_trigger_save)
                        else:
                            page = find_item("botCollectionPage")
                            print(f"[shot] {create_kind[:-1]} create did not complete: name={create_name.lower()!r} total={store.total} loading={store.loading} saving={store.saving} draft={(page.property('draft') if page is not None else None)!r}")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(120, confirm_trigger_save)

                QTimer.singleShot(160, fill_and_save)

            QTimer.singleShot(250, open_bot_editor)
        elif status_new_key:
            statuses = services.exposed.get("Statuses")
            if statuses is None:
                scenario_error = True
                scenario_ready = True
                return
            deadline = time.monotonic() + 8

            def open_status_composer_by_key() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or statuses.loading or not statuses.own:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_status_composer_by_key)
                    else:
                        print("[shot] status list did not load before keyboard activation")
                        scenario_error = True
                        scenario_ready = True
                    return
                button = find_item("statusNewButton")
                if button is None or not button.isVisible():
                    print("[shot] status New button is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                accessible = QAccessible.queryAccessibleInterface(button)
                name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                button.forceActiveFocus()
                QTest.keyClick(win, Qt.Key.Key_Space)

                def confirm_keyboard_activation() -> None:
                    nonlocal scenario_ready, scenario_error
                    editor = find_item("statusComposerEditor")
                    if editor is not None and editor.isVisible():
                        print(f"[shot] IconButton accessible name={name!r}; Space opened status composer")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(50, confirm_keyboard_activation)
                    else:
                        print(f"[shot] Space did not activate status New button (accessible name={name!r})")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(100, confirm_keyboard_activation)

            QTimer.singleShot(200, open_status_composer_by_key)
        elif status_reply_text is not None:
            statuses = services.exposed.get("Statuses")
            if statuses is None:
                scenario_error = True
                scenario_ready = True
                return
            reply_sent = [False]
            statuses.replySent.connect(lambda _chat_id: reply_sent.__setitem__(0, True))
            deadline = time.monotonic() + 8

            def open_status_reply() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or statuses.loading:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_status_reply)
                    else:
                        print("[shot] status list did not load before reply")
                        scenario_error = True
                        scenario_ready = True
                    return
                statuses.openGroup(status_reply_sender)
                QTimer.singleShot(100, fill_status_reply)

            def fill_status_reply() -> None:
                nonlocal scenario_ready, scenario_error
                editor = find_item("statusReplyInput")
                send = find_item("statusReplySend")
                if editor is None or send is None or not editor.isVisible():
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, fill_status_reply)
                    else:
                        print("[shot] status reply controls are unavailable")
                        scenario_error = True
                        scenario_ready = True
                    return
                editor.setProperty("text", status_reply_text)
                center = send.mapToScene(QPointF(send.width() / 2, send.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))
                QTimer.singleShot(100, confirm_status_reply)

            def confirm_status_reply() -> None:
                nonlocal scenario_ready, scenario_error
                if reply_sent[0]:
                    print(f"[shot] status reply acknowledged through QML: {status_reply_sender}")
                    scenario_ready = True
                elif time.monotonic() < deadline:
                    QTimer.singleShot(50, confirm_status_reply)
                else:
                    print(f"[shot] status reply was not acknowledged: {status_reply_sender}")
                    scenario_error = True
                    scenario_ready = True

            QTimer.singleShot(200, open_status_reply)
        elif status_post_text is not None:
            statuses = services.exposed.get("Statuses")
            if statuses is None:
                scenario_error = True
                scenario_ready = True
                return
            posted = [False]
            statuses.posted.connect(lambda: posted.__setitem__(0, True))
            baseline_count = [None]
            deadline = time.monotonic() + 6

            def open_status_composer() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or statuses.loading or not statuses.own:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_status_composer)
                    else:
                        print("[shot] status list did not load")
                        scenario_error = True
                        scenario_ready = True
                    return
                baseline_count[0] = int(statuses.own.get("count") or 0)
                button = find_item("statusNewButton")
                if button is None or not button.isVisible():
                    print("[shot] new-status button is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def fill_and_post() -> None:
                    nonlocal scenario_ready, scenario_error
                    editor = find_item("statusComposerEditor")
                    post_button = find_item("statusPostButton")
                    if editor is None or post_button is None:
                        print("[shot] status composer controls are unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    editor.setProperty("text", status_post_text)
                    app.processEvents()

                    accessible_name = ""
                    if status_post_key is not None:
                        accessible = QAccessible.queryAccessibleInterface(post_button)
                        accessible_name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                        post_button.forceActiveFocus()
                        QTest.keyClick(win, Qt.Key.Key_Space)
                    else:
                        center = post_button.mapToScene(QPointF(post_button.width() / 2, post_button.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))
                    app.processEvents()

                    def confirm_status_post() -> None:
                        nonlocal scenario_ready, scenario_error
                        count = int(statuses.own.get("count") or 0)
                        if posted[0] and baseline_count[0] is not None and count > baseline_count[0] and not statuses.posting:
                            if status_post_key is not None:
                                print(f"[shot] Btn accessible name={accessible_name!r}; Space posted status text")
                            else:
                                print("[shot] status text posted through QML mouse controls")
                            scenario_ready = True
                        elif time.monotonic() < deadline:
                            QTimer.singleShot(50, confirm_status_post)
                        else:
                            print(f"[shot] status text post did not complete (posted={posted[0]}, count={count}, baseline={baseline_count[0]})")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(120, confirm_status_post)

                QTimer.singleShot(240, fill_and_post)

            QTimer.singleShot(250, open_status_composer)
        elif click_status_sender:
            statuses = services.exposed.get("Statuses")
            if statuses is None:
                scenario_error = True
                scenario_ready = True
                return
            connection_settled = False

            def click_status_row() -> None:
                nonlocal scenario_ready, scenario_error, connection_settled
                if not services.hub.connected:
                    QTimer.singleShot(40, click_status_row)
                    return
                if not connection_settled:
                    connection_settled = True
                    QTimer.singleShot(300, click_status_row)
                    return
                if not statuses.model.contains(click_status_sender):
                    if statuses.loading:
                        QTimer.singleShot(40, click_status_row)
                    else:
                        print(f"[shot] status group not found: {click_status_sender}")
                        scenario_error = True
                        scenario_ready = True
                    return
                row = find_item("statusRow_" + click_status_sender)
                if row is None or not row.isVisible() or row.width() <= 0 or row.height() <= 0:
                    QTimer.singleShot(40, click_status_row)
                    return
                center = row.mapToScene(QPointF(row.width() / 2, row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def check_status_click() -> None:
                    nonlocal scenario_ready, scenario_error
                    print(f"[shot] status story mouse click: {click_status_sender} → viewer {statuses.viewerOpen}")
                    if not statuses.viewerOpen:
                        print("[shot] status row click did not open the viewer")
                        scenario_error = True
                    scenario_ready = True

                QTimer.singleShot(100, check_status_click)

            click_status_row()
        elif status_sender:
            statuses = services.exposed.get("Statuses")
            if statuses is None:
                scenario_ready = True
                return

            def open_status() -> None:
                nonlocal scenario_ready
                if statuses.model.contains(status_sender):
                    statuses.openGroup(status_sender)
                    scenario_ready = True
                elif not statuses.loading:
                    print(f"[shot] status group not found: {status_sender}")
                    scenario_ready = True
                else:
                    QTimer.singleShot(40, open_status)
            open_status()
        elif channel_follow_url is not None:
            channels = services.exposed.get("Channels")
            if channels is None:
                scenario_error = True
                scenario_ready = True
                return
            baseline_count = [None]
            followed = [False]
            channels.followed.connect(lambda: followed.__setitem__(0, True))
            deadline = time.monotonic() + 8

            def open_channel_discovery() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected or channels.loading:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, open_channel_discovery)
                    else:
                        print("[shot] channel list did not load")
                        scenario_error = True
                        scenario_ready = True
                    return
                baseline_count[0] = int(channels.model.property("count"))
                button = find_item("channelFindButton")
                if button is None or not button.isVisible():
                    print("[shot] channel discover button is unavailable")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def preview_channel() -> None:
                    nonlocal scenario_ready, scenario_error
                    field = find_item("channelDiscoverLink")
                    preview_button = find_item("channelDiscoverPreview")
                    if field is None or preview_button is None:
                        print("[shot] channel preview dialog controls are unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    field.setProperty("text", channel_follow_url)
                    center = preview_button.mapToScene(QPointF(preview_button.width() / 2, preview_button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))

                    def follow_preview() -> None:
                        nonlocal scenario_ready, scenario_error
                        if not channels.preview.get("jid") or channels.discovering:
                            if time.monotonic() < deadline:
                                QTimer.singleShot(40, follow_preview)
                            else:
                                print("[shot] channel preview did not load")
                                scenario_error = True
                                scenario_ready = True
                            return
                        button = find_item("channelDiscoverFollow")
                        if button is None or not button.isVisible() or not button.isEnabled():
                            print("[shot] channel Follow button is unavailable")
                            scenario_error = True
                            scenario_ready = True
                            return
                        center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))

                        def confirm_follow() -> None:
                            nonlocal scenario_ready, scenario_error
                            count = int(channels.model.property("count"))
                            if followed[0] and count > baseline_count[0] and not channels.discovering:
                                print("[shot] channel followed through QML mouse controls")
                                scenario_ready = True
                            elif time.monotonic() < deadline:
                                QTimer.singleShot(50, confirm_follow)
                            else:
                                print(f"[shot] channel follow did not complete (followed={followed[0]}, count={count})")
                                scenario_error = True
                                scenario_ready = True

                        QTimer.singleShot(100, confirm_follow)

                    QTimer.singleShot(100, follow_preview)

                QTimer.singleShot(120, preview_channel)

            QTimer.singleShot(250, open_channel_discovery)
        elif click_channel_jid:
            channels = services.exposed.get("Channels")
            if channels is None:
                scenario_error = True
                scenario_ready = True
                return
            connection_settled = False

            def click_channel_row() -> None:
                nonlocal scenario_ready, scenario_error, connection_settled
                if not services.hub.connected:
                    QTimer.singleShot(40, click_channel_row)
                    return
                if not connection_settled:
                    connection_settled = True
                    QTimer.singleShot(300, click_channel_row)
                    return
                if not channels.model.contains(click_channel_jid):
                    if channels.loading:
                        QTimer.singleShot(40, click_channel_row)
                    else:
                        print(f"[shot] channel not found: {click_channel_jid}")
                        scenario_error = True
                        scenario_ready = True
                    return
                row = find_item("channel-row-" + click_channel_jid)
                if row is None or not row.isVisible() or row.width() <= 0 or row.height() <= 0:
                    QTimer.singleShot(40, click_channel_row)
                    return
                center = row.mapToScene(QPointF(row.width() / 2, row.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def check_channel_click() -> None:
                    nonlocal scenario_ready, scenario_error
                    if channels.selectedJid != click_channel_jid:
                        print(f"[shot] channel row mouse click did not select {click_channel_jid}")
                        scenario_error = True
                        scenario_ready = True
                    elif channels.postsLoading:
                        QTimer.singleShot(40, check_channel_click)
                    elif channel_reaction:
                        post = channels.posts.get_row(channel_reaction_post)
                        if post is None:
                            print(f"[shot] channel post not found for reaction: {channel_reaction_post}")
                            scenario_error = True
                            scenario_ready = True
                            return
                        control = find_item(f"channelReaction_{channel_reaction_post}_{channel_reaction}")
                        if control is None or not control.isVisible():
                            print(f"[shot] channel reaction control is unavailable: {channel_reaction}")
                            scenario_error = True
                            scenario_ready = True
                            return
                        before = next((int(r.get("count") or 0) for r in post.get("reactions", [])
                                       if r.get("emoji") == channel_reaction), 0)
                        center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))
                        updated = channels.posts.get_row(channel_reaction_post) or {}
                        after = next((int(r.get("count") or 0) for r in updated.get("reactions", [])
                                      if r.get("emoji") == channel_reaction), 0)
                        if after == before + 1:
                            print(f"[shot] channel reaction mouse action applied: {channel_reaction}")
                            scenario_ready = True
                        else:
                            print(f"[shot] channel reaction did not update optimistically: {before} → {after}")
                            scenario_error = True
                            scenario_ready = True
                    elif channel_mute:
                        button = find_item("channelMuteToggle")
                        if button is None or not button.isVisible():
                            print("[shot] channel mute control is unavailable")
                            scenario_error = True
                            scenario_ready = True
                            return
                        expected = not bool(channels.selected.get("muted"))
                        center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                         QPoint(round(center.x()), round(center.y())))

                        def confirm_channel_mute() -> None:
                            nonlocal scenario_ready, scenario_error
                            if bool(channels.selected.get("muted")) == expected:
                                print(f"[shot] channel mute mouse action applied: {expected}")
                                scenario_ready = True
                            else:
                                print("[shot] channel mute mouse action did not update selection")
                                scenario_error = True
                                scenario_ready = True

                        QTimer.singleShot(120, confirm_channel_mute)
                    else:
                        print(f"[shot] channel row mouse click: {click_channel_jid} → {channels.posts.count} posts")
                        scenario_ready = True

                QTimer.singleShot(100, check_channel_click)

            QTimer.singleShot(300, click_channel_row)
        elif channel_jid:
            channels = services.exposed.get("Channels")
            if channels is None:
                scenario_ready = True
                return
            selection_started = False

            def select_channel() -> None:
                nonlocal scenario_ready, selection_started
                if channels.model.contains(channel_jid):
                    if not selection_started:
                        selection_started = True
                        channels.select(channel_jid)
                    if not channels.postsLoading:
                        scenario_ready = True
                    else:
                        QTimer.singleShot(40, select_channel)
                elif not channels.loading:
                    print(f"[shot] channel not found: {channel_jid}")
                    scenario_ready = True
                else:
                    QTimer.singleShot(40, select_channel)
            select_channel()
        elif calls_filter:
            calls = services.exposed.get("Calls")
            if calls is None:
                scenario_error = True
                scenario_ready = True
                return
            deadline = time.monotonic() + 8

            def click_calls_filter() -> None:
                nonlocal scenario_ready, scenario_error
                if not services.hub.connected:
                    if time.monotonic() < deadline:
                        QTimer.singleShot(40, click_calls_filter)
                    else:
                        print("[shot] calls backend did not connect before filter click")
                        scenario_error = True
                        scenario_ready = True
                    return
                chip = find_item(f"callsFilter_{calls_filter}")
                if chip is None or not chip.isVisible() or chip.width() <= 0 or chip.height() <= 0:
                    if time.monotonic() < deadline and (calls.loading or chip is None or chip.width() <= 0):
                        QTimer.singleShot(40, click_calls_filter)
                    else:
                        print(f"[shot] calls filter chip not found: {calls_filter}")
                        scenario_error = True
                        scenario_ready = True
                    return
                center = chip.mapToScene(QPointF(chip.width() / 2, chip.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))

                def check_filter_click() -> None:
                    nonlocal scenario_ready, scenario_error
                    print(f"[shot] calls filter mouse click: {calls_filter} → {calls.filter}")
                    if calls.filter != calls_filter:
                        print("[shot] calls filter click did not change the selected filter")
                        scenario_error = True
                    scenario_ready = True

                QTimer.singleShot(100, check_filter_click)

            QTimer.singleShot(300, click_calls_filter)
        elif call_id:
            calls = services.exposed.get("Calls")
            if calls is None:
                scenario_ready = True
                return

            def select_call() -> None:
                nonlocal scenario_ready
                if calls.history.contains(call_id):
                    calls.selectLog(call_id)
                    scenario_ready = True
                elif not calls.loading:
                    print(f"[shot] call not found: {call_id}")
                    scenario_ready = True
                else:
                    QTimer.singleShot(40, select_call)
            select_call()
        elif chat_id:
            chats = services.exposed.get("Chats")
            messages = services.exposed.get("Messages")
            if chats is None or messages is None:
                scenario_ready = True
                return

            def select_chat() -> None:
                nonlocal scenario_ready
                if chats.model.contains(chat_id):
                    if services.app.activeChatId != chat_id:
                        services.app.openChat(chat_id)
                    if messages.chatId == chat_id and not messages.loading:
                        if multi_forward_key:
                            multi_select_and_forward(messages, chats, multi_forward_ids, multi_forward_target)
                            return
                        if chat_info_media_message:
                            chats.requestSheet("info", chat_id)
                            deadline = time.monotonic() + 8

                            def open_info_media() -> None:
                                nonlocal scenario_ready, scenario_error
                                found = any(str(row.get("id") or "") == chat_info_media_message
                                            for row in chats.infoMedia)
                                control = find_item("chatInfoMediaThumb_" + chat_info_media_message)
                                if not chats.infoLoading and found and control is not None and control.isVisible():
                                    center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                     QPoint(round(center.x()), round(center.y())))
                                    def confirm_info_media_viewer() -> None:
                                        nonlocal scenario_ready, scenario_error
                                        if (messages.viewerOpen
                                                and str(messages.viewerItem.get("id") or "") == chat_info_media_message):
                                            in_page = messages.indexOfMessage(chat_info_media_message) >= 0
                                            print(f"[shot] Info drawer media thumbnail opened viewer: {chat_info_media_message}; loaded_page={in_page}")
                                            scenario_ready = True
                                        elif time.monotonic() < deadline:
                                            QTimer.singleShot(40, confirm_info_media_viewer)
                                        else:
                                            print(f"[shot] Info drawer media did not open viewer: {chat_info_media_message}")
                                            scenario_error = True
                                            scenario_ready = True
                                    QTimer.singleShot(50, confirm_info_media_viewer)
                                    return
                                if time.monotonic() < deadline:
                                    QTimer.singleShot(50, open_info_media)
                                else:
                                    print(f"[shot] Info drawer media thumbnail unavailable: {chat_info_media_message}; info={chats.infoMedia!r}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(150, open_info_media)
                            return
                        if disappearing_message_key:
                            if not messages.model.contains(disappearing_message_key):
                                print(f"[shot] disappearing-note message not found: {disappearing_message_key}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            QTimer.singleShot(120, lambda: verify_disappearing_message(messages, disappearing_message_key))
                            return
                        if react_chip_key:
                            if not messages.model.contains(react_chip_id):
                                print(f"[shot] message reaction target not found: {react_chip_id}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            QTimer.singleShot(120, lambda: react_chip_by_keyboard(messages, react_chip_id, react_chip_emoji))
                            return
                        if disappearing_seconds is not None:
                            QTimer.singleShot(120, lambda: set_disappearing_by_keyboard(chats, disappearing_seconds))
                            return
                        if select_message_id or forward_message_id:
                            message_id = select_message_id or forward_message_id
                            if not messages.model.contains(message_id):
                                if messages.loading:
                                    QTimer.singleShot(40, select_chat)
                                else:
                                    print(f"[shot] message action target not found: {message_id}")
                                    scenario_error = True
                                    scenario_ready = True
                                return
                            if select_message_id:
                                messages.enterSelect(select_message_id)
                                if selection_copy_key:
                                    QTimer.singleShot(120, lambda: copy_selected_message(messages))
                                    return
                                if selection_delete_key:
                                    QTimer.singleShot(120, lambda: delete_selected_message(messages))
                                    return
                            else:
                                chats.requestForward(chat_id, forward_message_id)
                                if forward_key:
                                    QTimer.singleShot(150, lambda: forward_targets_by_keyboard(chats, forward_key, [forward_message_id]))
                                    return
                            scenario_ready = True
                            return
                        context_action = (reply_message_key or edit_message_key or react_message_id
                                          or star_message_key or pin_message_key or delete_message_key
                                          or copy_message_key or context_forward_id or message_info_key)
                        if context_action:
                            if not messages.model.contains(context_action):
                                print(f"[shot] message action target not found: {context_action}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            action = ("Reply" if reply_message_key else "Edit" if edit_message_key
                                      else "Reaction:" + react_emoji if react_message_key else "Star" if star_message_key
                                      else "Pin" if pin_message_key else "Delete" if delete_message_key
                                      else "Copy" if copy_message_key else "Info" if message_info_key else "Forward")
                            QTimer.singleShot(120, lambda: message_context_action_by_keyboard(messages, context_action, action))
                            return
                        if compose_dialog or emoji_picker or sticker_picker or attach_menu:
                            show_compose_overlay()
                            return
                        if voice_send_synthetic:
                            from array import array

                            from bridge.composer_voice import encode_wav

                            composer = services.exposed.get("Composer")
                            record_button = find_item("composerVoiceButton")
                            if (composer is None or record_button is None or not record_button.isVisible()
                                    or not record_button.isEnabled()):
                                print("[shot] voice-note record control is unavailable")
                                scenario_error = True
                                scenario_ready = True
                                return
                            wav = encode_wav(array("h", [1800]) * 32_000, 16_000)
                            composer._voice.start = lambda: ""
                            composer._voice.finish = lambda: {"wav": wav, "seconds": 2.0, "peak": 1800}
                            record_point = record_button.mapToScene(
                                QPointF(record_button.width() / 2, record_button.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(record_point.x()), round(record_point.y())))
                            deadline = time.monotonic() + 8
                            send_clicked = False

                            def drive_synthetic_voice_send() -> None:
                                nonlocal scenario_ready, scenario_error, send_clicked
                                if not send_clicked and str(composer.voiceState) == "recording":
                                    send_button = find_item("composerSendVoiceButton")
                                    if send_button is None or not send_button.isVisible() or not send_button.isEnabled():
                                        print("[shot] voice-note send control did not become enabled")
                                        scenario_error = True
                                        scenario_ready = True
                                        return
                                    send_point = send_button.mapToScene(
                                        QPointF(send_button.width() / 2, send_button.height() / 2))
                                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                     QPoint(round(send_point.x()), round(send_point.y())))
                                    send_clicked = True
                                elif send_clicked and str(composer.voiceState) == "idle" and not composer.busy:
                                    print("[shot] QML voice recording flow uploaded a synthetic WAV push-to-talk message")
                                    scenario_ready = True
                                    return
                                if time.monotonic() < deadline:
                                    QTimer.singleShot(40, drive_synthetic_voice_send)
                                else:
                                    print(f"[shot] synthetic voice send did not settle: state={composer.voiceState!r} busy={composer.busy!r}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(40, drive_synthetic_voice_send)
                            return
                        if voice_no_device:
                            composer = services.exposed.get("Composer")
                            button = find_item("composerVoiceButton")
                            if composer is None or button is None or not button.isVisible() or not button.isEnabled():
                                print("[shot] voice-note record control is unavailable")
                                scenario_error = True
                                scenario_ready = True
                                return
                            composer._voice.start = lambda: "Mikrofon tidak ditemukan"
                            notices = []
                            services.toast.shown.connect(lambda _id, message, kind: notices.append((message, kind)))
                            center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            deadline = time.monotonic() + 4

                            def confirm_voice_error() -> None:
                                nonlocal scenario_ready, scenario_error
                                if str(composer.voiceState) == "idle" and ("Mikrofon tidak ditemukan", "error") in notices:
                                    print("[shot] voice record click reported unavailable microphone and returned to idle")
                                    scenario_ready = True
                                elif time.monotonic() < deadline:
                                    QTimer.singleShot(40, confirm_voice_error)
                                else:
                                    print(f"[shot] voice record error did not settle: state={composer.voiceState!r} notices={notices!r}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(120, confirm_voice_error)
                            return
                        if open_contact:
                            index = int(messages.model.indexOfKey(contact_message))
                            if index < 0:
                                print(f"[shot] contact message is not available: {contact_message}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            control = find_item("messageContactOpen_" + contact_message + "_" + contact_jid)
                            if control is None or not control.isVisible() or not control.isEnabled():
                                print(f"[shot] contact action is unavailable: {contact_jid}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            deadline = time.monotonic() + 6

                            def confirm_contact_open() -> None:
                                nonlocal scenario_ready, scenario_error
                                if services.app.activeChatId == contact_jid:
                                    print(f"[shot] contact card opened chat: {contact_jid}")
                                    scenario_ready = True
                                elif time.monotonic() < deadline:
                                    QTimer.singleShot(50, confirm_contact_open)
                                else:
                                    print(f"[shot] contact card did not open chat: {contact_jid}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(100, confirm_contact_open)
                            return
                        if open_location:
                            index = int(messages.model.indexOfKey(open_location))
                            row = messages.model.find(open_location) or {}
                            location = row.get("location") or {}
                            expected_url = str(location.get("mapsUrl") or "")
                            if index < 0 or not expected_url:
                                print(f"[shot] location message is not available: {open_location}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            control = find_item("messageLocationOpenMaps_" + open_location)
                            if control is None or not control.isVisible() or not control.isEnabled():
                                print("[shot] location Open in Maps control is unavailable")
                                scenario_error = True
                                scenario_ready = True
                                return
                            opened_urls = []
                            messages._external_url_opener = lambda url: opened_urls.append(url.toString()) or True
                            center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            if opened_urls == [expected_url]:
                                print(f"[shot] location card opened map URL: {expected_url}")
                                scenario_ready = True
                            else:
                                print(f"[shot] location map URL mismatch: expected {expected_url!r}, got {opened_urls!r}")
                                scenario_error = True
                                scenario_ready = True
                            return
                        if open_link_key:
                            index = int(messages.model.indexOfKey(open_link_key))
                            row = messages.model.find(open_link_key) or {}
                            preview = row.get("linkPreview") or {}
                            expected_url = str(preview.get("url") or "")
                            control = find_item("messageLinkPreviewOpen_" + open_link_key)
                            if index < 0 or not expected_url or control is None or not control.isVisible():
                                print(f"[shot] link preview control is unavailable: {open_link_key}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            accessible = QAccessible.queryAccessibleInterface(control)
                            name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                            opened_urls = []
                            messages._external_url_opener = lambda url: opened_urls.append(url.toString()) or True
                            control.forceActiveFocus()
                            QTest.keyClick(win, Qt.Key.Key_Space)
                            if opened_urls == [expected_url]:
                                print(f"[shot] link preview accessible name={name!r}; Space opened {expected_url}")
                                scenario_ready = True
                            else:
                                print(f"[shot] link preview Space did not open the URL (name={name!r}, got={opened_urls!r})")
                                scenario_error = True
                                scenario_ready = True
                            return
                        if save_document_message:
                            index = int(messages.model.indexOfKey(save_document_message))
                            if index < 0:
                                print(f"[shot] document message is not available: {save_document_message}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            control = find_item("messageDocumentSave_" + save_document_message)
                            if control is None or not control.isVisible() or not control.isEnabled():
                                print("[shot] document Save control is unavailable")
                                scenario_error = True
                                scenario_ready = True
                                return
                            media_item = find_item("messageMedia_" + save_document_message)
                            dialog = media_item.property("saveFileDialog") if media_item is not None else None
                            if dialog is None:
                                print("[shot] document save FileDialog is unavailable")
                                scenario_error = True
                                scenario_ready = True
                                return
                            dialog.setProperty("selectedFile", QUrl.fromLocalFile(save_document_path))
                            center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            if not bool(dialog.property("visible")):
                                dialog.open()
                            if not QMetaObject.invokeMethod(dialog, "accept"):
                                print("[shot] document save dialog did not accept the selected path")
                                scenario_error = True
                                scenario_ready = True
                                return
                            saved = []
                            messages.saved.connect(lambda label, ok: saved.append((str(label), bool(ok))))
                            deadline = time.monotonic() + 8

                            def confirm_document_saved() -> None:
                                nonlocal scenario_ready, scenario_error
                                if os.path.isfile(save_document_path) and os.path.getsize(save_document_path) > 0 and ("Dokumen", True) in saved:
                                    print(f"[shot] document QML save control downloaded to: {save_document_path}")
                                    scenario_ready = True
                                elif time.monotonic() < deadline:
                                    QTimer.singleShot(50, confirm_document_saved)
                                else:
                                    print(f"[shot] document download did not finish: file={os.path.isfile(save_document_path)} saved={saved!r}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(50, confirm_document_saved)
                            return
                        if vote_poll_key:
                            index = int(messages.model.indexOfKey(poll_vote_message))
                            control = find_item("messagePollOption_" + poll_vote_message + "_" + poll_vote_option)
                            if index < 0 or control is None or not control.isVisible():
                                print(f"[shot] keyboard poll option is unavailable: {poll_vote_option}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            accessible = QAccessible.queryAccessibleInterface(control)
                            name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                            control.forceActiveFocus()
                            QTest.keyClick(win, Qt.Key.Key_Space)
                            row = messages.model.find(poll_vote_message) or {}
                            poll = row.get("poll") or {}
                            voted = any(str(option.get("name") or "") == poll_vote_option and bool(option.get("voted"))
                                        for option in (poll.get("options") or []))
                            if voted:
                                print(f"[shot] poll accessible name={name!r}; Space voted {poll_vote_option}")
                                scenario_ready = True
                            else:
                                print(f"[shot] poll Space did not vote: {poll_vote_option} (name={name!r})")
                                scenario_error = True
                                scenario_ready = True
                            return
                        if vote_poll:
                            index = int(messages.model.indexOfKey(poll_vote_message))
                            if index < 0:
                                print(f"[shot] poll is not available: {poll_vote_message}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            control = find_item("messagePollOption_" + poll_vote_message + "_" + poll_vote_option)
                            if control is None or not control.isVisible():
                                print(f"[shot] poll option control is unavailable: {poll_vote_option}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            deadline = time.monotonic() + 5

                            def confirm_poll_vote() -> None:
                                nonlocal scenario_ready, scenario_error
                                row = messages.model.find(poll_vote_message) or {}
                                poll = row.get("poll") or {}
                                voted = any(str(option.get("name") or "") == poll_vote_option and bool(option.get("voted"))
                                            for option in (poll.get("options") or []))
                                if voted:
                                    print(f"[shot] poll option mouse click voted: {poll_vote_option}")
                                    scenario_ready = True
                                elif time.monotonic() < deadline:
                                    QTimer.singleShot(50, confirm_poll_vote)
                                else:
                                    print(f"[shot] poll option did not update optimistically: {poll_vote_option}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(100, confirm_poll_vote)
                            return
                        if jump_message:
                            index = int(messages.model.indexOfKey(jump_message))
                            if index < 0:
                                print(f"[shot] message is not available to reveal: {jump_message}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            messages.jumpTo(jump_message)
                            deadline = time.monotonic() + 6

                            def confirm_message_jump() -> None:
                                nonlocal scenario_ready, scenario_error
                                view = find_item("messageList")
                                bubble = find_item("messageBubble_" + jump_message)
                                top = None
                                if view is not None and bubble is not None and bubble.isVisible():
                                    top = bubble.mapToItem(view, QPointF(0, 0)).y()
                                    bottom = top + bubble.height()
                                    if bottom > 0 and top < view.height():
                                        print(f"[shot] message jump revealed {jump_message} at index {index}")
                                        scenario_ready = True
                                        return
                                if time.monotonic() < deadline:
                                    QTimer.singleShot(50, confirm_message_jump)
                                else:
                                    print(f"[shot] message jump did not reveal {jump_message}: top={top if bubble is not None else None} viewHeight={view.height() if view is not None else None}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(150, confirm_message_jump)
                            return
                        if keyboard_image_message:
                            deadline = time.monotonic() + 6

                            def activate_image_by_keyboard() -> None:
                                nonlocal scenario_error, scenario_ready
                                index = int(messages.model.indexOfKey(keyboard_image_message))
                                control = find_item("messageMediaImageOpen_" + keyboard_image_message)
                                if index < 0 or control is None or not control.isVisible() or not control.isEnabled():
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(50, activate_image_by_keyboard)
                                    else:
                                        print("[shot] keyboard image control did not become ready")
                                        scenario_error = True
                                        scenario_ready = True
                                    return
                                accessible = QAccessible.queryAccessibleInterface(control)
                                name = accessible.text(QAccessible.Text.Name) if accessible is not None else ""
                                control.forceActiveFocus()
                                QTest.keyClick(win, Qt.Key.Key_Space)
                                current = messages.viewerItem
                                if messages.viewerOpen and str(current.get("id") or "") == keyboard_image_message:
                                    print(f"[shot] image accessible name={name!r}; Space opened viewer")
                                    scenario_ready = True
                                else:
                                    print(f"[shot] image Space did not open viewer (name={name!r}, active={control.property('activeFocus')})")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(40, activate_image_by_keyboard)
                            return
                        if click_media_message:
                            index = int(messages.model.indexOfKey(click_media_message))
                            if index < 0:
                                print(f"[shot] media message is not available: {click_media_message}")
                                scenario_error = True
                                scenario_ready = True
                                return
                            deadline = time.monotonic() + 6

                            def confirm_media_open() -> None:
                                nonlocal scenario_ready, scenario_error
                                if messages.viewerOpen and str(messages.viewerItem.get("id") or "") == click_media_message:
                                    print(f"[shot] media thumbnail mouse click opened viewer: {click_media_message}")
                                    scenario_ready = True
                                    return
                                control = find_item("messageMediaImageOpen_" + click_media_message)
                                if control is None or not control.isVisible() or not control.isEnabled():
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(50, confirm_media_open)
                                    else:
                                        print(f"[shot] media thumbnail is unavailable: {click_media_message}")
                                        scenario_error = True
                                        scenario_ready = True
                                    return
                                center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                 QPoint(round(center.x()), round(center.y())))
                                QTimer.singleShot(100, confirm_media_open)

                            QTimer.singleShot(150, confirm_media_open)
                            return
                        if collapse_sidebar:
                            page = find_item("chatPage")
                            if page is not None:
                                page.setProperty("sidebarCollapsed", True)
                        if click_sidebar_toggle:
                            page = find_item("chatPage")
                            button = find_item("chatSidebarToggleButton")
                            if page is None or button is None:
                                print("[shot] chat sidebar toggle is not available")
                                scenario_error = True
                                scenario_ready = True
                                return
                            before = bool(page.property("sidebarCollapsed"))
                            center = button.mapToItem(win.contentItem(), QPointF(button.width() / 2, button.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            def check_sidebar_toggle() -> None:
                                nonlocal scenario_ready, scenario_error
                                after = bool(page.property("sidebarCollapsed"))
                                print(f"[shot] sidebar toggle mouse click: {before} -> {after}")
                                if after == before:
                                    print("[shot] sidebar toggle click did not change collapse state")
                                    scenario_error = True
                                scenario_ready = True
                            QTimer.singleShot(100, check_sidebar_toggle)
                            return
                        if click_chat_profile:
                            page = find_item("chatPage")
                            profile = find_item("chatHeaderProfile")
                            if page is None or profile is None:
                                print("[shot] chat profile area is not available")
                                scenario_error = True
                                scenario_ready = True
                                return
                            before = str(page.property("activeSheet") or "")
                            center = profile.mapToItem(win.contentItem(), QPointF(profile.width() / 2, profile.height() / 2))
                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                             QPoint(round(center.x()), round(center.y())))
                            def check_chat_profile() -> None:
                                nonlocal scenario_ready, scenario_error
                                after = str(page.property("activeSheet") or "")
                                print(f"[shot] chat profile mouse click opened info: {before!r} -> {after!r}")
                                if after != "info":
                                    print("[shot] chat profile click did not open info")
                                    scenario_error = True
                                scenario_ready = True
                            QTimer.singleShot(100, check_chat_profile)
                            return
                        if info_mute_mode or info_disappearing is not None:
                            chats.requestSheet("info", chat_id)
                            deadline = time.monotonic() + 5
                            notices = []
                            info_setting_scrolled = [False]
                            if info_disappearing is not None and services.toast is not None:
                                services.toast.shown.connect(lambda _id, message, kind: notices.append((message, kind)))

                            if info_mute_mode:
                                control_name = "chatInfoMute_" + info_mute_mode
                                action_name = "mute:" + info_mute_mode
                                expected_mode = "off" if info_mute_mode == "off" else "forever" if info_mute_mode == "forever" else "until"
                            else:
                                control_name = "chatInfoDisappearing_" + str(info_disappearing)
                                action_name = "disappearing:" + str(info_disappearing)
                                expected_mode = ""

                            def click_info_setting() -> None:
                                nonlocal scenario_ready, scenario_error
                                control = find_item(control_name)
                                if control is not None and not info_setting_scrolled[0]:
                                    flickable = find_item("chatInfoFlickable")
                                    if flickable is not None:
                                        height = float(flickable.property("height"))
                                        max_y = max(0.0, float(flickable.property("contentHeight")) - height)
                                        flickable.setProperty("contentY", max_y)
                                    info_setting_scrolled[0] = True
                                    QTimer.singleShot(100, click_info_setting)
                                    return
                                if control is None or not control.isVisible() or control.width() <= 0 or control.height() <= 0:
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(40, click_info_setting)
                                    else:
                                        print(f"[shot] chat info setting control not found or visible: {control_name}")
                                        scenario_error = True
                                        scenario_ready = True
                                    return
                                center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                 QPoint(round(center.x()), round(center.y())))

                                def confirm_info_setting() -> None:
                                    nonlocal scenario_ready, scenario_error
                                    chat = chats.activeChat
                                    if info_mute_mode:
                                        confirmed = str(chat.get("muteMode") or "off") == expected_mode
                                    else:
                                        success_text = "Pesan sementara dimatikan" if info_disappearing == 0 else "Pesan sementara aktif: "
                                        confirmed = (int(chat.get("disappearingSecs") or 0) == info_disappearing
                                                     and any(kind == "success" and message.startswith(success_text)
                                                             for message, kind in notices))
                                    if confirmed:
                                        print(f"[shot] chat info setting mouse action applied: {action_name}")
                                        scenario_ready = True
                                    elif time.monotonic() < deadline:
                                        QTimer.singleShot(40, confirm_info_setting)
                                    else:
                                        print(f"[shot] chat info setting did not settle: {action_name}; chat={chat!r}; notices={notices!r}")
                                        scenario_error = True
                                        scenario_ready = True

                                QTimer.singleShot(40, confirm_info_setting)

                            QTimer.singleShot(250, click_info_setting)
                            return
                        if group_member_action:
                            chats.requestSheet("info", chat_id)
                            deadline = time.monotonic() + 5

                            def click_group_control() -> None:
                                nonlocal scenario_ready, scenario_error
                                if not services.hub.connected or chats.groupLoading or chats.group.get("jid") != chat_id:
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(40, click_group_control)
                                    else:
                                        print(f"[shot] group details did not load for {chat_id}")
                                        scenario_error = True
                                        scenario_ready = True
                                    return

                                control_name = "groupAddMemberButton"
                                action_name = f"add:{group_member}"

                                control = find_item(control_name)
                                if control is None or not control.isVisible() or control.width() <= 0 or control.height() <= 0:
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(40, click_group_control)
                                    else:
                                        print(f"[shot] group control not found or visible: {control_name}")
                                        scenario_error = True
                                        scenario_ready = True
                                    return

                                center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                 QPoint(round(center.x()), round(center.y())))

                                def confirm_group_action() -> None:
                                    nonlocal scenario_ready, scenario_error
                                    group = chats.group
                                    confirmed = any(str(p.get("jid") or "") == group_member
                                                    for p in group.get("participants", []))
                                    if confirmed:
                                        print(f"[shot] group admin mouse action applied: {action_name}")
                                        scenario_ready = True
                                    elif time.monotonic() < deadline:
                                        QTimer.singleShot(40, confirm_group_action)
                                    else:
                                        print(f"[shot] group admin mouse action did not apply: {action_name}")
                                        scenario_error = True
                                        scenario_ready = True

                                contact_lookup_attempt = [0]

                                def submit_group_member() -> None:
                                    nonlocal scenario_ready, scenario_error
                                    member_row = find_item("groupAddContact_" + group_member)
                                    save_button = find_item("groupAddMemberSave")
                                    if member_row is None or save_button is None:
                                        if contact_lookup_attempt[0] == 0:
                                            contact_lookup_attempt[0] = 1
                                            contact_list = find_item("groupAddContactList")
                                            participants = {str(item.get("jid") or "") for item in (chats.group.get("participants") or [])}
                                            candidates = []
                                            for index in range(int(chats.model.count)):
                                                candidate = chats.model.get(index) or {}
                                                jid = str(candidate.get("id") or "")
                                                if (jid and not candidate.get("isGroup") and not candidate.get("archived")
                                                        and jid not in participants):
                                                    candidates.append(jid)
                                            target_index = candidates.index(group_member) if group_member in candidates else -1
                                            if contact_list is not None and target_index >= 0:
                                                max_y = max(0.0, float(contact_list.property("contentHeight"))
                                                            - float(contact_list.height()))
                                                contact_list.setProperty("contentY", min(max_y, target_index * 44.0))
                                                QTimer.singleShot(100, submit_group_member)
                                                return
                                        print("[shot] add-member dialog controls are unavailable")
                                        scenario_error = True
                                        scenario_ready = True
                                        return
                                    member_point = member_row.mapToScene(QPointF(member_row.width() / 2, member_row.height() / 2))
                                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                     QPoint(round(member_point.x()), round(member_point.y())))
                                    center = save_button.mapToScene(QPointF(save_button.width() / 2, save_button.height() / 2))
                                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                     QPoint(round(center.x()), round(center.y())))
                                    QTimer.singleShot(120, confirm_group_action)

                                QTimer.singleShot(120, submit_group_member)

                            QTimer.singleShot(300, click_group_control)
                            return
                        if chat_info_page:
                            chats.requestSheet("info", chat_id)
                            deadline = time.monotonic() + 6
                            page_stage = [0]

                            def drive_info_page() -> None:
                                nonlocal scenario_ready, scenario_error
                                sheet = find_item("chatInfoSheet")
                                if sheet is None or not sheet.isVisible():
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(50, drive_info_page)
                                    else:
                                        print("[shot] chat info drawer did not open")
                                        scenario_error = True
                                        scenario_ready = True
                                    return
                                if page_stage[0] == 0:
                                    star = find_item("chatInfoStarred")
                                    if star is not None:
                                        point = star.mapToScene(QPointF(0, 0))
                                        print(f"[shot] chat info starred control visible={star.isVisible()} y={point.y():.1f} height={star.height():.1f}")
                                    control = find_item("chatInfoMediaOpen")
                                    if control is not None and control.isVisible():
                                        center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                         QPoint(round(center.x()), round(center.y())))
                                        page_stage[0] = 1
                                    elif time.monotonic() >= deadline:
                                        print("[shot] chat info media summary is unavailable")
                                        scenario_error = True
                                        scenario_ready = True
                                        return
                                elif page_stage[0] == 1:
                                    control = find_item("chatInfoTab_" + chat_info_page)
                                    if control is not None and control.isVisible():
                                        center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                                        QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                         QPoint(round(center.x()), round(center.y())))
                                        page_stage[0] = 2
                                    elif time.monotonic() >= deadline:
                                        print(f"[shot] chat info subpage tab did not appear: {chat_info_page}")
                                        scenario_error = True
                                        scenario_ready = True
                                        return
                                elif page_stage[0] == 2:
                                    if str(sheet.property("tab")) == chat_info_page:
                                        if leave_info_page_open:
                                            print(f"[shot] chat info {chat_info_page} subpage rendered and left open")
                                            scenario_ready = True
                                            return
                                        back = find_item("chatInfoBack")
                                        if back is not None and back.isVisible():
                                            center = back.mapToScene(QPointF(back.width() / 2, back.height() / 2))
                                            QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                                             QPoint(round(center.x()), round(center.y())))
                                            page_stage[0] = 3
                                    elif time.monotonic() >= deadline:
                                        print(f"[shot] chat info did not enter subpage: {chat_info_page}")
                                        scenario_error = True
                                        scenario_ready = True
                                        return
                                else:
                                    if str(sheet.property("tab")) == "info":
                                        print(f"[shot] chat info {chat_info_page} subpage opened and returned through its header action")
                                        scenario_ready = True
                                        return
                                if time.monotonic() < deadline:
                                    QTimer.singleShot(80, drive_info_page)
                                else:
                                    print(f"[shot] chat info navigation did not settle: {chat_info_page}")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(120, drive_info_page)
                            return
                        if chat_sheet in ("info", "search", "starred"):
                            chats.requestSheet(chat_sheet, chat_id)
                            scenario_ready = True
                            return
                        if file_dialog_path:
                            composer = services.exposed.get("Composer")
                            bar = find_item("composerBar")
                            if composer is None or bar is None:
                                print("[shot] composer file dialog is unavailable")
                                scenario_error = True
                                scenario_ready = True
                                return
                            bar.setProperty("filePickerForceFallback", True)
                            bar.setProperty("filePickerSelectedFile", QUrl.fromLocalFile(file_dialog_path))
                            if not QMetaObject.invokeMethod(bar, "openDocumentDialog"):
                                print("[shot] composer file dialog did not open")
                                scenario_error = True
                                scenario_ready = True
                                return
                            deadline = time.monotonic() + 8
                            upload_started = [False]
                            composer.sent.connect(lambda _local_id: upload_started.__setitem__(0, True))

                            def accept_file_selection() -> None:
                                nonlocal scenario_ready, scenario_error
                                if not bool(bar.property("filePickerVisible")):
                                    if time.monotonic() < deadline:
                                        QTimer.singleShot(50, accept_file_selection)
                                    else:
                                        print("[shot] composer file dialog is not visible")
                                        scenario_error = True
                                        scenario_ready = True
                                    return
                                if not QMetaObject.invokeMethod(bar, "acceptFilePicker"):
                                    print("[shot] composer file dialog did not accept selection")
                                    scenario_error = True
                                    scenario_ready = True
                                    return
                                print(f"[shot] file dialog accepted selection: {Path(file_dialog_path).name}")

                            def confirm_file_upload() -> None:
                                nonlocal scenario_ready, scenario_error
                                if composer.busy:
                                    upload_started[0] = True
                                if upload_started[0] and not composer.busy:
                                    print("[shot] file dialog upload completed")
                                    scenario_ready = True
                                elif time.monotonic() < deadline:
                                    QTimer.singleShot(50, confirm_file_upload)
                                else:
                                    print("[shot] file dialog did not complete upload")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(160, accept_file_selection)
                            QTimer.singleShot(220, confirm_file_upload)
                            return
                        if clipboard_paste:
                            editor = find_item("composerEditor")
                            composer = services.exposed.get("Composer")
                            if editor is None or composer is None:
                                print("[shot] composer editor is unavailable for clipboard input")
                                scenario_error = True
                                scenario_ready = True
                                return

                            editor.forceActiveFocus()
                            if clipboard_paste == "image":
                                image = QImage(24, 16, QImage.Format.Format_RGB32)
                                image.fill(QColor("#3b82f6"))
                                app.clipboard().setImage(image)
                            else:
                                app.clipboard().setText("clipboard sample")
                            QTest.keyClick(win, Qt.Key_V, Qt.KeyboardModifier.ControlModifier)
                            deadline = time.monotonic() + 4

                            def confirm_clipboard_paste() -> None:
                                nonlocal scenario_ready, scenario_error
                                attachment = composer.pendingAttachment
                                if clipboard_paste == "text":
                                    applied = str(composer.text) == "clipboard sample" and not attachment
                                elif clipboard_paste == "image":
                                    image_path = str(attachment.get("path") or "")
                                    applied = (bool(attachment.get("isImage")) and bool(attachment.get("temp"))
                                               and os.path.isfile(image_path))
                                if applied:
                                    print(f"[shot] clipboard {clipboard_paste} paste applied")
                                    scenario_ready = True
                                elif time.monotonic() < deadline:
                                    QTimer.singleShot(40, confirm_clipboard_paste)
                                else:
                                    print(f"[shot] clipboard {clipboard_paste} paste did not apply")
                                    scenario_error = True
                                    scenario_ready = True

                            QTimer.singleShot(120, confirm_clipboard_paste)
                            return
                        if composer_key:
                            editor = find_item("composerEditor")
                            composer = services.exposed.get("Composer")
                            if editor is None or composer is None:
                                print("[shot] composer editor is unavailable for keyboard input")
                                scenario_error = True
                                scenario_ready = True
                                return
                            if composer_markdown:
                                composer.toggleMarkdown()
                            editor.forceActiveFocus()
                            for character in composer_text:
                                key = Qt.Key_Space if character == " " else getattr(Qt, f"Key_{character.upper()}", None)
                                if key is None:
                                    print(f"[shot] composer test text contains an unsupported key: {character!r}")
                                    scenario_error = True
                                    scenario_ready = True
                                    return
                                QTest.keyClick(win, key)
                            modifiers = {
                                "Enter": Qt.KeyboardModifier.NoModifier,
                                "ShiftEnter": Qt.KeyboardModifier.ShiftModifier,
                                "CtrlEnter": Qt.KeyboardModifier.ControlModifier,
                            }[composer_key]
                            QTest.keyClick(win, Qt.Key_Return, modifiers)
                            should_send = (composer_key == "CtrlEnter" if composer_markdown
                                           else composer_key != "ShiftEnter")

                            def check_composer_key() -> None:
                                nonlocal scenario_ready, scenario_error
                                text = str(composer.text)
                                sent = text == ""
                                accepted = sent if should_send else "\n" in text
                                print(f"[shot] composer {composer_key} (markdown={composer_markdown}): text={text!r} send={sent}")
                                if not accepted:
                                    print("[shot] composer key did not match the configured send/newline behavior")
                                    scenario_error = True
                                scenario_ready = True

                            QTimer.singleShot(100, check_composer_key)
                            return
                        scenario_ready = True
                    else:
                        QTimer.singleShot(40, select_chat)
                elif not chats.loading:
                    print(f"[shot] chat not found: {chat_id}")
                    scenario_ready = True
                else:
                    QTimer.singleShot(40, select_chat)
            select_chat()
        elif collapse_sidebar:
            page = find_item("chatPage")
            if page is not None:
                page.setProperty("sidebarCollapsed", True)
            scenario_ready = True
        elif incoming_call:
            calls = services.exposed.get("Calls")
            if calls is not None:
                def click_incoming_call_action() -> None:
                    nonlocal scenario_ready, scenario_error
                    control_name = "incomingCallAccept" if call_action == "answer" else "incomingCallDecline"
                    control = find_item(control_name)
                    if control is None or not control.isVisible():
                        print(f"[shot] incoming call button is unavailable: {control_name}")
                        scenario_error = True
                        scenario_ready = True
                        return
                    center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))

                    deadline = time.monotonic() + 5

                    def confirm_call_action() -> None:
                        nonlocal scenario_ready, scenario_error
                        call = calls.activeCall
                        applied = (str(call.get("status") or "") == "connected" if call_action == "answer"
                                   else not calls.hasActiveCall)
                        if applied:
                            print(f"[shot] incoming call mouse action applied: {call_action}")
                            scenario_ready = True
                        elif time.monotonic() < deadline:
                            QTimer.singleShot(40, confirm_call_action)
                        else:
                            print(f"[shot] incoming call mouse action did not apply: {call_action}")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(100, confirm_call_action)

                def show_incoming() -> None:
                    calls._set_active({
                        "id": "shot-incoming", "status": "ringing", "type": "audio",
                        "direction": "incoming", "source": "incoming", "media_mode": "tts",
                        "target": "6281111111111@s.whatsapp.net", "group_jid": None,
                        "participants": None, "started_at": int(time.time()),
                        "answered_at": None, "video_enabled": False, "remote_video_enabled": False,
                    })
                    if call_action:
                        QTimer.singleShot(160, click_incoming_call_action)
                QTimer.singleShot(900, show_incoming)
                scenario_ready = not bool(call_action)
            else:
                scenario_error = bool(call_action)
                scenario_ready = True
        elif active_call:
            calls = services.exposed.get("Calls")
            if calls is None:
                scenario_error = bool(call_action)
                scenario_ready = True
                return

            def click_active_call_action() -> None:
                nonlocal scenario_ready, scenario_error
                control_name = {
                    "hangup": "activeCallEnd",
                    "video-start": "activeCallVideo",
                    "video-stop": "activeCallVideo",
                    "mic-toggle": "activeCallMic",
                    "speaker-toggle": "activeCallSpeaker",
                }[call_action]
                control = find_item(control_name)
                if control is None or not control.isVisible():
                    print(f"[shot] active call control is unavailable: {control_name}")
                    scenario_error = True
                    scenario_ready = True
                    return
                center = control.mapToScene(QPointF(control.width() / 2, control.height() / 2))
                QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                 QPoint(round(center.x()), round(center.y())))
                deadline = time.monotonic() + 5

                def confirm_call_control() -> None:
                    nonlocal scenario_ready, scenario_error
                    active = calls.activeCall
                    if call_action == "hangup":
                        applied = not calls.hasActiveCall
                    elif call_action == "video-start":
                        applied = bool(active.get("video_enabled"))
                    elif call_action == "video-stop":
                        applied = not bool(active.get("video_enabled"))
                    elif call_action == "mic-toggle":
                        applied = calls.micMuted
                    else:
                        applied = calls.speakerMuted
                    if applied:
                        print(f"[shot] active call mouse action applied: {call_action}")
                        scenario_ready = True
                    elif time.monotonic() < deadline:
                        QTimer.singleShot(40, confirm_call_control)
                    else:
                        print(f"[shot] active call mouse action did not apply: {call_action}")
                        scenario_error = True
                        scenario_ready = True

                QTimer.singleShot(100, confirm_call_control)

            if calls is not None:
                def show_active() -> None:
                    now = int(time.time())
                    calls._set_active({
                        "id": "shot-active", "status": "connected", "type": "audio",
                        "direction": "outgoing", "source": "ui", "media_mode": "tts",
                        "target": "6281111111111@s.whatsapp.net", "group_jid": None,
                        "participants": None, "started_at": now - 92,
                        "answered_at": now - 92, "video_enabled": call_action == "video-stop", "remote_video_enabled": False,
                    })
                    if call_action in {"hangup", "video-start", "video-stop", "mic-toggle", "speaker-toggle"}:
                        QTimer.singleShot(160, click_active_call_action)
                QTimer.singleShot(900, show_active)
            scenario_ready = call_action not in {"hangup", "video-start", "video-stop", "mic-toggle", "speaker-toggle"}
        elif group_call:
            calls = services.exposed.get("Calls")
            if calls is not None:
                def add_group_participant() -> None:
                    nonlocal scenario_ready, scenario_error
                    field = find_item("callParticipantJid")
                    button = find_item("callAddParticipant")
                    if field is None or button is None or not button.isVisible():
                        print("[shot] group-call participant controls are unavailable")
                        scenario_error = True
                        scenario_ready = True
                        return
                    field.setProperty("text", call_participant_jid)
                    center = button.mapToScene(QPointF(button.width() / 2, button.height() / 2))
                    QTest.mouseClick(win, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier,
                                     QPoint(round(center.x()), round(center.y())))
                    deadline = time.monotonic() + 5

                    def confirm_participant() -> None:
                        nonlocal scenario_ready, scenario_error
                        participants = list(calls.activeCall.get("participants") or [])
                        if call_participant_jid in participants:
                            print("[shot] group-call participant added through QML controls")
                            scenario_ready = True
                        elif time.monotonic() < deadline:
                            QTimer.singleShot(40, confirm_participant)
                        else:
                            print("[shot] group-call participant was not added")
                            scenario_error = True
                            scenario_ready = True

                    QTimer.singleShot(100, confirm_participant)

                def show_group() -> None:
                    now = int(time.time())
                    group = "120363000000000001@g.us"
                    calls._set_active({
                        "id": "shot-group-call", "status": "connected", "type": "group_audio",
                        "direction": "outgoing", "source": "ui", "media_mode": "tts",
                        "target": group, "group_jid": group,
                        "participants": ["6281111111111@s.whatsapp.net", "6282222222222@s.whatsapp.net"],
                        "started_at": now - 37, "answered_at": now - 37,
                        "video_enabled": False, "remote_video_enabled": False,
                    })
                    if call_action == "add-participant":
                        QTimer.singleShot(160, add_group_participant)
                QTimer.singleShot(900, show_group)
                scenario_ready = call_action != "add-participant"
            else:
                scenario_error = call_action == "add-participant"
                scenario_ready = True
        elif media_id:
            messages = services.exposed.get("Messages")
            if messages is None:
                scenario_ready = True
                return

            def open_media() -> None:
                nonlocal scenario_ready
                if messages.model.contains(media_id):
                    messages.openViewer(media_id)
                    scenario_ready = True
                elif not messages.loading:
                    print(f"[shot] media message not found: {media_id}")
                    scenario_ready = True
                else:
                    QTimer.singleShot(40, open_media)
            open_media()
        elif message_context:
            messages = services.exposed.get("Messages")
            if messages is None:
                scenario_ready = True
                return

            def show_context_paging() -> None:
                nonlocal scenario_ready
                if not services.app.activeChatId:
                    services.app.openChat("6281111111111@s.whatsapp.net")
                    QTimer.singleShot(40, show_context_paging)
                    return
                if messages.chatId and not messages.loading:
                    now = int(time.time() * 1000)
                    context_rows = [{
                        "id": f"shot-context-{i:02}", "chatId": messages.chatId,
                        "from": messages.chatId, "to": "me", "content": f"Earlier message {i + 1}",
                        "timestamp": now - (40 - i) * 60_000, "status": "received", "type": "text",
                    } for i in range(40)]
                    messages._set_messages(context_rows, has_more=True, has_next=True)
                    scenario_ready = False

                    def center_context() -> None:
                        nonlocal scenario_ready
                        view = find_item("messageList")
                        if view is not None and int(view.property("count")) > 2:
                            # ListView.Center is enum value 1; center to show the live-tail affordance.
                            view.positionViewAtIndex(int(view.property("count") / 2), 1)
                        QTimer.singleShot(100, lambda: finish_context())

                    def finish_context() -> None:
                        nonlocal scenario_ready, scenario_error
                        if not scroll_key:
                            scenario_ready = True
                            return
                        view = find_item("messageList")
                        if view is None:
                            print("[shot] message list is unavailable for keyboard scrolling")
                            scenario_error = True
                            scenario_ready = True
                            return
                        before = float(view.property("contentY"))

                        def send_scroll_key() -> None:
                            nonlocal scenario_ready, scenario_error, before
                            focus_label = "none"
                            if focus_composer:
                                editor = find_item("composerEditor")
                                if editor is None:
                                    print("[shot] composer editor is unavailable for keyboard scrolling")
                                    scenario_error = True
                                    scenario_ready = True
                                    return
                                editor.forceActiveFocus()
                                focus_label = "composer"
                                if composer_text is not None:
                                    for character in composer_text:
                                        char_key = Qt.Key_Space if character == " " else getattr(Qt, f"Key_{character.upper()}", None)
                                        if char_key is None:
                                            print(f"[shot] draft text contains an unsupported key: {character!r}")
                                            scenario_error = True
                                            scenario_ready = True
                                            return
                                        QTest.keyClick(win, char_key)
                                if caret_start:
                                    editor.setProperty("cursorPosition", 0)
                            elif focus_chat_search:
                                search = find_item("chatSearchField")
                                if search is None:
                                    print("[shot] chat search field is unavailable for keyboard focus")
                                    scenario_error = True
                                    scenario_ready = True
                                    return
                                search.forceActiveFocus()
                                focus_label = "chat-search"
                            else:
                                view.forceActiveFocus()
                                focus_label = "message-list"
                            before = float(view.property("contentY"))
                            QTest.keyClick(win, scroll_keys[scroll_key])

                            def check_scroll_key() -> None:
                                nonlocal scenario_ready, scenario_error
                                current = find_item("messageList")
                                after = float(current.property("contentY")) if current is not None else before
                                editor = find_item("composerEditor") if focus_composer else None
                                cursor = int(editor.property("cursorPosition")) if editor is not None else -1
                                if expect_no_scroll:
                                    # The connection banner/virtualized list may settle by a few pixels
                                    # after focus changes; a real page scroll is roughly half a window.
                                    changed = abs(after - before) <= 16
                                    if focus_composer and scroll_key in ("Home", "End") and composer_text:
                                        expected_cursor = len(composer_text) if (scroll_key == "End" or caret_start) else 0
                                        changed = changed and cursor == expected_cursor
                                elif scroll_key == "End":
                                    changed = (not services.exposed["Messages"].hasMoreNext
                                               and not services.exposed["Messages"].loading
                                               and current is not None and bool(current.property("atYEnd")))
                                elif scroll_key == "Home":
                                    changed = current is not None and bool(current.property("atYBeginning"))
                                elif scroll_key == "PageUp":
                                    changed = after < before - 1
                                else:
                                    changed = after > before + 1
                                print(f"[shot] chat scroll key {scroll_key} (focus={focus_label}): {before:.1f} → {after:.1f} caret={cursor}")
                                if not changed:
                                    print("[shot] keyboard scroll action did not move the conversation")
                                    scenario_error = True
                                scenario_ready = True

                            QTimer.singleShot(400, check_scroll_key)

                        QTimer.singleShot(100, send_scroll_key)

                    QTimer.singleShot(250, center_context)
                else:
                    QTimer.singleShot(40, show_context_paging)
            show_context_paging()
        elif compose_dialog or emoji_picker or sticker_picker or attach_menu:
            QTimer.singleShot(500, show_compose_overlay)
        elif chat_sheet:
            chats = services.exposed.get("Chats")
            if chats is not None and chat_sheet in ("info", "search", "starred"):
                chats.requestSheet(chat_sheet, services.app.activeChatId)
            scenario_ready = True
        elif chat_new_action:
            method = {"menu": "showNewMenu", "group": "showGroupDialog", "join": "showJoinDialog",
                      "select": "enterSelectionMode"}.get(chat_new_action, "")
            target = find_item("chatSidebar")
            if target is not None and method:
                QMetaObject.invokeMethod(target, method)
            else:
                print(f"[shot] chat action unavailable: {chat_new_action}")
            scenario_ready = True
        elif auth_tab:
            auth = services.exposed.get("Auth")
            if auth is not None and auth_tab in ("qr", "phone"):
                auth.setTab(auth_tab)
            scenario_ready = True
        elif assistant_panel:
            def show_assistant() -> None:
                bot = services.exposed.get("BotAdmin")
                target = find_item("botCollectionPage")
                if target is not None and bot is not None:
                    QMetaObject.invokeMethod(target, "showAssistant")
                else:
                    print("[shot] bot editor not ready for assistant overlay")
            QTimer.singleShot(500, show_assistant)
            scenario_ready = True

    def navigate_route() -> None:
        if services.backend.apiUrl:
            services.app.navigate(route)
            run_scenario()

    services.backend.apiUrlChanged.connect(navigate_route)
    services.backend.start()
    services.hub.start()
    navigate_route()

    def grab():
        if not scenario_ready:
            QTimer.singleShot(50, grab)
            return
        if scenario_error:
            win.deleteLater()
            engine.deleteLater()
            QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
            app.processEvents()
            app.exit(1)
            return
        if message_context:
            view = find_item("messageList")
            if view is not None:
                print("[shot] message-list edge state:",
                      "beginning=", view.property("atYBeginning"),
                      "end=", view.property("atYEnd"),
                      "contentY=", view.property("contentY"),
                      "originY=", view.property("originY"),
                      "height=", view.property("height"),
                      "contentHeight=", view.property("contentHeight"))
        img = win.grabWindow()
        if img.hasAlphaChannel():
            # Offscreen QPA preserves the translucent window alpha, while GPUI
            # Xvfb captures are already composited against Xvfb's black root.
            # Flatten QML captures the same way for meaningful parity diffs.
            flattened = QImage(img.size(), QImage.Format.Format_RGB32)
            flattened.fill(Qt.GlobalColor.black)
            painter = QPainter(flattened)
            painter.drawImage(0, 0, img)
            painter.end()
            img = flattened
        img.save(out)
        print("saved", out, img.width(), img.height())
        if clipboard_paste == "image":
            composer = services.exposed.get("Composer")
            attachment = composer.pendingAttachment if composer is not None else {}
            image_path = str(attachment.get("path") or "")
            if composer is not None:
                composer.clearPendingAttachment()
            if image_path and not os.path.exists(image_path):
                print("[shot] clipboard image temporary file cleaned")
            else:
                print("[shot] clipboard image temporary file cleanup failed")
        # Destroy the QML tree while context-property QObjects are still alive.
        win.deleteLater()
        engine.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        app.processEvents()
        app.quit()

    QTimer.singleShot(wait, grab)
    code = app.exec()
    services.backend.shutdown()
    return code


if __name__ == "__main__":
    sys.exit(run(sys.argv))
