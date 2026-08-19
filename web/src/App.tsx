import { HashRouter, Routes, Route, Navigate } from "react-router-dom"
import { useState, useEffect } from "react"
import { ThemeProvider } from "@/components/theme-provider"
import { AppThemeProvider } from "@/components/AppThemeProvider"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Toaster } from "@/components/ui/sonner"
import { Loader2 } from "lucide-react"
import { AuthProvider, useAuth } from "@/contexts/AuthContext"
import { CallProvider } from "@/contexts/CallContext"
import { IncomingCallOverlay } from "@/components/call/IncomingCallOverlay"
import { CallOverlay } from "@/components/call/CallOverlay"
import { AppLayout } from "@/pages/layout/AppLayout"
import { LoginPage } from "@/pages/login/LoginPage"
import { ChatPage } from "@/pages/chat/ChatPage"
import { BotManagementPage } from "@/pages/bot/BotManagementPage"
import { TriggerEditorPage } from "@/pages/bot/TriggerEditorPage"
import { CronManagementPage } from "@/pages/bot/CronManagementPage"
import { CronEditorPage } from "@/pages/bot/CronEditorPage"
import { WebhookManagementPage } from "@/pages/bot/WebhookManagementPage"
import { WebhookEditorPage } from "@/pages/bot/WebhookEditorPage"
import { WebhookLogPage } from "@/pages/bot/WebhookLogPage"
import { DocumentationPage } from "@/pages/documentation/DocumentationPage"
import { SettingsPage } from "@/pages/settings/SettingsPage"
import { isDesktop, getWindowState, onWindowStateChange } from "@/lib/desktop-ipc"
import { cn } from "@/lib/utils"

function AppRoutes() {
	const { isLoggedIn } = useAuth()

	if (isLoggedIn === null) {
		return (
			<div className="h-screen w-full flex items-center justify-center bg-background">
				<Loader2 className="h-10 w-10 text-primary animate-spin" />
			</div>
		)
	}

	return (
		<Routes>
			<Route element={<AppLayout />}>
				<Route path="/chat" element={<ChatPage />} />
				<Route path="/chat/:id" element={<ChatPage />} />
				<Route path="/cron" element={<CronManagementPage />} />
				<Route path="/cron/new" element={<CronEditorPage />} />
				<Route path="/cron/:id" element={<CronEditorPage />} />
				<Route path="/triggers" element={<BotManagementPage />} />
				<Route path="/triggers/new" element={<TriggerEditorPage />} />
				<Route path="/triggers/:id" element={<TriggerEditorPage />} />
				<Route path="/webhooks" element={<WebhookManagementPage />} />
				<Route path="/webhooks/new" element={<WebhookEditorPage />} />
				<Route path="/webhooks/:id" element={<WebhookEditorPage />} />
				<Route path="/webhooks/logs" element={<WebhookLogPage />} />
				<Route path="/settings" element={<SettingsPage />} />
				<Route path="/documentation" element={<DocumentationPage />} />
				<Route path="*" element={<Navigate to="/chat" replace />} />
			</Route>
			<Route path="/login" element={
				isLoggedIn ? <Navigate to="/chat" replace /> : <LoginPage />
			} />
		</Routes>
	)
}

function App() {
	const [windowState, setWindowState] = useState<"maximized" | "restored">("restored")

	useEffect(() => {
		if (isDesktop) {
			getWindowState().then(setWindowState)
			return onWindowStateChange(setWindowState)
		}
	}, [])

	return (
		<AuthProvider>
			<CallProvider>
				<HashRouter>
					<div
						className={cn(
							isDesktop && "h-screen w-full bg-background text-foreground relative",
							isDesktop && windowState !== "maximized" && "rounded-xl shadow-lg overflow-hidden"
						)}
					>
						<ThemeProvider defaultTheme="system" storageKey="wa-bot-theme">
							<AppThemeProvider>
								<TooltipProvider>
									<AppRoutes />
									<IncomingCallOverlay />
									<CallOverlay />
									<Toaster position="top-center" />
								</TooltipProvider>
							</AppThemeProvider>
						</ThemeProvider>
					</div>
				</HashRouter>
			</CallProvider>
		</AuthProvider>
	)
}

export default App
