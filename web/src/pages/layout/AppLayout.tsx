import { useState, useEffect } from "react"
import { Outlet, useLocation, useNavigate } from "react-router-dom"
import { useIsMobile } from "@/hooks/use-mobile"
import { NavigationSidebar } from "./NavigationSidebar"
import { cn } from "@/lib/utils"
import { MessageSquare, Phone, Bot, Clock, Globe, FileText, Settings, Minus, Square, Copy, X } from "lucide-react"
import {
	isDesktop,
	platform,
	minimizeWindow,
	maximizeWindow,
	closeWindow,
	getWindowState,
	onWindowStateChange,
} from "@/lib/desktop-ipc"

export function AppLayout() {
	const isMobileView = useIsMobile()
	const location = useLocation()
	const navigate = useNavigate()
	const [autoOpen, setAutoOpen] = useState(false)
	const [windowState, setWindowState] = useState<"maximized" | "restored">("restored")

	const isWindows = platform === "win32"
	const showCustomControls = isDesktop && !isWindows

	useEffect(() => {
		if (isDesktop) {
			getWindowState().then(setWindowState)
			return onWindowStateChange(setWindowState)
		}
	}, [])

	// Hide mobile nav on detail pages (paths with 3+ segments except /new)
	const isDetailPage = location.pathname.split("/").filter(Boolean).length > 1 && !location.pathname.endsWith("/new") && !location.pathname.endsWith("/logs")

	const isActive = (path: string) => location.pathname.startsWith(path)
	const isAutoActive = isActive("/triggers") || isActive("/cron") || isActive("/webhooks") || isActive("/documentation")

	const autoItems = [
		{ path: "/triggers", icon: <Bot className="h-5 w-5" />, label: "Triggers" },
		{ path: "/cron", icon: <Clock className="h-5 w-5" />, label: "Cron" },
		{ path: "/webhooks", icon: <Globe className="h-5 w-5" />, label: "Webhooks" },
		{ path: "/documentation", icon: <FileText className="h-5 w-5" />, label: "Docs" },
	]

	return (
		<>
			{/* ── Electron frameless title bar (drag region) ── */}
			{isDesktop && (
				<header
					className="drag-region h-12 flex items-center justify-between px-3 bg-secondary/50 shrink-0 select-none"
					style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
				>
					{/* Left: app name in drag area */}
					<div className="flex items-center gap-3">
						<span className="text-sm font-semibold text-foreground/60 select-none ml-2">
							WA Bot
						</span>
					</div>

					{/* Custom window controls (Linux/macOS only) */}
					{showCustomControls && (
						<div className="flex items-center h-full">
							<button
								onClick={minimizeWindow}
								className="no-drag flex items-center justify-center h-9 w-11 text-foreground/70 hover:bg-muted hover:text-foreground transition-all duration-200"
								style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
								title="Minimize"
							>
								<Minus className="h-4 w-4 stroke-[1.5]" />
							</button>
							<button
								onClick={maximizeWindow}
								className="no-drag flex items-center justify-center h-9 w-11 text-foreground/70 hover:bg-muted hover:text-foreground transition-all duration-200"
								style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
								title={windowState === "maximized" ? "Restore" : "Maximize"}
							>
								{windowState === "maximized" ? (
									<Copy className="h-3.5 w-3.5 stroke-[1.5]" />
								) : (
									<Square className="h-3.5 w-3.5 stroke-[1.5]" />
								)}
							</button>
							<button
								onClick={closeWindow}
								className="no-drag flex items-center justify-center h-9 w-11 text-foreground/70 hover:bg-destructive hover:text-destructive-foreground transition-all duration-200"
								style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
								title="Close"
							>
								<X className="h-4 w-4 stroke-[1.5]" />
							</button>
						</div>
					)}

					{/* Reserve space for Windows native caption buttons */}
					{isDesktop && isWindows && (
						<div className="w-[138px] h-full shrink-0" />
					)}
				</header>
			)}

			{/* ── Main content ── */}
			<div className={cn("flex flex-1 bg-background text-foreground overflow-hidden", isMobileView && "flex-col")}
				style={isDesktop ? { height: "calc(100vh - 48px)" } : { height: "100dvh" }}
			>
				<div className="flex-1 flex w-full relative overflow-hidden min-h-0">
					{!isMobileView && (
						<NavigationSidebar />
					)}

					<div className="flex-1 flex flex-col min-h-0 w-full overflow-hidden">
						<Outlet />
					</div>
				</div>

				{isMobileView && !isDetailPage && (
					<>
						{/* Bottom Sheet Overlay */}
						{autoOpen && (
							<div className="fixed inset-0 z-40 bg-black/30 backdrop-blur-[1px]" onClick={() => setAutoOpen(false)} />
						)}

						{/* Automation Popover */}
						{autoOpen && (
							<div className="fixed bottom-20 left-4 right-4 z-50 bg-card rounded-2xl border shadow-2xl p-3 animate-in slide-in-from-bottom-4 fade-in duration-200">
								<div className="grid grid-cols-4 gap-2">
									{autoItems.map((item) => (
										<button
											key={item.path}
											onClick={() => { navigate(item.path); setAutoOpen(false) }}
											className={cn(
												"flex flex-col items-center gap-1.5 p-3 rounded-xl transition-all",
												isActive(item.path)
													? "bg-primary text-primary-foreground shadow-sm"
													: "hover:bg-muted text-muted-foreground"
											)}
										>
											{item.icon}
											<span className="text-[10px] font-bold uppercase leading-none">{item.label}</span>
										</button>
									))}
								</div>
							</div>
						)}

						<div className="h-16 bg-background border-t border-border/40 flex items-center justify-around px-6 shrink-0 z-50">
							<button onClick={() => navigate("/chat")} className={cn("flex flex-col items-center gap-1 p-2", isActive("/chat") ? "text-primary" : "text-muted-foreground")}>
								<MessageSquare className="h-5 w-5" /><span className="text-[10px] font-bold uppercase">Chats</span>
							</button>
							<button onClick={() => navigate("/calls")} className={cn("flex flex-col items-center gap-1 p-2", isActive("/calls") ? "text-primary" : "text-muted-foreground")}>
								<Phone className="h-5 w-5" /><span className="text-[10px] font-bold uppercase">Calls</span>
							</button>
							<button onClick={() => setAutoOpen(prev => !prev)} className={cn("flex flex-col items-center gap-1 p-2", isAutoActive ? "text-primary" : "text-muted-foreground")}>
								<Bot className="h-5 w-5" /><span className="text-[10px] font-bold uppercase">Automation</span>
							</button>
							<button onClick={() => navigate("/settings")} className={cn("flex flex-col items-center gap-1 p-2", isActive("/settings") ? "text-primary" : "text-muted-foreground")}>
								<Settings className="h-5 w-5" /><span className="text-[10px] font-bold uppercase">Settings</span>
							</button>
						</div>
					</>
				)}
			</div>
		</>
	)
}
