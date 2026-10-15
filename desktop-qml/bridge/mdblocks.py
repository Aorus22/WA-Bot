"""Small Markdown -> block converter for the docs page and the AI assistant (no dependencies).

`parse(md)` returns a list of plain dict blocks that QML renders one by one (heading underlines,
code cards with copy buttons, tables) — things `Text.MarkdownText` cannot style. Inline text becomes Qt
rich-text HTML whose theme dependent colours are placeholders (`%CODE_BG%`, `%LINK%`, `%TK_KEYWORD%`...)
that QML substitutes from the live theme (`BotUtil.themed`).

Block shapes:
  {type: heading, level, html, text, slug}   {type: paragraph, html}   {type: quote, html}   {type: hr}
  {type: list, rows: [{html, depth, marker}]}
  {type: code, lang, code, html}             {type: table, header: [html], align: [left|center|right], rows: [[html]]}
"""
from __future__ import annotations

import html as _html
import re

MONO = "JetBrains Mono"

# ---------------------------------------------------------------------------------------------
# Syntax highlighting (Lua is the product's scripting language; json/bash/js get a light touch)
# ---------------------------------------------------------------------------------------------
LUA_KEYWORDS = {
    "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in",
    "local", "not", "or", "repeat", "return", "then", "until", "while",
}
LUA_CONSTANTS = {"true", "false", "nil"}

# token kinds: keyword string comment number function constant punct plain
_NUM = re.compile(r"0[xX][0-9a-fA-F]+|\d+\.?\d*(?:[eE][+-]?\d+)?")
_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")

# line states for the stateful scanner: 0 normal, ("c", level) inside --[=*[ comment, ("s", level) inside [=*[ string
State = tuple | int


def _long_open(line: str, i: int) -> int | None:
    """If `line[i:]` starts a long bracket `[==[`, return its level."""
    if line[i] != "[":
        return None
    j = i + 1
    while j < len(line) and line[j] == "=":
        j += 1
    return j - i - 1 if j < len(line) and line[j] == "[" else None


def scan_lua_line(line: str, state: State = 0) -> tuple[list[tuple[int, int, str]], State]:
    """Tokenise one Lua line. Returns ([(start, end, kind)...], state_at_end)."""
    spans: list[tuple[int, int, str]] = []
    i, n = 0, len(line)

    def long_end(start: int, level: int, kind: str) -> int | None:
        close = "]" + "=" * level + "]"
        k = line.find(close, start)
        if k < 0:
            spans.append((start, n, kind))
            return None
        spans.append((start, k + len(close), kind))
        return k + len(close)

    if isinstance(state, tuple):
        kind = "comment" if state[0] == "c" else "string"
        end = long_end(0, state[1], kind)
        if end is None:
            return spans, state
        i = end
    while i < n:
        c = line[i]
        if c == "-" and line.startswith("--", i):
            lvl = _long_open(line, i + 2)
            if lvl is not None:
                end = long_end(i, lvl, "comment")
                if end is None:
                    return spans, ("c", lvl)
                i = end
                continue
            spans.append((i, n, "comment"))
            return spans, 0
        if c in "\"'":
            j = i + 1
            while j < n:
                if line[j] == "\\":
                    j += 2
                    continue
                if line[j] == c:
                    j += 1
                    break
                j += 1
            spans.append((i, min(j, n), "string"))
            i = min(j, n)
            continue
        if c == "[":
            lvl = _long_open(line, i)
            if lvl is not None:
                end = long_end(i, lvl, "string")
                if end is None:
                    return spans, ("s", lvl)
                i = end
                continue
        if c.isdigit():
            m = _NUM.match(line, i)
            if m:
                spans.append((i, m.end(), "number"))
                i = m.end()
                continue
        if c.isalpha() or c == "_":
            m = _IDENT.match(line, i)
            word = m.group()
            j = m.end()
            if word in LUA_KEYWORDS:
                spans.append((i, j, "keyword"))
            elif word in LUA_CONSTANTS:
                spans.append((i, j, "constant"))
            elif line[j:j + 1] == "(":
                spans.append((i, j, "function"))
            i = j
            continue
        i += 1
    return spans, 0


_JSON_TOKEN = re.compile(r'"(?:\\.|[^"\\])*"|-?\d+\.?\d*(?:[eE][+-]?\d+)?|\btrue\b|\bfalse\b|\bnull\b')
_GENERIC_TOKEN = re.compile(
    r'(?P<comment>#[^\n]*|//[^\n]*)|(?P<string>"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\')'
    r'|(?P<number>\b\d+\.?\d*\b)|(?P<keyword>\b(?:if|then|else|fi|for|while|do|done|function|return|local|const|let|var|'
    r'import|from|export|async|await|true|false|null|echo|curl)\b)')


def scan_json(text: str) -> list[tuple[int, int, str]]:
    spans = []
    for m in _JSON_TOKEN.finditer(text):
        tok = m.group()
        if tok.startswith('"'):
            rest = text[m.end():].lstrip(" \t")
            spans.append((m.start(), m.end(), "keyword" if rest.startswith(":") else "string"))
        elif tok in ("true", "false", "null"):
            spans.append((m.start(), m.end(), "constant"))
        else:
            spans.append((m.start(), m.end(), "number"))
    return spans


