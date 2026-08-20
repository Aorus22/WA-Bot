import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"
import {
	Phone,
	Video,
	PhoneMissed,
	PhoneOff,
	ArrowDownLeft,
	ArrowUpRight,
	Search,
	RefreshCw,
	MessageSquare,
	ChevronDown,
	X,
	Loader2,
} from "lucide-react"
import { api, type CallLog, type CallType } from "@/lib/api"
import { useCall } from "@/contexts/CallContext"
import { subscribeWS } from "@/lib/ws-bus"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import {
	Drawer,
	DrawerContent,
	DrawerHeader,
	DrawerTitle,
	DrawerDescription,
	DrawerFooter,
} from "@/components/ui/drawer"
import { cn } from "@/lib/utils"

const PAGE_SIZE = 25

type FilterKey = "all" | "incoming" | "outgoing" | "missed"

const FILTERS: { key: FilterKey; label: string }[] = [
	{ key: "all", label: "All" },
	{ key: "incoming", label: "Incoming" },
	{ key: "outgoing", label: "Outgoing" },
	{ key: "missed", label: "Missed" },
]

const MISSED_STATUSES = new Set(["missed", "rejected", "failed"])

/* ── Helpers ─────────────────────────────────────────────────────────── */

function isMissed(log: CallLog): boolean {
	return MISSED_STATUSES.has(log.status)
}

function displayName(log: CallLog): string {
	if (log.group_jid) return log.group_jid
	const target = log.target || "Unknown"
	// Strip the @s.whatsapp.net / @g.us suffix for a cleaner label
	return target.split("@")[0] || target
}

function initials(log: CallLog): string {
	const name = displayName(log)
	const parts = name.split(/[\s@._-]+/).filter(Boolean)
	const first = parts[0]?.[0] ?? "?"
	const second = parts[1]?.[0] ?? ""
	return (first + second).toUpperCase()
}

function formatDuration(ms?: number | null): string {
	if (!ms || ms <= 0) return "—"
	const totalSeconds = Math.floor(ms / 1000)
	const m = Math.floor(totalSeconds / 60)
	const s = totalSeconds % 60
	return `${m}:${s.toString().padStart(2, "0")}`
}

function isToday(ts: number): boolean {
	return new Date(ts).toDateString() === new Date().toDateString()
}

function isYesterday(ts: number): boolean {
	const d = new Date(ts)
	const y = new Date()
	y.setDate(y.getDate() - 1)
	return d.toDateString() === y.toDateString()
}

function formatTime(ts: number): string {
	return new Date(ts).toLocaleTimeString("en-US", {
		hour: "2-digit",
		minute: "2-digit",
	})
}

function formatDate(ts: number): string {
	return new Date(ts).toLocaleDateString("en-US", {
		day: "numeric",
		month: "short",
		year: "numeric",
	})
}

function groupLabel(ts: number): string {
	if (isToday(ts)) return "Today"
	if (isYesterday(ts)) return "Yesterday"
	return formatDate(ts)
}

function formatFullDateTime(ts: number): string {
	return new Date(ts).toLocaleString("en-US", {
		day: "numeric",
		month: "short",
		year: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	})
}

function statusLabel(status: string): string {
	return status.charAt(0).toUpperCase() + status.slice(1).replace(/_/g, " ")
}

/* ── Detail panel ────────────────────────────────────────────────────── */

function DetailRow({ label, value }: { label: string; value: string }) {
	return (
		<div className="flex items-start justify-between gap-4 py-2.5">
			<span className="text-sm text-muted-foreground shrink-0">{label}</span>
			<span className="text-sm font-medium text-foreground text-right break-all">{value || "—"}</span>
		</div>
	)
}

