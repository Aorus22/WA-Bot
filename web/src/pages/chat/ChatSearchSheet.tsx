import React, { useState, useEffect, useCallback } from "react"
import { Search, X, Loader2, MessageSquare } from "lucide-react"
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { api, type Message } from "@/lib/api"

interface ChatSearchSheetProps {
    chatId: string
    isOpen: boolean
    onClose: () => void
    onResultClick: (messageId: string) => void
}

export const ChatSearchSheet: React.FC<ChatSearchSheetProps> = ({
    chatId,
    isOpen,
    onClose,
    onResultClick
}) => {
    const [query, setQuery] = useState("")
    const [results, setResults] = useState<Message[]>([])
    const [loading, setLoading] = useState(false)

    const handleSearch = useCallback(async (searchQuery: string) => {
        if (!searchQuery.trim()) {
            setResults([])
            return
        }

        setLoading(true)
        try {
            const data = await api.searchMessages(chatId, searchQuery)
            setResults(data || [])
        } catch (error) {
            console.error("Search failed:", error)
        } finally {
            setLoading(false)
        }
    }, [chatId])

    useEffect(() => {
        const timer = setTimeout(() => {
            if (isOpen) {
                handleSearch(query)
            }
        }, 300)

        return () => clearTimeout(timer)
    }, [query, isOpen, handleSearch])

    const formatDate = (timestamp: number) => {
        const date = new Date(timestamp)
        return date.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })
    }

    const formatTime = (timestamp: number) => {
        return new Date(timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    }

    return (
        <Sheet open={isOpen} onOpenChange={(open) => !open && onClose()}>
            <SheetContent showCloseButton={false} side="right" className="w-full sm:max-w-md p-0 flex flex-col gap-0 border-l border-border/40 bg-background/95 backdrop-blur-xl">
                <SheetHeader className="p-4 border-b border-border/40">
                    <div className="flex items-center justify-between">
                        <SheetTitle className="text-lg font-bold flex items-center gap-2">
                            <Search className="h-5 w-5 text-primary" />
                            Search Messages
                        </SheetTitle>
                        <Button variant="ghost" size="icon" onClick={onClose} className="rounded-full h-8 w-8">
                            <X className="h-4 w-4" />
                        </Button>
                    </div>
                    <div className="mt-4 relative">
                        <Input
                            placeholder="Cari pesan..."
                            value={query}
                            onChange={(e) => setQuery(e.target.value)}
                            className="pl-10 h-11 bg-muted/50 border-none focus-visible:ring-1 focus-visible:ring-primary/30 rounded-xl"
                            autoFocus
                        />
                        <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                        {query && (
                            <button 
                                onClick={() => setQuery("")}
                                className="absolute right-3 top-1/2 -translate-y-1/2 p-1 hover:bg-muted rounded-full transition-colors"
                            >
                                <X className="h-3 w-3 text-muted-foreground" />
                            </button>
                        )}
                    </div>
                </SheetHeader>

                <div className="flex-1 overflow-y-auto custom-scrollbar">
                    {loading ? (
                        <div className="flex flex-col items-center justify-center h-40 gap-3">
                            <Loader2 className="h-6 w-6 animate-spin text-primary" />
                            <p className="text-sm text-muted-foreground animate-pulse">Searching history...</p>
                        </div>
                    ) : results.length > 0 ? (
                        <div className="flex flex-col">
                            {results.map((msg) => (
                                <button
                                    key={msg.id}
                                    onClick={() => onResultClick(msg.id)}
                                    className="flex flex-col gap-1 p-4 text-left hover:bg-muted/50 transition-all border-b border-border/5 group active:scale-[0.98]"
                                >
                                    <div className="flex justify-between items-center w-full">
                                        <span className="text-xs font-bold text-primary truncate max-w-[150px]">
                                            {msg.from === "me" ? "You" : (msg.senderName || msg.from.split("@")[0])}
                                        </span>
                                        <span className="text-[10px] text-muted-foreground font-medium uppercase tracking-wider">
                                            {formatDate(msg.timestamp)}
                                        </span>
                                    </div>
                                    <p className="text-sm line-clamp-2 text-foreground/80 group-hover:text-foreground transition-colors">
                                        {msg.content}
                                    </p>
                                    <div className="flex justify-end mt-1">
                                        <span className="text-[10px] opacity-50 font-bold tracking-tighter italic">
                                            {formatTime(msg.timestamp)}
                                        </span>
                                    </div>
                                </button>
                            ))}
                        </div>
                    ) : query.trim() ? (
                        <div className="flex flex-col items-center justify-center h-64 p-8 text-center gap-4">
                            <div className="w-16 h-16 bg-muted rounded-3xl flex items-center justify-center">
                                <MessageSquare className="h-8 w-8 text-muted-foreground/50" />
                            </div>
                            <div>
                                <h3 className="font-bold text-foreground/80">No results found</h3>
                                <p className="text-sm text-muted-foreground mt-1 px-4">
                                    We couldn't find any messages matching "{query}" in this chat.
                                </p>
                            </div>
                        </div>
                    ) : (
                        <div className="flex flex-col items-center justify-center h-64 p-8 text-center gap-4 text-muted-foreground/40">
                            <Search className="h-12 w-12 stroke-[1px]" />
                            <p className="text-sm font-medium">Search for keywords, dates, or phrases</p>
                        </div>
                    )}
                </div>
            </SheetContent>
        </Sheet>
    )
}