def scan_generic(text: str) -> list[tuple[int, int, str]]:
    return [(m.start(), m.end(), m.lastgroup) for m in _GENERIC_TOKEN.finditer(text)]


def scan(text: str, lang: str) -> list[tuple[int, int, str]]:
    """Whole-text spans (absolute offsets) for `lang`."""
    lang = (lang or "").lower()
    if lang == "lua":
        spans, state, off = [], 0, 0
        for line in text.split("\n"):
            ls, state = scan_lua_line(line, state)
            spans += [(a + off, b + off, k) for a, b, k in ls]
            off += len(line) + 1
        return spans
    if lang == "json":
        return scan_json(text)
    if lang in ("bash", "sh", "shell", "js", "javascript", "ts", "typescript", "python", "py", "go", "yaml", "yml", "http"):
        return scan_generic(text)
    return []


def highlight_html(code: str, lang: str) -> str:
    """Escaped HTML for a code block with theme placeholders (`%TK_<KIND>%`), newlines as <br>."""
    spans = sorted(scan(code, lang))
    out, pos = [], 0

    def esc(s: str) -> str:
        return (_html.escape(s, quote=False).replace(" ", "&nbsp;").replace("\t", "&nbsp;" * 4)
                .replace("\n", "<br>"))

    for a, b, kind in spans:
        if a < pos:
            continue
        out.append(esc(code[pos:a]))
        out.append(f'<span style="color:%TK_{kind.upper()}%">{esc(code[a:b])}</span>')
        pos = b
    out.append(esc(code[pos:]))
    return "".join(out)


