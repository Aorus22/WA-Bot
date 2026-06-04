import { useEffect, useRef, useState, useCallback } from "react"

export type WSMessage = {
	type: string
	payload: any
}

type WebSocketHook = {
	isConnected: boolean
	messages: WSMessage[]
	sendMessage: (type: string, payload: unknown) => void
	lastMessage: WSMessage | null
}

const getWsUrl = () => {
	const envUrl = import.meta.env.VITE_WS_URL
	if (envUrl) return envUrl

	if (typeof window !== "undefined") {
		const protocol = window.location.protocol === "https:" ? "wss:" : "ws:"
		return `${protocol}//${window.location.host}/ws`
	}
	return ""
}

const WS_URL = getWsUrl()

export function useWebSocket(userId?: string, onMessage?: (msg: WSMessage) => void): WebSocketHook {
	const [isConnected, setIsConnected] = useState(false)
	const [messages, setMessages] = useState<WSMessage[]>([])
	const [lastMessage, setLastMessage] = useState<WSMessage | null>(null)
	const wsRef = useRef<WebSocket | null>(null)
	const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
	const userIdRef = useRef(userId)
	const onMessageRef = useRef(onMessage)
	const isConnectingRef = useRef(false)

	// Update refs when they change
	useEffect(() => {
		userIdRef.current = userId
		onMessageRef.current = onMessage
	}, [userId, onMessage])

	const sendMessage = useCallback((type: string, payload: unknown) => {
		if (wsRef.current?.readyState === WebSocket.OPEN) {
			wsRef.current.send(JSON.stringify({ type, payload }))
		}
	}, [])

	const connect = useCallback(() => {
		// Prevent multiple simultaneous connections
		if (isConnectingRef.current || wsRef.current?.readyState === WebSocket.OPEN) {
			return
		}

		isConnectingRef.current = true

		try {
			const ws = new WebSocket(WS_URL)
			wsRef.current = ws

			ws.onopen = () => {
				setIsConnected(true)
				isConnectingRef.current = false
				console.log("WebSocket connected")

				// Authenticate if userId provided
				if (userIdRef.current) {
					sendMessage("authenticate", { userId: userIdRef.current })
				}
			}

			ws.onmessage = (event) => {
				try {
					const message: WSMessage = JSON.parse(event.data)
					setLastMessage(message)
					setMessages((prev) => [...prev, message])
					
					// Call the callback if provided
					if (onMessageRef.current) {
						onMessageRef.current(message)
					}
				} catch (err) {
					console.error("Failed to parse WS message:", err)
				}
			}

			ws.onclose = (event) => {
				setIsConnected(false)
				isConnectingRef.current = false
				wsRef.current = null

				// Only reconnect if it wasn't intentionally closed
				if (event.code !== 1000) {
					console.log("WebSocket disconnected, reconnecting...")
					reconnectTimeoutRef.current = setTimeout(() => {
						connect()
					}, 3000)
				}
			}

			ws.onerror = (error) => {
				console.error("WebSocket error:", error)
			}
		} catch (err) {
			console.error("Failed to create WebSocket:", err)
			isConnectingRef.current = false
		}
	}, [sendMessage])

	useEffect(() => {
		connect()

		return () => {
			if (reconnectTimeoutRef.current) {
				clearTimeout(reconnectTimeoutRef.current)
			}
			if (wsRef.current) {
				wsRef.current.close(1000, "Component unmounting")
			}
		}
	}, [connect])

	return {
		isConnected,
		messages,
		sendMessage,
		lastMessage,
	}
}
