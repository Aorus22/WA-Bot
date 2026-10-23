import { useCallback, useEffect, useState } from "react"
import { Star, X, Loader2 } from "lucide-react"
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Button } from "@/components/ui/button"
import { api, type StarredMessage } from "@/lib/api"
import { subscribeWS } from "@/lib/ws-bus"
import { messageSnippet } from "./messageSnippet"
import { BoxedList } from "@/components/ui/boxed-list"

interface StarredSheetProps {
	open: boolean
	onClose: () => void
	onSelect: (message: StarredMessage) => void
}

const formatDate = (timestamp: number) =>
	new Date(timestamp).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })

export function StarredSheet({ open, onClose, onSelect }: StarredSheetProps) {
	const [items, setItems] = useState<StarredMessage[]>([])
	const [loading, setLoading] = useState(false)

	const load = useCallback(async () => {
		setLoading(true)
		try {
			setItems((await api.getStarred({ limit: 100 })) || [])
		} catch (error) {
			console.error("Failed to load starred messages:", error)
		} finally {
			setLoading(false)
		}
	}, [])

	useEffect(() => {
		if (!open) return
		load()
		// Another device (or the context menu) starring or unstarring keeps the list current.
		return subscribeWS((msg) => {
			if (msg.type === "message_starred") load()
		})
	}, [open, load])

	return (
		<Sheet open={open} onOpenChange={(next) => !next && onClose()}>
			<SheetContent showCloseButton={false} side="left" className="w-full sm:max-w-md p-0 flex flex-col gap-0 border-r border-border/40 bg-background/95 backdrop-blur-xl">
				<SheetHeader className="p-4 border-b border-border/40">
					<div className="flex items-center justify-between">
						<SheetTitle className="text-lg font-bold flex items-center gap-2">
							<Star className="h-5 w-5 text-primary" />
							Starred messages
						</SheetTitle>
						<Button variant="ghost" size="icon" onClick={onClose} className="rounded-button h-8 w-8">
							<X className="h-4 w-4" />
						</Button>
					</div>
				</SheetHeader>

				<div className="flex-1 overflow-y-auto custom-scrollbar">
					{loading && items.length === 0 ? (
						<div className="flex justify-center py-12">
							<Loader2 className="h-6 w-6 animate-spin text-primary" />
						</div>
					) : items.length > 0 ? (
						<BoxedList className="flex flex-col">
							{items.map((msg) => (
								<button
									key={`${msg.chatId}-${msg.id}`}
									onClick={() => onSelect(msg)}
									className="flex flex-col gap-1 p-4 text-left hover:bg-muted/50 transition-all border-b border-border/5 active:scale-[0.98]"
								>
									<div className="flex justify-between items-center w-full gap-3">
										<span className="text-xs font-bold text-primary truncate">
											{msg.chatName || msg.chatId.split("@")[0]}
										</span>
										<span className="text-[10px] text-muted-foreground font-medium uppercase tracking-wider shrink-0">
											{formatDate(msg.timestamp)}
										</span>
									</div>
									<span className="text-[11px] text-muted-foreground truncate">
										{msg.from === "me" ? "You" : (msg.senderName || msg.from.split("@")[0])}
									</span>
									<p className="text-sm line-clamp-2 text-foreground/80">{messageSnippet(msg)}</p>
								</button>
							))}
						</BoxedList>
					) : (
						<div className="flex flex-col items-center justify-center h-64 p-8 text-center gap-4 text-muted-foreground/40">
							<Star className="h-12 w-12 stroke-[1px]" />
							<p className="text-sm font-medium">No starred messages. Star a message from its menu to find it here.</p>
						</div>
					)}
				</div>
			</SheetContent>
		</Sheet>
	)
}
