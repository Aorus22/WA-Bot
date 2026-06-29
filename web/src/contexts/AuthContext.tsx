import { createContext, useContext, useState, useEffect, useCallback, useRef, type ReactNode } from "react"
import { useWebSocket, type WSMessage } from "@/hooks/use-websocket"
import { api, type Message } from "@/lib/api"

interface ChatUpdate {
	chatId: string
	lastMsg: string
	lastTime: number
	msgId: string
	status?: string
	senderName?: string
	chatName?: string
	chatAvatar?: string
}

interface AuthState {
	isLoggedIn: boolean | null
	qrCode: string | null
	isConnected: boolean
	incomingMessage: { chatId: string; message: Message } | null
	chatUpdate: ChatUpdate | null
	statusUpdate: { id: string; status: string } | null
	isLogoutDialogOpen: boolean
	setIsLogoutDialogOpen: (open: boolean) => void
	logout: () => Promise<void>
}

const AuthContext = createContext<AuthState | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
	const [isLoggedIn, setIsLoggedIn] = useState<boolean | null>(null)
	const [qrCode, setQrCode] = useState<string | null>(null)
	const [isLogoutDialogOpen, setIsLogoutDialogOpen] = useState(false)
	const [incomingMessage, setIncomingMessage] = useState<{ chatId: string; message: Message } | null>(null)
	const [chatUpdate, setChatUpdate] = useState<ChatUpdate | null>(null)
	const [statusUpdate, setStatusUpdate] = useState<{ id: string; status: string } | null>(null)
	const processedMsgIds = useRef<Set<string>>(new Set())

	const handleWSMessage = useCallback((message: WSMessage) => {
		switch (message.type) {
			case "qr_code":
				if (message.payload && message.payload.code) {
					setQrCode(message.payload.code)
					setIsLoggedIn(false)
				}
				break
			case "auth_success":
				setIsLoggedIn(true)
				setQrCode(null)
				break
			case "new_message": {
				const payload = message.payload
				if (processedMsgIds.current.has(payload.id)) break
				processedMsgIds.current.add(payload.id)
				setIncomingMessage({
					chatId: payload.chatId,
					message: {
						id: payload.id,
						chatId: payload.chatId,
						from: payload.from,
						to: payload.to,
						content: payload.content,
						timestamp: payload.timestamp,
						status: payload.status,
						type: payload.type,
						mediaUrl: payload.mediaUrl,
						isAutomatic: payload.isAutomatic,
					},
				})
				setChatUpdate({
					chatId: payload.chatId,
					lastMsg: payload.content,
					lastTime: payload.timestamp,
					msgId: payload.id,
					status: payload.status,
					senderName: payload.senderName,
					chatName: payload.chatName,
				})
				break
			}
			case "message_deleted":
				if (message.payload) {
					setIncomingMessage({
						chatId: message.payload.chatId,
						message: { id: message.payload.id, type: "deleted" } as any,
					})
				}
				break
			case "message_edited":
				if (message.payload) {
					setIncomingMessage({
						chatId: message.payload.chatId,
						message: { id: message.payload.id, content: message.payload.content, type: "edited" } as any,
					})
				}
				break
			case "message_status":
				if (message.payload) {
					setStatusUpdate({
						id: message.payload.id,
						status: message.payload.status,
					})
				}
				break
			case "chat_name_update":
				if (message.payload) {
					setChatUpdate({
						chatId: message.payload.chatId,
						lastMsg: "",
						lastTime: Date.now(),
						msgId: "name-update-" + Date.now(),
						chatName: message.payload.name,
						chatAvatar: message.payload.avatar,
					})
				}
				break
		}
	}, [])

	const { isConnected } = useWebSocket("user-1", handleWSMessage)

	const checkLoginStatus = useCallback(async () => {
		try {
			const status = await api.getStatus()
			setIsLoggedIn(status.isLoggedIn)
		} catch {
			setIsLoggedIn(false)
		}
	}, [])

	const logout = async () => {
		try {
			await api.logout()
			setIsLoggedIn(false)
			setQrCode(null)
			setIsLogoutDialogOpen(false)
		} catch { /* ignore */ }
	}

	useEffect(() => {
		checkLoginStatus()
	}, [checkLoginStatus])

	return (
		<AuthContext.Provider value={{
			isLoggedIn, qrCode, isConnected,
			incomingMessage, chatUpdate, statusUpdate,
			isLogoutDialogOpen, setIsLogoutDialogOpen,
			logout,
		}}>
			{children}
		</AuthContext.Provider>
	)
}

export function useAuth() {
	const ctx = useContext(AuthContext)
	if (!ctx) throw new Error("useAuth must be used within AuthProvider")
	return ctx
}
