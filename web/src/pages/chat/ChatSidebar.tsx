import { useState, useEffect, useRef } from "react"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Input } from "@/components/ui/input"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { Search } from "lucide-react"
import { api, type Chat } from "@/lib/api"
import { cn } from "@/lib/utils"

interface ChatSidebarProps {
    selectedChatId: string | null
    onChatSelect: (chat: Chat) => void
    chatUpdate?: { chatId: string; lastMsg: string; lastTime: number; msgId: string; senderName?: string; chatName?: string; chatAvatar?: string } | null
    className?: string
}

// Get avatar URL - prefer actual avatar, fallback to proxy
const getAvatarUrl = (chat: Chat): string | undefined => {
    if (chat.avatar && chat.avatar.length > 0 && !chat.avatar.startsWith("data:")) {
        return chat.avatar
    }
    // Use avatar proxy endpoint
    if (chat.id.includes("@") || chat.id.match(/^\d+$/)) {
        const API_BASE = import.meta.env.VITE_API_URL || "http://localhost:3000"
        return `${API_BASE}/avatar/${encodeURIComponent(chat.id)}`
    }
    return undefined
}

export function ChatSidebar({
    selectedChatId,
    onChatSelect,
    chatUpdate,
    className,
}: ChatSidebarProps) {
    const [chats, setChats] = useState<Chat[]>([])
    const [searchQuery, setSearchQuery] = useState("")
    const [loading, setLoading] = useState(true)
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

            setChats(prevChats => {
                const chatExists = prevChats.some(c => c.id === chatUpdate.chatId);

                if (!chatExists) {
                    // Add new chat
                    const newChat: Chat = {
                        id: chatUpdate.chatId,
                        name: chatUpdate.chatName || chatUpdate.senderName || chatUpdate.chatId,
                        avatar: "",
                        lastMsg: chatUpdate.lastMsg,
                        lastTime: chatUpdate.lastTime,
                        unread: selectedChatId === chatUpdate.chatId ? 0 : 1,
                        isActive: true,
                        isGroup: chatUpdate.chatId.includes("@g.us"),
                    };
                    return [newChat, ...prevChats].sort((a, b) => b.lastTime - a.lastTime);
                }

                return prevChats.map(chat => {
                    if (chat.id === chatUpdate.chatId) {
                        // If it's a name/avatar update only (lastMsg is empty)
                        if (chatUpdate.lastMsg === "") {
                            return {
                                ...chat,
                                name: chatUpdate.chatName || chat.name,
                                avatar: chatUpdate.chatAvatar || chat.avatar
                            }
                        }
                        // Regular message update
                        return {
                            ...chat,
                            name: chatUpdate.chatName || chat.name,
                            avatar: chatUpdate.chatAvatar || chat.avatar,
                            lastMsg: chatUpdate.lastMsg,
                            lastTime: chatUpdate.lastTime,
                            unread: chat.id === selectedChatId ? 0 : chat.unread + 1,
                        }
                    }
                    return chat
                }).sort((a, b) => {												// Sort by lastTime descending - updated chat goes to top (only for messages)
                    if (chatUpdate.lastMsg !== "") {
                        if (a.id === chatUpdate.chatId) return -1
                        if (b.id === chatUpdate.chatId) return 1
                    }
                    return b.lastTime - a.lastTime
                })
            })
        }
    }, [chatUpdate, selectedChatId])

    // Clear unread when chat is selected
    useEffect(() => {
        if (selectedChatId) {
            setChats(prevChats =>
                prevChats.map(chat =>
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
        try {
            setLoading(true)
            const data = await api.getChats()
            setChats(data || [])
        } catch (error) {
            console.error("Failed to load chats:", error)
            setChats([])
        } finally {
            setLoading(false)
        }
    }

    const filteredChats = (chats || []).filter((chat) =>
        searchQuery === "" ||
        chat.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        chat.lastMsg.toLowerCase().includes(searchQuery.toLowerCase())
    )

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
                    <h2 className="text-2xl font-bold tracking-tight">Messages</h2>
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
            <ScrollArea className="flex-1 px-2 min-h-0">				{loading ? (
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
                        <button
                            key={chat.id}
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
                                        {chat.name.charAt(0).toUpperCase()}
                                    </AvatarFallback>
                                </Avatar>
                                {/* Status dot example - could be connected/away */}
                                <div className="absolute bottom-0 right-0 w-3.5 h-3.5 bg-green-500 border-2 border-background rounded-full" />
                            </div>

                            <div className="flex-1 min-w-0">
                                <div className="flex items-center justify-between mb-1">
                                    <h3 className={cn(
                                        "font-bold truncate group-hover:text-primary transition-colors",
                                        chat.unread > 0 ? "text-foreground" : "text-foreground/90"
                                    )}>
                                        {chat.name}
                                    </h3>
                                    <span className="text-[11px] font-medium text-muted-foreground/70 uppercase tracking-tighter">
                                        {formatTime(chat.lastTime)}
                                    </span>
                                </div>

                                <div className="flex items-center justify-between gap-2">
                                    <p className={cn(
                                        "text-sm truncate flex-1",
                                        chat.unread > 0 ? "text-foreground font-semibold" : "text-muted-foreground/80"
                                    )}>
                                        {chat.lastMsg || "Tap to chat"}
                                    </p>
                                    {chat.unread > 0 && (
                                        <Badge
                                            className="h-5 min-w-[20px] px-1.5 flex items-center justify-center bg-primary text-primary-foreground border-none rounded-full text-[10px] font-bold shadow-sm animate-in zoom-in-50 duration-300"
                                        >
                                            {chat.unread}
                                        </Badge>
                                    )}
                                </div>
                            </div>
                        </button>
                    ))}
                </div>
            )}
            </ScrollArea>
        </div>
    )
}
