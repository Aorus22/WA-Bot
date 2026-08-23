import { useState, useEffect, useRef, memo, useMemo } from "react"
import { Input } from "@/components/ui/input"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Archive, ChevronLeft, Pin, Search, VolumeX } from "lucide-react"
import { api, type Chat } from "@/lib/api"
import { cn } from "@/lib/utils"
import { useChatStore } from "@/stores/chatStore"
import { toast } from "sonner"
import {
    ContextMenu,
    ContextMenuContent,
    ContextMenuItem,
    ContextMenuSeparator,
    ContextMenuSub,
    ContextMenuSubContent,
    ContextMenuSubTrigger,
    ContextMenuTrigger,
} from "@/components/ui/context-menu"

interface ChatSidebarProps {
    selectedChatId: string | null
    onChatSelect: (chat: Chat) => void
    onChatsLoaded?: (chats: Chat[]) => void
    chatUpdate?: { 
        chatId: string; 
        lastMsg: string; 
        lastTime: number; 
        msgId: string; 
        status?: string; 
        senderName?: string; 
        chatName?: string; 
        chatAvatar?: string 
    } | null
    className?: string
}

// Get avatar URL - prefer actual avatar, fallback to proxy
const getAvatarUrl = (chat: Chat): string | undefined => {
	const avatar = chat.avatar || ""
	if (avatar.length > 0 && !avatar.startsWith("data:")) {
		return avatar
    }
    // Use avatar proxy endpoint
    if (chat.id.includes("@") || chat.id.match(/^\d+$/)) {
        return api.mediaURL(`/avatar/${encodeURIComponent(chat.id)}`)
    }
    return undefined
}

