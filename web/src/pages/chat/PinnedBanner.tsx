import { useState } from "react"
import { Pin, PinOff } from "lucide-react"
import type { ChatPin } from "@/lib/api"
import { messageSnippet } from "./messageSnippet"

interface PinnedBannerProps {
	/** Active pins with their message, newest first. */
	pins: ChatPin[]
	/** Name shown when the message carries none (the contact, in a private chat). */
	fallbackName?: string
	onJump: (messageId: string) => void
	onUnpin: (pin: ChatPin) => void
}

/** Bar above the conversation: the newest pin, or "n pinned messages" cycling through them on click. */
export function PinnedBanner({ pins, fallbackName, onJump, onUnpin }: PinnedBannerProps) {
	const [index, setIndex] = useState(0)

	if (pins.length === 0) return null
	const current = pins[index % pins.length]
	const message = current.message!
	const sender = message.from === "me" ? "You" : message.senderName || fallbackName || message.from.split("@")[0]

	return (
		<div data-pinned-banner className="flex items-center gap-1 border-b border-border/40 bg-background/80 backdrop-blur-xl z-10 shrink-0">
			<button
				type="button"
				onClick={() => {
					onJump(current.messageId)
					setIndex((i) => (i + 1) % pins.length)
				}}
				className="flex-1 min-w-0 flex items-center gap-3 px-4 py-2 text-left hover:bg-muted/40 transition-colors"
			>
				<Pin className="h-4 w-4 text-muted-foreground shrink-0" />
				{pins.length > 1 && (
					<div className="flex flex-col gap-0.5 shrink-0">
						{pins.map((pin, i) => (
							<span key={pin.messageId} className={`h-2 w-0.5 rounded-full ${i === index % pins.length ? "bg-primary" : "bg-muted-foreground/30"}`} />
						))}
					</div>
				)}
				<div className="min-w-0">
					<p className="text-xs font-bold text-primary truncate">
						{pins.length > 1 ? `${pins.length} pinned messages` : sender}
					</p>
					<p className="text-sm text-muted-foreground truncate">
						{pins.length > 1 ? `${sender}: ` : ""}{messageSnippet(message)}
					</p>
				</div>
			</button>
			<button
				type="button"
				onClick={() => onUnpin(current)}
				title="Unpin message"
				aria-label="Unpin message"
				className="mr-2 p-2 rounded-button text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
			>
				<PinOff className="h-4 w-4" />
			</button>
		</div>
	)
}
