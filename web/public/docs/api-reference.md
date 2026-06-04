# Bot Lua API Documentation

Welcome to the Lua API documentation for Dynamic Triggers and Cron Jobs. Use the functions below to create interactive and intelligent WhatsApp bots.

## Global Variables
These variables are directly available in your script:

| Variable | Type | Description |
| :--- | :--- | :--- |
| `msg` | `table` | **(Triggers Only)** Complete message state. Includes `id`, `sender`, `chat_id`, `content`, `timestamp`, `is_group`. |
| `sender` | `string` | **(Triggers Only)** WhatsApp ID of the sender (JID). Example: `628xxx@s.whatsapp.net` |
| `content` | `string` | **(Triggers Only)** The complete message text sent by the user |
| `matches` | `table` | **(Triggers Only)** Regex capture results. `matches[1]` is the first group. |

> **Note:** Global variables like `msg`, `sender`, and `matches` are **not available** in Cron Jobs as they are not triggered by an incoming message.

### The `msg` Table
Provides detailed information about the current message:
- `msg.id`: Unique message ID.
- `msg.sender`: The JID of the person who sent the message.
- `msg.chat_id`: The JID of the chat (can be a Group JID or User JID).
- `msg.content`: The message text.
- `msg.timestamp`: Unix timestamp (seconds).
- `msg.is_group`: Boolean, true if the message is from a group.
- `msg.is_media`: Boolean, true if the message contains an image, video, sticker, or document.
- `msg.type`: The type of media (`"image"`, `"video"`, `"sticker"`, `"document"`, or `"text"`).

---

## Messaging Functions

### `send_text(target, text)`
Sends a plain text message to the target JID.
- **target**: Recipient JID (e.g., `msg.chat_id` or a fixed JID like `628xxx@s.whatsapp.net`).
- **text**: The message content.

### `send_sticker(target, url_or_path)`
Sends a sticker to the target. Supports both web URLs and local paths from `storage_path`.

### `send_media(target, url_or_path, [type], [caption])`
Sends a media file (image, video, document).
- **url_or_path**: Can be a web URL or a local system path.
- **type**: `"image"`, `"video"`, or `"document"`.
- **caption**: Optional companion text.

---

## Utility Functions

### `fetch(url, [options])`
Performs an HTTP request to an external API. Returns a table with `status` and `body`.

### `fetch_to_file(url, filename, [options])`
Fetches binary data (like images) and saves it directly to storage.
- **Returns**: Table with `status`, `path` (absolute), and `filename`.

### `download_media(filename)`
**(Triggers Only)** Downloads the media attached to the current message and saves it to storage.
- **filename**: Desired local filename.
- **Returns**: The absolute path to the saved file.

### `gemini_chat(prompt, [model], [file_path])`
Interacts with the Gemini AI.
- **prompt**: Your instructions for the AI.
- **model**: Optional model name (default: `gemini-2.0-flash`).
- **file_path**: Optional local file path for multi-modal input.

### `get_instagram_url(url)`
Extracts the direct media URL from an Instagram post/reel link.

### `json_decode(json_string)`
Converts a JSON string into a Lua table.

### `json_encode(lua_table)`
Converts a Lua table into a JSON string.

---

## HTML Parsing (Cheerio)
Fast and flexible HTML manipulation based on GoQuery.

### `cheerio.load(html)`
Loads HTML content and returns a selection object.

**Selection Methods:**
- `sel:find(selector)`: Returns a new selection of matching elements.
- `sel:text()`: Gets the combined text content.
- `sel:html()`: Gets the outer HTML.
- `sel:attr(name)`: Gets the value of an attribute.
- `sel:len()`: Returns the number of elements in the selection.
- `sel:each(callback)`: Iterates over the selection. Callback: `function(index, element)`.

---

## Browser Simulation
Full headless browser simulation using Chrome.

### `browser.run(actions, [options])`
Executes a series of browser actions.
- **options**: `{ headless = true }`.

**Supported Actions:**
- `{ action = "navigate", url = "..." }`
- `{ action = "wait_visible", selector = "..." }`
- `{ action = "click", selector = "..." }`
- `{ action = "type", selector = "...", text = "..." }`
- `{ action = "text", selector = "...", key = "..." }`: Result saved in `key`.
- `{ action = "html", key = "..." }`: Outer HTML saved in `key`.
- `{ action = "screenshot", filename = "...", [selector = "..."], [quality = 90] }`
- `{ action = "sleep", ms = 1000 }`

---

## State Management
Used to store temporary conversation context.

### `set_state(jid, state_name)`
Saves a temporary state string for a specific user/chat.

### `get_state(jid)`
Retrieves the currently saved state for a user/chat.

---

## Storage and System

### `storage_save(filename, content)`
Saves text or binary string to a file in the bot's media folder.

### `storage_path(filename)`
Gets the absolute system path for a file.

### `sh(command)`
Executes an arbitrary terminal command.
- **Returns**: Table with `stdout`, `stderr`, `success` (bool), and `error`.

### `ffmpeg(args_table)`
Executes FFmpeg commands.

### `yt_dlp(args_table)`
Downloads videos/audio from supported sites.

---

## Example: Scheduled Web Scraper
```lua
-- Runs every hour (0 * * * *)
local res = fetch("https://news.ycombinator.com")
local $ = cheerio.load(res.body)

local titles = {}
$(".titleline > a"):each(function(i, el)
    if i < 5 then
        table.insert(titles, el:text())
    end
end)

local report = "Top 5 HN News:\n" .. table.concat(titles, "\n")
send_text("628123456789@s.whatsapp.net", report)
```
