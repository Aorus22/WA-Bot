import { useState, useEffect, useRef, useCallback, memo, useMemo } from "react"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Send, ArrowLeft, MessageSquare, X, MoreVertical, Search, FileText, Plus, Smile, Paperclip, Sticker } from "lucide-react"
import { api, type Chat, type Message } from "@/lib/api"
import { cn } from "@/lib/utils"
import { renderFormattedContent, encodeMarkdown } from "./renderMd"
import { toast } from "sonner"
import { ChatInfoSheetModal } from "./ChatInfoSheetModal"
import { ChatImageViewerModal } from "./ChatImageViewerModal"
import { ChatEmojiPickerPopover } from "./ChatEmojiPickerPopover"
import { ChatStickerPickerPopover } from "./ChatStickerPickerPopover"
import { ChatSearchSheet } from "./ChatSearchSheet"
import { ChatMessageItem } from "./ChatMessageItem"

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

const getApiBase = () => {
    const envUrl = import.meta.env.VITE_API_URL
    if (envUrl) return envUrl
    return typeof window !== "undefined" ? `${window.location.origin}/api` : ""
}

const getMediaUrl = (url: string | undefined): string | undefined => {
    if (!url) return undefined
    if (url.startsWith("http://") || url.startsWith("https://")) return url

    const apiBase = getApiBase()
    const parts = url.split("/")
    if (parts.length > 1) {
        const filename = parts.pop()!
        const path = parts.join("/")
        const encodedFilename = encodeURIComponent(filename)
        const fullUrl = `${apiBase}${path}/${encodedFilename}`
        return fullUrl
    }

    return `${apiBase}${url}`
}

