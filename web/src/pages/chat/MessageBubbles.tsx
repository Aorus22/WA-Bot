import { useState } from "react"
import { MapPin, Phone, User, Download, Eye, ArrowRight } from "lucide-react"
import { cn } from "@/lib/utils"
import type { LocationMeta, ContactMeta, LinkPreviewMeta, PollMeta, ReactionEntry, ViewOnceMeta } from "@/lib/api"
import { LazyMedia } from "@/components/LazyMedia"

/** Row of emoji reaction chips shown under a bubble. */
export function ReactionChips({ reactions, isMe, onReact }: {
	reactions: ReactionEntry[]
	isMe: boolean
	onReact?: (emoji: string) => void
}) {
	if (!reactions?.length) return null
	return (
		<div className={cn("flex flex-wrap gap-1 -mt-1 mb-1 relative z-10", isMe ? "justify-end" : "justify-start")}>
			{reactions.map((r) => (
				<button
					key={r.emoji}
					onClick={(e) => {
						e.stopPropagation()
						onReact?.(r.emoji)
					}}
					className="flex items-center gap-1 px-1.5 py-0.5 rounded-full bg-black/5 dark:bg-white/10 text-[12px] border border-black/5 dark:border-white/10 hover:bg-black/10 dark:hover:bg-white/20 transition-colors"
				>
					<span>{r.emoji}</span>
					{r.senders?.length > 1 && <span className="text-[10px] font-bold opacity-70">{r.senders.length}</span>}
				</button>
			))}
		</div>
	)
}

/** "Forwarded" marker above the bubble content. */
export function ForwardedLabel() {
	return (
		<div className="flex items-center gap-1 text-[11px] italic opacity-60 mb-0.5">
			<ArrowRight className="h-3 w-3" />
			<span>Forwarded</span>
		</div>
	)
}

/** Poll bubble: question, clickable options with counts and a tally bar. */
export function PollBubble({ poll, isMe, onVote, myVote }: {
	poll: PollMeta
	isMe: boolean
	onVote?: (options: string[]) => void
	myVote?: string[]
}) {
	const votes = poll.votes || {}
	const counts: Record<string, number> = {}
	for (const opts of Object.values(votes)) {
		for (const o of opts) counts[o] = (counts[o] || 0) + 1
	}
	const total = Object.keys(votes).length
	const mySet = new Set(myVote || [])

	const toggle = (option: string) => {
		if (!onVote) return
		if (poll.multiSelect) {
			const next = new Set(mySet)
			if (next.has(option)) next.delete(option)
			else next.add(option)
			onVote([...next])
		} else {
			onVote(mySet.has(option) ? [] : [option])
		}
	}

	return (
		<div className="flex flex-col gap-2 min-w-[220px] sm:min-w-[260px]">
			<p className="font-bold text-[14px] break-words">{poll.question}</p>
			<div className="flex flex-col gap-1.5">
				{poll.options?.map((opt) => {
					const count = counts[opt.name] || 0
					const pct = total > 0 ? Math.round((count / total) * 100) : 0
					const voted = mySet.has(opt.name)
					return (
						<button
							key={opt.name}
							onClick={(e) => {
								e.stopPropagation()
								toggle(opt.name)
							}}
							className="relative overflow-hidden text-left rounded-lg border border-black/10 dark:border-white/15 px-2.5 py-1.5 hover:border-primary/60 transition-colors"
						>
							<div
								className={cn("absolute inset-y-0 left-0", isMe ? "bg-black/10 dark:bg-white/15" : "bg-primary/15")}
								style={{ width: `${pct}%` }}
							/>
							<div className="relative flex items-center justify-between gap-3">
								<span className={cn("text-[13px] break-words", voted && "font-bold text-primary")}>
									{voted ? "● " : "○ "}{opt.name}
								</span>
								<span className="text-[11px] font-bold opacity-60 shrink-0">{count}</span>
							</div>
						</button>
					)
				})}
			</div>
			<p className="text-[11px] opacity-60">
				{total} voter{total === 1 ? "" : "s"}{poll.multiSelect ? " · select multiple" : ""}
			</p>
		</div>
	)
}

