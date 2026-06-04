const getApiBase = () => {
	const envUrl = import.meta.env.VITE_API_URL
	if (envUrl) return envUrl

	if (typeof window !== "undefined") {
		return `${window.location.origin}/api`
	}
	return ""
}

const API_BASE = getApiBase()

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
        created_at?: string
        updated_at?: string
}

export type CronJob = {
        id: string
        name: string
        schedule: string
        script: string
        is_active: boolean
        created_at?: string
        updated_at?: string
}

class ApiClient {	private baseUrl: string

	constructor(baseUrl: string = API_BASE) {
		this.baseUrl = baseUrl
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

	async getMessages(chatId: string, limit = 100): Promise<Message[]> {
		return this.request<Message[]>(`/chats/${chatId}/messages?limit=${limit}`)
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

	async sendMedia(
		target: string,
		file: File,
		type: "image" | "video" | "document",
		message: string = ""
	): Promise<{ status: string; id: string }> {
		const formData = new FormData()
		formData.append("secret", import.meta.env.VITE_API_SECRET || "default-secret")
		formData.append("target", target)
		formData.append("message", message)
		formData.append("type", type)
		formData.append("file", file)

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
}

export const api = new ApiClient()