function CallDetail({
	log,
	onClose,
}: {
	log: CallLog
	onClose: () => void
}) {
	const navigate = useNavigate()
	const { startCall } = useCall()
	const missed = isMissed(log)
	const isVideo = log.call_type === "video" || log.call_type === "group_video"
	const isGroup = log.call_type === "group_audio" || log.call_type === "group_video"

	const handleCall = () => {
		if (log.group_jid || isGroup) {
			toast.info("Group calls from history aren't supported yet")
			return
		}
		const type: CallType = isVideo ? "video" : "audio"
		startCall(log.target, type).catch(() => toast.error("Failed to start call"))
		onClose()
	}

	const handleChat = () => {
		if (log.target) navigate(`/chat/${encodeURIComponent(log.target)}`)
		onClose()
	}

	return (
		<div className="flex flex-col h-full">
			{/* Hero */}
			<div className="flex flex-col items-center gap-3 pt-6 pb-5 px-6 text-center">
				<div
					className={cn(
						"relative flex items-center justify-center rounded-full",
						missed ? "bg-destructive/10" : "bg-primary/10",
						isVideo ? "h-20 w-20" : "h-16 w-16"
					)}
				>
					{missed ? (
						<PhoneMissed className="h-8 w-8 text-destructive" />
					) : (
						<Avatar className="h-16 w-16">
							<AvatarFallback className="bg-primary/10 text-primary font-bold text-xl">
								{initials(log)}
							</AvatarFallback>
						</Avatar>
					)}
					{isVideo && !missed && (
						<div className="absolute -bottom-1 -right-1 bg-primary text-primary-foreground rounded-full p-1.5 ring-2 ring-background">
							<Video className="h-3.5 w-3.5" />
						</div>
					)}
				</div>

				<div className="space-y-1">
					<h3 className="text-lg font-bold tracking-tight">{displayName(log)}</h3>
					<p
						className={cn(
							"text-sm font-medium",
							missed ? "text-destructive" : "text-muted-foreground"
						)}
					>
						{missed ? "Missed call" : log.direction === "incoming" ? "Incoming call" : "Outgoing call"}
						{isVideo ? " · Video" : " · Voice"}
					</p>
				</div>

				{/* Actions */}
				<div className="flex items-center gap-2 mt-2">
					<Button variant="outline" size="sm" onClick={handleChat} disabled={!log.target}>
						<MessageSquare className="h-4 w-4" />
						View chat
					</Button>
					<Button
						size="sm"
						variant={missed ? "default" : "secondary"}
						onClick={handleCall}
						disabled={!!log.group_jid || isGroup}
					>
						<Phone className="h-4 w-4" />
						Call back
					</Button>
				</div>
			</div>

			{/* Details */}
			<div className="border-t border-border/40 px-6 py-4 space-y-1">
				<DetailRow label="Status" value={statusLabel(log.status)} />
				<DetailRow label="Direction" value={log.direction} />
				<DetailRow label="Type" value={log.call_type} />
				<DetailRow label="Duration" value={formatDuration(log.duration_ms)} />
				<DetailRow label="Started" value={formatFullDateTime(log.started_at)} />
				{log.ended_at != null && <DetailRow label="Ended" value={formatFullDateTime(log.ended_at)} />}
				<DetailRow label="Target" value={log.target} />
				{log.group_jid && <DetailRow label="Group" value={log.group_jid} />}
				{log.error_message && <DetailRow label="Error" value={log.error_message} />}
				<DetailRow label="ID" value={log.id} />
			</div>
		</div>
	)
}

/* ── Row ─────────────────────────────────────────────────────────────── */

