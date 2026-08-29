import { useState } from "react"
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
import { Checkbox } from "@/components/ui/checkbox"
import { Plus, Trash2 } from "lucide-react"
import { api, type Chat, type Message } from "@/lib/api"
import { toast } from "sonner"

export function PollDialog({ open, onOpenChange, chatId }: {
	open: boolean
	onOpenChange: (open: boolean) => void
	chatId: string
}) {
	const [question, setQuestion] = useState("")
	const [options, setOptions] = useState<string[]>(["", ""])
	const [multiSelect, setMultiSelect] = useState(false)
	const [sending, setSending] = useState(false)

	const send = async () => {
		const cleaned = options.map((o) => o.trim()).filter(Boolean)
		if (!question.trim() || cleaned.length < 2) {
			toast.error("Poll needs a question and at least 2 options")
			return
		}
		setSending(true)
		try {
			await api.sendPoll(chatId, question.trim(), cleaned, multiSelect)
			setQuestion("")
			setOptions(["", ""])
			setMultiSelect(false)
			onOpenChange(false)
		} catch (err) {
			toast.error("Failed to send poll: " + (err as Error).message)
		} finally {
			setSending(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Create poll</DialogTitle>
					<DialogDescription>Ask a question with selectable options.</DialogDescription>
				</DialogHeader>
				<div className="flex flex-col gap-3">
					<Input
						placeholder="Question"
						value={question}
						onChange={(e) => setQuestion(e.target.value)}
						autoFocus
					/>
					<div className="flex flex-col gap-2">
						{options.map((opt, i) => (
							<div key={i} className="flex items-center gap-2">
								<Input
									placeholder={`Option ${i + 1}`}
									value={opt}
									onChange={(e) => setOptions(options.map((o, j) => (j === i ? e.target.value : o)))}
									onKeyDown={(e) => {
										if (e.key === "Enter") {
											e.preventDefault()
											send()
										}
									}}
								/>
								{options.length > 2 && (
									<Button
										variant="ghost"
										size="icon"
										onClick={() => setOptions(options.filter((_, j) => j !== i))}
									>
										<Trash2 className="h-4 w-4" />
									</Button>
								)}
							</div>
						))}
					</div>
					<Button
						variant="outline"
						size="sm"
						className="w-fit"
						disabled={options.length >= 12}
						onClick={() => setOptions([...options, ""])}
					>
						<Plus className="h-4 w-4" /> Add option
					</Button>
					<label className="flex items-center gap-2 text-sm">
						<Checkbox checked={multiSelect} onCheckedChange={(v) => setMultiSelect(Boolean(v))} />
						Allow multiple answers
					</label>
				</div>
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={send} disabled={sending}>Send</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}

export function LocationDialog({ open, onOpenChange, chatId }: {
	open: boolean
	onOpenChange: (open: boolean) => void
	chatId: string
}) {
	const [lat, setLat] = useState("")
	const [lng, setLng] = useState("")
	const [name, setName] = useState("")
	const [address, setAddress] = useState("")
	const [live, setLive] = useState(false)
	const [sending, setSending] = useState(false)

	const send = async () => {
		const latitude = parseFloat(lat)
		const longitude = parseFloat(lng)
		if (!Number.isFinite(latitude) || !Number.isFinite(longitude)) {
			toast.error("Enter valid coordinates")
			return
		}
		setSending(true)
		try {
			await api.sendLocation(chatId, latitude, longitude, name.trim(), address.trim(), live)
			setLat("")
			setLng("")
			setName("")
			setAddress("")
			setLive(false)
			onOpenChange(false)
		} catch (err) {
			toast.error("Failed to share location: " + (err as Error).message)
		} finally {
			setSending(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Share location</DialogTitle>
					<DialogDescription>Share coordinates as a pin or a live location.</DialogDescription>
				</DialogHeader>
				<div className="flex flex-col gap-3">
					<div className="flex gap-2">
						<Input placeholder="Latitude (e.g. -6.2088)" value={lat} onChange={(e) => setLat(e.target.value)} />
						<Input placeholder="Longitude (e.g. 106.8456)" value={lng} onChange={(e) => setLng(e.target.value)} />
					</div>
					<Input placeholder="Name (optional)" value={name} onChange={(e) => setName(e.target.value)} />
					<Input placeholder="Address (optional)" value={address} onChange={(e) => setAddress(e.target.value)} />
					<label className="flex items-center gap-2 text-sm">
						<Checkbox checked={live} onCheckedChange={(v) => setLive(Boolean(v))} />
						Share live location
					</label>
				</div>
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={send} disabled={sending}>Share</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}

export function ContactDialog({ open, onOpenChange, chatId }: {
	open: boolean
	onOpenChange: (open: boolean) => void
	chatId: string
}) {
	const [name, setName] = useState("")
	const [phone, setPhone] = useState("")
	const [sending, setSending] = useState(false)

	const send = async () => {
		if (!name.trim() || !phone.trim()) {
			toast.error("Name and phone number are required")
			return
		}
		setSending(true)
		try {
			await api.sendContact(chatId, name.trim(), phone.trim())
			setName("")
			setPhone("")
			onOpenChange(false)
		} catch (err) {
			toast.error("Failed to share contact: " + (err as Error).message)
		} finally {
			setSending(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Share contact</DialogTitle>
					<DialogDescription>Send a contact card to this chat.</DialogDescription>
				</DialogHeader>
				<div className="flex flex-col gap-3">
					<Input placeholder="Contact name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
					<Input placeholder="Phone number (e.g. +62812…)" value={phone} onChange={(e) => setPhone(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Enter") {
								e.preventDefault()
								send()
							}
						}}
					/>
				</div>
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={send} disabled={sending}>Share</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}

export function ForwardDialog({ open, onOpenChange, message, chats, currentChatId, onForwarded }: {
	open: boolean
	onOpenChange: (open: boolean) => void
	message: Message | null
	chats: Chat[]
	currentChatId: string
	onForwarded?: () => void
}) {
	const [selected, setSelected] = useState<Set<string>>(new Set())
	const [sending, setSending] = useState(false)

	const toggle = (id: string) => {
		setSelected((prev) => {
			const next = new Set(prev)
			if (next.has(id)) next.delete(id)
			else if (next.size < 5) next.add(id)
			return next
		})
	}

	const send = async () => {
		if (!message || selected.size === 0) return
		setSending(true)
		try {
			await api.forwardMessage(currentChatId, message.id, [...selected])
			toast.success(`Forwarded to ${selected.size} chat${selected.size === 1 ? "" : "s"}`)
			setSelected(new Set())
			onOpenChange(false)
			onForwarded?.()
		} catch (err) {
			toast.error("Failed to forward: " + (err as Error).message)
		} finally {
			setSending(false)
		}
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Forward message</DialogTitle>
					<DialogDescription>Select up to 5 chats.</DialogDescription>
				</DialogHeader>
				<div className="max-h-[320px] overflow-y-auto flex flex-col gap-1">
					{chats
						.filter((c) => c.id !== currentChatId)
						.map((c) => (
							<button
								key={c.id}
								onClick={() => toggle(c.id)}
								className={`flex items-center gap-3 p-2 rounded-lg text-left transition-colors ${selected.has(c.id) ? "bg-primary/10" : "hover:bg-muted"}`}
							>
								<div className="w-9 h-9 rounded-full bg-primary/15 flex items-center justify-center text-sm font-bold text-primary shrink-0">
									{(c.name || c.id).charAt(0).toUpperCase()}
								</div>
								<span className="flex-1 truncate text-sm font-medium">{c.name || c.id}</span>
								<div className={`flex h-5 w-5 items-center justify-center rounded border ${selected.has(c.id) ? "bg-primary border-primary text-primary-foreground" : "border-muted-foreground/40"}`}>
									{selected.has(c.id) ? "✓" : ""}
								</div>
							</button>
						))}
				</div>
				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
					<Button onClick={send} disabled={sending || selected.size === 0}>
						Forward ({selected.size})
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	)
}
