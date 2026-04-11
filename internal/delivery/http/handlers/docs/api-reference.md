# Bot Lua API Documentation

Welcome to the Lua API documentation for Dynamic Triggers, Cron Jobs, and Webhooks. Use the functions below to create interactive and intelligent WhatsApp bots. 

## Global Variables

These variables are directly available in your script:

| Variable | Type | Description |
|:----------|:------|:------------|
| `msg` | `table` | **(Triggers Only)** Complete message state. Includes `id`, `sender`, `chat_id`, `content`, `timestamp`, `is_group`. |
| `sender` | `string` | **(Triggers Only)** WhatsApp ID of the sender (JID). Example: `628xxx@s.whatsapp.net` |
| `content` | `string` | **(Triggers Only)** The complete message text sent by the user |
| `matches` | `table` | **(Triggers Only)** Regex capture results. `matches[1]` is the first group. |
| `req` | `table` | **(Webhooks Only)** The incoming HTTP request data. See [The `req` Table](#the-req-table) for details. |

> **Note:** Global variables like `msg`, `sender`, and `matches` are **not available** in Cron Jobs and Webhooks as they are not triggered by an incoming message.

### The `msg` Table

Provides detailed information about the current message:

```lua
msg.id          -- Unique message ID
msg.sender      -- The JID of the person who sent the message
msg.chat_id     -- The JID of the chat (can be a Group JID or User JID)
msg.content     -- The message text
msg.timestamp   -- Unix timestamp (seconds)
msg.is_group    -- Boolean, true if the message is from a group
msg.is_media    -- Boolean, true if the message contains an image, video, sticker, or document
msg.type        -- The type of media ("image", "video", "sticker", "document", or "text")
```

---

### The `req` Table

**(Webhooks Only)** Provides full details about the incoming HTTP request that triggered the webhook:

```lua
req.method        -- HTTP method: "GET", "POST", "PUT", "DELETE", etc.
req.path          -- The webhook path segment (e.g., "my-hook")
req.body          -- Parsed JSON body as a Lua table, or nil if not valid JSON
req.raw_body      -- Raw request body as a string
req.headers       -- Table of request headers (key = header name, value = first value)
req.query_params  -- Table of URL query parameters (key = param name, value = first value)
```

#### `req.body`

If the request body is valid JSON, it is automatically parsed into a Lua table:

```lua
-- Request body: {"nama": "Budi", "order_id": 123}
local nama = req.body.nama          -- "Budi"
local order = req.body.order_id     -- 123
```

If the body is not valid JSON, `req.body` will be `nil` and you should use `req.raw_body` instead:

```lua
local raw = req.raw_body            -- raw string, always available
```

#### `req.headers`

```lua
local content_type = req.headers["Content-Type"]  -- "application/json"
local auth = req.headers["Authorization"]          -- "Bearer token123"
```

#### `req.query_params`

```lua
-- URL: /webhook/my-hook?source=stripe&event=payment
local source = req.query_params.source  -- "stripe"
local event = req.query_params.event    -- "payment"
```

#### Customizing the HTTP Response

By default, the webhook returns `200 OK` with `{"status": "ok"}`. You can customize this by setting the global `response` variable in your script:

```lua
response = { status = 200, body = "ok" }
response = { status = 404, body = "not found" }
response = { status = 201, body = "created successfully" }
```

---

## Messaging Functions

### `send_text(target, text)`

Sends a plain text message to the target JID.

```lua
send_text("628123456789@s.whatsapp.net", "Hello from bot!")
send_text(msg.chat_id, "Replying to your message")
```

**Parameters:**
- `target` - Recipient JID (e.g., `msg.chat_id` or a fixed JID like `628xxx@s.whatsapp.net`)
- `text` - The message content

---

### `send_sticker(target, url_or_path)`

Sends a sticker to the target. Supports both web URLs and local paths from `storage_path`.

```lua
send_sticker(sender, "https://example.com/sticker.webp")
send_sticker(sender, storage_path("my_sticker.webp"))
```

**Parameters:**
- `target` - Recipient JID
- `url_or_path` - Web URL or local file path

---

### `send_media(target, url_or_path, [type], [caption])`

Sends a media file (image, video, document).

```lua
send_media(sender, "https://example.com/image.jpg", "image", "Check this out!")
send_media(sender, storage_path("document.pdf"), "document")
```

**Parameters:**
- `target` - Recipient JID
- `url_or_path` - Can be a web URL or a local system path
- `type` - `"image"`, `"video"`, or `"document"`
- `caption` - Optional companion text

---

## Utility Functions

### `fetch(url, [options])`

Performs an HTTP request to an external API. Returns a table with `status` and `body`.

```lua
local res = fetch("https://api.example.com/data")
if res.status == 200 then
    local data = json_decode(res.body)
    send_text(sender, data.message)
end
```

**Parameters:**
- `url` - The URL to fetch
- `options` - Optional table with `method`, `headers`, `body`

**Returns:** `{ status: number, body: string }`

---

### `fetch_to_file(url, filename, [options])`

Fetches binary data (like images) and saves it directly to storage.

```lua
local res = fetch_to_file("https://example.com/image.jpg", "saved_image.jpg")
if res.status == 200 then
    send_media(sender, res.path, "image", "Here's your image!")
end
```

**Returns:** `{ status: number, path: string, filename: string }`

---

### `download_media(filename)`

**(Triggers Only)** Downloads the media attached to the current message and saves it to storage.

```lua
local path = download_media("downloaded_video.mp4")
send_text(sender, "Downloaded to: " .. path)
```

**Parameters:**
- `filename` - Desired local filename

**Returns:** The absolute path to the saved file

---

### `gemini_chat(prompt, [model], [file_path])`

Interacts with the Gemini AI.

```lua
local response = gemini_chat("What's in this image?", "gemini-2.0-flash", image_path)
send_text(sender, response)

-- Simple text query
local answer = gemini_chat("Explain quantum physics in simple terms")
send_text(sender, answer)
```

**Parameters:**
- `prompt` - Your instructions for the AI
- `model` - Optional model name (default: `gemini-2.0-flash`)
- `file_path` - Optional local file path for multi-modal input

---

### `get_instagram_url(url)`

Extracts the direct media URL from an Instagram post/reel link.

```lua
local direct_url = get_instagram_url("https://instagram.com/p/ABC123/")
send_media(sender, direct_url, "image")
```

---

### `get_groups()`

Returns a table of all groups the bot is currently in.

```lua
local groups = get_groups()
for _, group in ipairs(groups) do
    print(group.jid .. " - " .. group.name)
end
```

**Returns:** `{{ jid = "...", name = "..." }, ...}`

---

### `get_participants(group_jid)`

Returns a table of participants in a specific group.

```lua
local participants = get_participants("group_jid@s.whatsapp.net")
for _, p in ipairs(participants) do
    if p.is_admin then
        print("Admin: " .. p.jid)
    end
end
```

**Returns:** `{{ jid = "...", is_admin = bool }, ...}`

---

### `get_duration(file_path)`

Gets the duration of a media file (video/audio) in seconds.

```lua
local duration = get_duration("video.mp4")
print("Duration: " .. duration .. " seconds")
```

---

### `get_mime_type(file_path)`

Returns the MIME type of a local file.

```lua
local mime = get_mime_type("document.pdf")
print("MIME type: " .. mime) -- "application/pdf"
```

---

### `json_decode(json_string)`

Converts a JSON string into a Lua table.

```lua
local data = json_decode('{"name": "John", "age": 30}')
print(data.name) -- "John"
```

---

### `json_encode(lua_table)`

Converts a Lua table into a JSON string.

```lua
local json = json_encode({name = "John", age = 30})
print(json) -- {"name":"John","age":30}
```

---

## HTML Parsing (Cheerio)

Fast and flexible HTML manipulation based on GoQuery.

### `cheerio.load(html)`

Loads HTML content and returns a selection object.

```lua
local $ = cheerio.load(html)
local title = $("h1"):text()
```

### Selection Methods

#### Traversing

```lua
sel:find(selector)       -- Returns a new selection of matching descendant elements
sel:filter(selector)     -- Filters elements based on selector
sel:not(selector)        -- Returns elements that don't match the selector
sel:has(selector)        -- Returns elements that have at least one element matching the selector
sel:children([selector]) -- Gets the direct children of each element
sel:parent([selector])   -- Gets the direct parent of each element
sel:parents([selector])  -- Gets all ancestors of each element
sel:closest(selector)    -- Gets the first ancestor that matches the selector
sel:siblings([selector]) -- Gets the siblings of each element
sel:next([selector])     -- Gets the immediately following sibling
sel:prev([selector])     -- Gets the immediately preceding sibling
sel:nextAll([selector])  -- Gets all following siblings
sel:prevAll([selector])  -- Gets all preceding siblings
sel:first()              -- Reduces selection to the first element
sel:last()               -- Reduces selection to the last element
sel:eq(index)            -- Reduces selection to the element at the specified index
sel:slice(start, [end])   -- Reduces selection to a range of elements
sel:get([index])         -- Gets the DOM element(s) as HTML string
```

#### Manipulation

```lua
sel:each(callback)      -- Iterates over the selection. Callback: function(index, element)
sel:map(callback)       -- Maps selection to values. Returns table
sel:add(selector)       -- Creates new selection by adding elements to the set
sel:addBack([selector])  -- Adds the previous selection to the current selection
sel:end()                -- Ends the most recent filtering operation
```

#### Content & Attributes

```lua
sel:text()               -- Gets the combined text content
sel:html()               -- Gets the outer HTML of the first element
sel:attr(name)           -- Gets the value of an attribute
sel:data(name)           -- Gets the value of a data attribute (data-name)
sel:val()                -- Gets the value of form elements
sel:is(selector)        -- Checks if any element matches the selector. Returns boolean
sel:hasClass(class)      -- Checks if any element has the class. Returns boolean
```

#### Utility

```lua
sel:len()                -- Returns the number of elements in the selection
```

---

## Browser Simulation

Full headless browser simulation using Chrome.

### `browser.run(actions, [options])`

Executes a series of browser actions.

```lua
local actions = {
    { action = "navigate", url = "https://example.com" },
    { action = "wait_visible", selector = "h1" },
    { action = "click", selector = ".button" },
    { action = "type", selector = "input[name='search']", text = "Lua bot" },
    { action = "press_key", selector = "input[name='search']", key = "\r" },
    { action = "sleep", ms = 2000 },
    { action = "html", key = "page_html" }
}

local results = browser.run(actions, { headless = true })
print(results.page_html)
```

**Parameters:**
- `actions` - Array of action objects
- `options` - Optional table (e.g., `{ headless = true }`)

**Returns:** Table containing results from actions with `key` parameter

### Supported Actions

```lua
{ action = "navigate", url = "..." }
{ action = "wait_visible", selector = "..." }
{ action = "click", selector = "..." }
{ action = "type", selector = "...", text = "..." }
{ action = "press_key", selector = "...", key = "..." }           -- e.g., "\r" for Enter
{ action = "evaluate", script = "...", [key = "..."] }           -- Execute JavaScript
{ action = "attribute", selector = "...", attribute = "...", [key = "..."] }
{ action = "text", selector = "...", [key = "..."] }
{ action = "html", [key = "..."] }
{ action = "screenshot", filename = "...", [selector = "..."], [quality = 90] }
{ action = "sleep", ms = 1000 }
```

---

## Redis Persistent Storage

High-performance key-value storage that persists across bot restarts.

### `redis_set(key, value, [ttl])`

Stores a string value.

```lua
redis_set("user:628123:balance", 1000, 3600)  -- Expires in 1 hour
```

### `redis_get(key)`

Retrieves a string value.

```lua
local balance = redis_get("user:628123:balance")
print("Balance: " .. balance)
```

### `redis_del(key)`

Deletes the specified key.

```lua
redis_del("temp:cache")
```

### `redis_exists(key)`

Checks if a key exists.

```lua
if redis_exists("session:123") then
    print("Session exists")
end
```

### `redis_hset(key, field, value)`

Sets a field in a Redis hash.

```lua
redis_hset("user:628123", "name", "John")
redis_hset("user:628123", "age", "30")
```

### `redis_hget(key, field)`

Gets a field value from a Redis hash.

```lua
local name = redis_hget("user:628123", "name")
print("Name: " .. name)
```

### `redis_hgetall(key)`

Returns all fields and values from a hash as a Lua table.

```lua
local user = redis_hgetall("user:628123")
print(user.name)  -- "John"
print(user.age)   -- "30"
```

---

## State Management

Used to store temporary conversation context (stored in memory). For long-term persistence, use **Redis**.

### `set_state(jid, state_name)`

Saves a temporary state string for a specific user/chat.

```lua
set_state(sender, "waiting_for_name")
```

### `get_state(jid)`

Retrieves the currently saved state for a user/chat.

```lua
local state = get_state(sender)
if state == "waiting_for_name" then
    send_text(sender, "Please enter your name")
end
```

---

## Storage and System

### `storage_save(filename, content)`

Saves text or binary string to a file in the bot's media folder.

```lua
storage_save("data.json", json_encode(data))
```

### `storage_get(filename)`

Reads the content of a file from storage.

```lua
local content = storage_get("data.json")
local data = json_decode(content)
```

### `storage_delete(filename)`

Deletes a file from storage.

```lua
storage_delete("temp.txt")
```

### `storage_exists(filename)`

Checks if a file exists in storage.

```lua
if storage_exists("important.txt") then
    local content = storage_get("important.txt")
    send_text(sender, content)
end
```

### `storage_path(filename)`

Gets the absolute system path for a file.

```lua
local path = storage_path("video.mp4")
print("Full path: " .. path)
```

### `sh(command)`

Executes an arbitrary terminal command.

```lua
local result = sh("ls -la")
print(result.stdout)  -- Command output
print(result.success)  -- true/false
```

**Returns:** `{ stdout: string, stderr: string, success: bool, error: string }`

### `ffmpeg(args_table)`

Executes FFmpeg commands.

```lua
ffmpeg({"-i", "input.mp4", "-vf", "scale=640:480", "output.mp4"})
```

### `ffprobe(args_table)`

Executes ffprobe commands to get media information.

```lua
ffprobe({"-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1", "video.mp4"})
```

### `ffprobe_json(args_table)`

Executes ffprobe and returns the result as a decoded Lua table.

```lua
local info = ffprobe_json({"-v", "quiet", "-print_format", "json", "-show_format", "video.mp4"})
print(info.format.duration)
```

### `yt_dlp(args_table)`

Downloads videos/audio from supported sites.

```lua
yt_dlp({"-f", "best", "-o", "downloads/%(title)s.%(ext)s", "https://youtube.com/watch?v=ABC123"})
```

### `gallery_dl(args_table)`

Downloads image galleries from supported sites.

```lua
gallery_dl({"-d", "downloads", "https://example.com/gallery"})
```

### `webpmux(args_table)`

Manipulates WebP files (used for stickers).

```lua
webpmux({"-set", "exif", "exif_data.bin", "-o", "output.webp", "input.webp"})
```

### `whatsapp_exif(pack_name, author)`

Generates WhatsApp-compatible EXIF metadata for stickers.

```lua
local exif = whatsapp_exif("My Pack", "Bot Author")
storage_save("exif.bin", exif)
```

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

---

## Example: Complex HTML Parsing

```lua
local $ = cheerio.load(html)

-- Find all articles with specific class
local articles = $("article.post")
print("Found " .. articles:len() .. " articles")

-- Get the first article's title
local title = articles:find("h2.title"):first():text()

-- Get all links from the first article
local links = {}
articles:find("a"):each(function(i, el)
    table.insert(links, el:attr("href"))
end)

-- Check if element has specific class
if $("article"):hasClass("featured") then
    print("Featured article found!")
end

-- Get parent element
local parent = $(".content"):parent()

-- Get next sibling
local next_section = $(".section1"):next()
```

---

## Example: Browser Automation

```lua
-- Navigate to a page, interact with elements, and capture content
local actions = {
    { action = "navigate", url = "https://example.com/form" },
    { action = "wait_visible", selector = "form" },
    { action = "type", selector = "#name", text = "John Doe" },
    { action = "type", selector = "#email", text = "john@example.com" },
    { action = "click", selector = "button[type='submit']" },
    { action = "wait_visible", selector = ".success-message" },
    { action = "text", selector = ".success-message", key = "message" },
    { action = "screenshot", filename = "submission.png" }
}

local results = browser.run(actions, { headless = true })
send_text(sender, "Form submitted! " .. results.message)
```

---

## Example: Complete Chatbot Flow

```lua
-- Check user's state
local state = get_state(sender)

if state == nil then
    send_text(sender, "👋 Welcome! What's your name?")
    set_state(sender, "waiting_name")
elseif state == "waiting_name" then
    redis_hset("user:" .. sender, "name", content)
    send_text(sender, "Nice to meet you, " .. content .. "! How can I help you today?")
    set_state(sender, "ready")
elseif state == "ready" then
    -- Process user request with AI
    local response = gemini_chat("User: " .. content)
    send_text(sender, response)
end
```

---

## Example: Webhook - Order Notification

```lua
-- Triggered by POST /webhook/order-notify
-- Expected body: {"customer": "Budi", "item": "Laptop", "price": 15000000}

local customer = req.body.customer or "Unknown"
local item = req.body.item or "Unknown"
local price = req.body.price or 0

local msg = "New Order!\nCustomer: " .. customer .. "\nItem: " .. item .. "\nPrice: Rp " .. tostring(price)
send_text("628123456789@s.whatsapp.net", msg)

response = { status = 200, body = "notification sent" }
```

---

## Example: Webhook - External API Forwarder

```lua
-- Triggered by POST /webhook/forward
-- Forwards incoming webhook data to another API and notifies via WhatsApp

local data = json_decode(req.raw_body)

-- Store in Redis for later retrieval
redis_set("last_webhook:" .. req.path, req.raw_body, 3600)

-- Forward to external API
local res = fetch("https://api.example.com/ingest", {
    method = "POST",
    headers = { ["Content-Type"] = "application/json" },
    body = req.raw_body
})

if res.status == 200 then
    send_text("628123456789@s.whatsapp.net", "Webhook forwarded successfully to " .. req.path)
    response = { status = 200, body = "ok" }
else
    send_text("628123456789@s.whatsapp.net", "Webhook forward failed: HTTP " .. tostring(res.status))
    response = { status = 502, body = "upstream error" }
end
```

---

## Example: Webhook - GET Health Check with Query Params

```lua
-- Triggered by GET /webhook/health?source=monitor
-- Sends a WhatsApp notification when a health check is pinged

local source = req.query_params.source or "unknown"

if req.method == "GET" then
    send_text("628123456789@s.whatsapp.net", "Health check received from: " .. source)
    response = { status = 200, body = "healthy" }
else
    response = { status = 405, body = "method not allowed" }
end
```