function CallRow({
	log,
	selected,
	onSelect,
}: {
	log: CallLog
	selected: boolean
	onSelect: (log: CallLog) => void
}) {
	const missed = isMissed(log)
	const isVideo = log.call_type === "video" || log.call_type === "group_video"
	const isGroup = log.call_type === "group_audio" || log.call_type === "group_video"

	return (
		<button
			data-call-item
			onClick={() => onSelect(log)}
			className={cn(
				"w-full flex items-center gap-4 p-3.5 rounded-2xl transition-all duration-200 group text-left relative",
				selected ? "bg-primary/10 shadow-sm" : "hover:bg-muted/50 active:scale-[0.98]"
			)}
		>
			{selected && (
				<div className="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-8 bg-primary rounded-r-full" />
			)}

			{/* Direction / missed indicator */}
			<div
				className={cn(
					"relative flex-shrink-0 flex items-center justify-center rounded-full",
					missed ? "bg-destructive/10" : "bg-muted",
					isVideo ? "h-12 w-12" : "h-11 w-11"
				)}
			>
				{missed ? (
					<PhoneMissed className="h-5 w-5 text-destructive" />
				) : log.direction === "incoming" ? (
					<ArrowDownLeft className="h-5 w-5 text-primary" />
				) : (
					<ArrowUpRight className="h-5 w-5 text-primary" />
				)}
				{isVideo && !missed && (
					<div className="absolute -bottom-0.5 -right-0.5 bg-background rounded-full p-0.5 ring-1 ring-border/40">
						<Video className="h-3 w-3 text-muted-foreground" />
					</div>
				)}
			</div>

			{/* Name + meta */}
			<div className="flex-1 min-w-0 grid grid-cols-[1fr_auto] gap-3 items-center">
				<div className="min-w-0 flex flex-col justify-center h-full gap-0.5">
					<h3 className="font-bold truncate group-hover:text-primary transition-colors text-foreground">
						{displayName(log)}
					</h3>
					<p
						className={cn(
							"text-sm truncate flex items-center gap-1.5",
							missed ? "text-destructive font-medium" : "text-muted-foreground/80"
						)}
					>
						{missed ? (
							<>
								<PhoneOff className="h-3 w-3 shrink-0" />
								Missed
							</>
						) : (
							<>
								{log.direction === "incoming" ? (
									<ArrowDownLeft className="h-3 w-3 shrink-0" />
								) : (
									<ArrowUpRight className="h-3 w-3 shrink-0" />
								)}
								{isVideo ? "Video" : "Voice"}
							</>
						)}
						{!isGroup && <span className="text-muted-foreground/50">·</span>}
						{!isGroup && <span className="tabular-nums">{formatDuration(log.duration_ms)}</span>}
						{isGroup && (
							<span className="text-muted-foreground/50">· {log.participants?.length ?? 0} people</span>
						)}
					</p>
				</div>

				{/* Timestamp */}
				<div className="flex flex-col items-end justify-between shrink-0 self-stretch py-0.5 min-w-[52px]">
					<span className="text-[11px] font-medium uppercase tracking-tighter text-muted-foreground/70">
						{isToday(log.started_at) ? formatTime(log.started_at) : formatDate(log.started_at)}
					</span>
					{missed && <div className="h-2 w-2 rounded-full bg-destructive" />}
				</div>
			</div>
		</button>
	)
}

/* ── Main page ───────────────────────────────────────────────────────── */

