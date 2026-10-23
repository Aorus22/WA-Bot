import React from "react"
import { Check, CheckCheck, X, Loader2 } from "lucide-react"
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Button } from "@/components/ui/button"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import type { GroupCache, Message, MessageReceipt } from "@/lib/api"
import { isMarkdownContent } from "./renderMd"
import { BoxedList } from "@/components/ui/boxed-list"

interface MessageInfoSheetProps {
    message: Message | null
    receipts: MessageReceipt[]
    recipients: string[]
    loading: boolean
    group: GroupCache | null
    getAvatarUrl: (jid: string) => string | undefined
    onClose: () => void
}

const sameUser = (a: string, b: string) => a === b || a.split("@")[0] === b.split("@")[0]

// "Today at 22:03", "Yesterday at 22:03" or "7 Sep 2026 at 22:03".
const formatReceiptTime = (ms: number) => {
    const date = new Date(ms)
    const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })
    const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
    const days = Math.round((startOfDay(new Date()) - startOfDay(date)) / 86_400_000)
    if (days === 0) return `Today at ${time}`
    if (days === 1) return `Yesterday at ${time}`
    return `${date.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })} at ${time}`
}

const decodeContent = (content: string) => {
    if (!isMarkdownContent(content)) return content
    try {
        return decodeURIComponent(escape(atob(content.trim().slice(5, -2))))
    } catch {
        return content
    }
}

export const MessageInfoSheet: React.FC<MessageInfoSheetProps> = ({ message, receipts, recipients, loading, group, getAvatarUrl, onClose }) => {
    const isGroup = message?.chatId.endsWith("@g.us") ?? false
    const read = receipts.filter(r => r.readAt > 0).sort((a, b) => a.readAt - b.readAt)
    const delivered = receipts.filter(r => r.readAt === 0 && r.deliveredAt > 0).sort((a, b) => a.deliveredAt - b.deliveredAt)
    // Members WhatsApp has not reported on yet.
    const remaining = recipients.filter(jid => !receipts.some(r => sameUser(r.jid, jid) && (r.readAt > 0 || r.deliveredAt > 0)))

    const nameOf = (jid: string) => group?.participants.find(member => sameUser(member.jid, jid))?.name?.trim() || jid.split("@")[0]
    const row = (jid: string, at?: number) => {
        const name = nameOf(jid)
        return (
            <div key={jid} className="flex items-center gap-3 px-4 py-2">
                <Avatar className="h-10 w-10"><AvatarImage src={getAvatarUrl(jid)} /><AvatarFallback>{name.slice(0, 1).toUpperCase()}</AvatarFallback></Avatar>
                <div className="min-w-0">
                    <div className="truncate text-sm">{name}</div>
                    {at ? <div className="text-xs text-muted-foreground">{formatReceiptTime(at)}</div> : null}
                </div>
            </div>
        )
    }
    const header = (label: string, icon: React.ReactNode) => (
        <div className="flex items-center gap-3 px-4 pt-4 pb-1 text-sm text-muted-foreground">{icon}{label}</div>
    )

    // A private chat has one recipient, so each stage is one line with its time.
    // The stored tick covers messages that predate per-stage times.
    const readAt = read[0]?.readAt ?? 0
    const deliveredAt = receipts.map(r => r.deliveredAt || r.readAt).find(at => at > 0) ?? 0
    const isRead = readAt > 0 || message?.status === "read"
    const isDelivered = isRead || deliveredAt > 0 || message?.status === "delivered"
    const stage = (label: string, icon: React.ReactNode, at: number) => (
        <div className="px-4 py-2">
            <div className="flex items-center gap-3 text-sm">{icon}{label}</div>
            <div className="pl-7 text-xs text-muted-foreground">{at > 0 ? formatReceiptTime(at) : "Time not recorded"}</div>
        </div>
    )

    const text = message ? (decodeContent(message.content).trim() || `[${message.type}]`) : ""

    return (
        <Sheet open={message !== null} onOpenChange={open => { if (!open) onClose() }}>
            <SheetContent showCloseButton={false} side="right" className="w-full sm:max-w-md p-0 flex flex-col gap-0 border-l border-border/40 bg-background/95 backdrop-blur-xl">
                <SheetHeader className="p-4 border-b border-border/40">
                    <div className="flex items-center gap-3">
                        <Button variant="ghost" size="icon" onClick={onClose} className="rounded-button h-8 w-8"><X className="h-4 w-4" /></Button>
                        <SheetTitle className="text-lg font-bold">Message info</SheetTitle>
                    </div>
                </SheetHeader>
                <div className="flex-1 overflow-y-auto pb-4">
                    <div className="flex justify-end px-4 py-4">
                        <div className="max-w-[80%] whitespace-pre-wrap break-words rounded-lg bg-primary/20 px-3 py-2 text-sm">{text}</div>
                    </div>
                    {loading ? (
                        <div className="flex justify-center py-8"><Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /></div>
                    ) : !isGroup ? (
                        <>
                            {isRead && stage("Read", <CheckCheck className="h-4 w-4 text-sky-400" />, readAt)}
                            {isDelivered && stage("Delivered", <CheckCheck className="h-4 w-4 text-muted-foreground" />, deliveredAt)}
                            {!isDelivered && <p className="px-4 py-8 text-center text-sm text-muted-foreground">Not delivered yet.</p>}
                        </>
                    ) : read.length === 0 && delivered.length === 0 && remaining.length === 0 ? (
                        <p className="px-4 py-8 text-center text-sm text-muted-foreground">No receipts reported yet.</p>
                    ) : (
                        <>
                            {read.length > 0 && (
                                <section>
                                    {header("Read by", <CheckCheck className="h-4 w-4 text-sky-400" />)}
                                    <BoxedList>{read.map(r => row(r.jid, r.readAt))}</BoxedList>
                                </section>
                            )}
                            {delivered.length > 0 && (
                                <section>
                                    {header("Delivered to", <CheckCheck className="h-4 w-4" />)}
                                    <BoxedList>{delivered.map(r => row(r.jid, r.deliveredAt))}</BoxedList>
                                </section>
                            )}
                            {remaining.length > 0 && (
                                <section>
                                    {header("Remaining", <Check className="h-4 w-4" />)}
                                    <BoxedList>{remaining.map(jid => row(jid))}</BoxedList>
                                </section>
                            )}
                        </>
                    )}
                </div>
            </SheetContent>
        </Sheet>
    )
}
