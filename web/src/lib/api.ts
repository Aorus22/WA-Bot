const getApiBase = () => {
	// Electron desktop mode: use discovered backend port
	if (typeof window !== "undefined") {
		const port = (window as any).__BACKEND_PORT__ as number | undefined
		if (port) return `http://localhost:${port}/api`
	}

	const envUrl = import.meta.env.VITE_API_URL
	if (envUrl) return envUrl

	if (typeof window !== "undefined") {
		// Electron file:// has origin "null" — need explicit fallback
		const origin = window.location.origin
		if (origin && origin !== "null" && origin !== "file://") {
			return `${origin}/api`
		}
		// Desktop fallback: try common backend ports before port is discovered
		return "http://localhost:8080/api"
	}
	return "http://localhost:8080/api"
}

export function setBackendPort(port: number) {
	if (typeof window !== "undefined") {
		;(window as any).__BACKEND_PORT__ = port
	}
}

export type ReactionEntry = {
	emoji: string
	senders: string[]
}

export type PollMeta = {
	question: string
	options: Array<{ name: string }>
	multiSelect: boolean
	votes?: Record<string, string[]>
}

export type LocationMeta = {
	latitude: number
	longitude: number
	name?: string
	address?: string
	live?: boolean
	thumbnailUrl?: string
}

export type ContactMeta = {
	displayName?: string
	contacts: Array<{ displayName: string; vcard?: string }>
}

export type ViewOnceMeta = {
	mediaType: string
	viewed: boolean
}

export type LinkPreviewMeta = {
	url: string
	title?: string
	description?: string
	thumbnailUrl?: string
}

export type MessageExtra = {
	poll?: PollMeta
	location?: LocationMeta
	contact?: ContactMeta
	viewOnce?: ViewOnceMeta
	gif?: boolean
	linkPreview?: LinkPreviewMeta
}

export type Message = {
	id: string
	chatId: string
	from: string
	to: string
	content: string
	timestamp: number
	status: string
	type: string
	mediaUrl?: string
	isAutomatic?: boolean
	senderName?: string
	replyToId?: string
	forwarded?: boolean
	reactions?: ReactionEntry[]
	extra?: MessageExtra
}

export type Chat = {
	id: string
	name: string
	avatar: string
	lastMsg: string
	lastTime: number
	unread: number
	isActive: boolean
	isGroup: boolean
	archived: boolean
	pinnedAt: number | null
	muteMode: "off" | "until" | "forever"
	mutedUntil: number | null
}

/**
 * API responses can come from an older database where nullable chat columns
 * were serialized as JSON null. Keep that legacy data at the API boundary so
 * UI components can safely treat chat fields as non-nullable.
 */
export function normalizeChat(raw: Partial<Chat> | null | undefined): Chat {
	const value = raw ?? {}
	const pinnedAt = value.pinnedAt == null ? null : Number(value.pinnedAt)
	const mutedUntil = value.mutedUntil == null ? null : Number(value.mutedUntil)
	return {
		id: typeof value.id === "string" ? value.id : String(value.id ?? ""),
		name: typeof value.name === "string" ? value.name : "",
		avatar: typeof value.avatar === "string" ? value.avatar : "",
		lastMsg: typeof value.lastMsg === "string" ? value.lastMsg : "",
		lastTime: Number.isFinite(Number(value.lastTime)) ? Number(value.lastTime) : 0,
		unread: Number.isFinite(Number(value.unread)) ? Number(value.unread) : 0,
		isActive: Boolean(value.isActive),
		isGroup: Boolean(value.isGroup),
		archived: Boolean(value.archived),
		pinnedAt: pinnedAt != null && Number.isFinite(pinnedAt) ? pinnedAt : null,
		muteMode: value.muteMode === "until" || value.muteMode === "forever" ? value.muteMode : "off",
		mutedUntil: mutedUntil != null && Number.isFinite(mutedUntil) ? mutedUntil : null,
	}
}

export type ChatState = Pick<Chat, "archived" | "pinnedAt" | "muteMode" | "mutedUntil"> & {
	chatId: string
}

export type HistorySyncError = {
	chatId?: string
	message: string
}

