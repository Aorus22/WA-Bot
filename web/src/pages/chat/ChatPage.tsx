import { useState, useCallback, useEffect, useRef } from "react"
import { MessageSquare } from "lucide-react"
import { useParams, useNavigate } from "react-router-dom"
import { ChatSidebar } from "./ChatSidebar"
import { ChatArea } from "./ChatArea"
import { cn } from "@/lib/utils"
import { type Chat } from "@/lib/api"
import { useAuth } from "@/contexts/AuthContext"
import { useIsMobile } from "@/hooks/use-mobile"
import { useChatStore } from "@/stores/chatStore"

export function ChatPage() {
	const isMobileView = useIsMobile()
	const navigate = useNavigate()
	const { id: chatId } = useParams<{ id: string }>()
	const { incomingMessage, chatUpdate, statusUpdate, isLoggedIn } = useAuth()
	const [selectedChat, setSelectedChat] = useState<Chat | null>(null)
	const chats = useChatStore(s => s.chats)
	const setChats = useChatStore(s => s.setChats)
	const [showSidebar, setShowSidebar] = useState(true)
	const containerRef = useRef<HTMLDivElement>(null)

	// Wire WS events into the store
	useEffect(() => {
		if (!incomingMessage) return
		const { chatId: cid, message } = incomingMessage
		const type = (message as any).type
		if (type === "deleted") {
			useChatStore.getState().deleteMessage(cid, message.id)
		} else if (type === "edited") {
			useChatStore.getState().patchMessage(cid, message.id, { content: (message as any).content })
		} else {
			useChatStore.getState().upsertMessage(cid, message)
		}
	}, [incomingMessage])

	useEffect(() => {
		if (!statusUpdate) return
		// Patch status across all chats (message id is globally unique)
		const { messagesByChat } = useChatStore.getState()
		for (const cid of Object.keys(messagesByChat)) {
			useChatStore.getState().patchMessage(cid, statusUpdate.id, { status: statusUpdate.status })
		}
	}, [statusUpdate])

	// Auto-select chat from URL param when chats are loaded
	useEffect(() => {
		if (chatId && chats.length > 0) {
			const found = chats.find(c => c.id === chatId)
			if (found) {
				setSelectedChat(found)
				setShowSidebar(false)
			}
		} else if (!chatId) {
			setSelectedChat(null)
			setShowSidebar(true)
		}
	}, [chatId, chats])

	// Sidebar resize (desktop only)
	const [sidebarWidth, setSidebarWidth] = useState(400)
	const isResizing = useRef(false)
	const [activeResizing, setActiveResizing] = useState(false)
	const rafId = useRef<number | null>(null)

	const startResizing = useCallback(() => {
		isResizing.current = true
		setActiveResizing(true)
		document.body.classList.add("is-resizing")
	}, [])

	const stopResizing = useCallback(() => {
		isResizing.current = false
		setActiveResizing(false)
		document.body.classList.remove("is-resizing")
		if (rafId.current) { cancelAnimationFrame(rafId.current); rafId.current = null }
	}, [])

	const resize = useCallback((e: MouseEvent) => {
		if (!isResizing.current) return
		if (rafId.current) return
		rafId.current = requestAnimationFrame(() => {
			const containerLeft = containerRef.current?.getBoundingClientRect().left ?? 0
			const newWidth = Math.max(280, Math.min(600, e.clientX - containerLeft))
			setSidebarWidth(newWidth)
			rafId.current = null
		})
	}, [])

	useEffect(() => {
		if (!activeResizing) return
		window.addEventListener("mousemove", resize)
		window.addEventListener("mouseup", stopResizing)
		return () => {
			window.removeEventListener("mousemove", resize)
			window.removeEventListener("mouseup", stopResizing)
		}
	}, [resize, stopResizing, activeResizing])

	const handleChatSelect = useCallback((chat: Chat) => {
		navigate(`/chat/${chat.id}`)
	}, [navigate])

	const handleBack = useCallback(() => {
		navigate("/chat")
	}, [navigate])

	if (!isLoggedIn) {
		return (
			<div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground">
				<div className="w-16 h-16 rounded-2xl bg-muted flex items-center justify-center">
					<MessageSquare className="h-8 w-8" />
				</div>
				<div className="text-center space-y-1">
					<h2 className="text-lg font-semibold text-foreground">No account connected</h2>
					<p className="text-sm">Link your WhatsApp account to start chatting.</p>
				</div>
			</div>
		)
	}

	if (isMobileView) {
		return (
			<div ref={containerRef} className="flex w-full relative h-full">
				<div
					className={cn(
						"h-full transition-all duration-300 ease-in-out border-r border-border/40",
						showSidebar ? "w-full absolute inset-0 z-40 bg-background" : "w-0 overflow-hidden"
					)}
				>
					<ChatSidebar selectedChatId={selectedChat?.id || null} onChatSelect={handleChatSelect} chatUpdate={chatUpdate} onChatsLoaded={setChats} />
				</div>

				<main className={cn("flex-1 h-full relative overflow-hidden bg-muted/30", showSidebar ? "hidden" : "flex")}>
					{selectedChat && (
						<ChatArea
							chat={selectedChat}
							onBack={handleBack}
							className="w-full"
						/>
					)}
				</main>
			</div>
		)
	}

	return (
		<div
			ref={containerRef}
			className="flex w-full h-full overflow-hidden bg-background relative"
			style={{ "--sidebar-width": `${sidebarWidth}px` } as React.CSSProperties}
		>
			{activeResizing && (
				<div className="fixed inset-0 z-[100] cursor-col-resize" />
			)}

			{/* Resize handle */}
			<div
				className="absolute top-0 bottom-0 z-50 cursor-col-resize hover:bg-primary/10 transition-colors duration-200"
				style={{
					left: `calc(var(--sidebar-width) - 2px)`,
					width: "6px",
				}}
				onMouseDown={startResizing}
			/>

			{/* Sidebar */}
			<div
				data-sidebar-container
				style={{ width: "var(--sidebar-width)" }}
				className="h-full flex-shrink-0 border-r border-border/40 overflow-hidden"
			>
				<ChatSidebar
					selectedChatId={selectedChat?.id || null}
					onChatSelect={handleChatSelect}
					chatUpdate={chatUpdate}
					onChatsLoaded={setChats}
				/>
			</div>

			{/* Main */}
			<main data-main-chat className="flex-1 min-w-0 h-full">
				{selectedChat ? (
					<ChatArea
						chat={selectedChat}
						className="h-full"
					/>
				) : (
					<div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground bg-muted/10">
						<div className="w-20 h-20 rounded-3xl bg-primary/5 flex items-center justify-center mb-2">
							<MessageSquare className="h-10 w-10" />
						</div>
						<div className="text-center space-y-1">
							<h2 className="text-xl font-semibold text-foreground">WhatsApp Bot</h2>
							<p className="text-sm">Select a chat to start messaging</p>
						</div>
					</div>
				)}
			</main>
		</div>
	)
}
