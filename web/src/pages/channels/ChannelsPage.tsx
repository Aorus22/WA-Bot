import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { api, type Message } from "@/lib/api"
import { subscribeWS } from "@/lib/ws-bus"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"
import { toast } from "sonner"
import { Megaphone, Eye, Search, Volume2, VolumeX, UserMinus, Plus, ArrowLeft } from "lucide-react"
import { ChatMessageItem } from "@/pages/chat/ChatMessageItem"
import { renderFormattedContent } from "@/pages/chat/renderMd"

type Channel = {
	jid: string
	name: string
	description?: string
	subscribers: number
	inviteCode?: string
	avatar?: string
	muted: boolean
	verified: boolean
}

type ChannelPost = {
	id: string
	serverId: number
	type: string
	timestamp: number
	viewsCount: number
	reactions?: Record<string, number>
	content?: string
	mediaType?: string
}

const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"]

/** Frontend ordering guard: channel posts must render oldest -> newest. */
function sortPostsAsc(posts: ChannelPost[]): ChannelPost[] {
	return [...posts].sort((a, b) => a.serverId - b.serverId)
}

export default function ChannelsPage() {
	const [channels, setChannels] = useState<Channel[]>([])
	const [loading, setLoading] = useState(true)
	const [discoverOpen, setDiscoverOpen] = useState(false)
	const [selected, setSelected] = useState<Channel | null>(null)

	const refresh = useCallback(async () => {
		try {
			const list = (await api.listChannels()) || []
			setChannels(list)
			setSelected((prev) => (prev ? list.find((c) => c.jid === prev.jid) || prev : prev))
		} catch (err) {
			console.error("Failed to load channels:", err)
		} finally {
			setLoading(false)
		}
	}, [])

	useEffect(() => {
		refresh()
		const unsub = subscribeWS((msg) => {
			if (msg.type === "channels_changed") refresh()
		})
		return unsub
	}, [refresh])

	return (
		<div className="flex h-full max-h-full overflow-hidden">
			<aside className="w-full md:w-[340px] shrink-0 flex flex-col border-r border-border/40 bg-background">
				<div className="flex items-center justify-between p-5 pb-3">
					<h2 className="text-2xl font-bold tracking-tight">Channels</h2>
					<Button size="icon" variant="ghost" className="rounded-full" onClick={() => setDiscoverOpen(true)}>
						<Plus className="h-5 w-5" />
					</Button>
				</div>
				<div className="flex-1 overflow-y-auto px-3 pb-6">
					{loading ? (
						<p className="text-sm text-muted-foreground text-center py-10">Loading channels…</p>
					) : channels.length === 0 ? (
						<div className="flex flex-col items-center justify-center py-16 text-muted-foreground opacity-50">
							<Megaphone className="h-10 w-10 mb-2" />
							<p className="text-sm">No channels followed</p>
						</div>
					) : (
						<div className="space-y-1">
							{channels.map((ch) => (
								<button
									key={ch.jid}
									onClick={() => setSelected(ch)}
									className={cn(
										"w-full flex items-center gap-3 p-3 rounded-xl transition-colors text-left",
										selected?.jid === ch.jid ? "bg-primary/10" : "hover:bg-muted/50"
									)}
								>
									<Avatar className="h-11 w-11">
										<AvatarImage src={ch.avatar} />
										<AvatarFallback className="bg-primary/10 text-primary font-bold">
											{ch.name.charAt(0).toUpperCase()}
										</AvatarFallback>
									</Avatar>
									<div className="flex-1 min-w-0">
										<p className="text-sm font-bold truncate">
											{ch.name} {ch.verified && <span className="text-primary">✓</span>}
										</p>
										<p className="text-xs text-muted-foreground truncate">
											{ch.subscribers.toLocaleString()} subscribers{ch.muted ? " · muted" : ""}
										</p>
									</div>
								</button>
							))}
						</div>
					)}
					<div className="mt-4 px-1">
						<p className="text-xs font-bold text-muted-foreground uppercase tracking-wider mb-2">Find channels</p>
						<button
							onClick={() => setDiscoverOpen(true)}
							className="w-full flex items-center gap-3 p-3 rounded-xl hover:bg-muted/50 transition-colors text-left"
						>
							<div className="w-11 h-11 rounded-full bg-primary/10 flex items-center justify-center">
								<Search className="h-5 w-5 text-primary" />
							</div>
							<div className="flex-1 min-w-0">
								<p className="text-sm font-bold">Follow via link</p>
								<p className="text-xs text-muted-foreground">Paste a whatsapp.com/channel/… link</p>
							</div>
						</button>
					</div>
				</div>
			</aside>

			<main className="hidden md:flex flex-1 flex-col min-w-0">
				{selected ? (
					<ChannelConversation
						key={selected.jid}
						channel={selected}
						onChange={setSelected}
						onUnfollowed={() => {
							setSelected(null)
							refresh()
						}}
					/>
				) : (
					<div className="flex-1 flex flex-col items-center justify-center bg-muted/10">
						<div className="w-20 h-20 rounded-3xl bg-primary/5 flex items-center justify-center mb-6">
							<Megaphone className="h-10 w-10 text-primary/40" />
						</div>
						<h2 className="text-xl font-bold tracking-tight">Select a channel</h2>
						<p className="text-muted-foreground text-sm mt-1">Follow channels to see their posts here.</p>
					</div>
				)}
			</main>

			<DiscoverDialog open={discoverOpen} onOpenChange={setDiscoverOpen} onFollowed={refresh} />
		</div>
	)
}