/** Location card with optional thumbnail and an open-in-Maps action. */
export function LocationBubble({ location, getMediaUrl }: {
	location: LocationMeta
	getMediaUrl: (url: string) => string
}) {
	const title = location.name || location.address || (location.live ? "Live location" : "Location")
	const mapsUrl = `https://www.google.com/maps?q=${location.latitude},${location.longitude}`
	return (
		<div className="flex flex-col gap-2 min-w-[220px] sm:min-w-[260px]">
			{location.thumbnailUrl && (
				<div className="rounded-lg overflow-hidden">
					<LazyMedia
						src={getMediaUrl(location.thumbnailUrl)}
						alt="Location preview"
						className="w-full h-[120px] object-cover"
						containerClassName="w-full h-[120px]"
						loading="lazy"
					/>
				</div>
			)}
			<div className="flex items-center gap-2">
				<div className="w-9 h-9 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
					<MapPin className="h-5 w-5 text-primary" />
				</div>
				<div className="min-w-0">
					<p className="text-[13px] font-bold truncate">{title}</p>
					<p className="text-[11px] opacity-60">
						{location.latitude.toFixed(5)}, {location.longitude.toFixed(5)}
						{location.live && " · live"}
					</p>
				</div>
			</div>
			<a
				href={mapsUrl}
				target="_blank"
				rel="noopener noreferrer"
				className="flex items-center justify-center gap-1.5 py-2 rounded-lg bg-primary/10 hover:bg-primary/20 text-[12px] font-bold text-primary transition-colors"
			>
				<MapPin className="h-3.5 w-3.5" />
				Open in Maps
			</a>
		</div>
	)
}

/** Extracts the first TEL number from a vCard string. */
function vcardPhone(vcard?: string): string {
	if (!vcard) return ""
	for (const line of vcard.split("\n")) {
		if (line.startsWith("TEL")) {
			const i = line.indexOf(":")
			if (i >= 0) return line.slice(i + 1).trim()
		}
	}
	return ""
}

/** Shared contact card(s) with a save-as-.vcf action for single contacts. */
export function ContactBubble({ contact }: { contact: ContactMeta }) {
	const contacts = contact.contacts?.length ? contact.contacts : [{ displayName: contact.displayName || "Contact" }]
	const saveVcard = (entry: { displayName: string; vcard?: string }) => {
		const vcard = entry.vcard || `BEGIN:VCARD\r\nVERSION:3.0\r\nFN:${entry.displayName}\r\nEND:VCARD\r\n`
		const blob = new Blob([vcard], { type: "text/vcard" })
		const url = URL.createObjectURL(blob)
		const a = document.createElement("a")
		a.href = url
		a.download = `${entry.displayName.replace(/[^\w\s-]/g, "") || "contact"}.vcf`
		a.click()
		URL.revokeObjectURL(url)
	}
	return (
		<div className="flex flex-col gap-2 min-w-[220px] sm:min-w-[260px]">
			{contacts.map((c, i) => (
				<div key={i} className="flex items-center gap-3">
					<div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
						<User className="h-5 w-5 text-primary" />
					</div>
					<div className="min-w-0 flex-1">
						<p className="text-[13px] font-bold truncate">{c.displayName}</p>
						{vcardPhone(c.vcard) && (
							<p className="text-[11px] opacity-60 flex items-center gap-1">
								<Phone className="h-3 w-3" />
								{vcardPhone(c.vcard)}
							</p>
						)}
					</div>
					{contacts.length === 1 && (
						<button
							onClick={(e) => {
								e.stopPropagation()
								saveVcard(c)
							}}
							className="p-2 rounded-full hover:bg-black/10 dark:hover:bg-white/10 transition-colors"
							title="Save contact"
						>
							<Download className="h-4 w-4" />
						</button>
					)}
				</div>
			))}
		</div>
	)
}

/** Link preview card rendered above the text of messages carrying one. */
export function LinkPreviewCard({ preview, getMediaUrl }: {
	preview: LinkPreviewMeta
	getMediaUrl: (url: string) => string
}) {
	return (
		<a
			href={preview.url}
			target="_blank"
			rel="noopener noreferrer"
			onClick={(e) => e.stopPropagation()}
			className="flex gap-3 items-start rounded-xl overflow-hidden bg-black/5 dark:bg-white/5 mb-1 -mx-1 p-2 hover:bg-black/10 dark:hover:bg-white/10 transition-colors"
		>
			{preview.thumbnailUrl && (
				<img
					src={getMediaUrl(preview.thumbnailUrl)}
					alt=""
					className="w-[68px] h-[68px] object-cover rounded-lg shrink-0"
					loading="lazy"
				/>
			)}
			<div className="min-w-0 flex flex-col">
				<span className="text-[12px] font-bold truncate">{preview.title || preview.url}</span>
				{preview.description && <span className="text-[11px] opacity-70 line-clamp-2">{preview.description}</span>}
				<span className="text-[10px] opacity-50 truncate mt-0.5">{preview.url}</span>
			</div>
		</a>
	)
}

/** "View once" strip above view-once media; tracks opened state locally. */
export function ViewOnceStrip({ meta, type }: { meta: ViewOnceMeta; type: string }) {
	const [opened, setOpened] = useState(meta.viewed)
	return (
		<button
			onClick={(e) => {
				e.stopPropagation()
				setOpened(true)
			}}
			className="flex items-center gap-1.5 text-[11px] italic opacity-70 mb-1"
		>
			<Eye className="h-3 w-3" />
			{opened ? "Viewed" : `View once · ${type === "video" ? "video" : "photo"}`}
		</button>
	)
}
