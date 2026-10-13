import { create } from "zustand"
import { subscribeWS } from "@/lib/ws-bus"

/** How long a typing indicator lasts unless renewed (WhatsApp repeats "composing" every few seconds). */
const TYPING_TTL_MS = 6000

export interface Typist {
	sender: string
	name: string
	audio: boolean
}

type Entry = { name?: string; audio: boolean; at: number }

interface TypingStore {
	/** chat id → sender → entry */
	typing: Record<string, Record<string, Entry>>
	/** Bumped when entries may have gone stale, so subscribers re-read the clock. */
	tick: number
	report: (chatId: string, sender: string, name: string | undefined, composing: boolean, audio: boolean) => void
}

export const useTypingStore = create<TypingStore>((set) => ({
	typing: {},
	tick: 0,
	report: (chatId, sender, name, composing, audio) => {
		const who = sender || chatId
		set((state) => {
			const entries = { ...(state.typing[chatId] ?? {}) }
			if (composing) entries[who] = { name, audio, at: Date.now() }
			else delete entries[who]
			return { typing: { ...state.typing, [chatId]: entries } }
		})
		if (composing) {
			setTimeout(() => set((state) => ({ tick: state.tick + 1 })), TYPING_TTL_MS + 300)
		}
	},
}))

// Fed by the WebSocket bus for the whole app, wherever a chat is shown.
subscribeWS((msg) => {
	if (msg.type !== "chat_presence" || !msg.payload?.chatId) return
	const p = msg.payload
	useTypingStore.getState().report(p.chatId, p.sender || "", p.senderName || undefined, p.state === "composing", p.media === "audio")
})

/** Who is typing in a chat right now, oldest first. `names` resolves a sender JID to a chat name. */
export function typistsOf(
	typing: TypingStore["typing"],
	chatId: string,
	names: (jid: string) => string | undefined,
): Typist[] {
	const now = Date.now()
	return Object.entries(typing[chatId] ?? {})
		.filter(([, entry]) => now - entry.at < TYPING_TTL_MS)
		.sort(([, a], [, b]) => a.at - b.at)
		.map(([sender, entry]) => ({
			sender,
			audio: entry.audio,
			name: entry.name || names(sender) || sender.split("@")[0],
		}))
}

/** The chat-list line, like WhatsApp's: "typing…", or "Name is typing…" in a group. */
export function typingLabel(typists: Typist[], isGroup: boolean): string | null {
	if (typists.length === 0) return null
	const verb = typists[0].audio ? "recording audio…" : "typing…"
	if (!isGroup) return verb
	if (typists.length === 1) return `${typists[0].name} is ${verb}`
	if (typists.length === 2) return `${typists[0].name} and ${typists[1].name} are ${verb}`
	return `${typists[0].name} and ${typists.length - 1} others are ${verb}`
}
