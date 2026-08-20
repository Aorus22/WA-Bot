import { create } from "zustand"
import { type Chat, type Message } from "@/lib/api"

export interface ChatMessagesEntry {
	messages: Message[]
	hasMore: boolean
	hasMoreNext: boolean
	loaded: boolean
	loading: boolean
	loadingMore: boolean
	loadingNewer: boolean
}

interface ChatStore {
	chats: Chat[]
	chatsLoaded: boolean
	chatsLoading: boolean
	messagesByChat: Record<string, ChatMessagesEntry>

	setChats: (chats: Chat[]) => void
	setChatsLoaded: (v: boolean) => void
	setChatsLoading: (v: boolean) => void
	upsertChat: (chat: Chat) => void

	ensureChatState: (chatId: string) => void
	setMessages: (chatId: string, msgs: Message[], hasMore: boolean) => void
	prependMessages: (chatId: string, msgs: Message[], hasMore: boolean) => void
	appendMessages: (chatId: string, msgs: Message[]) => void
	upsertMessage: (chatId: string, msg: Message) => void
	patchMessage: (chatId: string, msgId: string, patch: Partial<Message>) => void
	deleteMessage: (chatId: string, msgId: string) => void
	setMessageLoaded: (chatId: string, loaded: boolean) => void
	setMessageLoading: (chatId: string, field: "loading" | "loadingMore" | "loadingNewer", v: boolean) => void
}

const emptyEntry = (): ChatMessagesEntry => ({
	messages: [],
	hasMore: true,
	hasMoreNext: false,
	loaded: false,
	loading: false,
	loadingMore: false,
	loadingNewer: false,
})

export const useChatStore = create<ChatStore>((set) => ({
	chats: [],
	chatsLoaded: false,
	chatsLoading: false,
	messagesByChat: {},

	setChats: (chats) => set({ chats }),

	setChatsLoaded: (v) => set({ chatsLoaded: v }),

	setChatsLoading: (v) => set({ chatsLoading: v }),

	upsertChat: (chat) =>
		set((state) => {
			const existing = state.chats.find((c) => c.id === chat.id)
			if (!existing) {
				return { chats: [chat, ...state.chats] }
			}

			// Preserve unread unless the update explicitly carries one
			const merged: Chat = {
				...existing,
				...chat,
				unread: chat.unread !== undefined ? chat.unread : existing.unread,
			}

			// Move to top if last message/time changed
			const lastChanged =
				chat.lastMsg !== undefined && chat.lastMsg !== existing.lastMsg
				|| chat.lastTime !== undefined && chat.lastTime !== existing.lastTime

			if (lastChanged) {
				return { chats: [merged, ...state.chats.filter((c) => c.id !== chat.id)] }
			}

			return {
				chats: state.chats.map((c) => (c.id === chat.id ? merged : c)),
			}
		}),

	ensureChatState: (chatId) =>
		set((state) => {
			if (state.messagesByChat[chatId]) return state
			return { messagesByChat: { ...state.messagesByChat, [chatId]: emptyEntry() } }
		}),

	setMessages: (chatId, msgs, hasMore) =>
		set((state) => {
			const prev = state.messagesByChat[chatId] ?? emptyEntry()
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: {
						...prev,
						messages: msgs,
						hasMore,
						hasMoreNext: false,
						loaded: true,
						loading: false,
					},
				},
			}
		}),

	prependMessages: (chatId, msgs, hasMore) =>
		set((state) => {
			const prev = state.messagesByChat[chatId] ?? emptyEntry()
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: {
						...prev,
						messages: [...msgs, ...prev.messages],
						hasMore,
						loadingMore: false,
					},
				},
			}
		}),

	appendMessages: (chatId, msgs) =>
		set((state) => {
			const prev = state.messagesByChat[chatId] ?? emptyEntry()
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: {
						...prev,
						messages: [...prev.messages, ...msgs],
						hasMoreNext: msgs.length > 0,
						loadingNewer: false,
					},
				},
			}
		}),

	upsertMessage: (chatId, msg) =>
		set((state) => {
			const prev = state.messagesByChat[chatId] ?? emptyEntry()
			const existingIndex = prev.messages.findIndex((m) => m.id === msg.id)

			// If the incoming message has a real id and matches a pending temp message,
			// replace the temp one in place.
			if (existingIndex === -1 && !msg.id.startsWith("temp-")) {
				const pendingIndex = prev.messages.findIndex(
					(m) =>
						m.status === "pending" &&
						m.id.startsWith("temp-") &&
						(m.content === msg.content ||
							(m.type === msg.type &&
								["image", "video", "sticker", "document", "audio", "ptt", "voice"].includes(m.type)))
				)
				if (pendingIndex !== -1) {
					const next = [...prev.messages]
					next[pendingIndex] = msg
					return {
						messagesByChat: {
							...state.messagesByChat,
							[chatId]: { ...prev, messages: next },
						},
					}
				}
			}

			if (existingIndex !== -1) {
				const next = [...prev.messages]
				next[existingIndex] = msg
				return {
					messagesByChat: {
						...state.messagesByChat,
						[chatId]: { ...prev, messages: next },
					},
				}
			}

			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: { ...prev, messages: [...prev.messages, msg] },
				},
			}
		}),

	patchMessage: (chatId, msgId, patch) =>
		set((state) => {
			const prev = state.messagesByChat[chatId]
			if (!prev) return state
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: {
						...prev,
						messages: prev.messages.map((m) =>
							m.id === msgId ? { ...m, ...patch } : m
						),
					},
				},
			}
		}),

	deleteMessage: (chatId, msgId) =>
		set((state) => {
			const prev = state.messagesByChat[chatId]
			if (!prev) return state
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: {
						...prev,
						messages: prev.messages.filter((m) => m.id !== msgId),
					},
				},
			}
		}),

	setMessageLoaded: (chatId, loaded) =>
		set((state) => {
			const prev = state.messagesByChat[chatId] ?? emptyEntry()
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: { ...prev, loaded },
				},
			}
		}),

	setMessageLoading: (chatId, field, v) =>
		set((state) => {
			const prev = state.messagesByChat[chatId] ?? emptyEntry()
			return {
				messagesByChat: {
					...state.messagesByChat,
					[chatId]: { ...prev, [field]: v },
				},
			}
		}),
}))