const getAvatarUrl = (target: Chat | string): string | undefined => {
    const apiBase = getApiBase()

    if (typeof target !== "string") {
        if (target.avatar && target.avatar.length > 0 && !target.avatar.startsWith("data:")) {
            return target.avatar
        }
        return `${apiBase}/avatar/${encodeURIComponent(target.id)}`
    }

    return `${apiBase}/avatar/${encodeURIComponent(target)}`
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

    export const ChatArea = memo(({
 
    chat, 
    incomingMessage, 
    statusUpdate, 
    onBack, 
    className,
    cachedMessages,
    cachedHasMore,
    onCacheUpdate
}: ChatAreaProps) => {
    const [messages, setMessages] = useState<Message[]>([])
    const [loadingMore, setLoadingMore] = useState(false)
    const [loadingNewer, setLoadingNewer] = useState(false)
    const [hasMore, setHasMore] = useState(true)
    const [hasMoreNext, setHasMoreNext] = useState(false)
    const [inputMessage, setInputMessage] = useState("")
    const [loading, setLoading] = useState(false)
    const [sending, setSending] = useState(false)
    const [initialLoad, setInitialLoad] = useState(true)
    const [showFavoriteBtn, setShowFavoriteBtn] = useState<string | null>(null)
    const [replyTo, setReplyTo] = useState<Message | null>(null)
    const [editingMessage, setEditingMessage] = useState<Message | null>(null)
	const [isMdMode, setIsMdMode] = useState(false)
	const [plusOpen, setPlusOpen] = useState(false)
	const [emojiOpen, setEmojiOpen] = useState(false)
	const [stickerOpen, setStickerOpen] = useState(false)
    const [isMediaSheetOpen, setIsMediaSheetOpen] = useState(false)
    const [isSearchOpen, setIsSearchOpen] = useState(false)
    const [highlightedMessageId, setHighlightedMessageId] = useState<string | null>(null)
    const [selectedImageUrl, setSelectedImageUrl] = useState<string | null>(null)
    const [imageSourceRect, setImageSourceRect] = useState<DOMRect | null>(null)
    const lastStickerMsgRef = useRef<{ id: string; mediaUrl: string }>({ id: "", mediaUrl: "" })

    const scrollRef = useRef<HTMLDivElement>(null)
    const mediaInputRef = useRef<HTMLInputElement>(null)
    const documentInputRef = useRef<HTMLInputElement>(null)
    const inputRef = useRef<HTMLInputElement>(null)

    const scrollToBottom = useCallback((behavior: ScrollBehavior = "smooth") => {
        if (scrollRef.current) {
            const { scrollHeight, clientHeight } = scrollRef.current
            scrollRef.current.scrollTo({
                top: scrollHeight - clientHeight,
                behavior
            })
        }
    }, [])

    // Sync local state back to cache
    useEffect(() => {
        if (chat && onCacheUpdate && messages.length > 0) {
            onCacheUpdate(messages, hasMore)
        }
    }, [messages, hasMore, chat?.id, onCacheUpdate])

    useEffect(() => {
        if (chat) {
            if (cachedMessages && cachedMessages.length > 0) {
                setMessages(cachedMessages)
                setHasMore(cachedHasMore ?? true)
                setInitialLoad(false)
                // Optional: scrollToBottom("auto") if switching back
                setTimeout(() => scrollToBottom("auto"), 50)
            } else {
                setHasMore(true)
                loadMessages()
            }
        } else {
            setMessages([])
            setInitialLoad(true)
        }
    }, [chat?.id])

    useEffect(() => {
        if (incomingMessage && chat && incomingMessage.chatId === chat.id) {
            const { message } = incomingMessage
            if ((message as any).type === "deleted") {
                setMessages(prev => prev.filter(m => m.id !== message.id))
            } else if ((message as any).type === "edited") {
                setMessages(prev => prev.map(m => m.id === message.id ? { ...m, content: (message as any).content } : m))
            } else {
                setMessages(prev => {
                    if (prev.some(m => m.id === incomingMessage.message.id)) return prev

                    if (incomingMessage.message.from === "me") {
                        const pendingIndex = prev.findIndex(m =>
                            m.status === "pending" &&
                            m.id.startsWith("temp-") &&
                            (
                                m.content === incomingMessage.message.content ||
                                (m.type === incomingMessage.message.type && ["image", "video", "sticker", "document"].includes(m.type))
                            )
                        )

                        if (pendingIndex !== -1) {
                            const next = [...prev]
                            next[pendingIndex] = incomingMessage.message
                            return next
                        }
                    }

                    return [...prev, incomingMessage.message]
                })
            }
            setTimeout(() => scrollToBottom(), 100)
        }
    }, [incomingMessage, chat?.id, scrollToBottom])
    useEffect(() => {
        if (statusUpdate) {
            setMessages(prev =>
                prev.map(m =>
                    m.id === statusUpdate.id ? { ...m, status: statusUpdate.status } : m
                )
            )
        }
    }, [statusUpdate])

    useEffect(() => {
        if (replyTo || editingMessage) {
            inputRef.current?.focus()
        }
    }, [replyTo, editingMessage])

    const loadMessages = async () => {
        if (!chat) return
        try {
            setLoading(true)
            const data = await api.getMessages(chat.id, 30)
            setMessages(data || [])
            setHasMore((data || []).length === 30)
            setHasMoreNext(false)
            setInitialLoad(false)
            setTimeout(() => scrollToBottom("auto"), 50)
        } catch (error) {
            console.error("Failed to load messages:", error)
            toast.error("Failed to load conversation")
        } finally {
            setLoading(false)
        }
    }

    const loadMoreMessages = async () => {
        if (!chat || loadingMore || !hasMore || messages.length === 0) return
        
        const oldestMsg = messages[0]
        try {
            setLoadingMore(true)
            const data = await api.getMessages(chat.id, 30, oldestMsg.timestamp)
            
            if (data && data.length > 0) {
                // Prepend older messages
                setMessages(prev => [...data, ...prev])
                setHasMore(data.length === 30)
                
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
                setHasMore(false)
            }
        } catch (error) {
            console.error("Failed to load more messages:", error)
        } finally {
            setLoadingMore(false)
        }
    }

    const loadNewerMessages = async () => {
        if (!chat || loadingNewer || !hasMoreNext || messages.length === 0) return

        const latestMsg = messages[messages.length - 1]
        try {
            setLoadingNewer(true)
            const data = await api.getMessages(chat.id, 30, undefined, latestMsg.timestamp)

            if (data && data.length > 0) {
                setMessages(prev => [...prev, ...data])
                setHasMoreNext(data.length === 30)
            } else {
                setHasMoreNext(false)
            }
        } catch (error) {
            console.error("Failed to load newer messages:", error)
        } finally {
            setLoadingNewer(false)
        }
    }

    const teleportToMessage = async (messageId: string) => {
        if (!chat) return
        try {
            setLoading(true)
            setIsSearchOpen(false)
            const data = await api.getMessageContext(chat.id, messageId, 30)
            
            if (data && data.length > 0) {
                setMessages(data)
                
                // If we got 30 messages, we might have more in both directions
                // Since GetMessageContext fetches half before and half after (15 before, 15 after)
                // we check if those lists are full
                const targetIndex = data.findIndex(m => m.id === messageId)
                setHasMore(targetIndex === 0 && data.length >= 15) // Simplified check
                setHasMoreNext(data.length - 1 === targetIndex && data.length >= 15) // Simplified
                
                // Better heuristic: assume there's more unless it's a small result
                setHasMore(true)
                setHasMoreNext(true)

                setInitialLoad(false)
                
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
        } finally {
            setLoading(false)
        }
    }

    const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
        const target = e.currentTarget
        const isNearTop = target.scrollTop < 100
        const isNearBottom = target.scrollHeight - target.scrollTop - target.clientHeight < 100

        if (isNearTop && !loadingMore && hasMore) {
            loadMoreMessages()
        }
        if (isNearBottom && !loadingNewer && hasMoreNext) {
            loadNewerMessages()
        }
    }

    const handleSendMessage = async () => {
        if (!inputMessage.trim() || !chat || sending) return
        const raw = inputMessage.trim()
        const text = isMdMode ? encodeMarkdown(raw) : raw
        setInputMessage("")
        setSending(true)

        try {
            if (editingMessage) {
                await api.editMessage(chat.id, editingMessage.id, text)
                setEditingMessage(null)
            } else if (replyTo) {
                await api.replyMessage(chat.id, replyTo.id, text)
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

                setMessages(prev => [...prev, newMsg])
                setTimeout(() => scrollToBottom(), 50)

                const res = await api.sendMessage(chat.id, text)
                setMessages(prev => {
                    if (prev.some(m => m.id === res.id)) {
                        return prev.filter(m => m.id !== tempId)
                    }
                    return prev.map(m => (m.id === tempId ? { ...m, id: res.id, status: "sent" } : m))
                })
            }
        } catch (error) {
            console.error("Failed to send message:", error)
            toast.error("Gagal memproses pesan")
            setInputMessage(text)
        } finally {
            setSending(false)
        }
    }

    const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>, type: "image" | "video" | "document") => {
        const file = e.target.files?.[0]
        if (!file || !chat) return

        const tempId = "temp-media-" + Date.now()
        const newMsg: Message = {
            id: tempId,
            chatId: chat.id,
            from: "me",
            to: chat.id,
            content: type === "image" ? "[Image]" : type === "video" ? "[Video]" : file.name,
            timestamp: Date.now(),
            status: "pending",
            type: type,
            mediaUrl: URL.createObjectURL(file)
        }

        setMessages(prev => [...prev, newMsg])
        setTimeout(() => scrollToBottom(), 50)

        try {
            const res = await api.sendMedia(chat.id, file, type, "")
            setMessages(prev => {
                if (prev.some(m => m.id === res.id)) {
                    return prev.filter(m => m.id !== tempId)
                }
                return prev.map(m => (m.id === tempId ? { ...m, id: res.id, status: "sent" } : m))
            })
        } catch (error) {
            console.error("Failed to send media:", error)
            setMessages(prev =>
                prev.map(m => (m.id === tempId ? { ...m, status: "failed" } : m))
            )
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

        setMessages(prev => [...prev, newMsg])
        setTimeout(() => scrollToBottom(), 50)

        try {
            const res = await api.sendSticker(chat.id, sticker.mediaUrl, sticker.isAnimated)
            setMessages(prev => {
                if (prev.some(m => m.id === res.id)) {
                    return prev.filter(m => m.id !== tempId)
                }
                return prev.map(m => (m.id === tempId ? { ...m, id: res.id, status: "sent" } : m))
            })
        } catch (error) {
            console.error("Failed to send sticker:", error)
            setMessages(prev =>
                prev.map(m => (m.id === tempId ? { ...m, status: "failed" } : m))
            )
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
            setMessages(prev => prev.filter(m => m.id !== messageId))
        } catch (err) {
            toast.error("Failed to delete message")
        }
    }

    const handleEditMessage = (message: Message) => {
        setEditingMessage(message)
        setInputMessage(message.content)
        setReplyTo(null)
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
            <header
                className="h-16 flex items-center justify-between px-4 border-b border-border/40 bg-background/80 backdrop-blur-xl z-20 sticky top-0 cursor-pointer hover:bg-muted/30 transition-colors group"
                onClick={() => setIsMediaSheetOpen(true)}
            >
                <div className="flex items-center gap-3">
                    {onBack && (
                        <Button variant="ghost" size="icon" onClick={(e) => { e.stopPropagation(); onBack(); }} className="md:hidden -ml-2 rounded-full">
                            <ArrowLeft className="h-5 w-5" />
                        </Button>
                    )}
                    <div className="relative">
                        <Avatar className="h-10 w-10 border-2 border-background shadow-sm group-hover:scale-105 transition-transform">
                            <AvatarImage src={getAvatarUrl(chat)} />
                            <AvatarFallback className="bg-primary/10 text-primary font-bold">
                                {chat.name.charAt(0).toUpperCase()}
                            </AvatarFallback>
                        </Avatar>
                    </div>
                    <div className="flex flex-col">
                        <h3 className="font-bold text-base leading-tight tracking-tight group-hover:text-primary transition-colors">{chat.name}</h3>
                        <p className="text-[11px] font-medium text-muted-foreground truncate max-w-[180px] md:max-w-[250px]">
                            {chat.id}
                        </p>
                    </div>
                </div>

                <div className="flex items-center gap-1">
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
                                    <span className="text-[10px] font-bold text-muted-foreground/80 bg-background/60 backdrop-blur-md px-3 py-1 rounded-full border border-border/40 uppercase tracking-widest shadow-sm">
                                        {dateKey}
                                    </span>
                                </div>
                                {msgs.map((message, idx) => {
                                    const isMe = message.from === "me"
                                    const repliedMsg = messages.find(m => m.id === message.replyToId)
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
                                    <button onClick={() => { setIsMdMode(v => !v); setPlusOpen(false) }} className={cn("flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm hover:bg-muted transition-colors text-left w-full", isMdMode && "text-primary")}>
                                        <FileText className="h-4 w-4" />
                                        <span>Markdown {isMdMode ? '(on)' : '(off)'}</span>
                                    </button>
                                </div>
                            </PopoverContent>
                        </Popover>
                    </div>
                    <div className="flex-1 relative">
                        {isMdMode ? (
                            <Textarea
                                ref={inputRef as React.Ref<HTMLTextAreaElement>}
                                placeholder={editingMessage ? "Edit message..." : replyTo ? "Reply to message..." : "Write markdown..."}
                                value={inputMessage}
                                onChange={e => setInputMessage(e.target.value)}
                                onKeyDown={e => {
                                    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                                        e.preventDefault()
                                        handleSendMessage()
                                    } else if (e.key === "Escape") {
                                        e.preventDefault()
                                        setEditingMessage(null)
                                        setReplyTo(null)
                                        setInputMessage("")
                                    }
                                }}
                                className="pr-12 min-h-[44px] max-h-[200px] resize-none bg-muted/50 border-none focus-visible:ring-1 focus-visible:ring-primary/20 rounded-2xl py-3 text-sm"
                                disabled={sending}
                                rows={3}
                            />
                        ) : (
                            <Input
                                ref={inputRef}
                                placeholder={editingMessage ? "Edit message..." : replyTo ? "Reply to message..." : "Type a message..."}
                                value={inputMessage}
                                onChange={e => setInputMessage(e.target.value)}
                                onKeyDown={e => {
                                    if (e.key === "Enter" && !e.shiftKey) {
                                        e.preventDefault()
                                        handleSendMessage()
                                    } else if (e.key === "Escape") {
                                        e.preventDefault()
                                        setEditingMessage(null)
                                        setReplyTo(null)
                                        setInputMessage("")
                                    }
                                }}
                                className="pr-12 min-h-[44px] bg-muted/50 border-none focus-visible:ring-1 focus-visible:ring-primary/20 rounded-2xl py-3"
                                disabled={sending}
                            />
                        )}
                        <Button size="icon" onClick={handleSendMessage} disabled={!inputMessage.trim() || sending} className={cn("absolute right-1 top-1/2 -translate-y-1/2 h-8 w-8 rounded-xl transition-all duration-300", inputMessage.trim() ? "bg-primary text-primary-foreground scale-100 shadow-md" : "bg-transparent text-muted-foreground scale-90")}>
                            <Send className="h-4 w-4" />
                        </Button>
                    </div>
                </div>
            </footer>

            <input
                type="file"
                ref={mediaInputRef}
                onChange={e => handleFileUpload(e, "image")}
                accept="image/*,video/*"
                className="hidden"
            />
            <input
                type="file"
                ref={documentInputRef}
                onChange={e => handleFileUpload(e, "document")}
                accept=".pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.txt"
                className="hidden"
            />

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
        </div>
    )
})