/** Maps a channel post into the message shape ChatMessageItem renders. */
function postToMessage(post: ChannelPost, channel: Channel): Message {
	return {
		id: post.id,
		chatId: channel.jid,
		from: channel.jid,
		to: "me",
		content: post.content?.trim() || `[${(post.mediaType || "post").toUpperCase()}]`,
		timestamp: post.timestamp,
		status: "delivered",
		type: "text",
		senderName: channel.name,
		reactions: Object.entries(post.reactions || {}).map(([emoji, count]) => ({
			emoji,
			senders: Array.from({ length: count }, (_, i) => `r${i}`),
		})),
	}
}

function ChannelConversation({ channel, onChange, onUnfollowed }: {
	channel: Channel
	onChange: (c: Channel | null) => void
	onUnfollowed: () => void
}) {
	const [posts, setPosts] = useState<ChannelPost[]>([])
	const [loading, setLoading] = useState(true)
	const [loadingMore, setLoadingMore] = useState(false)
	const [hasMore, setHasMore] = useState(false)
	const [confirmLeave, setConfirmLeave] = useState(false)
	const scrollRef = useRef<HTMLDivElement>(null)

	const load = useCallback(async () => {
		try {
			const fresh = sortPostsAsc((await api.getChannelMessages(channel.jid, 30)) || [])
			setPosts(fresh)
			setHasMore(fresh.length === 30)
		} catch (err) {
			console.error("Failed to load channel posts:", err)
		} finally {
			setLoading(false)
		}
	}, [channel.jid])

	const loadMore = useCallback(async () => {
		if (loadingMore || posts.length === 0) return
		setLoadingMore(true)
		try {
			const older = sortPostsAsc((await api.getChannelMessages(channel.jid, 30, posts[posts.length - 1].serverId)) || [])
			setPosts((prev) => sortPostsAsc([...prev, ...older.filter((o) => !prev.some((p) => p.id === o.id))]))
			setHasMore(older.length === 30)
		} catch (err) {
			console.error("Failed to load older posts:", err)
		} finally {
			setLoadingMore(false)
		}
	}, [channel.jid, posts, loadingMore])

	useEffect(() => {
		load()
	}, [load])

	useEffect(() => {
		const unsub = subscribeWS((msg) => {
			if (msg.type === "channel_message" && msg.payload?.channelId === channel.jid) load()
			if (msg.type === "channel_update" && msg.payload?.channelId === channel.jid) load()
		})
		return unsub
	}, [channel.jid, load])

	useEffect(() => {
		if (posts.length > 0) scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight })
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [posts.length === 0])

	const react = async (post: ChannelPost, emoji: string) => {
		// Optimistic count bump; authoritative counts arrive via channel_update.
		setPosts((prev) =>
			prev.map((p) => {
				if (p.id !== post.id) return p
				const reactions = { ...(p.reactions || {}) }
				reactions[emoji] = (reactions[emoji] || 0) + 1
				return { ...p, reactions }
			})
		)
		try {
			await api.reactChannelMessage(channel.jid, post.serverId, post.id, emoji)
		} catch (err: any) {
			toast.error("Failed to react: " + err.message)
			load()
		}
	}

	const toggleMute = async () => {
		try {
			await api.setChannelMute(channel.jid, !channel.muted)
			onChange({ ...channel, muted: !channel.muted })
		} catch (err: any) {
			toast.error("Failed to update mute: " + err.message)
		}
	}

	const unfollow = async () => {
		try {
			await api.unfollowChannel(channel.jid)
			toast.success("Unfollowed")
			onUnfollowed()
		} catch (err: any) {
			toast.error("Failed to unfollow: " + err.message)
		} finally {
			setConfirmLeave(false)
		}
	}

	// Day grouping mirrors ChatArea's sticky date pills.
	const grouped = useMemo(() => {
		const groups: Array<{ dateKey: string; posts: ChannelPost[] }> = []
		for (const post of posts) {
			const key = new Date(post.timestamp).toLocaleDateString([], {
				weekday: "long", year: "numeric", month: "long", day: "numeric",
			})
			const last = groups[groups.length - 1]
			if (last && last.dateKey === key) last.posts.push(post)
			else groups.push({ dateKey: key, posts: [post] })
		}
		return groups
	}, [posts])

	return (
		<div className="flex-1 flex flex-col bg-background relative overflow-hidden">
			<header className="h-16 flex items-center justify-between px-4 border-b border-border/40 bg-background/80 backdrop-blur-xl z-20">
				<div className="flex items-center gap-3 min-w-0">
					<Button variant="ghost" size="icon" className="md:hidden rounded-full -ml-2" onClick={() => onChange(null)}>
						<ArrowLeft className="h-5 w-5" />
					</Button>
					<Avatar className="h-10 w-10 border-2 border-background shadow-sm">
						<AvatarImage src={channel.avatar} />
						<AvatarFallback className="bg-primary/10 text-primary font-bold">
							{channel.name.charAt(0).toUpperCase()}
						</AvatarFallback>
					</Avatar>
					<div className="flex flex-col min-w-0">
						<h3 className="font-bold text-base leading-tight tracking-tight truncate">
							{channel.name} {channel.verified && <span className="text-primary">✓</span>}
						</h3>
						<p className="text-[11px] font-medium text-muted-foreground truncate">
							{channel.subscribers.toLocaleString()} followers
						</p>
					</div>
				</div>
				<div className="flex items-center gap-1">
					<Button variant="ghost" size="icon" className="rounded-full text-muted-foreground hover:text-primary" onClick={toggleMute} title={channel.muted ? "Unmute" : "Mute"}>
						{channel.muted ? <VolumeX className="h-5 w-5" /> : <Volume2 className="h-5 w-5" />}
					</Button>
					<Button variant="ghost" size="icon" className="rounded-full text-muted-foreground hover:text-destructive" onClick={() => setConfirmLeave(true)} title="Unfollow">
						<UserMinus className="h-5 w-5" />
					</Button>
				</div>
			</header>

			<div className="flex-1 overflow-y-auto px-2 md:px-6 pt-3 pb-6" ref={scrollRef}>
				{loading ? (
					<p className="text-sm text-muted-foreground text-center py-10">Loading posts…</p>
				) : posts.length === 0 ? (
					<p className="text-sm text-muted-foreground text-center py-16 opacity-60">No posts yet</p>
				) : (
					<div className="space-y-8 max-w-3xl mx-auto">
						{loadingMore && (
							<div className="flex justify-center py-2">
								<div className="w-6 h-6 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
							</div>
						)}
						{hasMore && !loadingMore && (
							<div className="flex justify-center pb-2">
								<Button variant="outline" size="sm" onClick={loadMore}>Load older posts</Button>
							</div>
						)}
						{grouped.map((group) => (
							<div key={group.dateKey}>
								<div className="flex justify-center sticky top-0 z-20 py-2 pointer-events-none">
									<span className="text-[10px] font-bold text-muted-foreground/80 bg-background/60 backdrop-blur-md px-3 py-1 rounded-full border border-border/40 uppercase tracking-widest shadow-sm">
										{group.dateKey}
									</span>
								</div>
								<div className="space-y-2">
									{group.posts.map((post, idx) => {
										const msg = postToMessage(post, channel)
										const prev = group.posts[idx - 1]
										const next = group.posts[idx + 1]
										return (
											<div key={post.id} className="group/post">
												<ChatMessageItem
													message={msg}
													isMe={false}
													isFirstInSequence={!prev}
													isLastInSequence={!next}
													chat={{ isGroup: true, id: channel.jid, name: channel.name }}
													getMediaUrl={(url?: string) => url}
													getAvatarUrl={() => channel.avatar || api.mediaURL(`/avatar/${encodeURIComponent(channel.jid)}`)}
													formatTime={(ts: number) => new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
													renderFormattedContent={renderFormattedContent}
													isHighlighted={false}
													onReact={(emoji: string) => react(post, emoji)}
												/>
												<div className="flex items-center gap-3 px-14 -mt-1 mb-2 text-[10px] text-muted-foreground/70">
													<span className="flex items-center gap-1">
														<Eye className="h-3 w-3" /> {post.viewsCount.toLocaleString()}
													</span>
													<div className="flex gap-1.5">
														{QUICK_REACTIONS.filter((e) => !(post.reactions || {})[e]).map((emoji) => (
															<button
																key={emoji}
																onClick={() => react(post, emoji)}
																className="opacity-0 group-hover/post:opacity-60 hover:!opacity-100 transition-opacity"
															>
																{emoji}
															</button>
														))}
													</div>
												</div>
											</div>
										)
									})}
								</div>
							</div>
						))}
					</div>
				)}
			</div>

			<footer className="p-3 bg-background/80 backdrop-blur-xl border-t border-border/40">
				<p className="max-w-3xl mx-auto text-center text-xs text-muted-foreground">
					Channels are one-way — you can react to posts but not reply.
				</p>
			</footer>

			<Dialog open={confirmLeave} onOpenChange={setConfirmLeave}>
				<DialogContent className="sm:max-w-sm">
					<DialogHeader>
						<DialogTitle>Unfollow {channel.name}?</DialogTitle>
					</DialogHeader>
					<DialogFooter>
						<Button variant="outline" onClick={() => setConfirmLeave(false)}>Cancel</Button>
						<Button variant="destructive" onClick={unfollow}>Unfollow</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</div>
	)
}