# ---------------------------------------------------------------------------------------------
# Inline markdown
# ---------------------------------------------------------------------------------------------
_CODE_SPAN = re.compile(r"(`+)(.+?)\1", re.S)
_LINK = re.compile(r"\[([^\]]+)\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
_IMG = re.compile(r"!\[([^\]]*)\]\(([^)\s]+)[^)]*\)")
_AUTOLINK = re.compile(r"(?<![\"'>=(])(https?://[^\s<>()]+[^\s<>().,;:!?])")
_BOLD = re.compile(r"(\*\*|__)(?=\S)(.+?)(?<=\S)\1")
_ITAL = re.compile(r"(?<![\w*])(\*|_)(?=\S)(.+?)(?<=\S)\1(?![\w*])")
_DEL = re.compile(r"~~(?=\S)(.+?)(?<=\S)~~")


def inline(text: str) -> str:
    """Inline markdown -> Qt rich text (placeholders for colours)."""
    stash: list[str] = []

    def keep(s: str) -> str:
        stash.append(s)
        return f"\x00{len(stash) - 1}\x00"

    def code(m: re.Match) -> str:
        body = _html.escape(m.group(2).strip(), quote=False).replace(" ", "&nbsp;")
        return keep(f'<span style="font-family:\'{MONO}\'; background-color:%CODE_BG%; color:%CODE_FG%">'
                    f"&nbsp;{body}&nbsp;</span>")

    s = _CODE_SPAN.sub(code, text)
    s = _html.escape(s, quote=False)
    s = _IMG.sub(lambda m: keep(f"[image: {_html.escape(m.group(1) or m.group(2), quote=False)}]"), s)
    s = _LINK.sub(lambda m: keep(f'<a href="{_html.escape(m.group(2), quote=True)}" style="color:%LINK%">{m.group(1)}</a>'), s)
    s = _AUTOLINK.sub(lambda m: keep(f'<a href="{m.group(1)}" style="color:%LINK%">{m.group(1)}</a>'), s)
    s = _BOLD.sub(lambda m: f"<b>{m.group(2)}</b>", s)
    s = _ITAL.sub(lambda m: f"<i>{m.group(2)}</i>", s)
    s = _DEL.sub(lambda m: f"<s>{m.group(1)}</s>", s)
    s = s.replace("\n", "<br>")
    while "\x00" in s:
        s = re.sub(r"\x00(\d+)\x00", lambda m: stash[int(m.group(1))], s)
    return s


def plain(text: str) -> str:
    """Inline markdown stripped to plain text (heading labels, slugs)."""
    s = _CODE_SPAN.sub(lambda m: m.group(2), text)
    s = _LINK.sub(lambda m: m.group(1), s)
    s = re.sub(r"[*_~]{1,3}", "", s)
    return s.strip()


def slug(text: str) -> str:
    """Same rule as the web page's `h2` ids: lowercase, drop punctuation, spaces -> dashes."""
    s = re.sub(r"[^\w\s-]", "", plain(text).lower()).strip()
    return re.sub(r"\s+", "-", s)


# ---------------------------------------------------------------------------------------------
# Block parser
# ---------------------------------------------------------------------------------------------
_FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})\s*([\w+#.-]*)[^\n]*$")
_HEADING = re.compile(r"^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$")
_HR = re.compile(r"^\s{0,3}([-*_])(?:\s*\1){2,}\s*$")
_LIST = re.compile(r"^(\s*)([-*+]|\d+[.)])\s+(.*)$")
_TABLE_SEP = re.compile(r"^\s*\|?\s*:?-{1,}:?\s*(\|\s*:?-{1,}:?\s*)*\|?\s*$")


def _split_row(line: str) -> list[str]:
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    cells, cur, i = [], "", 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] == "|":
            cur += "|"
            i += 2
            continue
        if s[i] == "`":  # pipes inside code spans do not split
            j = s.find("`", i + 1)
            if j > 0:
                cur += s[i:j + 1]
                i = j + 1
                continue
        if s[i] == "|":
            cells.append(cur.strip())
            cur = ""
        else:
            cur += s[i]
        i += 1
    cells.append(cur.strip())
    return cells


def parse(md: str) -> list[dict]:
    lines = (md or "").replace("\r\n", "\n").replace("\r", "\n").split("\n")
    blocks: list[dict] = []
    i, n = 0, len(lines)
    para: list[str] = []

    def flush() -> None:
        if para:
            blocks.append({"type": "paragraph", "html": inline(" ".join(p.strip() for p in para))})
            para.clear()

    while i < n:
        line = lines[i]
        m = _FENCE.match(line)
        if m:
            flush()
            fence, lang = m.group(1), m.group(2)
            body = []
            i += 1
            while i < n and not (lines[i].strip().startswith(fence[0] * len(fence)) and set(lines[i].strip()) <= {fence[0]}):
                body.append(lines[i])
                i += 1
            i += 1  # closing fence (or EOF)
            code = "\n".join(body)
            blocks.append({"type": "code", "lang": lang, "code": code, "html": highlight_html(code, lang)})
            continue
        if not line.strip():
            flush()
            i += 1
            continue
        m = _HEADING.match(line)
        if m:
            flush()
            level, text = len(m.group(1)), m.group(2)
            blocks.append({"type": "heading", "level": level, "html": inline(text), "text": plain(text), "slug": slug(text)})
            i += 1
            continue
        if _HR.match(line) and not para:
            blocks.append({"type": "hr"})
            i += 1
            continue
        # table: header row followed by a separator row
        if "|" in line and i + 1 < n and _TABLE_SEP.match(lines[i + 1]) and "-" in lines[i + 1]:
            flush()
            header = _split_row(line)
            aligns = []
            for c in _split_row(lines[i + 1]):
                c = c.strip()
                aligns.append("center" if c.startswith(":") and c.endswith(":") else "right" if c.endswith(":") else "left")
            i += 2
            rows = []
            while i < n and lines[i].strip() and "|" in lines[i]:
                cells = _split_row(lines[i])
                cells += [""] * (len(header) - len(cells))
                rows.append([inline(c) for c in cells[:len(header)]])
                i += 1
            blocks.append({"type": "table", "header": [inline(h) for h in header],
                           "align": (aligns + ["left"] * len(header))[:len(header)], "rows": rows})
            continue
        if line.lstrip().startswith(">"):
            flush()
            q = []
            while i < n and lines[i].lstrip().startswith(">"):
                q.append(re.sub(r"^\s*>\s?", "", lines[i]))
                i += 1
            blocks.append({"type": "quote", "html": inline("\n".join(q))})
            continue
        m = _LIST.match(line)
        if m and (not para or m.group(2) in ("-", "*", "+", "1.")):
            flush()
            rows = []
            counters: dict[int, int] = {}
            while i < n:
                lm = _LIST.match(lines[i])
                if lm:
                    indent = len(lm.group(1).replace("\t", "    "))
                    depth = indent // 2 if indent < 4 else indent // 4 if indent % 4 == 0 else indent // 2
                    depth = min(depth, 4)
                    mk = lm.group(2)
                    if mk[0].isdigit():
                        counters[depth] = counters.get(depth, 0) + 1
                        marker = f"{counters[depth]}."
                    else:
                        marker = "•" if depth == 0 else "◦"
                        counters.pop(depth, None)
                    rows.append({"html": inline(lm.group(3)), "depth": depth, "marker": marker})
                    i += 1
                elif lines[i].strip() and lines[i].startswith((" ", "\t")) and rows and not _FENCE.match(lines[i]):
                    rows[-1]["html"] += " " + inline(lines[i].strip())  # lazy continuation
                    i += 1
                else:
                    break
            blocks.append({"type": "list", "rows": rows})
            continue
        para.append(line)
        i += 1
    flush()
    return blocks


def headings(blocks: list[dict], max_level: int = 2) -> list[dict]:
    """Table of contents entries `{index, label, slug}` for headings up to `max_level` (h1 is the title)."""
    return [{"index": i, "label": b["text"], "slug": b["slug"], "level": b["level"]}
            for i, b in enumerate(blocks) if b["type"] == "heading" and 2 <= b["level"] <= max_level]


def code_blocks(md: str) -> list[tuple[str, str]]:
    """[(lang, code)] of every fenced block (used to pick the Lua a reply proposes)."""
    return [(b["lang"], b["code"]) for b in parse(md) if b["type"] == "code"]
