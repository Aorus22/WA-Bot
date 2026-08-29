import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { api, type StatusEntry, type StatusGroup } from "@/lib/api"
import { subscribeWS } from "@/lib/ws-bus"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Textarea } from "@/components/ui/textarea"
import { cn } from "@/lib/utils"
import { toast } from "sonner"
import { Plus, ChevronLeft, ChevronRight, ImagePlus, Type, CircleDashed } from "lucide-react"

const BACKGROUNDS = [0xff075e54, 0xff128c7e, 0xff777a77, 0xff2c3e50, 0xff6a3080, 0xffc43e00, 0xffd4a017, 0xff0e5a8a]

function argbHex(argb: number): string {
	return "#" + (argb & 0xffffff).toString(16).padStart(6, "0")
}

function formatStatusTime(ts: number): string {
	const d = new Date(ts)
	const now = new Date()
	if (d.toDateString() === now.toDateString()) {
		return `Today at ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
	}
	const yesterday = new Date(now)
	yesterday.setDate(now.getDate() - 1)
	if (d.toDateString() === yesterday.toDateString()) {
		return `Yesterday at ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
	}
	return d.toLocaleDateString([], { day: "numeric", month: "short" }) + ` at ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
}

const OWN_MARKER = "__own__"

export default function StatusPage() {
	const [groups, setGroups] = useState<StatusGroup[]>([])
	const [loading, setLoading] = useState(true)
	const [composerOpen, setComposerOpen] = useState(false)
	// viewing: group + index; own statuses are keyed as OWN_MARKER.
	const [viewing, setViewing] = useState<{ group: StatusGroup; index: number } | null>(null)

	const refresh = useCallback(async () => {
		try {
			setGroups((await api.listStatuses()) || [])
		} catch (err) {
			console.error("Failed to load statuses:", err)
		} finally {
			setLoading(false)
		}
	}, [])

	useEffect(() => {
		refresh()
		const unsub = subscribeWS((msg) => {
			if (msg.type === "status_new") refresh()
		})
		return unsub
	}, [refresh])

	const own = useMemo(() => groups.find((g) => g.name === "Status Saya" || g.name === "My status"), [groups])
	const recent = useMemo(
		() => groups.filter((g) => !(g.name === "Status Saya" || g.name === "My status")),
		[groups]
	)

	const openGroup = (group: StatusGroup) => {
		if (group.statuses.length === 0) {
			setComposerOpen(true)
			return
		}
		setViewing({ group, index: 0 })
	}

	return (
		<div className="flex h-full max-h-full overflow-hidden">
			{/* Left: status list */}
			<aside className="w-full md:w-[340px] shrink-0 flex flex-col border-r border-border/40 bg-background">
				<div className="flex items-center justify-between p-5 pb-3">
					<h2 className="text-2xl font-bold tracking-tight">Status</h2>
					<Button size="icon" variant="ghost" className="rounded-full" onClick={() => setComposerOpen(true)}>
						<Plus className="h-5 w-5" />
					</Button>
				</div>

				<div className="flex-1 overflow-y-auto px-3 pb-6">
					{loading && <p className="text-xs text-muted-foreground text-center pb-2 opacity-60">Memuat…</p>}
					{/* My status row */}
					<button
						onClick={() => openGroup(own || { sender: OWN_MARKER, allViewed: true, statuses: [], latestTime: 0 })}
						className="w-full flex items-center gap-3 p-3 rounded-xl hover:bg-muted/50 transition-colors text-left"
					>
						<div className="relative">
							<Avatar className="h-12 w-12">
								<AvatarFallback className="bg-primary/10 text-primary font-bold">
									<Type className="h-5 w-5" />
								</AvatarFallback>
							</Avatar>
							{!own && (
								<div className="absolute -bottom-0.5 -right-0.5 w-5 h-5 rounded-full bg-primary text-primary-foreground flex items-center justify-center border-2 border-background">
									<Plus className="h-3 w-3" />
								</div>
							)}
						</div>
						<div className="flex-1 min-w-0">
							<p className="text-sm font-bold">Status Saya</p>
							<p className="text-xs text-muted-foreground truncate">
								{own ? formatStatusTime(own.latestTime) : "Klik untuk tambah status"}
							</p>
						</div>
					</button>

					{recent.length > 0 && (
						<>
							<p className="text-xs font-bold text-muted-foreground uppercase tracking-wider px-3 pt-5 pb-2">Recent</p>
							<div className="space-y-1">
								{recent.map((group) => {
									const name = group.name || group.sender.split("@")[0]
									const unseen = !group.allViewed
									return (
										<button
											key={group.sender}
											onClick={() => openGroup(group)}
											className="w-full flex items-center gap-3 p-3 rounded-xl hover:bg-muted/50 transition-colors text-left"
										>
											<div className={cn("rounded-full p-[2px]", unseen ? "ring-2 ring-primary ring-offset-2 ring-offset-background" : "ring-2 ring-muted-foreground/30 ring-offset-2 ring-offset-background")}>
												<Avatar className="h-11 w-11">
													<AvatarImage src={group.avatar || api.mediaURL(`/avatar/${encodeURIComponent(group.sender)}`)} />
													<AvatarFallback className="bg-primary/10 text-primary font-bold">
														{name.charAt(0).toUpperCase()}
													</AvatarFallback>
												</Avatar>
											</div>
											<div className="flex-1 min-w-0">
												<p className="text-sm font-bold truncate">{name}</p>
												<p className="text-xs text-muted-foreground">{formatStatusTime(group.latestTime)}</p>
											</div>
										</button>
									)
								})}
							</div>
						</>
					)}
				</div>
			</aside>

			{/* Right: placeholder or in-pane viewer */}
			<main className="hidden md:flex flex-1 flex-col min-w-0 bg-muted/10">
				{viewing ? (
					<StatusViewerPane
						key={viewing.group.sender + "-" + viewing.index}
						state={viewing}
						onClose={() => {
							setViewing(null)
							refresh()
						}}
						onNavigate={(index) => {
							if (index < 0 || index >= viewing.group.statuses.length) {
								setViewing(null)
								refresh()
								return
							}
							setViewing({ group: viewing.group, index })
						}}
					/>
				) : (
					<div className="flex-1 flex flex-col items-center justify-center gap-4 opacity-60">
						<CircleDashed className="h-16 w-16 text-muted-foreground animate-spin [animation-duration:3s]" />
						<h2 className="text-3xl font-bold tracking-tight">Bagikan status</h2>
						<p className="text-muted-foreground text-sm">
							Bagikan foto, video, dan teks yang hilang setelah 24 jam.
						</p>
					</div>
				)}
			</main>

			<StatusComposer open={composerOpen} onOpenChange={setComposerOpen} onPosted={refresh} />
		</div>
	)
}

/** In-pane story viewer with progress segments and click zones. */
function StatusViewerPane({ state, onClose, onNavigate }: {
	state: { group: StatusGroup; index: number }
	onClose: () => void
	onNavigate: (index: number) => void
}) {
	const { group, index } = state
	const entry: StatusEntry | undefined = group.statuses[index]
	const name = group.name || group.sender.split("@")[0]
	const isOwn = group.name === "Status Saya" || group.name === "My status"
	const advanceTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

	const mediaSrc = entry?.mediaUrl
		? api.mediaURL(entry.mediaUrl)
		: entry
			? api.statusMediaURL(entry.id)
			: ""

	useEffect(() => {
		if (!entry) return
		if (!entry.viewed && !isOwn) {
			api.markStatusViewed(entry.id).catch(() => {})
		}
		// Auto-advance still/text statuses like the official clients.
		if (entry.type !== "video") {
			advanceTimer.current = setTimeout(() => onNavigate(index + 1), 5000)
		}
		return () => {
			if (advanceTimer.current) clearTimeout(advanceTimer.current)
		}
	}, [entry, isOwn]) // eslint-disable-line react-hooks/exhaustive-deps

	if (!entry) return null

	return (
		<div className="flex-1 flex flex-col bg-[#0b141a] relative">
			{/* Progress segments */}
			<div className="absolute top-2 left-2 right-2 flex gap-1 z-20">
				{group.statuses.map((_, i) => (
					<div key={i} className="h-[3px] flex-1 rounded-full overflow-hidden bg-white/25">
						<div
							className={cn("h-full bg-white transition-all", i === index ? "w-full duration-5000 ease-linear" : i < index ? "w-full" : "w-0")}
						/>
					</div>
				))}
			</div>

			{/* Header */}
			<div className="flex items-center gap-3 p-4 pt-6 text-white z-10">
				<Avatar className="h-9 w-9">
					<AvatarFallback className="bg-primary/20 text-white text-xs font-bold">
						{name.charAt(0).toUpperCase()}
					</AvatarFallback>
				</Avatar>
				<div className="flex-1">
					<p className="text-sm font-bold">{name}</p>
					<p className="text-[11px] opacity-60">{formatStatusTime(entry.timestamp)}</p>
				</div>
			</div>

			{/* Content + click zones */}
			<div className="flex-1 flex items-center justify-center min-h-0 relative">
				{entry.type === "image" ? (
					<img src={mediaSrc} alt="" className="max-h-full max-w-full object-contain" />
				) : entry.type === "video" ? (
					<video
						src={mediaSrc}
						controls
						autoPlay
						className="max-h-full max-w-full object-contain"
						onEnded={() => onNavigate(index + 1)}
					/>
				) : (
					<div
						className="h-full w-full flex items-center justify-center p-10"
						style={{ backgroundColor: argbHex(0xff075e54) }}
					>
						<p className="text-white text-2xl font-medium text-center whitespace-pre-wrap break-words max-w-xl">
							{entry.content}
						</p>
					</div>
				)}

				<button
					className="absolute left-0 top-0 bottom-0 w-1/4 flex items-center justify-start pl-3 text-white/70 hover:text-white"
					onClick={() => onNavigate(index - 1)}
					aria-label="Previous"
				>
					{index > 0 && <ChevronLeft className="h-8 w-8" />}
				</button>
				<button
					className="absolute right-0 top-0 bottom-0 w-1/4 flex items-center justify-end pr-3 text-white/70 hover:text-white"
					onClick={() => onNavigate(index + 1)}
					aria-label="Next"
				>
					{index < group.statuses.length - 1 && <ChevronRight className="h-8 w-8" />}
				</button>
			</div>

			<div className="flex justify-end p-3">
				<Button variant="ghost" size="sm" className="text-white/70 hover:text-white hover:bg-white/10" onClick={onClose}>
					Tutup
				</Button>
			</div>
		</div>
	)
}

function StatusComposer({ open, onOpenChange, onPosted }: {
	open: boolean
	onOpenChange: (open: boolean) => void
	onPosted: () => void
}) {
	const [text, setText] = useState("")
	const [background, setBackground] = useState(BACKGROUNDS[0])
	const [busy, setBusy] = useState(false)

	const postText = async () => {
		if (!text.trim()) return
		setBusy(true)
		try {
			await api.postStatusText(text.trim(), background)
			toast.success("Status posted")
			setText("")
			onOpenChange(false)
			onPosted()
		} catch (err: any) {
			toast.error("Failed to post status: " + err.message)
		} finally {
			setBusy(false)
		}
	}

	const postFile = async (file: File) => {
		setBusy(true)
		try {
			const type = file.type.startsWith("video/") ? "video" : "image"
			await api.postStatusMedia(file, type)
			toast.success("Status posted")
			onOpenChange(false)
			onPosted()
		} catch (err: any) {
			toast.error("Failed to post status: " + err.message)
		} finally {
			setBusy(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>New status</DialogTitle>
				</DialogHeader>
				<div
					className="rounded-xl p-4 min-h-[140px] flex items-center justify-center"
					style={{ backgroundColor: argbHex(background) }}
				>
					<Textarea
						placeholder="Type a status…"
						value={text}
						onChange={(e) => setText(e.target.value)}
						className="bg-transparent border-none text-white placeholder:text-white/60 text-center text-lg focus-visible:ring-0"
						rows={4}
					/>
				</div>
				<div className="flex gap-2 flex-wrap">
					{BACKGROUNDS.map((c) => (
						<button
							key={c}
							onClick={() => setBackground(c)}
							className={cn("w-7 h-7 rounded-full border-2", background === c ? "border-primary scale-110" : "border-transparent")}
							style={{ backgroundColor: argbHex(c) }}
							aria-label="Background color"
						/>
					))}
				</div>
				<DialogFooter className="flex-row gap-2 sm:justify-between">
					<label className="cursor-pointer">
						<input
							type="file"
							accept="image/*,video/*"
							className="hidden"
							onChange={(e) => {
								const file = e.target.files?.[0]
								e.target.value = ""
								if (file) postFile(file)
							}}
						/>
						<span className="inline-flex items-center gap-1.5 px-3 py-2 rounded-lg border border-border text-sm font-medium hover:bg-muted transition-colors">
							<ImagePlus className="h-4 w-4" /> Media
						</span>
					</label>
					<Button onClick={postText} disabled={busy || !text.trim()}>Post</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}
