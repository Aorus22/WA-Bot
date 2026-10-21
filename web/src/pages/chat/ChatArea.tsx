import { useState, useEffect, useLayoutEffect, useRef, useCallback, memo, useMemo } from "react"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Send, ArrowLeft, MessageSquare, X, MoreVertical, Search, FileText, Plus, Smile, Paperclip, Sticker, Phone, Video, Mic, Square, Trash2, SendHorizontal, ChevronDown, Hourglass } from "lucide-react"
import { api, type Chat, type ChatPin, type GroupCache, type GroupParticipantInfo, type LinkPreviewMeta, type Message, type MessageReceipt, type PinDuration } from "@/lib/api"
import { useCall } from "@/contexts/CallContext"
import { cn } from "@/lib/utils"
import { useTypingStore, typistsOf } from "@/stores/typingStore"
import { renderFormattedContent, encodeMarkdown } from "./renderMd"
import { toast } from "sonner"
import { ChatInfoSheetModal } from "./ChatInfoSheetModal"
import { ChatImageViewerModal } from "./ChatImageViewerModal"
import { ChatEmojiPickerPopover } from "./ChatEmojiPickerPopover"
import { ChatStickerPickerPopover } from "./ChatStickerPickerPopover"
import { ChatSearchSheet } from "./ChatSearchSheet"
import { MessageInfoSheet } from "./MessageInfoSheet"
import { ChatMessageItem } from "./ChatMessageItem"
import { PinnedBanner } from "./PinnedBanner"
import { ComposerLinkPreview } from "./ComposerLinkPreview"
import { RecordingWaveform } from "./RecordingWaveform"
import { disappearingTimerLabel } from "./disappearing"
import { WAAudioPlayer } from "./WAAudioPlayer"
import { PollDialog, LocationDialog, ContactDialog, ForwardDialog } from "./ChatComposeDialogs"
import { useChatStore } from "@/stores/chatStore"
import { subscribeWS } from "@/lib/ws-bus"
import { MapPin, User, BarChart3, ImagePlay } from "lucide-react"

interface ChatAreaProps {
    chat: Chat | null
    incomingMessage?: { chatId: string; message: Message } | null
    statusUpdate?: { id: string; status: string } | null
    onBack?: () => void
    className?: string
    cachedMessages?: Message[]
    cachedHasMore?: boolean
    onCacheUpdate?: (messages: Message[], hasMore: boolean) => void
}

const getMediaUrl = (url: string | undefined): string | undefined => {
    return api.mediaURL(url)
}

const getAvatarUrl = (target: Chat | string): string | undefined => {
    if (typeof target !== "string") {
        const avatar = target.avatar || ""
        if (avatar.length > 0 && !avatar.startsWith("data:")) {
            return avatar
        }
        return api.mediaURL(`/avatar/${encodeURIComponent(target.id)}`)
    }

    return api.mediaURL(`/avatar/${encodeURIComponent(target)}`)
}

const formatTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
}

const handleDownload = async (url: string, filename: string) => {
    try {
        const response = await fetch(url)
        const blob = await response.blob()
        const blobUrl = window.URL.createObjectURL(blob)
        const link = document.createElement("a")
        link.href = blobUrl
        link.download = filename
        document.body.appendChild(link)
        link.click()
        document.body.removeChild(link)
        window.URL.revokeObjectURL(blobUrl)
    } catch (error) {
        console.error("Download failed:", error)
        window.open(url, "_blank")
    }
}

const formatDate = (timestamp: number) => {
    const date = new Date(timestamp)
    const today = new Date()
    const yesterday = new Date()
    yesterday.setDate(today.getDate() - 1)

    if (date.toDateString() === today.toDateString()) return "Today"
    if (date.toDateString() === yesterday.toDateString()) return "Yesterday"

    return date.toLocaleDateString([], { day: "numeric", month: "long", year: "numeric" })
}

const AUDIO_EXTENSIONS = ["mp3", "ogg", "oga", "m4a", "wav", "opus", "webm", "aac", "flac"]

const isAudioFile = (file: File): boolean => {
    if (file.type.startsWith("audio/")) return true
    const ext = file.name.split(".").pop()?.toLowerCase() || ""
    return AUDIO_EXTENSIONS.includes(ext)
}

const formatRecordingTime = (seconds: number) => {
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    return `${m}:${s.toString().padStart(2, "0")}`
}

const URL_PATTERN = /https?:\/\/[^\s<>"']+/i

// First link of a draft, without the punctuation that ends the sentence around it.
const firstUrl = (text: string): string | null => {
    const match = text.match(URL_PATTERN)
    return match ? match[0].replace(/[.,!?;:)\]]+$/, "") : null
}

// Previews fetched this session; null = the server had nothing for that URL.
const linkPreviewCache = new Map<string, LinkPreviewMeta | null>()

const participantLabel = (participant: GroupParticipantInfo) => participant.name?.trim() || participant.jid.split("@")[0]