export function CallHistoryPage() {
	const [logs, setLogs] = useState<CallLog[]>([])
	const [loading, setLoading] = useState(true)
	const [loadingMore, setLoadingMore] = useState(false)
	const [error, setError] = useState<string | null>(null)
	const [hasMore, setHasMore] = useState(true)
	const [filter, setFilter] = useState<FilterKey>("all")
	const [search, setSearch] = useState("")
	const [selected, setSelected] = useState<CallLog | null>(null)

	const filterRef = useRef(filter)
	filterRef.current = filter

	// Initial load + filter change
	const load = useCallback(
		async (reset = true) => {
			const f = filterRef.current
			const apiFilter: { limit?: number; before?: number; direction?: string; status?: string } = {
				limit: PAGE_SIZE,
			}
			if (f === "incoming" || f === "outgoing") apiFilter.direction = f
			if (f === "missed") apiFilter.status = "missed"

			if (reset) setLoading(true)
			else setLoadingMore(true)
			setError(null)

			try {
				const res = await api.getCallHistory(apiFilter)
				const fetched = res.logs ?? []
				setLogs((prev) => (reset ? fetched : [...prev, ...fetched]))
				setHasMore(fetched.length >= PAGE_SIZE)
			} catch (e) {
				if (reset) setError(e instanceof Error ? e.message : "Failed to load call history")
				else toast.error("Failed to load more calls")
			} finally {
				setLoading(false)
				setLoadingMore(false)
			}
		},
		[]
	)

	useEffect(() => {
		load(true)
	}, [load, filter])

	// Auto-refresh via WS (call.ended triggers getHistory in CallContext)
	useEffect(() => {
		const unsub = subscribeWS((msg) => {
			if (msg.type === "call.ended") {
				load(true).catch(() => {})
			}
		})
		return unsub
	}, [load])

	// Load more (pagination with before = last startedAt)
	const loadMore = useCallback(() => {
		if (loadingMore || !hasMore || logs.length === 0) return
		const last = logs[logs.length - 1]
		const f = filterRef.current
		const apiFilter: { limit?: number; before?: number; direction?: string; status?: string } = {
			limit: PAGE_SIZE,
			before: last.started_at,
		}
		if (f === "incoming" || f === "outgoing") apiFilter.direction = f
		if (f === "missed") apiFilter.status = "missed"

		setLoadingMore(true)
		api
			.getCallHistory(apiFilter)
			.then((res) => {
				const fetched = res.logs ?? []
				setLogs((prev) => [...prev, ...fetched])
				setHasMore(fetched.length >= PAGE_SIZE)
			})
			.catch(() => toast.error("Failed to load more calls"))
			.finally(() => setLoadingMore(false))
	}, [loadingMore, hasMore, logs])

	// Infinite scroll
	const handleScroll = useCallback(
		(e: React.UIEvent<HTMLDivElement>) => {
			const el = e.currentTarget
			if (el.scrollTop + el.clientHeight >= el.scrollHeight - 200) {
				loadMore()
			}
		},
		[loadMore]
	)

	// Filtered + searched (client-side)
	const visibleLogs = useMemo(() => {
		let list = logs
		if (filter === "missed") list = list.filter(isMissed)
		if (search.trim()) {
			const q = search.trim().toLowerCase()
			list = list.filter(
				(l) =>
					displayName(l).toLowerCase().includes(q) ||
					l.target.toLowerCase().includes(q) ||
					(l.group_jid ?? "").toLowerCase().includes(q)
			)
		}
		return list
	}, [logs, filter, search])

	// Group by date
	const groups = useMemo(() => {
		const map = new Map<string, CallLog[]>()
		for (const log of visibleLogs) {
			const key = groupLabel(log.started_at)
			const arr = map.get(key) ?? []
			arr.push(log)
			map.set(key, arr)
		}
		return Array.from(map.entries())
	}, [visibleLogs])

	const totalCount = logs.length

	return (
		<div className="flex flex-col h-full max-h-full bg-background overflow-hidden">
			{/* Header */}
			<div className="flex flex-col p-5 space-y-4 shrink-0">
				<div className="flex items-center justify-between">
					<div className="flex items-baseline gap-2.5">
						<h2 className="text-2xl font-bold tracking-tight">Calls</h2>
						<span className="text-sm text-muted-foreground">{totalCount}</span>
					</div>
					{error && (
						<Button variant="ghost" size="sm" onClick={() => load(true)}>
							<RefreshCw className="h-4 w-4" />
							Retry
						</Button>
					)}
				</div>

				{/* Search */}
				<div className="relative group">
					<Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground transition-colors group-focus-within:text-primary" />
					<Input
						placeholder="Search calls"
						value={search}
						onChange={(e) => setSearch(e.target.value)}
						className="pl-10 h-10 bg-muted/50 border-none focus-visible:ring-1 focus-visible:ring-primary/20 rounded-xl"
					/>
				</div>

				{/* Filter chips */}
				<div className="flex items-center gap-2 overflow-x-auto hide-scrollbar -mx-1 px-1">
					{FILTERS.map((f) => (
						<button
							key={f.key}
							onClick={() => setFilter(f.key)}
							className={cn(
								"px-3.5 py-1.5 rounded-full text-sm font-semibold transition-all duration-200 shrink-0",
								filter === f.key
									? "bg-primary text-primary-foreground shadow-sm"
									: "bg-muted/60 text-muted-foreground hover:bg-muted hover:text-foreground"
							)}
						>
							{f.label}
						</button>
					))}
				</div>
			</div>

			{/* Body */}
			<div className="flex-1 min-h-0 flex">
				{/* List */}
				<div
					data-sidebar-scroll
					onScroll={handleScroll}
					className={cn(
						"flex-1 overflow-y-auto px-2 min-w-0",
						selected && "hidden md:block"
					)}
				>
					{loading ? (
						<div className="space-y-2 p-1">
							{Array.from({ length: 8 }).map((_, i) => (
								<div key={i} className="flex items-center gap-4 p-3.5">
									<Skeleton className="h-11 w-11 rounded-full" />
									<div className="flex-1 space-y-2">
										<Skeleton className="h-4 w-1/3" />
										<Skeleton className="h-3 w-1/2" />
									</div>
									<Skeleton className="h-3 w-10" />
								</div>
							))}
						</div>
					) : error && logs.length === 0 ? (
						<div className="flex flex-col items-center justify-center py-16 px-6 text-center gap-3">
							<div className="w-14 h-14 bg-destructive/10 rounded-full flex items-center justify-center">
								<PhoneOff className="h-7 w-7 text-destructive" />
							</div>
							<div className="space-y-1">
								<p className="text-sm font-semibold">Couldn't load calls</p>
								<p className="text-xs text-muted-foreground">{error}</p>
							</div>
							<Button variant="outline" size="sm" onClick={() => load(true)}>
								<RefreshCw className="h-4 w-4" />
								Try again
							</Button>
						</div>
					) : visibleLogs.length === 0 ? (
						<div className="flex flex-col items-center justify-center py-16 px-6 text-center gap-3 opacity-70">
							<div className="w-14 h-14 bg-muted rounded-full flex items-center justify-center">
								{search ? (
									<Search className="h-6 w-6 text-muted-foreground" />
								) : (
									<PhoneOff className="h-6 w-6 text-muted-foreground" />
								)}
							</div>
							<div className="space-y-1">
								<p className="text-sm font-semibold">
									{search ? "No results found" : "No calls yet"}
								</p>
								<p className="text-xs text-muted-foreground">
									{search
										? `Nothing matches "${search}"`
										: "Your call history will appear here."}
								</p>
							</div>
						</div>
					) : (
						<div className="space-y-5 pb-4">
							{groups.map(([label, items]) => (
								<div key={label}>
									{/* Sticky group header */}
									<div className="sticky top-0 z-10 bg-background/95 backdrop-blur-sm px-3 pt-3 pb-1.5">
										<span className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
											{label}
										</span>
									</div>
									<div className="space-y-1">
										{items.map((log) => (
											<CallRow
												key={log.id}
												log={log}
												selected={selected?.id === log.id}
												onSelect={setSelected}
											/>
										))}
									</div>
								</div>
							))}

							{/* Load more */}
							{hasMore && (
								<div className="flex justify-center py-3">
									{loadingMore ? (
										<Loader2 className="h-5 w-5 text-muted-foreground animate-spin" />
									) : (
										<Button variant="ghost" size="sm" onClick={loadMore}>
											<ChevronDown className="h-4 w-4" />
											Load more
										</Button>
									)}
								</div>
							)}
						</div>
					)}
				</div>

				{/* Desktop detail panel */}
				{selected && (
					<div className="hidden md:flex w-[360px] shrink-0 border-l border-border/40 bg-background flex-col overflow-y-auto">
						<div className="flex items-center justify-between px-4 pt-3">
							<span className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
								Call details
							</span>
							<Button variant="ghost" size="icon-sm" onClick={() => setSelected(null)}>
								<X className="h-4 w-4" />
							</Button>
						</div>
						<CallDetail log={selected} onClose={() => setSelected(null)} />
					</div>
				)}
			</div>

			{/* Mobile bottom drawer */}
			<Drawer
				open={!!selected}
				onOpenChange={(open) => {
					if (!open) setSelected(null)
				}}
			>
				<DrawerContent className="max-h-[85dvh]">
					<DrawerHeader className="text-left">
						<DrawerTitle className="sr-only">Call details</DrawerTitle>
						<DrawerDescription className="sr-only">Details for the selected call</DrawerDescription>
					</DrawerHeader>
					<div className="overflow-y-auto pb-2">
						{selected && <CallDetail log={selected} onClose={() => setSelected(null)} />}
					</div>
					<DrawerFooter>
						<Button variant="outline" onClick={() => setSelected(null)}>
							Close
						</Button>
					</DrawerFooter>
				</DrawerContent>
			</Drawer>
		</div>
	)
}