export type HistorySyncStatus = {
	state: "idle" | "running" | "completed" | "partial" | "failed"
	pendingChats: number
	pendingMessages: number
	chatsTotal: number
	chatsProcessed: number
	messagesAdded: number
	errors: HistorySyncError[]
	startedAt: number | null
	finishedAt: number | null
	lastRunAt: number | null
}

export function normalizeHistorySyncStatus(raw: Partial<HistorySyncStatus> | null | undefined): HistorySyncStatus {
	const value = raw ?? {}
	const states: HistorySyncStatus["state"][] = ["idle", "running", "completed", "partial", "failed"]
	const state = states.includes(value.state as HistorySyncStatus["state"])
		? value.state as HistorySyncStatus["state"]
		: "idle"
	const errors = Array.isArray(value.errors)
		? value.errors.filter((error): error is HistorySyncError => Boolean(error) && typeof error === "object")
			.map((error) => ({
				chatId: typeof error.chatId === "string" ? error.chatId : undefined,
				message: typeof error.message === "string" ? error.message : "Unknown history sync error",
			}))
		: []
	return {
		state,
		pendingChats: Number(value.pendingChats) || 0,
		pendingMessages: Number(value.pendingMessages) || 0,
		chatsTotal: Number(value.chatsTotal) || 0,
		chatsProcessed: Number(value.chatsProcessed) || 0,
		messagesAdded: Number(value.messagesAdded) || 0,
		errors,
		startedAt: value.startedAt == null ? null : Number(value.startedAt),
		finishedAt: value.finishedAt == null ? null : Number(value.finishedAt),
		lastRunAt: value.lastRunAt == null ? null : Number(value.lastRunAt),
	}
}

export type Contact = {
	id: string
	name: string
	jid: string
	avatar: string
}

export type GroupParticipantInfo = {
	jid: string
	name?: string
	isAdmin: boolean
	isSuperAdmin?: boolean
}

export type GroupCache = {
	jid: string
	name: string
	description?: string
	owner?: string
	locked: boolean
	announce: boolean
	joinApproval: boolean
	memberAddMode?: string
	ownRole: string
	participantCount: number
	participants: GroupParticipantInfo[]
	updatedAt: number
}

export type GroupPreview = {
	jid: string
	name: string
	participantCount: number
	description?: string
}

export type StatusEntry = {
	id: string
	sender: string
	senderName?: string
	content?: string
	mediaUrl?: string
	type: string
	timestamp: number
	expiresAt: number
	viewed: boolean
}

export type StatusGroup = {
	sender: string
	name?: string
	avatar?: string
	allViewed: boolean
	statuses: StatusEntry[]
	latestTime: number
}

export type UpdateGroupChanges = {
	name?: string
	description?: string
	locked?: boolean
	announce?: boolean
	joinApproval?: boolean
	memberAddMode?: string
}

export type Trigger = {
        id: string
        name: string
        pattern: string
        script: string
        priority: number
        is_active: boolean
        description?: string
        created_at?: string
        updated_at?: string
}

export type CronJob = {
        id: string
        name: string
        schedule: string
        script: string
        is_active: boolean
        description?: string
        created_at?: string
        updated_at?: string
}

export type Webhook = {
        id: string
        name: string
        path: string
        script: string
        secret?: string
        is_active: boolean
        description?: string
        created_at?: string
        updated_at?: string
}

export type WebhookLog = {
        id: string
        webhook_id: string
        webhook_path: string
        source_ip: string
        method: string
        headers: string
        body: string
        query_params: string
        status_code: number
        created_at: number
}

export type CallStatus =
        | "preparing"
        | "initiating"
        | "ringing"
        | "connecting"
        | "connected"
        | "ending"
        | "ended"
        | "rejected"
        | "missed"
        | "busy"
        | "failed"
        | "interrupted"

export type CallType = "audio" | "video" | "group_audio" | "group_video"
export type CallDirection = "incoming" | "outgoing"
export type CallSource = "ui" | "external_api" | "incoming"
export type MediaMode = "live" | "tts" | "audio_file"