const buildMentionPayload = (text: string, members: GroupParticipantInfo[]) => {
    let content = text
    const mentions: string[] = []
    const candidates = [...members].sort((a, b) => participantLabel(b).length - participantLabel(a).length)
    for (const member of candidates) {
        const label = `@${participantLabel(member)}`
        let offset = 0
        let replaced = false
        while ((offset = content.indexOf(label, offset)) !== -1) {
            const after = content[offset + label.length]
            const before = content[offset - 1]
            const boundaryBefore = offset === 0 || /\s/.test(before)
            const boundaryAfter = after == null || /[\s.,!?;:)]/.test(after)
            if (boundaryBefore && boundaryAfter) {
                content = `${content.slice(0, offset)}@${member.jid.split("@")[0]}${content.slice(offset + label.length)}`
                offset += member.jid.split("@")[0].length + 1
                replaced = true
            } else {
                offset += label.length
            }
        }
        if (replaced && !mentions.includes(member.jid)) mentions.push(member.jid)
    }
    return { content, mentions }
}

    export const ChatArea = memo(({
 
    chat, 
    onBack, 
    className,
}: ChatAreaProps) => {
    const { startCall, startGroupCall, activeCall } = useCall()
    const entry = useChatStore(s => s.messagesByChat[chat?.id ?? ""])
    const messages = entry?.messages ?? []
    const hasMore = entry?.hasMore ?? true
    const hasMoreNext = entry?.hasMoreNext ?? false
    const loading = entry?.loading ?? false
    const loadingMore = entry?.loadingMore ?? false
    const loadingNewer = entry?.loadingNewer ?? false
    const initialLoad = !(entry?.loaded ?? false)
    const [inputMessage, setInputMessage] = useState("")
    const [groupInfo, setGroupInfo] = useState<GroupCache | null>(null)
    const [mentionTrigger, setMentionTrigger] = useState<{ start: number; end: number; query: string } | null>(null)
    // Highlighted row of the @-list; the arrow keys move it.
    const [mentionIndex, setMentionIndex] = useState(0)
    const typingByChat = useTypingStore(s => s.typing)
    useTypingStore(s => s.tick)
    const [viewedByMessage, setViewedByMessage] = useState<Message | null>(null)
    const [viewedByReceipts, setViewedByReceipts] = useState<MessageReceipt[]>([])
    const [viewedByRecipients, setViewedByRecipients] = useState<string[]>([])
    const [loadingViewedBy, setLoadingViewedBy] = useState(false)
    const [sending, setSending] = useState(false)
    const [showFavoriteBtn, setShowFavoriteBtn] = useState<string | null>(null)
    const [replyTo, setReplyTo] = useState<Message | null>(null)
    const [editingMessage, setEditingMessage] = useState<Message | null>(null)
	const [isMdMode, setIsMdMode] = useState(false)
	const [plusOpen, setPlusOpen] = useState(false)
	const [pollOpen, setPollOpen] = useState(false)
	const [locationOpen, setLocationOpen] = useState(false)
	const [contactOpen, setContactOpen] = useState(false)
	const [forwardingMessage, setForwardingMessage] = useState<Message | null>(null)
	const [showScrollBottom, setShowScrollBottom] = useState(false)
	const gifInputRef = useRef<HTMLInputElement>(null)
	const [headerStatus, setHeaderStatus] = useState("")
	const [pins, setPins] = useState<ChatPin[]>([])
	const [linkPreview, setLinkPreview] = useState<{ url: string; meta: LinkPreviewMeta | null } | null>(null)
	// Links whose preview the user closed; those go out with noLinkPreview.
	const [dismissedLinks, setDismissedLinks] = useState<string[]>([])
	const [recordingStream, setRecordingStream] = useState<MediaStream | null>(null)

	// Typing indicator + availability for the open chat. The typing line
	// auto-clears if no renewal arrives (WhatsApp repeats composing ~every 2.5s).
	useEffect(() => {
		if (!chat) return
		setHeaderStatus("")
		// WhatsApp only sends typing while the account is online; the backend
		// goes online on subscribe, for groups too.
		api.subscribeChatPresence(chat.id).catch(() => {})
		let clearTimer: ReturnType<typeof setTimeout> | undefined
		const armClear = (ms: number) => {
			if (clearTimer) clearTimeout(clearTimer)
			clearTimer = setTimeout(() => setHeaderStatus(""), ms)
		}
		const unsubPresence = subscribeWS((msg) => {
			if (!chat) return
			if (msg.type === "chat_presence" && msg.payload?.chatId === chat.id) {
				if (msg.payload.state === "composing") {
					setHeaderStatus(msg.payload.media === "audio" ? "merekam suara…" : "mengetik…")
					armClear(6000)
				} else {
					setHeaderStatus("")
					if (clearTimer) clearTimeout(clearTimer)
				}
				return
			}
			if (msg.type === "presence" && msg.payload?.jid === chat.id) {
				if (msg.payload.available) {
					setHeaderStatus("online")
					if (clearTimer) clearTimeout(clearTimer)
				} else {
					const ls = Number(msg.payload.lastSeen)
					if (ls > 0) {
						const d = new Date(ls)
						const today = new Date().toDateString() === d.toDateString()
						const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
						setHeaderStatus(today ? `terakhir dilihat ${time}` : `terakhir dilihat ${d.toLocaleDateString()} ${time}`)
					} else {
						setHeaderStatus("")
					}
				}
			}
		})
		return () => {
			unsubPresence()
			if (clearTimer) clearTimeout(clearTimer)
		}
	}, [chat?.id])

    useEffect(() => {
        let active = true
        setGroupInfo(null)
        setMentionTrigger(null)
        if (chat?.isGroup) {
            api.getGroup(chat.id).then(info => {
                if (active) setGroupInfo(info)
            }).catch(error => console.warn("Failed to load group members:", error))
        }
        return () => { active = false }
    }, [chat?.id, chat?.isGroup])

	// In-chat pins: loaded on open and whenever the server says they changed.
	const loadPins = useCallback((chatId: string, isCurrent: () => boolean = () => true) =>
		api.getChatPins(chatId)
			.then(data => { if (isCurrent()) setPins((data || []).filter(pin => pin.message)) })
			.catch(error => console.warn("Failed to load pinned messages:", error)), [])

	useEffect(() => {
		setPins([])
		if (!chat) return
		let active = true
		const chatId = chat.id
		loadPins(chatId, () => active)
		const unsub = subscribeWS((msg) => {
			if (msg.type === "chat_pins_changed" && msg.payload?.chatId === chatId) loadPins(chatId, () => active)
		})
		return () => {
			active = false
			unsub()
		}
	}, [chat?.id, loadPins])

	// Link preview for the first URL of the draft, after a short pause in typing.
	const draftUrl = editingMessage || replyTo ? null : firstUrl(inputMessage)
	useEffect(() => {
		if (!draftUrl) return
		if (linkPreviewCache.has(draftUrl)) {
			setLinkPreview({ url: draftUrl, meta: linkPreviewCache.get(draftUrl) ?? null })
			return
		}
		let active = true
		const timer = setTimeout(() => {
			api.getLinkPreview(draftUrl)
				.then(meta => {
					linkPreviewCache.set(draftUrl, meta)
					if (active) setLinkPreview({ url: draftUrl, meta })
				})
				.catch(() => {})
		}, 600)
		return () => {
			active = false
			clearTimeout(timer)
		}
	}, [draftUrl])
	const composerPreview = linkPreview && linkPreview.url === draftUrl && !dismissedLinks.includes(draftUrl)
		&& linkPreview.meta && (linkPreview.meta.title || linkPreview.meta.description || linkPreview.meta.thumbnailUrl)
		? linkPreview.meta
		: null

	const [emojiOpen, setEmojiOpen] = useState(false)
	const [stickerOpen, setStickerOpen] = useState(false)
    const [isMediaSheetOpen, setIsMediaSheetOpen] = useState(false)
    const [isSearchOpen, setIsSearchOpen] = useState(false)
    const [highlightedMessageId, setHighlightedMessageId] = useState<string | null>(null)
    const [selectedImageUrl, setSelectedImageUrl] = useState<string | null>(null)
    const [imageSourceRect, setImageSourceRect] = useState<DOMRect | null>(null)
    const lastStickerMsgRef = useRef<{ id: string; mediaUrl: string }>({ id: "", mediaUrl: "" })

    // --- Audio recording state ---
    const [isRecording, setIsRecording] = useState(false)
    const [recordingSeconds, setRecordingSeconds] = useState(0)
    const [pendingAudio, setPendingAudio] = useState<{ file: File; seconds: number; url: string } | null>(null)
    const mediaRecorderRef = useRef<MediaRecorder | null>(null)
    const mediaStreamRef = useRef<MediaStream | null>(null)
    const recordedChunksRef = useRef<Blob[]>([])
    const recordingTimerRef = useRef<number | null>(null)
    const recordingStartRef = useRef(0)
    const cancelRecordingRef = useRef(false)

    const scrollRef = useRef<HTMLDivElement>(null)
    const mediaInputRef = useRef<HTMLInputElement>(null)
    const documentInputRef = useRef<HTMLInputElement>(null)
    const audioInputRef = useRef<HTMLInputElement>(null)
    const inputRef = useRef<HTMLTextAreaElement>(null)

    useLayoutEffect(() => {
        const textarea = inputRef.current
        if (!textarea) return
        const maxHeight = 150
        textarea.style.height = "auto"
        const contentHeight = textarea.scrollHeight
        textarea.style.height = `${Math.min(contentHeight, maxHeight)}px`
        textarea.style.overflowY = contentHeight > maxHeight ? "auto" : "hidden"
    }, [inputMessage])

    const scrollToBottom = useCallback((behavior: ScrollBehavior = "smooth") => {
        if (scrollRef.current) {
            const { scrollHeight, clientHeight } = scrollRef.current
            scrollRef.current.scrollTo({
                top: scrollHeight - clientHeight,
                behavior
            })
            setShowScrollBottom(false)
        }
    }, [])

    // Hydrate from store or load messages for the current chat
    useEffect(() => {
        if (!chat) return
        setShowScrollBottom(false)
        const storeEntry = useChatStore.getState().messagesByChat[chat.id]
        if (storeEntry?.loaded) {
            setTimeout(() => scrollToBottom("auto"), 50)
        } else {
            loadMessages()
        }
    }, [chat?.id, entry?.loaded])

    // A draft voice message or dismissed link preview belongs to the chat it was made in.
    useEffect(() => {
        setDismissedLinks([])
        setPendingAudio(prev => {
            if (prev) URL.revokeObjectURL(prev.url)
            return null
        })
    }, [chat?.id])

    // Stop any in-progress recording when switching chats
    useEffect(() => {
        return () => {
            cancelRecording()
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [chat?.id])

    // Cleanup media resources on unmount
    useEffect(() => {
        return () => {
            if (recordingTimerRef.current) {
                clearInterval(recordingTimerRef.current)
                recordingTimerRef.current = null
            }
            if (mediaStreamRef.current) {
                mediaStreamRef.current.getTracks().forEach(track => track.stop())
                mediaStreamRef.current = null
            }
            if (mediaRecorderRef.current && mediaRecorderRef.current.state !== "inactive") {
                mediaRecorderRef.current.stop()
                mediaRecorderRef.current = null
            }
        }
    }, [])

    // When the list shrinks or grows (pinned banner, composer link preview,
    // reply bar) a reader at the bottom stays at the bottom.
    const stickToBottomRef = useRef(true)
    useEffect(() => {
        const list = scrollRef.current
        if (!list || typeof ResizeObserver === "undefined") return
        const observer = new ResizeObserver(() => {
            if (stickToBottomRef.current) list.scrollTop = list.scrollHeight
        })
        observer.observe(list)
        return () => observer.disconnect()
    }, [chat?.id])

    // Scroll to bottom when a new message arrives at the end of the list
    const lastMessageId = messages[messages.length - 1]?.id
    useEffect(() => {
        if (chat && lastMessageId) {
            scrollToBottom()
        }
    }, [lastMessageId, chat?.id, scrollToBottom])

    useEffect(() => {
        if (replyTo || editingMessage) {
            inputRef.current?.focus()
        }
    }, [replyTo, editingMessage])

    const updateMentionTrigger = (value: string, caret: number) => {
        if (!chat?.isGroup || editingMessage) {
            setMentionTrigger(null)
            return
        }
        const beforeCaret = value.slice(0, caret)
        const match = beforeCaret.match(/(?:^|\s)@([^\s@]*)$/)
        if (!match) {
            setMentionTrigger(null)
            return
        }
        const query = match[1]
        const start = caret - query.length - 1
        if (query !== mentionTrigger?.query) setMentionIndex(0)
        setMentionTrigger({ start, end: caret, query })
    }

    const mentionMatches: GroupParticipantInfo[] = chat?.isGroup && mentionTrigger && !editingMessage
        ? (groupInfo?.participants ?? []).filter(member => {
            const query = mentionTrigger.query.toLowerCase()
            return participantLabel(member).toLowerCase().includes(query) || member.jid.toLowerCase().includes(query)
        }).slice(0, 8)
        : []
    const activeMention = Math.min(mentionIndex, Math.max(mentionMatches.length - 1, 0))

    const insertMention = (participant: GroupParticipantInfo) => {
        if (!mentionTrigger) return
        const label = `@${participantLabel(participant)} `
        const next = `${inputMessage.slice(0, mentionTrigger.start)}${label}${inputMessage.slice(mentionTrigger.end)}`
        const caret = mentionTrigger.start + label.length
        setInputMessage(next)
        setMentionTrigger(null)
        requestAnimationFrame(() => {
            inputRef.current?.focus()
            inputRef.current?.setSelectionRange(caret, caret)
        })
    }

    const handleViewedBy = async (message: Message) => {
        setViewedByMessage(message)
        setViewedByReceipts([])
        setViewedByRecipients([])
        setLoadingViewedBy(true)
        try {
            const info = await api.getMessageReceipts(message.chatId, message.id)
            setViewedByReceipts(info.receipts)
            setViewedByRecipients(info.recipients ?? [])
        } catch (error) {
            console.error("Failed to load read receipts:", error)
            toast.error("Gagal memuat Message info")
        } finally {
            setLoadingViewedBy(false)
        }
    }

    const loadMessages = async () => {
        if (!chat) return
        const store = useChatStore.getState()
        const storeEntry = store.messagesByChat[chat.id]
        if (storeEntry?.loading || storeEntry?.loaded) return
        store.setMessageLoading(chat.id, "loading", true)
        try {
            const data = await api.getMessages(chat.id, 30)
            store.setMessages(chat.id, data || [], (data || []).length === 30)
            setTimeout(() => scrollToBottom("auto"), 50)
        } catch (error) {
            console.error("Failed to load messages:", error)
            toast.error("Failed to load conversation")
            store.setMessageLoading(chat.id, "loading", false)
        }
    }

    const loadMoreMessages = async () => {
        if (!chat || loadingMore || !hasMore || messages.length === 0) return
        
        const oldestMsg = messages[0]
        const store = useChatStore.getState()
        store.setMessageLoading(chat.id, "loadingMore", true)
        try {
            const data = await api.getMessages(chat.id, 30, oldestMsg.timestamp)
            
            if (data && data.length > 0) {
                // Prepend older messages
                store.prependMessages(chat.id, data, data.length === 30)
                
                // Maintain scroll position after prepending
                if (scrollRef.current) {
                    const scrollContainer = scrollRef.current
                    const oldHeight = scrollContainer.scrollHeight
                    
                    requestAnimationFrame(() => {
                        const newHeight = scrollContainer.scrollHeight
                        scrollContainer.scrollTop = newHeight - oldHeight
                    })
                }
            } else {
                store.prependMessages(chat.id, [], false)
            }
        } catch (error) {
            console.error("Failed to load more messages:", error)
            store.setMessageLoading(chat.id, "loadingMore", false)
        }
    }

    const loadNewerMessages = async () => {
        if (!chat || loadingNewer || !hasMoreNext || messages.length === 0) return

        const latestMsg = messages[messages.length - 1]
        const store = useChatStore.getState()
        store.setMessageLoading(chat.id, "loadingNewer", true)
        try {
            const data = await api.getMessages(chat.id, 30, undefined, latestMsg.timestamp)

            if (data && data.length > 0) {
                store.appendMessages(chat.id, data)
            } else {
                store.appendMessages(chat.id, [])
            }
        } catch (error) {
            console.error("Failed to load newer messages:", error)
            store.setMessageLoading(chat.id, "loadingNewer", false)
        }
    }

    const teleportToMessage = async (messageId: string) => {
        if (!chat) return
        const store = useChatStore.getState()
        store.setMessageLoading(chat.id, "loading", true)
        setIsSearchOpen(false)
        try {
            const data = await api.getMessageContext(chat.id, messageId, 30)
            
            if (data && data.length > 0) {
                store.setMessages(chat.id, data, true)
                
                // Scroll to the message after render
                setTimeout(() => {
                    const element = document.getElementById(messageId)
                    if (element) {
                        element.scrollIntoView({ behavior: "auto", block: "center" })
                        setHighlightedMessageId(messageId)
                        setTimeout(() => setHighlightedMessageId(null), 2500)
                    }
                }, 100)
            }
        } catch (error) {
            console.error("Teleport failed:", error)
            toast.error("Gagal berpindah ke pesan")
            store.setMessageLoading(chat.id, "loading", false)
        }
    }

    // Scrolls to a message already on screen, loading its surroundings when it is not.
    const jumpToMessage = (messageId: string) => {
        const element = document.getElementById(messageId)
        if (!element) {
            teleportToMessage(messageId)
            return
        }
        element.scrollIntoView({ behavior: "smooth", block: "center" })
        setHighlightedMessageId(messageId)
        setTimeout(() => setHighlightedMessageId(null), 2500)
    }

    // The starred list asks the sidebar's chat to open on a message; this chat answers once it is loaded.
    const jumpTarget = useChatStore(s => s.jumpTarget)
    useEffect(() => {
        if (!chat || !jumpTarget || jumpTarget.chatId !== chat.id || !entry?.loaded) return
        useChatStore.getState().setJumpTarget(null)
        // Let the chat-open scroll to the bottom settle first.
        setTimeout(() => jumpToMessage(jumpTarget.messageId), 120)
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [jumpTarget, chat?.id, entry?.loaded])

    const handleStar = (message: Message) => {
        if (!chat || message.id.startsWith("temp-")) return
        const starred = !message.starred
        useChatStore.getState().patchMessage(chat.id, message.id, { starred })
        api.starMessage(chat.id, message.id, starred).catch((err: Error) => {
            useChatStore.getState().patchMessage(chat.id, message.id, { starred: !starred })
            toast.error(`Failed to ${starred ? "star" : "unstar"} message: ${err.message}`)
        })
    }

    // duration in seconds pins the message; null unpins it.
    const handlePin = async (message: Message, duration: number | null) => {
        if (!chat || message.id.startsWith("temp-")) return
        try {
            await api.pinMessage(chat.id, message.id, duration !== null, (duration ?? undefined) as PinDuration | undefined)
            await loadPins(chat.id)
        } catch (err) {
            toast.error(`Failed to ${duration !== null ? "pin" : "unpin"} message: ${err instanceof Error ? err.message : err}`)
        }
    }

    const activePins = useMemo(() => pins.filter(pin => pin.message && pin.expiresAt > Date.now()), [pins])
    const pinnedIds = useMemo(() => new Set(activePins.map(pin => pin.messageId)), [activePins])

    const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
        const target = e.currentTarget
        const distanceFromBottom = target.scrollHeight - target.scrollTop - target.clientHeight
        stickToBottomRef.current = distanceFromBottom < 100
        const isNearTop = target.scrollTop < 100
        const isNearBottom = distanceFromBottom < 100

        setShowScrollBottom(distanceFromBottom > 150)

        if (isNearTop && !loadingMore && hasMore) {
            loadMoreMessages()
        }
        if (isNearBottom && !loadingNewer && hasMoreNext) {
            loadNewerMessages()
        }
    }

    const handleScrollToBottom = () => {
        if (hasMoreNext) {
            loadMessages()
        } else {
            scrollToBottom("smooth")
        }
    }

    // PageUp/PageDown/Home/End scroll the conversation. They act with the
    // composer or nothing focused; Home/End stay caret keys once there is a
    // draft. The latest closure sits in a ref so one listener serves every render.
    const scrollKeyRef = useRef<(e: KeyboardEvent) => void>(() => {})
    scrollKeyRef.current = (e: KeyboardEvent) => {
        if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return
        if (!["PageUp", "PageDown", "Home", "End"].includes(e.key)) return
        const list = scrollRef.current
        if (!list || document.querySelector('[role="dialog"]')) return
        const target = e.target as HTMLElement | null
        const inField = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || !!target?.isContentEditable
        if (inField) {
            if (target !== inputRef.current) return
            if ((e.key === "Home" || e.key === "End") && inputRef.current.value) return
        }
        e.preventDefault()
        if (e.key === "End") {
            handleScrollToBottom()
        } else if (e.key === "Home") {
            list.scrollTo({ top: 0, behavior: "auto" })
        } else {
            const page = list.clientHeight * 0.9
            list.scrollBy({ top: e.key === "PageUp" ? -page : page, behavior: "smooth" })
        }
    }
    useEffect(() => {
        const onKey = (e: KeyboardEvent) => scrollKeyRef.current(e)
        window.addEventListener("keydown", onKey)
        return () => window.removeEventListener("keydown", onKey)
    }, [])

    const handleSendMessage = async () => {
        if (!inputMessage.trim() || !chat || sending) return
        const raw = inputMessage.trim()
        const mentionPayload = buildMentionPayload(raw, groupInfo?.participants ?? [])
        const text = isMdMode ? encodeMarkdown(mentionPayload.content) : mentionPayload.content
        const noLinkPreview = !!draftUrl && dismissedLinks.includes(draftUrl)
        const sentPreview = noLinkPreview ? null : composerPreview
        setInputMessage("")
        setDismissedLinks([])
        setMentionTrigger(null)
        setSending(true)

        try {
            if (editingMessage) {
                await api.editMessage(chat.id, editingMessage.id, text)
                setEditingMessage(null)
            } else if (replyTo) {
                await api.replyMessage(chat.id, replyTo.id, text, mentionPayload.mentions)
                setReplyTo(null)
            } else {
                const tempId = "temp-" + Date.now()
                const newMsg: Message = {
                    id: tempId,
                    chatId: chat.id,
                    from: "me",
                    to: chat.id,
                    content: text,
                    timestamp: Date.now(),
                    status: "pending",
                    type: "text"
                }
                if (mentionPayload.mentions.length || sentPreview) {
                    newMsg.extra = {
                        ...(mentionPayload.mentions.length ? { mentions: mentionPayload.mentions } : {}),
                        ...(sentPreview ? { linkPreview: sentPreview } : {}),
                    }
                }

                useChatStore.getState().upsertMessage(chat.id, newMsg)
                setTimeout(() => scrollToBottom(), 50)

                const res = await api.sendMessage(chat.id, text, mentionPayload.mentions, { noLinkPreview })
                const store = useChatStore.getState()
                const entry = store.messagesByChat[chat.id]
                if (entry?.messages.some(m => m.id === res.id)) {
                    store.deleteMessage(chat.id, tempId)
                } else {
                    store.patchMessage(chat.id, tempId, { id: res.id, status: "sent" })
                }
            }
        } catch (error) {
            console.error("Failed to send message:", error)
            toast.error("Gagal memproses pesan")
            setInputMessage(text)
        } finally {
            setSending(false)
        }
    }

    // --- Audio recording ----------------------------------------------------

    const stopStream = () => {
        if (mediaStreamRef.current) {
            mediaStreamRef.current.getTracks().forEach(track => track.stop())
            mediaStreamRef.current = null
        }
        setRecordingStream(null)
    }

    const startRecording = async () => {
        if (!chat || isRecording || sending) return
        try {
            const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
            const mimeType = MediaRecorder.isTypeSupported("audio/ogg; codecs=opus")
                ? "audio/ogg; codecs=opus"
                : MediaRecorder.isTypeSupported("audio/webm")
                    ? "audio/webm"
                    : ""

            const recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined)
            recordedChunksRef.current = []
            cancelRecordingRef.current = false

            recorder.ondataavailable = (e) => {
                if (e.data.size > 0) recordedChunksRef.current.push(e.data)
            }

            recorder.onstop = () => {
                if (cancelRecordingRef.current) {
                    cancelRecordingRef.current = false
                    stopStream()
                    return
                }
                // Without the ";codecs=" part, which servers and file pickers treat as noise.
                const blob = new Blob(recordedChunksRef.current, { type: (recorder.mimeType || "audio/ogg").split(";")[0] })
                const seconds = Math.max(1, Math.round((Date.now() - recordingStartRef.current) / 1000))
                const ext = blob.type.includes("webm") ? "webm" : "ogg"
                const file = new File([blob], `recording_${Date.now()}.${ext}`, { type: blob.type })
                setPendingAudio({ file, seconds, url: URL.createObjectURL(blob) })
                setIsRecording(false)
                setRecordingSeconds(0)
                stopStream()
            }

            recorder.start()
            mediaRecorderRef.current = recorder
            mediaStreamRef.current = stream
            setRecordingStream(stream)
            recordingStartRef.current = Date.now()
            setRecordingSeconds(0)
            setIsRecording(true)
            recordingTimerRef.current = window.setInterval(() => {
                setRecordingSeconds(Math.floor((Date.now() - recordingStartRef.current) / 1000))
            }, 1000)
        } catch (error) {
            console.error("Microphone access denied:", error)
            toast.error("Microphone access denied. Check browser permissions.")
        }
    }

    const stopRecording = () => {
        if (recordingTimerRef.current) {
            clearInterval(recordingTimerRef.current)
            recordingTimerRef.current = null
        }
        if (mediaRecorderRef.current && mediaRecorderRef.current.state === "recording") {
            mediaRecorderRef.current.stop()
        }
    }

    const cancelRecording = () => {
        cancelRecordingRef.current = true
        if (recordingTimerRef.current) {
            clearInterval(recordingTimerRef.current)
            recordingTimerRef.current = null
        }
        if (mediaRecorderRef.current && mediaRecorderRef.current.state === "recording") {
            mediaRecorderRef.current.stop()
        }
        stopStream()
        setIsRecording(false)
        setRecordingSeconds(0)
    }

    const cancelPendingAudio = () => {
        if (pendingAudio) {
            URL.revokeObjectURL(pendingAudio.url)
        }
        setPendingAudio(null)
    }

    const sendPendingAudio = async () => {
        if (!chat || !pendingAudio || sending) return
        const { file, seconds, url } = pendingAudio
        const tempId = "temp-audio-" + Date.now()
        const newMsg: Message = {
            id: tempId,
            chatId: chat.id,
            from: "me",
            to: chat.id,
            content: "[Voice Message]",
            timestamp: Date.now(),
            status: "pending",
            type: "ptt",
            mediaUrl: url
        }

        useChatStore.getState().upsertMessage(chat.id, newMsg)
        setTimeout(() => scrollToBottom(), 50)
        setPendingAudio(null)
        setSending(true)

        try {
            const res = await api.sendMedia(chat.id, file, "ptt", "", { ptt: true, seconds })
            const store = useChatStore.getState()
            const entry = store.messagesByChat[chat.id]
            if (entry?.messages.some(m => m.id === res.id)) {
                store.deleteMessage(chat.id, tempId)
            } else {
                store.patchMessage(chat.id, tempId, { id: res.id, status: "sent" })
            }
        } catch (error) {
            console.error("Failed to send voice message:", error)
            useChatStore.getState().patchMessage(chat.id, tempId, { status: "failed" })
            toast.error("Failed to send voice message")
        } finally {
            setSending(false)
        }
    }

    const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>, type: "image" | "video" | "document" | "audio") => {
        const file = e.target.files?.[0]
        if (!file || !chat) return

        // Detect audio files by extension/mime so audio attachments route to the audio path
        const effectiveType = isAudioFile(file) ? "audio" : type

        const tempId = "temp-media-" + Date.now()
        const newMsg: Message = {
            id: tempId,
            chatId: chat.id,
            from: "me",
            to: chat.id,
            content: effectiveType === "image" ? "[Image]" : effectiveType === "video" ? "[Video]" : effectiveType === "audio" ? "[Audio]" : file.name,
            timestamp: Date.now(),
            status: "pending",
            type: effectiveType,
            mediaUrl: URL.createObjectURL(file)
        }

        useChatStore.getState().upsertMessage(chat.id, newMsg)
        setTimeout(() => scrollToBottom(), 50)

        try {
            const res = await api.sendMedia(chat.id, file, effectiveType, "")
            const store = useChatStore.getState()
            const entry = store.messagesByChat[chat.id]
            if (entry?.messages.some(m => m.id === res.id)) {
                store.deleteMessage(chat.id, tempId)
            } else {
                store.patchMessage(chat.id, tempId, { id: res.id, status: "sent" })
            }
        } catch (error) {
            console.error("Failed to send media:", error)
            useChatStore.getState().patchMessage(chat.id, tempId, { status: "failed" })
            toast.error("Failed to send file")
        } finally {
            if (e.target) e.target.value = ""
        }
    }

    const handleStickerSelect = async (sticker: any) => {
        if (!chat) return

        const tempId = "temp-sticker-" + Date.now()
        const newMsg: Message = {
            id: tempId,
            chatId: chat.id,
            from: "me",
            to: chat.id,
            content: "[Sticker]",
            timestamp: Date.now(),
            status: "pending",
            type: "sticker",
            mediaUrl: sticker.mediaUrl
        }

        useChatStore.getState().upsertMessage(chat.id, newMsg)
        setTimeout(() => scrollToBottom(), 50)

        try {
            const res = await api.sendSticker(chat.id, sticker.mediaUrl, sticker.isAnimated)
            const store = useChatStore.getState()
            const entry = store.messagesByChat[chat.id]
            if (entry?.messages.some(m => m.id === res.id)) {
                store.deleteMessage(chat.id, tempId)
            } else {
                store.patchMessage(chat.id, tempId, { id: res.id, status: "sent" })
            }
        } catch (error) {
            console.error("Failed to send sticker:", error)
            useChatStore.getState().patchMessage(chat.id, tempId, { status: "failed" })
        }
    }

    const handleFavoriteSticker = async (msgId: string, mediaUrl: string, isAnimated: boolean) => {
        try {
            await api.favoriteSticker(msgId, mediaUrl, isAnimated)
            toast.success("Sticker added to favorites")
            setShowFavoriteBtn(null)
        } catch (err) {
            toast.error("Failed to favorite sticker")
        }
    }

    const handleDeleteMessage = async (messageId: string) => {
        if (!chat) return
        try {
            await api.deleteMessage(chat.id, messageId)
            // Deleted for everyone, but never from our own database.
            useChatStore.getState().patchMessage(chat.id, messageId, { revokedAt: Date.now() })
        } catch (err) {
            toast.error("Failed to delete message")
        }
    }

    const handleEditMessage = (message: Message) => {
        setEditingMessage(message)
        setInputMessage(message.content)
        setReplyTo(null)
    }

    const applyReactionLocal = (existing: any[] | undefined, emoji: string): any[] => {
	const out: any[] = []
	for (const r of existing || []) {
		const kept = (r.senders || []).filter((s: string) => s !== "me")
		if (r.emoji === emoji && emoji) kept.push("me")
		if (kept.length > 0) out.push({ emoji: r.emoji, senders: kept })
	}
	if (emoji && !out.some((r) => r.emoji === emoji)) out.push({ emoji, senders: ["me"] })
	return out
}

const handleReact = (message: Message, emoji: string) => {
	if (!chat) return
	const reactions = applyReactionLocal(message.reactions, emoji)
	useChatStore.getState().patchMessage(chat.id, message.id, { reactions })
	api.reactToMessage(chat.id, message.id, emoji, message.from === "me" ? "" : message.from).catch((err: any) => {
		toast.error("Failed to react: " + err.message)
	})
}

const handleVote = (message: Message, options: string[]) => {
	if (!chat) return
	const extra = message.extra ? { ...message.extra } : {}
	if (!extra.poll) return
	const votes = { ...(extra.poll.votes || {}) }
	if (options.length === 0) delete votes["me"]
	else votes["me"] = options
	extra.poll = { ...extra.poll, votes }
	useChatStore.getState().patchMessage(chat.id, message.id, { extra })
	api.sendPollVote(chat.id, message.id, options).catch((err: any) => {
		toast.error("Failed to vote: " + err.message)
	})
}

const handleGifSelected = async (e: React.ChangeEvent<HTMLInputElement>) => {
	const file = e.target.files?.[0]
	e.target.value = ""
	if (!file || !chat) return
	try {
		await api.sendMedia(chat.id, file, "gif", "")
	} catch (err: any) {
		toast.error("Failed to send GIF: " + err.message)
	}
}

const handleReplyMessage = (message: Message) => {
        setReplyTo(message)
        setEditingMessage(null)
    }

    const addEmoji = (emoji: string) => {
        setInputMessage(prev => prev + emoji)
    }

    const groupedMessages = useMemo(() => {
        return messages.reduce((groups: { [key: string]: Message[] }, message) => {
            const date = formatDate(message.timestamp)
            if (!groups[date]) groups[date] = []
            groups[date].push(message)
            return groups
        }, {})
    }, [messages])

    if (!chat) {
        return (
            <div className={cn("flex-1 flex flex-col items-center justify-center bg-muted/10", className)}>
                <div className="w-20 h-20 rounded-3xl bg-primary/5 flex items-center justify-center mb-6">
                    <MessageSquare className="h-10 w-10 text-primary/40" />
                </div>
                <h2 className="text-xl font-bold tracking-tight">Select a conversation</h2>
                <p className="text-muted-foreground text-sm mt-1">Pick a chat from the sidebar to start messaging.</p>
            </div>
        )
    }

    return (
        <div className={cn("flex-1 flex flex-col bg-background relative overflow-x-hidden", className)}>
            <header className="h-16 flex items-center justify-between px-4 border-b border-border/40 bg-background/80 backdrop-blur-xl z-20 sticky top-0">
                <div className="flex items-center gap-3">
                    {onBack && (
                        <Button variant="ghost" size="icon" onClick={(e) => { e.stopPropagation(); onBack(); }} className="md:hidden -ml-2 rounded-full">
                            <ArrowLeft className="h-5 w-5" />
                        </Button>
                    )}
                    {/* Only the avatar and name open the info sheet; the rest of the header does nothing. */}
                    <div
                        className="group flex items-center gap-3 cursor-pointer rounded-xl px-2 py-1 hover:bg-muted/40 transition-colors"
                        onClick={() => setIsMediaSheetOpen(true)}
                    >
                    <div className="relative">
                        <Avatar className="h-10 w-10 border-2 border-background shadow-sm group-hover:scale-105 transition-transform">
                            <AvatarImage src={getAvatarUrl(chat)} />
                            <AvatarFallback className="bg-primary/10 text-primary font-bold">
                                {(chat.name || "?").charAt(0).toUpperCase()}
                            </AvatarFallback>
                        </Avatar>
                    </div>
                    <div className="flex flex-col">
                        <h3 className="font-bold text-base leading-tight tracking-tight group-hover:text-primary transition-colors flex items-center gap-1.5">
                            {chat.name || chat.id}
                            {!!chat.disappearingSecs && (
                                <span title={`Disappearing messages: ${disappearingTimerLabel(chat.disappearingSecs)}`} className="text-muted-foreground flex items-center">
                                    <Hourglass className="h-3.5 w-3.5" />
                                </span>
                            )}
                        </h3>
                        <p className={cn("text-[11px] font-medium truncate max-w-[180px] md:max-w-[250px]", headerStatus && chat.isGroup === false ? "text-primary" : "text-muted-foreground")}>
                            {headerStatus || chat.id}
                        </p>
                    </div>
                    </div>
                </div>

                <div className="flex items-center gap-1">
                    {chat.isGroup ? (
                        <>
                            <Button 
                                variant="ghost" 
                                size="icon" 
                                title="Group audio call"
                                disabled={!!activeCall}
                                onClick={(e) => { e.stopPropagation(); startGroupCall(chat.id, [], "group_audio").catch(() => toast.error("Failed to start group call")); }}
                                className="rounded-full text-muted-foreground hover:text-primary transition-all active:scale-90"
                            >
                                <Phone className="h-5 w-5" />
                            </Button>
                            <Button 
                                variant="ghost" 
                                size="icon" 
                                title="Group video call"
                                disabled={!!activeCall}
                                onClick={(e) => { e.stopPropagation(); startGroupCall(chat.id, [], "group_video").catch(() => toast.error("Failed to start group call")); }}
                                className="rounded-full text-muted-foreground hover:text-primary transition-all active:scale-90"
                            >
                                <Video className="h-5 w-5" />
                            </Button>
                        </>
                    ) : (
                        <>
                            <Button 
                                variant="ghost" 
                                size="icon" 
                                title="Audio call"
                                disabled={!!activeCall}
                                onClick={(e) => { e.stopPropagation(); startCall(chat.id, "audio").catch(() => toast.error("Failed to start call")); }}
                                className="rounded-full text-muted-foreground hover:text-primary transition-all active:scale-90"
                            >
                                <Phone className="h-5 w-5" />
                            </Button>
                            <Button 
                                variant="ghost" 
                                size="icon" 
                                title="Video call"
                                disabled={!!activeCall}
                                onClick={(e) => { e.stopPropagation(); startCall(chat.id, "video").catch(() => toast.error("Failed to start call")); }}
                                className="rounded-full text-muted-foreground hover:text-primary transition-all active:scale-90"
                            >
                                <Video className="h-5 w-5" />
                            </Button>
                        </>
                    )}
                    <Button 
                        variant="ghost" 
                        size="icon" 
                        onClick={(e) => { e.stopPropagation(); setIsSearchOpen(true); }}
                        className="rounded-full text-muted-foreground hover:text-primary transition-all active:scale-90"
                    >
                        <Search className="h-5 w-5" />
                    </Button>
                    <Button 
                        variant="ghost" 
                        size="icon" 
                        className="rounded-full text-muted-foreground group-hover:text-primary transition-all"
                    >
                        <MoreVertical className="h-5 w-5" />
                    </Button>
                </div>
            </header>

            <div className="flex-1 relative min-h-0 flex flex-col">
                <PinnedBanner
                    pins={activePins}
                    fallbackName={chat.isGroup ? undefined : chat.name}
                    onJump={jumpToMessage}
                    onUnpin={(pin) => handlePin(pin.message!, null)}
                />
                <div data-messages-list className="flex-1 overflow-y-auto px-2 md:px-6 pt-3 pb-6 z-10" ref={scrollRef} onScroll={handleScroll}>
                    {loadingMore && (
                        <div className="flex justify-center py-4">
                            <div className="w-6 h-6 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
                        </div>
                    )}
                    {initialLoad && loading ? (
                        <div className="flex flex-col items-center justify-center py-12 space-y-4 opacity-40">
                            <div className="w-10 h-10 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
                            <p className="text-sm font-medium tracking-tight">Loading secure conversation...</p>
                        </div>
                    ) : (
                        <div className="space-y-10">
                            {Object.entries(groupedMessages).map(([dateKey, msgs]) => (
                                <div key={dateKey}>
                                    <div className="flex justify-center sticky top-0 z-20 pointer-events-none py-2">
                                        <span className="w-[200px] text-center text-[10px] font-bold text-muted-foreground/80 bg-background/60 backdrop-blur-md py-1 rounded-full border border-border/40 uppercase tracking-widest shadow-sm">
                                            {dateKey}
                                        </span>
                                    </div>
                                    {msgs.map((message, idx) => {
                                        const isMe = message.from === "me"
                                        const repliedMsg = message.replyToId
                                            ? messages.find(m => m.id === message.replyToId) ?? {
                                                id: message.replyToId,
                                                content: "",
                                                from: "",
                                            }
                                            : undefined
                                        const nextMsg = msgs[idx + 1]
                                        const prevMsg = msgs[idx - 1]
                                        const isLastInSequence = !nextMsg || nextMsg.from !== message.from
                                        const isFirstInSequence = !prevMsg || prevMsg.from !== message.from

                                        return (
                                            <div key={message.id} data-message-item>
                                                <ChatMessageItem
                                                    message={message}
                                                    isMe={isMe}
                                                    isLastInSequence={isLastInSequence}
                                                    isFirstInSequence={isFirstInSequence}
                                                    chat={chat}
                                                    repliedMsg={repliedMsg}
                                                    groupMembers={groupInfo?.participants ?? []}
                                                    onNavigateToReply={teleportToMessage}
                                                    onViewedBy={() => handleViewedBy(message)}
                                                    onReply={() => handleReplyMessage(message)}
                                                    onEdit={() => handleEditMessage(message)}
                                                    onDelete={() => handleDeleteMessage(message.id)}
                                                    onStickerFavorite={(mediaUrl: string) => handleFavoriteSticker(message.id, mediaUrl || "", false)}
                                                    onImageClick={(url: string, el?: HTMLElement, msgId?: string, isSticker?: boolean) => { setSelectedImageUrl(url || null); setImageSourceRect(el?.getBoundingClientRect() || null); if (isSticker && msgId) lastStickerMsgRef.current = { id: msgId, mediaUrl: url } }}
                                                    onDownload={handleDownload}
                                                    formatTime={formatTime}
                                                    renderFormattedContent={renderFormattedContent}
                                                    getMediaUrl={getMediaUrl}
                                                    getAvatarUrl={getAvatarUrl}
                                                    showFavoriteBtn={showFavoriteBtn}
                                                    setShowFavoriteBtn={setShowFavoriteBtn}
                                                    isHighlighted={highlightedMessageId === message.id}
                                                    onReact={(emoji: string) => handleReact(message, emoji)}
                                                    onForward={() => setForwardingMessage(message)}
                                                    onVote={(options: string[]) => handleVote(message, options)}
                                                    onStar={() => handleStar(message)}
                                                    onPin={(duration: number | null) => handlePin(message, duration)}
                                                    isPinned={pinnedIds.has(message.id)}
                                                />
                                            </div>
                                        )
                                    })}
                                </div>
                            ))}
                        </div>
                    )}
                    {loadingNewer && (
                        <div className="flex justify-center py-4">
                            <div className="w-6 h-6 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
                        </div>
                    )}
                </div>

                {showScrollBottom && (
                    <button
                        type="button"
                        onClick={handleScrollToBottom}
                        title="Scroll ke paling baru"
                        aria-label="Scroll to newest messages"
                        className="absolute right-5 bottom-4 z-20 w-10 h-10 rounded-full bg-background/90 hover:bg-background text-muted-foreground hover:text-foreground shadow-md hover:shadow-lg border border-border/60 flex items-center justify-center transition-all duration-200 active:scale-95 cursor-pointer backdrop-blur-sm animate-in fade-in zoom-in-75"
                    >
                        <ChevronDown className="h-5 w-5" />
                    </button>
                )}
            </div>

            {(() => {
                const typists = chat ? typistsOf(typingByChat, chat.id, () => undefined) : []
                if (typists.length === 0 || !chat) return null
                return (
                    <div className="px-6 pb-2 pt-1 flex items-end gap-2 shrink-0">
                        {chat.isGroup && (
                            <Avatar className="h-7 w-7">
                                <AvatarImage src={getAvatarUrl(typists[0].sender)} />
                                <AvatarFallback>{typists[0].name.slice(0, 1).toUpperCase()}</AvatarFallback>
                            </Avatar>
                        )}
                        <div className="flex items-center gap-1.5 rounded-2xl bg-card px-3 py-3 shadow-sm border border-border/40" title={`${typists[0].name} is ${typists[0].audio ? "recording audio" : "typing"}`}>
                            {[0, 1, 2].map(i => (
                                <span key={i} className="h-1.5 w-1.5 rounded-full bg-primary animate-pulse" style={{ animationDelay: `${i * 180}ms` }} />
                            ))}
                        </div>
                    </div>
                )
            })()}

            <footer className="p-4 bg-background/80 backdrop-blur-xl border-t border-border/40 z-10">
                {(replyTo || editingMessage) && (
                    <div className="max-w-4xl mx-auto mb-2">
                        {replyTo && (
                            <div className="flex items-center justify-between p-2 bg-muted/50 border-l-4 border-primary rounded-r-lg">
                                <div className="flex-1 min-w-0">
                                    <p className="text-xs font-bold text-primary">Membalas {replyTo.senderName || "Pesan"}</p>
                                    <p className="text-sm truncate opacity-70">{replyTo.content}</p>
                                </div>
                                <Button size="icon" variant="ghost" className="h-7 w-7" onClick={() => setReplyTo(null)}><X className="h-4 w-4" /></Button>
                            </div>
                        )}
                        {editingMessage && (
                            <div className="flex items-center justify-between p-2 bg-muted/50 border-l-4 border-orange-500 rounded-r-lg">
                                <div className="flex-1 min-w-0">
                                    <p className="text-xs font-bold text-orange-500">Edit Pesan</p>
                                    <p className="text-sm truncate opacity-70">{editingMessage.content}</p>
                                </div>
                                <Button size="icon" variant="ghost" className="h-7 w-7" onClick={() => { setEditingMessage(null); setInputMessage(""); }}><X className="h-4 w-4" /></Button>
                            </div>
                        )}
                    </div>
                )}
{composerPreview && !isRecording && !pendingAudio && (
                    <div className="max-w-4xl mx-auto mb-2">
                        <ComposerLinkPreview
                            preview={composerPreview}
                            onDismiss={() => draftUrl && setDismissedLinks(prev => [...prev, draftUrl])}
                        />
                    </div>
                )}
<div className="max-w-4xl mx-auto flex items-end gap-3 px-2">
                    <div className="flex items-center mb-1 relative">
                        <div className="absolute invisible">
                            <ChatEmojiPickerPopover onEmojiSelect={addEmoji} open={emojiOpen} onOpenChange={setEmojiOpen} />
                            <ChatStickerPickerPopover onStickerSelect={handleStickerSelect} open={stickerOpen} onOpenChange={setStickerOpen} />
                        </div>
                        <Popover open={plusOpen} onOpenChange={setPlusOpen}>
                            <PopoverTrigger asChild>
                                <Button
                                    variant="ghost"
                                    size="icon"
                                    className={cn(
                                        "rounded-full transition-all",
                                        plusOpen ? "text-primary bg-primary/10" : "text-muted-foreground hover:bg-muted"
                                    )}
                                >
                                    <Plus className="h-5 w-5" />
                                </Button>
                            </PopoverTrigger>
                            <PopoverContent side="top" align="start" className="w-44 p-1.5 rounded-xl">
                                <div className="flex flex-col gap-0.5">
                                    <button onClick={() => { setPlusOpen(false); setTimeout(() => setEmojiOpen(true), 0) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <Smile className="h-4 w-4 text-yellow-500" />
                                        <span>Emoji</span>
                                    </button>
                                    <button onClick={() => { setPlusOpen(false); setTimeout(() => setStickerOpen(true), 0) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <Sticker className="h-4 w-4 text-purple-500" />
                                        <span>Sticker</span>
                                    </button>
                                    <button onClick={() => { mediaInputRef.current?.click(); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <Paperclip className="h-4 w-4 text-blue-500" />
                                        <span>Media</span>
                                    </button>
                                    <button onClick={() => { documentInputRef.current?.click(); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <FileText className="h-4 w-4 text-orange-500" />
                                        <span>Document</span>
                                    </button>
                                    <button onClick={() => { audioInputRef.current?.click(); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <Mic className="h-4 w-4 text-red-500" />
                                        <span>Audio</span>
                                    </button>
                                    <button onClick={() => { gifInputRef.current?.click(); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <ImagePlay className="h-4 w-4 text-pink-500" />
                                        <span>GIF</span>
                                    </button>
                                    <button onClick={() => { setPollOpen(true); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <BarChart3 className="h-4 w-4 text-cyan-500" />
                                        <span>Poll</span>
                                    </button>
                                    <button onClick={() => { setLocationOpen(true); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <MapPin className="h-4 w-4 text-emerald-500" />
                                        <span>Location</span>
                                    </button>
                                    <button onClick={() => { setContactOpen(true); setPlusOpen(false) }} className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full">
                                        <User className="h-4 w-4 text-indigo-500" />
                                        <span>Contact</span>
                                    </button>
                                    <button onClick={() => { setIsMdMode(v => !v); setPlusOpen(false) }} className={cn("flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full", isMdMode && "text-primary")}>
                                        <FileText className="h-4 w-4" />
                                        <span>Markdown {isMdMode ? '(on)' : '(off)'}</span>
                                    </button>
                                </div>
                            </PopoverContent>
                        </Popover>
                    </div>
                    {isRecording ? (
                        <div className="flex-1 flex items-center gap-3 bg-muted/50 rounded-2xl px-4 min-h-[44px]">
                            <span className="relative flex h-3 w-3 flex-shrink-0">
                                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-500 opacity-75" />
                                <span className="relative inline-flex rounded-full h-3 w-3 bg-red-500" />
                            </span>
                            <span className="text-sm font-bold tabular-nums tracking-tight">{formatRecordingTime(recordingSeconds)}</span>
                            <RecordingWaveform stream={recordingStream} />
                            <div className="flex items-center gap-1.5 flex-shrink-0">
                                <Button size="icon" variant="ghost" className="h-9 w-9 rounded-full text-muted-foreground hover:text-destructive hover:bg-destructive/10" onClick={cancelRecording} title="Cancel recording">
                                    <Trash2 className="h-4 w-4" />
                                </Button>
                                <Button size="icon" className="h-9 w-9 rounded-full bg-destructive text-destructive-foreground hover:bg-destructive/90 active:scale-90 transition-all" onClick={stopRecording} title="Stop and review">
                                    <Square className="h-4 w-4 fill-current" />
                                </Button>
                            </div>
                        </div>
                    ) : pendingAudio ? (
                        <div className="flex-1 flex items-center gap-3 bg-muted/50 rounded-2xl px-4 min-h-[44px]">
                            <WAAudioPlayer
                                src={pendingAudio.url}
                                isMe
                                type="ptt"
                                messageId={pendingAudio.url}
                                durationHint={pendingAudio.seconds}
                                className="mx-0 mt-0 p-0 py-1.5 min-w-0 sm:min-w-0 flex-1"
                            />
                            <div className="flex items-center gap-1.5 flex-shrink-0">
                                <Button size="icon" variant="ghost" className="h-9 w-9 rounded-full text-muted-foreground hover:text-destructive hover:bg-destructive/10" onClick={cancelPendingAudio} title="Discard recording">
                                    <Trash2 className="h-4 w-4" />
                                </Button>
                                <Button size="icon" className="h-9 w-9 rounded-xl bg-primary text-primary-foreground hover:bg-primary/90 active:scale-90 transition-all" onClick={sendPendingAudio} disabled={sending} title="Send voice message">
                                    <SendHorizontal className="h-4 w-4" />
                                </Button>
                            </div>
                        </div>
                    ) : (
                        <>
                            <Button
                                variant="ghost"
                                size="icon"
                                onClick={startRecording}
                                disabled={sending}
                                className="rounded-full text-muted-foreground hover:text-primary hover:bg-muted transition-all active:scale-90 mb-1"
                                title="Record voice message"
                            >
                                <Mic className="h-5 w-5" />
                            </Button>
                            <div className="flex-1 relative">
                        {mentionMatches.length > 0 && (
                            <div className="absolute bottom-full left-0 z-40 mb-2 max-h-64 w-full overflow-y-auto rounded-xl border border-border bg-popover p-1.5 shadow-xl">
                                {mentionMatches.map((member, index) => (
                                    <button
                                        key={member.jid}
                                        type="button"
                                        ref={index === activeMention ? (el) => el?.scrollIntoView({ block: "nearest" }) : undefined}
                                        onMouseDown={event => event.preventDefault()}
                                        onMouseEnter={() => setMentionIndex(index)}
                                        onClick={() => insertMention(member)}
                                        className={cn("flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left hover:bg-muted", index === activeMention && "bg-muted")}
                                    >
                                        <Avatar className="h-8 w-8"><AvatarImage src={getAvatarUrl(member.jid)} /><AvatarFallback>{participantLabel(member).slice(0, 1).toUpperCase()}</AvatarFallback></Avatar>
                                        <span className="min-w-0 flex-1 truncate text-sm">{participantLabel(member)}</span>
                                        {member.isAdmin && <span className="text-[10px] text-muted-foreground">admin</span>}
                                    </button>
                                ))}
                            </div>
                        )}
                        <Textarea
                            ref={inputRef}
                            placeholder={editingMessage ? "Edit message..." : replyTo ? "Reply to message..." : isMdMode ? "Write markdown..." : "Type a message..."}
                            value={inputMessage}
                            onChange={e => {
                                setInputMessage(e.target.value)
                                updateMentionTrigger(e.target.value, e.currentTarget.selectionStart ?? e.target.value.length)
                            }}
                            onKeyDown={e => {
                                if (mentionMatches.length > 0) {
                                    const count = mentionMatches.length
                                    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                                        e.preventDefault()
                                        setMentionIndex((activeMention + (e.key === "ArrowDown" ? 1 : count - 1)) % count)
                                        return
                                    }
                                    if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") {
                                        e.preventDefault()
                                        insertMention(mentionMatches[activeMention])
                                        return
                                    }
                                    if (e.key === "Escape") {
                                        e.preventDefault()
                                        setMentionTrigger(null)
                                        return
                                    }
                                }
                                const send = isMdMode
                                    ? e.key === "Enter" && (e.ctrlKey || e.metaKey)
                                    : e.key === "Enter" && !e.shiftKey
                                if (send) {
                                    e.preventDefault()
                                    handleSendMessage()
                                } else if (e.key === "Escape") {
                                    e.preventDefault()
                                    setEditingMessage(null)
                                    setReplyTo(null)
                                    setInputMessage("")
                                }
                            }}
                            className="pr-12 min-h-[44px] max-h-[150px] resize-none overflow-y-hidden bg-muted/50 border-none focus-visible:ring-1 focus-visible:ring-primary/20 rounded-2xl py-3 text-sm"
                            disabled={sending}
                            rows={1}
                        />
                        <Button size="icon" onClick={handleSendMessage} disabled={!inputMessage.trim() || sending} className={cn("absolute right-1 top-1/2 -translate-y-1/2 h-8 w-8 rounded-xl transition-all duration-300", inputMessage.trim() ? "bg-primary text-primary-foreground scale-100 shadow-md" : "bg-transparent text-muted-foreground scale-90")}>
                            <Send className="h-4 w-4" />
                        </Button>
                    </div>
                        </>
                    )}
                </div>
            </footer>

            <input
                type="file"
                ref={mediaInputRef}
                onChange={e => handleFileUpload(e, "image")}
                accept="image/*,video/*,audio/*"
                className="hidden"
            />
            <input
                type="file"
                ref={documentInputRef}
                onChange={e => handleFileUpload(e, "document")}
                accept=".pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.txt"
                className="hidden"
            />
            <input
                type="file"
                ref={audioInputRef}
                onChange={e => handleFileUpload(e, "audio")}
                accept="audio/*,.mp3,.ogg,.m4a,.wav,.opus,.webm"
                className="hidden"
            />
            <input
                type="file"
                ref={gifInputRef}
                onChange={handleGifSelected}
                accept="image/gif,video/mp4"
                className="hidden"
            />
            {chat && (
                <>
                    <PollDialog open={pollOpen} onOpenChange={setPollOpen} chatId={chat.id} />
                    <LocationDialog open={locationOpen} onOpenChange={setLocationOpen} chatId={chat.id} />
                    <ContactDialog open={contactOpen} onOpenChange={setContactOpen} chatId={chat.id} />
                    <ForwardDialog
                        open={forwardingMessage !== null}
                        onOpenChange={(v) => { if (!v) setForwardingMessage(null) }}
                        message={forwardingMessage}
                        chats={useChatStore.getState().chats}
                        currentChatId={chat.id}
                    />
                </>
            )}

            <ChatInfoSheetModal
                open={isMediaSheetOpen}
                onOpenChange={setIsMediaSheetOpen}
                chat={chat}
                getAvatarUrl={getAvatarUrl}
                getMediaUrl={getMediaUrl}
                formatDate={formatDate}
                onSelectImage={setSelectedImageUrl}
            />

            <ChatImageViewerModal
                open={!!selectedImageUrl}
                onOpenChange={(open) => !open && setSelectedImageUrl(null)}
                imageUrl={selectedImageUrl}
                sourceRect={imageSourceRect}
                onClose={() => { setSelectedImageUrl(null); setImageSourceRect(null) }}
                onFavorite={(url) => handleFavoriteSticker(lastStickerMsgRef.current.id, url, false)}
            />

            <ChatSearchSheet
                chatId={chat.id}
                isOpen={isSearchOpen}
                onClose={() => setIsSearchOpen(false)}
                onResultClick={teleportToMessage}
            />
            <MessageInfoSheet
                message={viewedByMessage}
                receipts={viewedByReceipts}
                recipients={viewedByRecipients}
                loading={loadingViewedBy}
                group={groupInfo}
                getAvatarUrl={getAvatarUrl}
                onClose={() => setViewedByMessage(null)}
            />
        </div>
    )
})
