import type { Message } from "@/lib/api"
import { isMarkdownContent } from "./renderMd"

/** One-line plain text for a message: its text, or "Photo", "Voice message"… for media. */
export function messageSnippet(message: Pick<Message, "content" | "type">): string {
	let text = message.content || ""
	if (isMarkdownContent(text)) {
		try {
			text = decodeURIComponent(escape(atob(text.trim().slice(5, -2))))
		} catch {
			// keep the encoded text rather than lose the row
		}
	}
	text = text.replace(/\s+/g, " ").trim()
	if (text && !/^\[[^\]]+\]$/.test(text)) return text
	switch (message.type) {
		case "image": return "Photo"
		case "video": return "Video"
		case "gif": return "GIF"
		case "sticker": return "Sticker"
		case "audio": return "Audio"
		case "ptt":
		case "voice": return "Voice message"
		case "document": return "Document"
		case "poll": return "Poll"
		case "location": return "Location"
		case "contact": return "Contact"
		default: return text || "Message"
	}
}