export type CallState = {
        id: string
        status: CallStatus
        type: CallType
        direction: CallDirection
        source: CallSource
        media_mode: MediaMode
        target: string
        group_jid?: string
        participants?: string[]
        started_at: number
        answered_at?: number | null
        video_enabled: boolean
        remote_video_enabled: boolean
}

export type CallLog = {
        id: string
        meow_call_id: string
        direction: CallDirection
        call_type: CallType
        target: string
        group_jid?: string
        participants?: string[]
        source: CallSource
        media_mode: MediaMode
        status: CallStatus
        error_message?: string
        api_key_id?: string
        started_at: number
        answered_at?: number | null
        ended_at?: number | null
        duration_ms?: number | null
        created_at: number
}

export type CallHistoryResponse = {
        logs: CallLog[]
}

export type CallHistoryFilter = {
        limit?: number
        before?: number
        direction?: string
        type?: string
        status?: string
        target?: string
}

export type SettingsMap = Record<string, string>

export type SettingsResponse = SettingsMap & {
        hasGeminiKey?: boolean
        hasFishKey?: boolean
}

class ApiClient {	private baseUrl: string

	constructor(baseUrl?: string) {
		this.baseUrl = baseUrl || getApiBase()
	}

	setBaseUrl(url: string) {
		this.baseUrl = url
	}

	mediaURL(value: string | undefined): string | undefined {
		if (!value) return undefined
		if (value.startsWith("http://") || value.startsWith("https://")) return value
		if (value === "/api" || value.startsWith("/api/")) {
			return this.baseUrl.replace(/\/api\/?$/, "") + value
		}
		return this.baseUrl + (value.startsWith("/") ? value : `/${value}`)
	}

	private async request<T>(
		endpoint: string,
		options?: RequestInit
	): Promise<T> {
		const response = await fetch(`${this.baseUrl}${endpoint}`, {
			headers: {
				"Content-Type": "application/json",
				...options?.headers,
			},
			...options,
		})

		if (!response.ok) {
			const error = await response.json().catch(() => ({
				error: response.statusText,
			}))
			throw new Error(error.error || "Request failed")
		}

		return response.json()
	}

	async getChats(): Promise<Chat[]> {
		const raw = await this.request<unknown[]>("/chats")
		return (Array.isArray(raw) ? raw : [])
			.map((chat) => normalizeChat(chat as Partial<Chat>))
			.filter((chat) => chat.id.length > 0)
	}