function DiscoverDialog({ open, onOpenChange, onFollowed }: {
	open: boolean
	onOpenChange: (open: boolean) => void
	onFollowed: () => void
}) {
	const [link, setLink] = useState("")
	const [preview, setPreview] = useState<Channel | null>(null)
	const [busy, setBusy] = useState(false)

	const doPreview = async () => {
		if (!link.trim()) return
		setBusy(true)
		try {
			const p = await api.previewChannel(link.trim())
			setPreview({ jid: p.jid, name: p.name, description: p.description, subscribers: p.subscribers, muted: false, verified: false })
		} catch {
			setPreview(null)
			toast.error("Invalid channel link")
		} finally {
			setBusy(false)
		}
	}

	const follow = async () => {
		setBusy(true)
		try {
			await api.followChannel(link.trim())
			toast.success("Channel followed")
			setLink("")
			setPreview(null)
			onOpenChange(false)
			onFollowed()
		} catch (err: any) {
			toast.error("Failed to follow: " + err.message)
		} finally {
			setBusy(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Find channel</DialogTitle>
				</DialogHeader>
				<Input
					placeholder="https://whatsapp.com/channel/…"
					value={link}
					onChange={(e) => {
						setLink(e.target.value)
						setPreview(null)
					}}
					autoFocus
				/>
				<Button variant="outline" size="sm" onClick={doPreview} disabled={busy || !link.trim()}>Preview</Button>
				{preview && (
					<div className="rounded-lg bg-muted/50 p-3 text-sm">
						<p className="font-bold">{preview.name}</p>
						<p className="text-xs text-muted-foreground">{preview.subscribers.toLocaleString()} subscribers</p>
					</div>
				)}
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={follow} disabled={busy || !link.trim()}>Follow</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}
