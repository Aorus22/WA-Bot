import { useState, useEffect, useRef, useCallback } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Send, FileText, Star, Trash2, ArrowLeft, MessageSquare, Reply, Edit3, X, MoreVertical } from "lucide-react"
import { api, type Chat, type Message } from "@/lib/api"
import { cn } from "@/lib/utils"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { toast } from "sonner"
import { ChatInfoSheetModal } from "./ChatInfoSheetModal"
import { ChatImageViewerModal } from "./ChatImageViewerModal"
import { ChatEmojiPickerPopover } from "./ChatEmojiPickerPopover"
import { ChatStickerPickerPopover } from "./ChatStickerPickerPopover"
import { ChatAttachmentPopover } from "./ChatAttachmentPopover"

interface ChatAreaProps {
    chat: Chat | null
    incomingMessage?: { chatId: string; message: Message } | null
    statusUpdate?: { id: string; status: string } | null
    onBack?: () => void
    className?: string
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

const formatDate = (timestamp: number) => {
    const date = new Date(timestamp)
    const today = new Date()
    const yesterday = new Date()
    yesterday.setDate(today.getDate() - 1)

    if (date.toDateString() === today.toDateString()) return "Today"
    if (date.toDateString() === yesterday.toDateString()) return "Yesterday"

    return date.toLocaleDateString([], { day: "numeric", month: "long", year: "numeric" })
}

const renderFormattedContent = (content: string) => {
    if (!content) return null

    const parts = content.split(/(\*.*?\*|_.*?_|~.*?~|`.*?`|\n)/g)

    return parts.map((part, index) => {
        if (part.startsWith("*") && part.endsWith("*")) {
            return <strong key={index}>{part.slice(1, -1)}</strong>
        }
        if (part.startsWith("_") && part.endsWith("_")) {
            return <em key={index}>{part.slice(1, -1)}</em>
        }
        if (part.startsWith("~") && part.endsWith("~")) {
            return <del key={index}>{part.slice(1, -1)}</del>
        }
        if (part.startsWith("`") && part.endsWith("`")) {
            return <code key={index} className="bg-muted px-1 rounded">{part.slice(1, -1)}</code>
        }
        if (part === "\n") {
            return <br key={index} />
        }
        return part
    })
}

function MessageItem({
    message, isMe, isLastInSequence, isFirstInSequence, chat, repliedMsg,
    onReply, onEdit, onDelete, onStickerFavorite, onImageClick,
    formatTime, renderFormattedContent, getMediaUrl, getAvatarUrl,
    showFavoriteBtn, setShowFavoriteBtn
}: any) {
    const [swipeX, setSwipeX] = useState(0)
    const startX = useRef(0)
    const threshold = 60

    const handleTouchStart = (e: React.TouchEvent) => {
        startX.current = e.touches[0].clientX
    }

    const handleTouchMove = (e: React.TouchEvent) => {
        const currentX = e.touches[0].clientX
        const diff = currentX - startX.current
        if (diff > 0) {
            setSwipeX(Math.min(diff, threshold + 20))
        }
    }

    const handleTouchEnd = () => {
        if (swipeX >= threshold) {
            onReply()
        }
        setSwipeX(0)
    }

    const isPending = message.status === "pending"
    const isFailed = message.status === "failed"
    const isImage = message.type === "image"
    const isVideo = message.type === "video"
    const isSticker = message.type === "sticker"
    const isDocument = message.type === "document"
    const isMedia = message.mediaUrl && message.mediaUrl.length > 0
    const showSenderInfo = !isMe && chat?.isGroup && isFirstInSequence

    return (
        <div
            id={message.id}
            className={cn(
                "flex w-full group animate-in fade-in slide-in-from-bottom-2 duration-300 relative",
                isMe ? "justify-end" : "justify-start",
                isLastInSequence ? "mb-4" : "mb-1"
            )}
        >
            <div
                className="absolute left-0 top-1/2 -translate-y-1/2 transition-opacity duration-200"
                style={{
                    opacity: swipeX / threshold,
                    transform: `translateX(${swipeX - 40}px)`
                }}
            >
                <div className="bg-primary/20 p-2 rounded-full">
                    <Reply className="h-4 w-4 text-primary" />
                </div>
            </div>

            <div
                className={cn(
                    "max-w-[85%] sm:max-w-[70%] flex relative gap-3 transition-transform duration-200 will-change-transform min-w-0",
                    isMe ? "flex-row-reverse" : "flex-row"
                )}
                style={{ transform: `translateX(${swipeX}px)` }}
                onTouchStart={handleTouchStart}
                onTouchMove={handleTouchMove}
                onTouchEnd={handleTouchEnd}
            >
                {!isMe && chat?.isGroup && (
                    <div className="w-8 flex-shrink-0 mt-0">
                        {isFirstInSequence && (
                            <Avatar className="h-8 w-8 border-2 border-background shadow-sm">
                                <AvatarImage src={getAvatarUrl(message.from)} />
                                <AvatarFallback className="text-[10px] bg-primary/10 text-primary font-bold">
                                    {message.senderName?.charAt(0).toUpperCase() || "?"}
                                </AvatarFallback>
                            </Avatar>
                        )}
                    </div>
                )}

                <div className={cn("flex flex-col", isMe ? "items-end" : "items-start")}>
                    {showSenderInfo && (
                        <span className="text-[11px] font-bold mb-1 ml-1 text-primary/80">
                            {message.senderName || message.from.split("@")[0]}
                        </span>
                    )}

                    {message.isAutomatic && (
                        <span className={cn(
                            "text-[10px] font-bold mb-1 px-1 uppercase tracking-widest flex items-center gap-1",
                            isMe ? "text-indigo-500 dark:text-indigo-400" : "text-blue-500"
                        )}>
                            <span className={cn("w-1 h-1 rounded-full", isMe ? "bg-indigo-500 dark:bg-indigo-400" : "bg-blue-500")} />
                            Bot Response
                        </span>
                    )}

                    {repliedMsg && (
                        <div
                            className="mb-2 p-2 bg-black/5 dark:bg-white/5 rounded-lg border-l-4 border-primary text-[12px] opacity-80 cursor-pointer hover:opacity-100 transition-opacity relative z-10"
                            onClick={() => document.getElementById(repliedMsg.id)?.scrollIntoView({ behavior: "smooth", block: "center" })}
                        >
                            <p className="font-bold text-primary">{repliedMsg.from === "me" ? "You" : (repliedMsg.senderName || "Unknown")}</p>
                            <p className="truncate opacity-70">{repliedMsg.content}</p>
                        </div>
                    )}

                    {isSticker ? (
                        <div
                            className="relative group/sticker cursor-pointer z-10"
                            onClick={() => setShowFavoriteBtn(showFavoriteBtn === message.id ? null : message.id)}
                        >
                            <div className="w-[160px] h-[160px] flex items-center justify-center">
                                {isMedia ? (
                                    <img src={getMediaUrl(message.mediaUrl)} alt="Sticker" className="max-w-full max-h-full object-contain" />
                                ) : (
                                    <div className="w-full h-full bg-muted rounded-xl flex items-center justify-center text-[10px] text-muted-foreground uppercase tracking-widest">
                                        Sticker
                                    </div>
                                )}
                            </div>
                            {showFavoriteBtn === message.id && (
                                <button
                                    onClick={(e) => { e.stopPropagation(); onStickerFavorite(message.mediaUrl); }}
                                    className={cn(
                                        "absolute top-0 p-1.5 bg-background/90 backdrop-blur-md rounded-full shadow-lg transition-all animate-in zoom-in-50 z-20",
                                        isMe ? "left-0" : "right-0"
                                    )}
                                >
                                    <Star className="h-4 w-4 text-yellow-500 fill-yellow-500" />
                                </button>
                            )}
                            <div className={cn("absolute bottom-1 right-1 px-1.5 py-0.5 rounded-full bg-black/30 backdrop-blur-sm text-[10px] text-white opacity-0 group-hover/sticker:opacity-100 transition-opacity", isMe ? "right-1" : "left-1")}>
                                {formatTime(message.timestamp)}
                            </div>
                        </div>
                    ) : (
                        <div
                            className={cn(
                                "px-3.5 py-2 rounded-2xl text-[14.5px] relative transition-all duration-200 shadow-[0_1px_0.5px_rgba(0,0,0,0.13)]",
                                isMe
                                    ? cn("bg-[#dcf8c6] dark:bg-[#005c4b] text-[#303030] dark:text-[#e9edef]", message.isAutomatic && "bg-[#e8eaff] dark:bg-[#2e334d]", isLastInSequence ? "rounded-tr-none" : "", isPending && "opacity-70", isFailed && "bg-destructive text-destructive-foreground")
                                    : cn("bg-white dark:bg-[#202c33] text-[#303030] dark:text-[#e9edef]", message.isAutomatic && "bg-[#eef2ff] dark:bg-[#1e2235] border-l-[3px] border-indigo-500 dark:border-indigo-400", isLastInSequence ? "rounded-tl-none" : "")
                            )}
                        >
                            {isImage && isMedia && (
                                <div className="mb-2 -mx-1 -mt-1 rounded-lg overflow-hidden border border-black/5 dark:border-white/5 relative z-10">
                                    <img
                                        src={getMediaUrl(message.mediaUrl)}
                                        alt="Image"
                                        className="w-full max-w-[320px] h-auto object-cover hover:scale-[1.02] transition-transform duration-500 cursor-zoom-in"
                                        onClick={(e) => {
                                            e.stopPropagation()
                                            onImageClick(getMediaUrl(message.mediaUrl))
                                        }}
                                    />
                                </div>
                            )}
                            {isVideo && isMedia && (
                                <div className="mb-2 -mx-1 -mt-1 rounded-lg overflow-hidden border border-black/5 dark:border-white/5 bg-black/10 flex items-center justify-center aspect-video relative z-10">
                                    <video src={getMediaUrl(message.mediaUrl)} className="w-full h-auto max-h-[300px]" controls />
                                </div>
                            )}
                            {isDocument && isMedia && (
                                <a href={getMediaUrl(message.mediaUrl)} target="_blank" rel="noopener noreferrer" className="flex items-center gap-3 mb-1 p-2 bg-black/5 dark:bg-white/5 rounded-lg border border-black/5 dark:border-white/5 relative z-10">
                                    <div className="w-9 h-9 bg-primary/10 rounded flex items-center justify-center">
                                        <FileText className="h-5 w-5 text-primary" />
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <p className="text-xs font-bold truncate">Document</p>
                                        <p className="text-[10px] opacity-60">PDF • View</p>
                                    </div>
                                </a>
                            )}
                            {message.content && !["[Image]", "[Video]", "[Sticker]"].includes(message.content) && (
                                <div className="break-words [word-break:break-word] leading-relaxed whitespace-pre-wrap relative z-10">
                                    {renderFormattedContent(message.content)}
                                </div>
                            )}
                            <div className={cn("flex items-center gap-1 mt-1 justify-end", "text-[10px] font-medium opacity-50 uppercase tracking-tight")}>
                                <span>{formatTime(message.timestamp)}</span>
                                {isMe && (
                                    <span className="flex items-center ml-0.5">
                                        {isPending ? (
                                            <div className="w-2.5 h-2.5 border border-current/30 border-t-current rounded-full animate-spin" />
                                        ) : isFailed ? "!" : (
                                            <div className="flex items-center">
                                                {message.status === "sent" ? (
                                                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                        <path d="M13.5 4.5L6.5 11.5L3 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                                                    </svg>
                                                ) : (
                                                    <div className="relative flex items-center">
                                                        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" className={cn(message.status === "read" ? "text-[#53bdeb]" : "text-current opacity-70")}>
                                                            <path d="M13.5 4.5L6.5 11.5L3 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                                                        </svg>
                                                        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" className={cn("absolute left-[4px]", message.status === "read" ? "text-[#53bdeb]" : "text-current opacity-70")}>
                                                            <path d="M13.5 4.5L6.5 11.5L3 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                                                        </svg>
                                                    </div>
                                                )}
                                            </div>
                                        )
                                        }
                                    </span>
                                )}
                                {isMe && !isSticker && (
                                    <div className="absolute top-1/2 -translate-y-1/2 -right-2 opacity-0 group-hover:opacity-100 transition-opacity hidden md:block z-20">
                                        <Popover>
                                            <PopoverTrigger asChild>
                                                <Button variant="ghost" size="icon" className="h-6 w-6 text-muted-foreground hover:text-foreground bg-background/80 backdrop-blur-sm rounded-full shadow-sm">
                                                    <MoreVertical className="h-3 w-3" />
                                                </Button>
                                            </PopoverTrigger>
                                            <PopoverContent className="w-auto p-1" align="end">
                                                <div className="flex flex-col">
                                                    <button onClick={onReply} className="flex items-center gap-2 w-full p-2 hover:bg-muted rounded text-sm"><Reply className="h-4 w-4 text-primary" /><span>Reply</span></button>
                                                    <button onClick={onEdit} className="flex items-center gap-2 w-full p-2 hover:bg-muted rounded text-sm"><Edit3 className="h-4 w-4 text-orange-500" /><span>Edit</span></button>
                                                    <button onClick={onDelete} className="flex items-center gap-2 w-full p-2 hover:bg-muted rounded text-sm text-destructive"><Trash2 className="h-4 w-4" /><span>Delete</span></button>
                                                </div>
                                            </PopoverContent>
                                        </Popover>
                                    </div>
                                )}
                                <Popover>
                                    <PopoverTrigger asChild>
                                        <div
                                            className="absolute inset-0 z-0 cursor-context-menu"
                                            onContextMenu={(e) => { if (isSticker) return; e.preventDefault(); e.currentTarget.click(); }}
                                        />
                                    </PopoverTrigger>
                                    <PopoverContent className="w-48 p-1 shadow-xl border-border/40 backdrop-blur-xl bg-background/95" align={isMe ? "end" : "start"}>
                                        <div className="flex flex-col">
                                            <button onClick={onReply} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors"><Reply className="h-4 w-4 text-primary" /><span className="font-medium">Reply</span></button>
                                            {isMe && (
                                                <>
                                                    <button onClick={onEdit} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors"><Edit3 className="h-4 w-4 text-orange-500" /><span className="font-medium">Edit</span></button>
                                                    <button onClick={onDelete} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors text-destructive"><Trash2 className="h-4 w-4" /><span className="font-medium">Delete for everyone</span></button>
                                                </>
                                            )}
                                        </div>
                                    </PopoverContent>
                                </Popover>
                                {!isMe && !isSticker && (
                                    <div className="absolute top-1/2 -translate-y-1/2 -left-2 opacity-0 group-hover:opacity-100 transition-opacity hidden md:block z-20">
                                        <Button variant="ghost" size="icon" onClick={onReply} className="h-6 w-6 text-muted-foreground hover:text-primary bg-background/80 backdrop-blur-sm rounded-full shadow-sm"><Reply className="h-3 w-3" /></Button>
                                    </div>
                                )}
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    )
}

export function ChatArea({ chat, incomingMessage, statusUpdate, onBack, className }: ChatAreaProps) {
    const [messages, setMessages] = useState<Message[]>([])
    const [inputMessage, setInputMessage] = useState("")
    const [loading, setLoading] = useState(false)
    const [sending, setSending] = useState(false)
    const [initialLoad, setInitialLoad] = useState(true)
    const [showFavoriteBtn, setShowFavoriteBtn] = useState<string | null>(null)
    const [replyTo, setReplyTo] = useState<Message | null>(null)
    const [editingMessage, setEditingMessage] = useState<Message | null>(null)
    const [isMediaSheetOpen, setIsMediaSheetOpen] = useState(false)
    const [selectedImageUrl, setSelectedImageUrl] = useState<string | null>(null)

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

    useEffect(() => {
        if (chat) {
            loadMessages()
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
            const data = await api.getMessages(chat.id)
            setMessages(data || [])
            setInitialLoad(false)
            setTimeout(() => scrollToBottom("auto"), 50)
        } catch (error) {
            console.error("Failed to load messages:", error)
            toast.error("Failed to load conversation")
        } finally {
            setLoading(false)
        }
    }

    const handleSendMessage = async () => {
        if (!inputMessage.trim() || !chat || sending) return
        const text = inputMessage.trim()
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
            content: type === "image" ? "[Image]" : type === "video" ? "[Video]" : "[Document]",
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

    const groupedMessages = messages.reduce((groups: { [key: string]: Message[] }, message) => {
        const date = formatDate(message.timestamp)
        if (!groups[date]) groups[date] = []
        groups[date].push(message)
        return groups
    }, {})

    const sharedMedia = messages.filter(m => (m.type === "image" || m.type === "video") && m.mediaUrl)
    const sharedDocs = messages.filter(m => m.type === "document" && m.mediaUrl)
    const sharedLinks = messages.filter(m => {
        const urlRegex = /(https?:\/\/[^\s]+)/g
        return m.type === "text" && urlRegex.test(m.content)
    })

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
            </header>

            <div className="flex-1 overflow-y-auto px-6 pt-3 pb-6 space-y-8 z-10" ref={scrollRef}>
                {initialLoad && loading ? (
                    <div className="flex flex-col items-center justify-center py-12 space-y-4 opacity-40">
                        <div className="w-10 h-10 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
                        <p className="text-sm font-medium tracking-tight">Loading secure conversation...</p>
                    </div>
                ) : (
                    <div className="space-y-10">
                        {Object.entries(groupedMessages).map(([dateKey, msgs]) => (
                            <div key={dateKey} className="space-y-4">
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
                                        <MessageItem
                                            key={message.id}
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
                                            onImageClick={(url: string) => setSelectedImageUrl(url || null)}
                                            formatTime={formatTime}
                                            renderFormattedContent={renderFormattedContent}
                                            getMediaUrl={getMediaUrl}
                                            getAvatarUrl={getAvatarUrl}
                                            showFavoriteBtn={showFavoriteBtn}
                                            setShowFavoriteBtn={setShowFavoriteBtn}
                                        />
                                    )
                                })}
                            </div>
                        ))}
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
                    <div className="flex items-center mb-1">
                        <ChatEmojiPickerPopover onEmojiSelect={addEmoji} />
                        <ChatStickerPickerPopover onStickerSelect={handleStickerSelect} />
                        <ChatAttachmentPopover onPickMedia={() => mediaInputRef.current?.click()} onPickDocument={() => documentInputRef.current?.click()} />
                    </div>
                    <div className="flex-1 relative">
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
                sharedMedia={sharedMedia}
                sharedDocs={sharedDocs}
                sharedLinks={sharedLinks}
                getAvatarUrl={getAvatarUrl}
                getMediaUrl={getMediaUrl}
                formatDate={formatDate}
                onSelectImage={setSelectedImageUrl}
            />

            <ChatImageViewerModal
                open={!!selectedImageUrl}
                onOpenChange={(open) => !open && setSelectedImageUrl(null)}
                imageUrl={selectedImageUrl}
                onClose={() => setSelectedImageUrl(null)}
            />
        </div>
    )
}