	async markAsRead(chatId: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/read`, {
			method: "POST",
		})
	}

	async pinChat(chatId: string, pinned: boolean): Promise<ChatState> {
		return this.request<ChatState>(`/chats/${encodeURIComponent(chatId)}/pin`, {
			method: "POST",
			body: JSON.stringify({ pinned }),
		})
	}

	async archiveChat(chatId: string, archived: boolean): Promise<ChatState> {
		return this.request<ChatState>(`/chats/${encodeURIComponent(chatId)}/archive`, {
			method: "POST",
			body: JSON.stringify({ archived }),
		})
	}

	async muteChat(chatId: string, mode: "off" | "8h" | "1w" | "forever"): Promise<ChatState> {
		return this.request<ChatState>(`/chats/${encodeURIComponent(chatId)}/mute`, {
			method: "POST",
			body: JSON.stringify({ mode }),
		})
	}

	async getHistorySyncStatus(): Promise<HistorySyncStatus> {
		const raw = await this.request<unknown>("/history-sync/status")
		return normalizeHistorySyncStatus(raw as Partial<HistorySyncStatus>)
	}

	async startHistorySync(): Promise<HistorySyncStatus> {
		const raw = await this.request<unknown>("/history-sync", { method: "POST" })
		return normalizeHistorySyncStatus(raw as Partial<HistorySyncStatus>)
	}

	async getMessages(chatId: string, limit = 100, before?: number, after?: number): Promise<Message[]> {
		let url = `/chats/${chatId}/messages?limit=${limit}`
		if (before) {
			url += `&before=${before}`
		}
		if (after) {
			url += `&after=${after}`
		}
		return this.request<Message[]>(url)
	}

	async searchMessages(chatId: string, query: string, limit = 50): Promise<Message[]> {
		return this.request<Message[]>(`/chats/${chatId}/search?q=${encodeURIComponent(query)}&limit=${limit}`)
	}

	async getMessageContext(chatId: string, messageId: string, limit = 50): Promise<Message[]> {
		return this.request<Message[]>(`/chats/${chatId}/messages/${messageId}/context?limit=${limit}`)
	}

	async getContacts(): Promise<Contact[]> {
		return this.request<Contact[]>("/contacts")
	}

	async getFavorites(): Promise<Array<{ id: string; mediaUrl: string; isAnimated: boolean }>> {
		return this.request<any[]>("/stickers/favorites")
	}

	async favoriteSticker(messageId: string, mediaUrl: string, isAnimated: boolean): Promise<{ status: string }> {
		return this.request<{ status: string }>("/stickers/favorite", {
			method: "POST",
			body: JSON.stringify({
				secret: import.meta.env.VITE_API_SECRET || "default-secret",
				messageId,
				mediaUrl,
				isAnimated,
			}),
		})
	}

	async deleteFavorite(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/stickers/favorites/${id}`, {
			method: "DELETE",
		})
	}

	async sendSticker(target: string, mediaUrl: string, isAnimated: boolean): Promise<{ status: string; id: string }> {
		return this.request<{ status: string; id: string }>("/send-sticker", {
			method: "POST",
			body: JSON.stringify({
				secret: import.meta.env.VITE_API_SECRET || "default-secret",
				target,
				mediaUrl,
				isAnimated,
			}),
		})
	}

	async deleteMessage(chatId: string, id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/messages/${id}/delete`, {
			method: "POST",
		})
	}

	async editMessage(chatId: string, id: string, content: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/messages/${id}/edit`, {
			method: "POST",
			body: JSON.stringify({ content }),
		})
	}

	async replyMessage(chatId: string, id: string, content: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/messages/${id}/reply`, {
			method: "POST",
			body: JSON.stringify({ content }),
		})
	}

	/** Subscribe to a 1:1 contact's availability updates. */
	async subscribeChatPresence(chatId: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/presence-subscribe`, { method: "POST" })
	}

	/** React to a message; an empty emoji removes the reaction. */
	async reactToMessage(chatId: string, id: string, emoji: string, from?: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/messages/${id}/react`, {
			method: "POST",
			body: JSON.stringify({ emoji, from: from || "" }),
		})
	}

	/** Forward a message to other chats. */
	async forwardMessage(chatId: string, id: string, targets: string[]): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/messages/${id}/forward`, {
			method: "POST",
			body: JSON.stringify({ targets }),
		})
	}

	/** Create a poll in a chat. */
	async sendPoll(chatId: string, question: string, options: string[], multiSelect = false): Promise<{ status: string; id?: string }> {
		return this.request<{ status: string; id?: string }>(`/chats/${chatId}/poll`, {
			method: "POST",
			body: JSON.stringify({
				secret: import.meta.env.VITE_API_SECRET || "default-secret",
				question,
				options,
				multiSelect,
			}),
		})
	}

	/** Vote on a poll; an empty options array retracts the vote. */
	async sendPollVote(chatId: string, messageId: string, options: string[]): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/messages/${messageId}/vote`, {
			method: "POST",
			body: JSON.stringify({ options }),
		})
	}

	/** Share a static or live location. */
	async sendLocation(
		chatId: string,
		latitude: number,
		longitude: number,
		name = "",
		address = "",
		live = false,
		caption = ""
	): Promise<{ status: string; id?: string }> {
		return this.request<{ status: string; id?: string }>(`/chats/${chatId}/location`, {
			method: "POST",
			body: JSON.stringify({
				secret: import.meta.env.VITE_API_SECRET || "default-secret",
				latitude,
				longitude,
				name,
				address,
				live,
				caption,
			}),
		})
	}

	/** Share a contact card. */
	async sendContact(chatId: string, displayName: string, phone: string, vcard = ""): Promise<{ status: string; id?: string }> {
		return this.request<{ status: string; id?: string }>(`/chats/${chatId}/contact`, {
			method: "POST",
			body: JSON.stringify({
				secret: import.meta.env.VITE_API_SECRET || "default-secret",
				displayName,
				phone,
				vcard,
			}),
		})
	}

	// --- Group management ---

	/** Fetch the cached group snapshot (server-refreshed on miss). */
	async getGroup(groupId: string): Promise<GroupCache> {
		return this.request<GroupCache>(`/groups/${groupId}`)
	}

	/** Patch group name/description/settings; returns the refreshed snapshot. */
	async updateGroup(groupId: string, changes: UpdateGroupChanges): Promise<GroupCache> {
		return this.request<GroupCache>(`/groups/${groupId}`, {
			method: "PATCH",
			body: JSON.stringify(changes),
		})
	}

	/** add | remove | promote | demote participants. */
	async updateGroupParticipants(groupId: string, action: string, jids: string[]): Promise<GroupCache> {
		return this.request<GroupCache>(`/groups/${groupId}/participants`, {
			method: "POST",
			body: JSON.stringify({ action, jids }),
		})
	}

	/** Fetch the invite link, optionally revoking the previous one. */
	async getGroupInviteLink(groupId: string, reset = false): Promise<{ status: string; link: string }> {
		return this.request<{ status: string; link: string }>(`/groups/${groupId}/invite-link${reset ? "?reset=true" : ""}`)
	}

	/** Peek at a group via invite link without joining. */
	async previewGroupLink(url: string): Promise<GroupPreview> {
		return this.request<GroupPreview>(`/groups/preview?url=${encodeURIComponent(url)}`)
	}

	/** Join a group via invite link; returns the group JID. */
	async joinGroupWithLink(url: string): Promise<{ status: string; jid: string }> {
		return this.request<{ status: string; jid: string }>("/groups/join", {
			method: "POST",
			body: JSON.stringify({ url }),
		})
	}

	/** Create a group and return its snapshot. */
	async createGroup(name: string, participants: string[]): Promise<{ status: string; group: GroupCache }> {
		return this.request<{ status: string; group: GroupCache }>("/groups/create", {
			method: "POST",
			body: JSON.stringify({ name, participants }),
		})
	}

	/** Leave a group. */
	async leaveGroup(groupId: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/groups/${groupId}/leave`, { method: "POST" })
	}

	// --- Status (stories) ---

	/** List active statuses grouped per sender. */
	async listStatuses(): Promise<StatusGroup[]> {
		return this.request<StatusGroup[]>("/statuses")
	}

	/** Post a text status with a background color (ARGB). */
	async postStatusText(text: string, background = 0xff075e54): Promise<{ status: string; id: string }> {
		return this.request<{ status: string; id: string }>("/statuses/text", {
			method: "POST",
			body: JSON.stringify({ text, background }),
		})
	}

	/** Post an image/video status. */
	async postStatusMedia(file: File, type: "image" | "video", caption = ""): Promise<{ status: string; id: string }> {
		const formData = new FormData()
		formData.append("type", type)
		formData.append("caption", caption)
		formData.append("file", file)
		const response = await fetch(`${this.baseUrl}/statuses/media`, { method: "POST", body: formData })
		if (!response.ok) {
			const error = await response.json().catch(() => ({ error: response.statusText }))
			throw new Error(error.error || "Request failed")
		}
		return response.json()
	}

	/** Mark a status viewed (sends the read receipt). */
	async markStatusViewed(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/statuses/${id}/viewed`, { method: "POST" })
	}

	/** On-demand status media endpoint. */
	statusMediaURL(id: string): string {
		return `${this.baseUrl}/statuses/${encodeURIComponent(id)}/media`
	}

	// --- Channels (newsletters) ---

	/** List followed channels. */
	async listChannels(): Promise<Array<{
		jid: string
		name: string
		description?: string
		subscribers: number
		inviteCode?: string
		avatar?: string
		muted: boolean
		verified: boolean
	}>> {
		return this.request("/channels")
	}

	/** Peek at a channel via invite link without following. */
	async previewChannel(url: string): Promise<{
		jid: string
		name: string
		subscribers: number
		description?: string
	}> {
		return this.request(`/channels/preview?url=${encodeURIComponent(url)}`)
	}

	/** Follow a channel via invite link. */
	async followChannel(url: string): Promise<{ status: string }> {
		return this.request("/channels", { method: "POST", body: JSON.stringify({ url }) })
	}

	/** Unfollow a channel. */
	async unfollowChannel(jid: string): Promise<{ status: string }> {
		return this.request(`/channels/${jid}`, { method: "DELETE" })
	}

	/** Toggle channel notifications. */
	async setChannelMute(jid: string, muted: boolean): Promise<{ status: string }> {
		return this.request(`/channels/${jid}/mute`, { method: "POST", body: JSON.stringify({ muted }) })
	}

	/** Fetch the channel post feed. */
	async getChannelMessages(jid: string, count = 30, before?: number): Promise<Array<{
		id: string
		serverId: number
		type: string
		timestamp: number
		viewsCount: number
		reactions?: Record<string, number>
		content?: string
		mediaType?: string
	}>> {
		let url = `/channels/${jid}/messages?count=${count}`
		if (before) url += `&before=${before}`
		return this.request(url)
	}

	/** React to a channel post (empty emoji removes the reaction). */
	async reactChannelMessage(jid: string, serverId: number, messageId: string, emoji: string): Promise<{ status: string }> {
		return this.request(`/channels/${jid}/messages/${serverId}/react`, {
			method: "POST",
			body: JSON.stringify({ emoji, messageId }),
		})
	}

	/** Upload a new group photo (multipart "file"). */
	async setGroupPhoto(groupId: string, file: File): Promise<GroupCache> {
		const formData = new FormData()
		formData.append("file", file)
		const response = await fetch(`${this.baseUrl}/groups/${groupId}/photo`, {
			method: "POST",
			body: formData,
		})
		if (!response.ok) {
			const error = await response.json().catch(() => ({ error: response.statusText }))
			throw new Error(error.error || "Request failed")
		}
		return response.json()
	}

	async getStatus(): Promise<{ isLoggedIn: boolean }> {
		return this.request<{ isLoggedIn: boolean }>("/status")
	}

	async getQrCode(): Promise<{ code: string }> {
		return this.request<{ code: string }>("/qr-code")
	}

	async logout(): Promise<{ status: string }> {
		return this.request<{ status: string }>("/logout", {
			method: "POST",
		})
	}

	async getTriggers(): Promise<Trigger[]> {
		return this.request<Trigger[]>("/triggers")
	}

	async createTrigger(trigger: Partial<Trigger>): Promise<Trigger> {
		return this.request<Trigger>("/triggers", {
			method: "POST",
			body: JSON.stringify(trigger),
		})
	}

	async updateTrigger(id: string, trigger: Partial<Trigger>): Promise<Trigger> {
		return this.request<Trigger>(`/triggers/${id}`, {
			method: "PUT",
			body: JSON.stringify(trigger),
		})
	}

	async deleteTrigger(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/triggers/${id}`, {
			method: "DELETE",
		})
	}

	async deleteAllTriggers(): Promise<{ status: string }> {
		return this.request<{ status: string }>("/triggers", {
			method: "DELETE",
		})
	}

	async testTrigger(data: { pattern: string; script: string; message: string }): Promise<any> {
		return this.request<any>("/triggers/test", {
			method: "POST",
			body: JSON.stringify(data),
		})
	}

	async getCronJobs(): Promise<CronJob[]> {
		return this.request<CronJob[]>("/cron")
	}

	async createCronJob(job: Partial<CronJob>): Promise<CronJob> {
		return this.request<CronJob>("/cron", {
			method: "POST",
			body: JSON.stringify(job),
		})
	}

	async updateCronJob(id: string, job: Partial<CronJob>): Promise<CronJob> {
		return this.request<CronJob>(`/cron/${id}`, {
			method: "PUT",
			body: JSON.stringify(job),
		})
	}

	async deleteCronJob(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/cron/${id}`, {
			method: "DELETE",
		})
	}

	async deleteAllCronJobs(): Promise<{ status: string }> {
		return this.request<{ status: string }>("/cron", {
			method: "DELETE",
		})
	}

	async testCronJob(script: string): Promise<any> {
		return this.request<any>("/cron/test", {
			method: "POST",
			body: JSON.stringify({ script }),
		})
	}

	async getWebhooks(): Promise<Webhook[]> {
		return this.request<Webhook[]>("/webhooks")
	}

	async createWebhook(webhook: Partial<Webhook>): Promise<Webhook> {
		return this.request<Webhook>("/webhooks", {
			method: "POST",
			body: JSON.stringify(webhook),
		})
	}

	async updateWebhook(id: string, webhook: Partial<Webhook>): Promise<Webhook> {
		return this.request<Webhook>(`/webhooks/${id}`, {
			method: "PUT",
			body: JSON.stringify(webhook),
		})
	}

	async deleteWebhook(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/webhooks/${id}`, {
			method: "DELETE",
		})
	}

	async deleteAllWebhooks(): Promise<{ status: string }> {
		return this.request<{ status: string }>("/webhooks", {
			method: "DELETE",
		})
	}

	async testWebhook(data: { path: string; script: string; method: string; body: string }): Promise<any> {
		return this.request<any>("/webhooks/test", {
			method: "POST",
			body: JSON.stringify(data),
		})
	}

	async getWebhookLogs(params?: { webhook_id?: string; limit?: number; offset?: number }): Promise<{
		logs: WebhookLog[]; total: number; limit: number; offset: number
	}> {
		const query = new URLSearchParams()
		if (params?.webhook_id) query.set("webhook_id", params.webhook_id)
		if (params?.limit) query.set("limit", String(params.limit))
		if (params?.offset) query.set("offset", String(params.offset))
		const qs = query.toString()
		return this.request(`/webhooks/logs${qs ? "?" + qs : ""}`)
	}

	async deleteAllWebhookLogs(): Promise<void> {
		const response = await fetch(`${this.baseUrl}/webhooks/logs`, {
			method: "DELETE",
			headers: { "Content-Type": "application/json" },
		})
		if (!response.ok) throw new Error("Failed to clear logs")
	}

	async getDocs(): Promise<string> {
		const response = await fetch(`${this.baseUrl}/docs`)
		return response.text()
	}

	async chatAssistant(prompt: string, currentCode?: string, model?: string): Promise<{ answer: string }> {
		return this.request<{ answer: string }>("/ai/assistant", {
			method: "POST",
			body: JSON.stringify({ prompt, currentCode, model }),
		})
	}

	async sendMessage(target: string, message: string): Promise<{ status: string; id: string }> {
		return this.request<{ status: string; id: string }>("/send-message", {
			method: "POST",
			body: JSON.stringify({
				secret: import.meta.env.VITE_API_SECRET || "default-secret",
				target,
				message,
			}),
		})
	}

	async getChatMedia(chatId: string, limit = 30, before?: number): Promise<Message[]> {
		let url = `/chats/${chatId}/media?limit=${limit}`
		if (before) {
			url += `&before=${before}`
		}
		return this.request<Message[]>(url)
	}

	async getChatDocs(chatId: string, limit = 30, before?: number): Promise<Message[]> {
		let url = `/chats/${chatId}/docs?limit=${limit}`
		if (before) {
			url += `&before=${before}`
		}
		return this.request<Message[]>(url)
	}

	async getChatLinks(chatId: string, limit = 30, before?: number): Promise<Message[]> {
		let url = `/chats/${chatId}/links?limit=${limit}`
		if (before) {
			url += `&before=${before}`
		}
		return this.request<Message[]>(url)
	}

	async sendMedia(
		target: string,
		file: File,
		type: "image" | "video" | "document" | "audio" | "ptt" | "voice" | "gif",
		message: string = "",
		options?: { ptt?: boolean; seconds?: number; waveform?: string; viewOnce?: boolean }
	): Promise<{ status: string; id: string }> {
		const formData = new FormData()
		formData.append("secret", import.meta.env.VITE_API_SECRET || "default-secret")
		formData.append("target", target)
		formData.append("message", message)
		formData.append("type", type)
		formData.append("file", file)

		// Optional audio metadata (ptt/seconds/waveform) — only sent when provided,
		// so existing image/video/document callers behave exactly as before.
		if (options?.ptt !== undefined) {
			formData.append("ptt", String(options.ptt))
		}
		if (options?.seconds !== undefined) {
			formData.append("seconds", String(options.seconds))
		}
		if (options?.waveform) {
			formData.append("waveform", options.waveform)
		}
		if (options?.viewOnce) {
			formData.append("viewOnce", "true")
		}

		const response = await fetch(`${this.baseUrl}/send-media`, {
			method: "POST",
			body: formData,
		})

		if (!response.ok) {
			const error = await response.json().catch(() => ({
				error: response.statusText,
			}))
			throw new Error(error.error || "Request failed")
		}

		return response.json()
	}

	/**
	 * Convenience helper for audio messages. Sends as "ptt" when ptt=true,
	 * otherwise as a regular "audio" attachment. Optional seconds/waveform are
	 * forwarded to the backend for WhatsApp voice-note metadata.
	 */
	async sendAudio(
		target: string,
		file: File,
		ptt = false,
		seconds?: number,
		waveform?: string
	): Promise<{ status: string; id: string }> {
		return this.sendMedia(target, file, ptt ? "ptt" : "audio", "", {
			ptt,
			seconds,
			waveform,
		})
	}

	// --- Calls ---------------------------------------------------------------

	async getActiveCall(): Promise<CallState | null> {
		return this.request<CallState | null>("/calls/active")
	}

	async createCall(payload: { target: string; type: CallType }): Promise<CallState> {
		return this.request<CallState>("/calls", {
			method: "POST",
			body: JSON.stringify(payload),
		})
	}

	async createGroupCall(payload: { group_jid: string; participants: string[]; type: CallType }): Promise<CallState> {
		return this.request<CallState>("/calls/group", {
			method: "POST",
			body: JSON.stringify(payload),
		})
	}

	async addCallParticipants(id: string, targets: string[]): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/participants`, {
			method: "POST",
			body: JSON.stringify({ targets }),
		})
	}

	async ringCallParticipant(id: string, target: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/ring?target=${encodeURIComponent(target)}`, {
			method: "POST",
		})
	}

	async answerCall(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/answer`, {
			method: "POST",
		})
	}

	async rejectCall(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/reject`, {
			method: "POST",
		})
	}

	async hangupCall(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/hangup`, {
			method: "POST",
		})
	}

	async getCallHistory(filter?: CallHistoryFilter): Promise<CallHistoryResponse> {
		const q = new URLSearchParams()
		if (filter?.limit) q.set("limit", String(filter.limit))
		if (filter?.before) q.set("before", String(filter.before))
		if (filter?.direction) q.set("direction", filter.direction)
		if (filter?.type) q.set("type", filter.type)
		if (filter?.status) q.set("status", filter.status)
		if (filter?.target) q.set("target", filter.target)
		const qs = q.toString()
		return this.request<CallHistoryResponse>(`/calls/history${qs ? `?${qs}` : ""}`)
	}

	async startVideo(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/video/start`, {
			method: "POST",
		})
	}

	async acceptVideo(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/video/accept`, {
			method: "POST",
		})
	}

	async rejectVideo(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/video/reject`, {
			method: "POST",
		})
	}

	async stopVideo(id: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/calls/${encodeURIComponent(id)}/video/stop`, {
			method: "POST",
		})
	}

	// --- Settings -------------------------------------------------------------

	async getSettings(): Promise<SettingsResponse> {
		const resp = await this.request<{
			settings: SettingsMap
			hasGeminiKey?: boolean
			hasFishKey?: boolean
		}>("/settings")
		return {
			...resp.settings,
			hasGeminiKey: resp.hasGeminiKey,
			hasFishKey: resp.hasFishKey,
		} as SettingsResponse
	}

	async updateSettings(data: SettingsMap): Promise<SettingsResponse> {
		const resp = await this.request<{
			settings: SettingsMap
			hasGeminiKey?: boolean
			hasFishKey?: boolean
		}>("/settings", {
			method: "PUT",
			body: JSON.stringify(data),
		})
		return {
			...resp.settings,
			hasGeminiKey: resp.hasGeminiKey,
			hasFishKey: resp.hasFishKey,
		} as SettingsResponse
	}
}

export const api = new ApiClient()