export const ChatSidebar = memo(({
    selectedChatId,
    onChatSelect,
    onChatsLoaded,
    chatUpdate,
    className,
}: ChatSidebarProps) => {
    const chats = useChatStore(s => s.chats)
    const chatsLoaded = useChatStore(s => s.chatsLoaded)
    const chatsLoading = useChatStore(s => s.chatsLoading)
    const setChats = useChatStore(s => s.setChats)
    const setChatsLoaded = useChatStore(s => s.setChatsLoaded)
    const setChatsLoading = useChatStore(s => s.setChatsLoading)
    const upsertChat = useChatStore(s => s.upsertChat)
    const [searchQuery, setSearchQuery] = useState("")
    const [loading, setLoading] = useState(false)
    const [archivedMode, setArchivedMode] = useState(false)
    const processedUpdateIds = useRef<Set<string>>(new Set())

    useEffect(() => {
        loadChats()
    }, [])

    // Update chat in-place when new message arrives (no flicker)
    useEffect(() => {
        if (chatUpdate && chatUpdate.msgId) {
            // Skip if already processed this update
            if (processedUpdateIds.current.has(chatUpdate.msgId)) {
                return
            }
            processedUpdateIds.current.add(chatUpdate.msgId)

            // If this is a message for the currently selected chat, mark it as read in the backend
            if (chatUpdate.chatId === selectedChatId && chatUpdate.lastMsg !== "") {
                api.markAsRead(selectedChatId).catch(console.error)
            }

            const existingChat = chats.find(c => c.id === chatUpdate.chatId)

            if (!existingChat) {
                const newChat: Chat = {
                    id: chatUpdate.chatId,
                    name: chatUpdate.chatName || chatUpdate.senderName || chatUpdate.chatId,
                    avatar: "",
                    lastMsg: chatUpdate.lastMsg,
                    lastTime: chatUpdate.lastTime,
                    unread: selectedChatId === chatUpdate.chatId ? 0 : 1,
                    isActive: true,
                    isGroup: chatUpdate.chatId.includes("@g.us"),
                    archived: false,
                    pinnedAt: null,
                    muteMode: "off",
                    mutedUntil: null,
                }
                upsertChat(newChat)
            } else {
                const isIncoming = chatUpdate.status === "received" || (!chatUpdate.status && chatUpdate.lastMsg !== "")
                const currentUnread = Number(existingChat.unread) || 0
                const newUnread = existingChat.id === selectedChatId ? 0 : (isIncoming ? currentUnread + 1 : currentUnread)

                upsertChat({
                    ...existingChat,
                    name: chatUpdate.chatName || existingChat.name,
                    avatar: chatUpdate.chatAvatar || existingChat.avatar,
                    lastMsg: chatUpdate.lastMsg,
                    lastTime: chatUpdate.lastTime,
                    unread: newUnread,
                })
            }
        }
    }, [chatUpdate, selectedChatId, chats, upsertChat])

    // Clear unread when chat is selected
    useEffect(() => {
        if (selectedChatId) {
            setChats(
                chats.map(chat =>
                    chat.id === selectedChatId
                        ? { ...chat, unread: 0 }
                        : chat
                )
            )
            // Mark as read in API (local DB update only)
            api.markAsRead(selectedChatId).catch(console.error)
        }
    }, [selectedChatId])

    const loadChats = async () => {
        if (chatsLoaded || chatsLoading) return
        try {
            setChatsLoading(true)
            setLoading(true)
            const data = await api.getChats()
            // Deduplicate to avoid rendering issues
            const chatMap = new Map<string, Chat>();
            (data || []).forEach(chat => chatMap.set(chat.id, chat));
            const deduped = Array.from(chatMap.values())
            setChats(deduped)
            setChatsLoaded(true)
            onChatsLoaded?.(deduped)
        } catch (error) {
            console.error("Failed to load chats:", error)
            setChats([])
        } finally {
            setChatsLoading(false)
            setLoading(false)
        }
    }

    const archivedChats = useMemo(() => (chats || []).filter(chat => chat.archived), [chats])
    const archivedUnread = useMemo(
        () => archivedChats.reduce((total, chat) => total + (Number(chat.unread) || 0), 0),
        [archivedChats]
    )
    const filteredChats = useMemo(() => {
        const query = searchQuery.toLowerCase()
		return (chats || [])
            .filter((chat) => Boolean(chat.archived) === archivedMode)
            .filter((chat) =>
                query === "" ||
				(chat.name || "").toLowerCase().includes(query) ||
				(chat.lastMsg || "").toLowerCase().includes(query)
            )
            .sort((a, b) => {
                if (a.pinnedAt && !b.pinnedAt) return -1
                if (!a.pinnedAt && b.pinnedAt) return 1
                if (a.pinnedAt && b.pinnedAt && a.pinnedAt !== b.pinnedAt) return b.pinnedAt - a.pinnedAt
                return b.lastTime - a.lastTime
            })
    }, [chats, searchQuery, archivedMode])

    const applyChatAction = async (chat: Chat, action: "pin" | "archive" | "mute", value: boolean | "off" | "8h" | "1w" | "forever") => {
        try {
            const state = action === "pin"
                ? await api.pinChat(chat.id, Boolean(value))
                : action === "archive"
                    ? await api.archiveChat(chat.id, Boolean(value))
                    : await api.muteChat(chat.id, value as "off" | "8h" | "1w" | "forever")
            useChatStore.getState().patchChatState(state)
        } catch (error) {
            toast.error(error instanceof Error ? error.message : "Failed to update chat")
        }
    }

    const formatTime = (timestamp: number) => {
        const date = new Date(timestamp)
        const now = new Date()
        const isToday = date.toDateString() === now.toDateString()

        if (isToday) {
            return date.toLocaleTimeString("en-US", {
                hour: "2-digit",
                minute: "2-digit",
            })
        }

        return date.toLocaleDateString("en-US", {
            month: "short",
            day: "numeric",
        })
    }

    return (
        <div className={cn("flex flex-col h-full max-h-full bg-background border-r border-border/40 overflow-hidden", className)}>
            {/* Header */}
            <div className="flex flex-col p-5 space-y-4 shrink-0">
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        {archivedMode && (
                            <button
                                aria-label="Back to messages"
                                onClick={() => setArchivedMode(false)}
                                className="rounded-full p-1.5 hover:bg-muted"
                            >
                                <ChevronLeft className="h-5 w-5" />
                            </button>
                        )}
                        <h2 className="text-2xl font-bold tracking-tight">{archivedMode ? "Archived" : "Messages"}</h2>
                    </div>
                </div>

                {/* Search - Expandable or always visible */}
                <div className="relative group">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground transition-colors group-focus-within:text-primary" />
                    <Input
                        placeholder="Search conversations..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="pl-10 h-10 bg-muted/50 border-none focus-visible:ring-1 focus-visible:ring-primary/20 rounded-xl"
                    />
                </div>
            </div>

            {/* Chat List */}
            <div data-sidebar-scroll className="flex-1 overflow-y-auto px-2 min-h-0">
            {!archivedMode && !searchQuery && archivedChats.length > 0 && !loading && (
                <button
                    onClick={() => setArchivedMode(true)}
                    className="w-full flex items-center gap-4 px-4 py-3 rounded-2xl hover:bg-muted/50 text-left mb-1"
                >
                    <span className="h-11 w-11 rounded-full bg-muted flex items-center justify-center">
                        <Archive className="h-5 w-5 text-[#00a884]" />
                    </span>
                    <span className="flex-1 font-semibold">Archived</span>
                    {archivedUnread > 0 && <span className="text-xs font-bold text-[#00a884]">{archivedUnread}</span>}
                </button>
            )}
				{loading ? (
                <div className="flex flex-col items-center justify-center py-12 space-y-3 opacity-50">
                    <div className="w-8 h-8 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
                    <p className="text-sm font-medium">Syncing chats...</p>
                </div>
            ) : filteredChats.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 px-6 text-center space-y-2 opacity-60">
                    <div className="w-12 h-12 bg-muted rounded-full flex items-center justify-center mb-2">
                        <Search className="h-6 w-6" />
                    </div>
                    <p className="text-sm font-semibold">{searchQuery ? "No results found" : "No messages yet"}</p>
                    <p className="text-xs text-muted-foreground">
                        {searchQuery ? `We couldn't find anything for "${searchQuery}"` : "New messages will appear here as they arrive."}
                    </p>
                </div>
            ) : (
                <div className="space-y-1 pb-4">
                    {filteredChats.map((chat) => (
                        <ContextMenu key={chat.id}>
                            <ContextMenuTrigger asChild>
                            <div data-chat-item>
                            <button
                                onClick={() => onChatSelect(chat)}
                                className={cn(
                                    "w-full flex items-center gap-4 p-3.5 rounded-2xl transition-all duration-200 group text-left relative",
                                    selectedChatId === chat.id
                                        ? "bg-primary/10 shadow-sm"
                                        : "hover:bg-muted/50 active:scale-[0.98]"
                                )}
                            >
                                {/* Active Indicator Line */}
                                {selectedChatId === chat.id && (
                                    <div className="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-8 bg-primary rounded-r-full" />
                                )}

                                <div className="relative flex-shrink-0">
                                    <Avatar className="h-14 w-14 border-2 border-background shadow-sm group-hover:scale-105 transition-transform duration-200">
                                        <AvatarImage src={getAvatarUrl(chat)} />
                                        <AvatarFallback className="bg-primary/10 text-primary font-bold text-lg">
	                                            {(chat.name || "?").charAt(0).toUpperCase()}
                                        </AvatarFallback>
                                    </Avatar>
                                </div>

                                <div className="flex-1 min-w-0 grid grid-cols-[1fr_auto] gap-3 items-center">
                                    {/* Left: Name and Message */}
                                    <div className="min-w-0 flex flex-col justify-center h-full">
                                        <h3 className={cn(
                                            "font-bold truncate group-hover:text-primary transition-colors flex items-center gap-1.5",
                                            (Number(chat.unread) || 0) > 0 ? "text-foreground" : "text-foreground/90"
                                        )}>
                                            <span className="truncate">{chat.name}</span>
                                            {chat.pinnedAt && <Pin className="h-3.5 w-3.5 text-muted-foreground shrink-0" />}
                                        </h3>
                                        <p className={cn(
                                            "text-sm truncate",
                                            (Number(chat.unread) || 0) > 0 ? "text-foreground font-semibold" : "text-muted-foreground/80"
                                        )}>
                                            <span className="inline-flex items-center gap-1 min-w-0">
                                                {chat.muteMode !== "off" && <VolumeX className="h-3.5 w-3.5 shrink-0" />}
                                                <span className="truncate">{chat.lastMsg || "Tap to chat"}</span>
                                            </span>
                                        </p>
                                    </div>

                                    {/* Right: Time and Badge */}
                                    <div className="flex flex-col items-end justify-between shrink-0 self-stretch py-0.5 min-w-[48px]">
                                        <span className={cn(
                                            "text-[11px] font-medium uppercase tracking-tighter transition-colors duration-300",
                                            (Number(chat.unread) || 0) > 0 ? "text-[#00a884] font-bold" : "text-muted-foreground/70"
                                        )}>
                                            {formatTime(chat.lastTime)}
                                        </span>
                                        
                                        {(Number(chat.unread) || 0) > 0 && (
                                            <div className="bg-[#00a884] text-white min-w-[20px] h-5 px-1.5 rounded-full text-[10px] font-bold flex items-center justify-center shadow-sm ring-1 ring-background">
                                                {chat.unread}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </button>
                            </div>
                            </ContextMenuTrigger>
                            <ContextMenuContent className="w-48">
                                <ContextMenuItem onSelect={() => applyChatAction(chat, "pin", !chat.pinnedAt)}>
                                    <Pin className="h-4 w-4" /> {chat.pinnedAt ? "Unpin" : "Pin"}
                                </ContextMenuItem>
                                <ContextMenuItem onSelect={() => applyChatAction(chat, "archive", !chat.archived)}>
                                    <Archive className="h-4 w-4" /> {chat.archived ? "Unarchive" : "Archive"}
                                </ContextMenuItem>
                                <ContextMenuSeparator />
                                {chat.muteMode !== "off" ? (
                                    <ContextMenuItem onSelect={() => applyChatAction(chat, "mute", "off")}>
                                        <VolumeX className="h-4 w-4" /> Unmute
                                    </ContextMenuItem>
                                ) : (
                                    <ContextMenuSub>
                                        <ContextMenuSubTrigger><VolumeX className="h-4 w-4" /> Mute</ContextMenuSubTrigger>
                                        <ContextMenuSubContent className="w-40">
                                            <ContextMenuItem onSelect={() => applyChatAction(chat, "mute", "8h")}>8 hours</ContextMenuItem>
                                            <ContextMenuItem onSelect={() => applyChatAction(chat, "mute", "1w")}>1 week</ContextMenuItem>
                                            <ContextMenuItem onSelect={() => applyChatAction(chat, "mute", "forever")}>Always</ContextMenuItem>
                                        </ContextMenuSubContent>
                                    </ContextMenuSub>
                                )}
                            </ContextMenuContent>
                        </ContextMenu>
                    ))}
                </div>
            )}
            </div>
        </div>
    )
})
