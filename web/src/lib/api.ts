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
}

export type Contact = {
	id: string
	name: string
	jid: string
	avatar: string
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
		return this.request<Chat[]>("/chats")
	}

	async markAsRead(chatId: string): Promise<{ status: string }> {
		return this.request<{ status: string }>(`/chats/${chatId}/read`, {
			method: "POST",
		})
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
		type: "image" | "video" | "document" | "audio" | "ptt" | "voice",
		message: string = "",
		options?: { ptt?: boolean; seconds?: number; waveform?: string }
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
