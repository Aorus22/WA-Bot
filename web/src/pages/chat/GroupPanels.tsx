import { useCallback, useEffect, useState } from "react"
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Switch } from "@/components/ui/switch"
import { api, type Chat, type GroupCache, type GroupParticipantInfo } from "@/lib/api"
import { useChatStore } from "@/stores/chatStore"
import { toast } from "sonner"
import { Copy, RefreshCw, ShieldCheck, UserMinus, Shield, ShieldOff, LogOut, Loader2, Users, Link as LinkIcon } from "lucide-react"

/** Members & settings tab for group chats (used inside ChatInfoSheetModal). */
export function GroupMembersTab({ chat, getAvatarUrl }: {
	chat: Chat
	getAvatarUrl: (target: string) => string | undefined
}) {
	const [group, setGroup] = useState<GroupCache | null>(null)
	const [loading, setLoading] = useState(true)

	const refresh = useCallback(async () => {
		setLoading(true)
		try {
			setGroup(await api.getGroup(chat.id))
		} catch (err) {
			console.error("Failed to load group:", err)
		} finally {
			setLoading(false)
		}
	}, [chat.id])

	useEffect(() => {
		refresh()
	}, [refresh])

	const isAdmin = group?.ownRole === "admin" || group?.ownRole === "superadmin"

	const patch = async (changes: Parameters<typeof api.updateGroup>[1]) => {
		try {
			const updated = await api.updateGroup(chat.id, changes)
			setGroup(updated)
		} catch (err: any) {
			toast.error("Failed to update group: " + err.message)
		}
	}

	const participantAction = async (action: string, jid: string) => {
		try {
			setGroup(await api.updateGroupParticipants(chat.id, action, [jid]))
		} catch (err: any) {
			toast.error("Failed to update member: " + err.message)
		}
	}

	const copyLink = async () => {
		try {
			const { link } = await api.getGroupInviteLink(chat.id)
			await navigator.clipboard.writeText(link)
			toast.success("Invite link copied")
		} catch (err: any) {
			toast.error("Failed to fetch invite link: " + err.message)
		}
	}

	const resetLink = async () => {
		try {
			await api.getGroupInviteLink(chat.id, true)
			toast.success("Previous link revoked")
		} catch (err: any) {
			toast.error("Failed to reset link: " + err.message)
		}
	}

	const leaveGroup = async () => {
		if (!window.confirm("Leave this group?")) return
		try {
			await api.leaveGroup(chat.id)
			toast.success("Left the group")
		} catch (err: any) {
			toast.error("Failed to leave group: " + err.message)
		}
	}

	if (loading && !group) {
		return (
			<div className="flex items-center justify-center h-full py-12 text-muted-foreground">
				<Loader2 className="h-6 w-6 animate-spin text-primary" />
			</div>
		)
	}
	if (!group) {
		return <div className="flex items-center justify-center h-full py-12 text-sm text-muted-foreground">Group info unavailable</div>
	}

	return (
		<div className="h-full overflow-y-auto p-4 space-y-4">
			{group.description && (
				<p className="text-sm text-muted-foreground whitespace-pre-wrap">{group.description}</p>
			)}

			<div className="flex gap-2">
				<Button variant="outline" size="sm" className="flex-1" onClick={copyLink}>
					<Copy className="h-3.5 w-3.5" /> Copy invite link
				</Button>
				{isAdmin && (
					<Button variant="outline" size="sm" onClick={resetLink} title="Revoke & regenerate">
						<RefreshCw className="h-3.5 w-3.5" />
					</Button>
				)}
			</div>

			{isAdmin && (
				<div className="space-y-3 rounded-xl border border-border/40 p-3">
					<label className="flex items-center justify-between text-sm">
						<span>Only admins can edit group info</span>
						<Switch checked={group.locked} onCheckedChange={(v) => patch({ locked: v })} />
					</label>
					<label className="flex items-center justify-between text-sm">
						<span>Only admins can send messages</span>
						<Switch checked={group.announce} onCheckedChange={(v) => patch({ announce: v })} />
					</label>
					<label className="flex items-center justify-between text-sm">
						<span>Approve new members</span>
						<Switch checked={group.joinApproval} onCheckedChange={(v) => patch({ joinApproval: v })} />
					</label>
				</div>
			)}

			<div>
				<p className="text-xs font-bold text-muted-foreground uppercase tracking-wide mb-2">
					{group.participantCount} members
				</p>
				<div className="space-y-1">
					{group.participants.map((p) => (
						<MemberRow
							key={p.jid}
							participant={p}
							group={group}
							getAvatarUrl={getAvatarUrl}
							onAction={participantAction}
						/>
					))}
				</div>
			</div>

			<Button variant="destructive" size="sm" className="w-full" onClick={leaveGroup}>
				<LogOut className="h-3.5 w-3.5" /> Leave group
			</Button>
		</div>
	)
}

function MemberRow({ participant, group, getAvatarUrl, onAction }: {
	participant: GroupParticipantInfo
	group: GroupCache
	getAvatarUrl: (target: string) => string | undefined
	onAction: (action: string, jid: string) => void
}) {
	const name = participant.name || participant.jid.split("@")[0]
	const isOwner = group.owner === participant.jid || participant.isSuperAdmin
	const isAdmin = group.ownRole === "admin" || group.ownRole === "superadmin"
	const canAct = isAdmin && !isOwner

	return (
		<div className="flex items-center gap-3 p-2 rounded-lg hover:bg-muted/50 transition-colors group">
			<Avatar className="h-9 w-9">
				<AvatarImage src={getAvatarUrl(participant.jid)} />
				<AvatarFallback className="bg-primary/10 text-primary text-xs font-bold">
					{name.charAt(0).toUpperCase()}
				</AvatarFallback>
			</Avatar>
			<div className="flex-1 min-w-0">
				<p className="text-sm font-medium truncate">{name}</p>
				{isOwner ? (
					<p className="text-[10px] text-muted-foreground flex items-center gap-1">
						<ShieldCheck className="h-3 w-3" /> Group owner
					</p>
				) : participant.isAdmin && (
					<p className="text-[10px] text-muted-foreground">Admin</p>
				)}
			</div>
			{canAct && (
				<div className="hidden group-hover:flex items-center gap-1">
					{participant.isAdmin ? (
						<Button variant="ghost" size="icon" className="h-7 w-7" title="Demote" onClick={() => onAction("demote", participant.jid)}>
							<ShieldOff className="h-3.5 w-3.5" />
						</Button>
					) : (
						<Button variant="ghost" size="icon" className="h-7 w-7" title="Make admin" onClick={() => onAction("promote", participant.jid)}>
							<Shield className="h-3.5 w-3.5" />
						</Button>
					)}
					<Button variant="ghost" size="icon" className="h-7 w-7 text-destructive" title="Remove" onClick={() => onAction("remove", participant.jid)}>
						<UserMinus className="h-3.5 w-3.5" />
					</Button>
				</div>
			)}
		</div>
	)
}

/** Create-group dialog with a contact picker built from existing 1:1 chats. */
export function NewGroupDialog({ open, onOpenChange }: {
	open: boolean
	onOpenChange: (open: boolean) => void
}) {
	const chats = useChatStore((s) => s.chats)
	const [name, setName] = useState("")
	const [selected, setSelected] = useState<Set<string>>(new Set())
	const [creating, setCreating] = useState(false)

	const contacts = chats.filter((c) => !c.isGroup && !c.archived)

	const create = async () => {
		if (!name.trim()) {
			toast.error("Group name is required")
			return
		}
		setCreating(true)
		try {
			await api.createGroup(name.trim(), [...selected])
			toast.success("Group created")
			setName("")
			setSelected(new Set())
			onOpenChange(false)
			useChatStore.getState().invalidateMessages()
			api.getChats().then((fresh) => useChatStore.getState().setChats(fresh || [])).catch(() => {})
		} catch (err: any) {
			toast.error("Failed to create group: " + err.message)
		} finally {
			setCreating(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>New group</DialogTitle>
					<DialogDescription>Pick contacts to add to the group.</DialogDescription>
				</DialogHeader>
				<Input placeholder="Group name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
				<div className="max-h-[280px] overflow-y-auto flex flex-col gap-1">
					{contacts.map((c) => {
						const active = selected.has(c.id)
						return (
							<button
								key={c.id}
								onClick={() => {
									const next = new Set(selected)
									if (active) next.delete(c.id)
									else next.add(c.id)
									setSelected(next)
								}}
								className={`flex items-center gap-3 p-2 rounded-lg text-left transition-colors ${active ? "bg-primary/10" : "hover:bg-muted"}`}
							>
								<Avatar className="h-8 w-8">
									<AvatarFallback className="bg-primary/10 text-primary text-xs font-bold">
										{(c.name || c.id).charAt(0).toUpperCase()}
									</AvatarFallback>
								</Avatar>
								<span className="flex-1 truncate text-sm font-medium">{c.name || c.id}</span>
								<div className={`flex h-5 w-5 items-center justify-center rounded border ${active ? "bg-primary border-primary text-primary-foreground" : "border-muted-foreground/40"}`}>
									{active ? "✓" : ""}
								</div>
							</button>
						)
					})}
				</div>
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={create} disabled={creating}>
						<Users className="h-4 w-4" /> Create ({selected.size})
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}

/** Join-group-via-link dialog with preview. */
export function JoinGroupDialog({ open, onOpenChange }: {
	open: boolean
	onOpenChange: (open: boolean) => void
}) {
	const [link, setLink] = useState("")
	const [preview, setPreview] = useState<{ name: string; participantCount: number } | null>(null)
	const [busy, setBusy] = useState(false)

	const doPreview = async () => {
		if (!link.trim()) return
		setBusy(true)
		try {
			const p = await api.previewGroupLink(link.trim())
			setPreview({ name: p.name, participantCount: p.participantCount })
		} catch {
			setPreview(null)
			toast.error("Invalid or revoked invite link")
		} finally {
			setBusy(false)
		}
	}

	const join = async () => {
		setBusy(true)
		try {
			await api.joinGroupWithLink(link.trim())
			toast.success("Joined group")
			setLink("")
			setPreview(null)
			onOpenChange(false)
			useChatStore.getState().invalidateMessages()
			api.getChats().then((fresh) => useChatStore.getState().setChats(fresh || [])).catch(() => {})
		} catch (err: any) {
			toast.error("Failed to join: " + err.message)
		} finally {
			setBusy(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Join group</DialogTitle>
					<DialogDescription>Paste an invite link (chat.whatsapp.com/…).</DialogDescription>
				</DialogHeader>
				<Input
					placeholder="https://chat.whatsapp.com/…"
					value={link}
					onChange={(e) => {
						setLink(e.target.value)
						setPreview(null)
					}}
					autoFocus
				/>
				<Button variant="outline" size="sm" onClick={doPreview} disabled={busy || !link.trim()}>
					<LinkIcon className="h-3.5 w-3.5" /> Preview
				</Button>
				{preview && (
					<div className="rounded-lg bg-muted/50 p-3 text-sm">
						<p className="font-bold">{preview.name}</p>
						<p className="text-xs text-muted-foreground">{preview.participantCount} members</p>
					</div>
				)}
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={join} disabled={busy || !link.trim()}>Join</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}
