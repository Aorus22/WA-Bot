import { Check, Paintbrush, LogOut, LogIn, Wifi, WifiOff, AlertCircle, Sparkles, Mic, Loader2, History } from 'lucide-react'
import { themes } from '@/data/themes'
import { useAppTheme, setAppTheme } from '@/components/AppThemeProvider'
import { cn } from '@/lib/utils'
import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '@/contexts/AuthContext'
import { api, type HistorySyncStatus, type SettingsMap } from '@/lib/api'
import { useChatStore } from '@/stores/chatStore'
import { toast } from 'sonner'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@/components/ui/select'
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from '@/components/ui/alert-dialog'

type ThemeMode = 'all' | 'dark' | 'light'

function getLuminance(hex: string) {
	const r = parseInt(hex.slice(1, 3), 16) / 255
	const g = parseInt(hex.slice(3, 5), 16) / 255
	const b = parseInt(hex.slice(5, 7), 16) / 255
	const a = [r, g, b].map(v => v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4))
	return a[0] * 0.2126 + a[1] * 0.7152 + a[2] * 0.0722
}

export function SettingsPage() {
	const [current, setCurrent] = useState(() => useAppTheme())
	const [mode, setMode] = useState<ThemeMode>(() => {
		const stored = typeof window !== 'undefined' ? localStorage.getItem('wa-bot-theme-mode') : null
		return (stored === 'all' || stored === 'dark' || stored === 'light') ? stored : 'dark'
	})
	const [logoutOpen, setLogoutOpen] = useState(false)
	const navigate = useNavigate()
	const { isLoggedIn, isConnected, logout } = useAuth()

	// ── AI + TTS settings ─────────────────────────────────────────────────────
	const [settingsLoading, setSettingsLoading] = useState(true)
	const [hasGeminiKey, setHasGeminiKey] = useState(false)
	const [hasFishKey, setHasFishKey] = useState(false)

	const [geminiApiKey, setGeminiApiKey] = useState('')
	const [aiServerUrl, setAiServerUrl] = useState('')
	const [savingAi, setSavingAi] = useState(false)

	const [ttsProvider, setTtsProvider] = useState('')
	const [ttsDefaultVoice, setTtsDefaultVoice] = useState('')
	const [fishAudioKey, setFishAudioKey] = useState('')
	const [fishAudioModel, setFishAudioModel] = useState('')
	const [fishAudioVoiceId, setFishAudioVoiceId] = useState('')
	const [savingTts, setSavingTts] = useState(false)
	const [historyStatus, setHistoryStatus] = useState<HistorySyncStatus | null>(null)
	const [historyLoading, setHistoryLoading] = useState(true)
	const historyWasRunning = useRef(false)

	const loadHistoryStatus = useCallback(async () => {
		try {
			const status = await api.getHistorySyncStatus()
			setHistoryStatus(status)
			if (historyWasRunning.current && status.state !== 'running') {
				const chats = await api.getChats()
				useChatStore.getState().setChats(chats || [])
				useChatStore.getState().invalidateMessages()
				toast.success(`History sync added ${status.messagesAdded} messages`)
			}
			historyWasRunning.current = status.state === 'running'
		} catch {
			// The card remains available and will retry when the user clicks it.
		} finally {
			setHistoryLoading(false)
		}
	}, [])

	useEffect(() => {
		void loadHistoryStatus()
	}, [loadHistoryStatus])

	useEffect(() => {
		if (historyStatus?.state !== 'running') return
		const timer = window.setInterval(() => void loadHistoryStatus(), 1000)
		return () => window.clearInterval(timer)
	}, [historyStatus?.state, loadHistoryStatus])

	const startHistorySync = async () => {
		setHistoryLoading(true)
		try {
			const status = await api.startHistorySync()
			historyWasRunning.current = true
			setHistoryStatus(status)
			toast.success('History sync started')
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Failed to start history sync')
			await loadHistoryStatus()
		} finally {
			setHistoryLoading(false)
		}
	}

	useEffect(() => {
		let cancelled = false
		api
			.getSettings()
			.then((res) => {
				if (cancelled) return
				setHasGeminiKey(Boolean(res.hasGeminiKey))
				setHasFishKey(Boolean(res.hasFishKey))
				setAiServerUrl(res.ai_server_url ?? '')
				setTtsProvider(res.call_tts_provider ?? '')
				setTtsDefaultVoice(res.call_tts_default_voice ?? '')
				setFishAudioModel(res.call_tts_fish_audio_model ?? '')
				setFishAudioVoiceId(res.call_tts_fish_audio_voice_id ?? '')
			})
			.catch(() => {
				if (!cancelled) toast.error('Failed to load settings')
			})
			.finally(() => {
				if (!cancelled) setSettingsLoading(false)
			})
		return () => {
			cancelled = true
		}
	}, [])

	const saveAiSettings = async () => {
		setSavingAi(true)
		try {
			const data: SettingsMap = { ai_server_url: aiServerUrl.trim() }
			// Only send the secret if the user typed a new value — blank keeps the stored one.
			if (geminiApiKey.trim()) data.gemini_api_key = geminiApiKey.trim()
			const res = await api.updateSettings(data)
			setHasGeminiKey(Boolean(res.hasGeminiKey))
			setAiServerUrl(res.ai_server_url ?? '')
			setGeminiApiKey('')
			toast.success('AI configuration saved')
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Failed to save AI configuration')
		} finally {
			setSavingAi(false)
		}
	}

	const saveTtsSettings = async () => {
		setSavingTts(true)
		try {
			const data: SettingsMap = {
				call_tts_provider: ttsProvider,
				call_tts_default_voice: ttsDefaultVoice.trim(),
				call_tts_fish_audio_model: fishAudioModel.trim(),
				call_tts_fish_audio_voice_id: fishAudioVoiceId.trim(),
			}
			if (fishAudioKey.trim()) data.call_tts_fish_audio_key = fishAudioKey.trim()
			const res = await api.updateSettings(data)
			setHasFishKey(Boolean(res.hasFishKey))
			setTtsProvider(res.call_tts_provider ?? '')
			setTtsDefaultVoice(res.call_tts_default_voice ?? '')
			setFishAudioModel(res.call_tts_fish_audio_model ?? '')
			setFishAudioVoiceId(res.call_tts_fish_audio_voice_id ?? '')
			setFishAudioKey('')
			toast.success('Call TTS settings saved')
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Failed to save TTS settings')
		} finally {
			setSavingTts(false)
		}
	}

	useEffect(() => {
		const handler = () => setCurrent(useAppTheme())
		window.addEventListener('storage', handler)
		return () => window.removeEventListener('storage', handler)
	}, [])

	const handleSelect = (name: string) => {
		setAppTheme(name)
		setCurrent(name)
	}

	const filtered = useMemo(() => {
		if (mode === 'all') return themes
		return themes.filter(t => {
			const lum = getLuminance(t.colors.background)
			return mode === 'dark' ? lum < 0.5 : lum >= 0.5
		})
	}, [mode])

	// Persist mode filter to localStorage
	useEffect(() => {
		localStorage.setItem('wa-bot-theme-mode', mode)
	}, [mode])

	// Auto-match theme when switching filter mode
	useEffect(() => {
		if (mode === 'all') return
		const currentTheme = themes.find(t => t.name === current)
		if (!currentTheme) return
		const currentLum = getLuminance(currentTheme.colors.background)
		const matches = mode === 'dark' ? currentLum < 0.5 : currentLum >= 0.5
		if (!matches) {
			const base = currentTheme.name.replace(/-(dark|light)$/, '')
			const counterpart = mode === 'dark' ? `${base}-dark` : `${base}-light`
			const found = themes.find(t => t.name === counterpart)
			if (found) {
				setAppTheme(found.name)
				setCurrent(found.name)
			}
		}
	}, [mode, current])

	return (
		<>
			<div className="flex flex-col items-center justify-start min-h-full p-4 md:p-8 overflow-y-auto pt-12">
				<div className="w-full max-w-2xl space-y-8 pb-12">
					<h1 className="text-2xl font-bold tracking-tight">Settings</h1>

					<div className="space-y-4">
						<h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Appearance</h2>
						<div className="bg-card rounded-lg border divide-y overflow-hidden">
							<div className="flex items-center justify-between px-4 py-3">
								<div className="flex items-center gap-3">
									<Paintbrush className="h-4 w-4 text-muted-foreground" />
									<div className="flex flex-col">
										<span className="text-sm font-medium">Theme Mode</span>
										<span className="text-xs text-muted-foreground">Filter by dark or light appearance</span>
									</div>
								</div>
								<Select value={mode} onValueChange={(v) => setMode(v as ThemeMode)}>
									<SelectTrigger className="w-[110px] h-8 text-xs">
										<SelectValue />
									</SelectTrigger>
									<SelectContent>
										<SelectItem value="all">All themes</SelectItem>
										<SelectItem value="dark">Dark</SelectItem>
										<SelectItem value="light">Light</SelectItem>
									</SelectContent>
								</Select>
							</div>

							<div className="px-4 py-4 space-y-4">
								<div className="flex flex-col gap-0.5">
									<span className="text-sm font-medium">Color Theme</span>
									<span className="text-xs text-muted-foreground">{filtered.length} themes available</span>
								</div>
								<div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
									{filtered.map((theme) => {
										const isActive = current === theme.name
										const c = theme.colors
										return (
											<button
												key={theme.name}
												onClick={() => handleSelect(theme.name)}
												className={cn(
													"group relative flex flex-col gap-2 p-2 rounded-lg border-2 transition-all text-left",
													isActive
														? "border-primary/40 bg-accent"
														: "border-border/50 bg-secondary hover:bg-secondary/80"
												)}
											>
												<div
													className="h-14 w-full rounded-md flex flex-col p-2 gap-1 overflow-hidden"
													style={{ backgroundColor: c.background }}
												>
													<div className="flex gap-1">
														<div className="h-2 w-2 rounded-full" style={{ backgroundColor: c.primary }} />
														<div className="h-2 w-2 rounded-full" style={{ backgroundColor: c.accent }} />
														<div className="h-2 w-2 rounded-full" style={{ backgroundColor: c.destructive }} />
													</div>
													<div className="flex-1 flex items-end gap-1">
														<div className="w-2/3 h-1 rounded-sm" style={{ backgroundColor: c.foreground }} />
														<div className="w-1/4 h-1 rounded-sm" style={{ backgroundColor: c.mutedForeground }} />
													</div>
												</div>
												<span className="text-xs font-medium truncate px-1 text-muted-foreground group-hover:text-foreground">
													{theme.label}
												</span>
												{isActive && (
													<Check className="absolute top-2 right-2 h-3.5 w-3.5 text-primary" />
												)}
											</button>
										)
									})}
								</div>
							</div>
						</div>
					</div>

					<div className="space-y-4">
						<h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Account</h2>
						<div className="bg-card rounded-lg border divide-y overflow-hidden">
							<div className="flex items-center justify-between px-4 py-3">
								<div className="flex items-center gap-3">
									{isConnected ? (
										<Wifi className="h-4 w-4 text-green-500" />
									) : (
										<WifiOff className="h-4 w-4 text-muted-foreground" />
									)}
									<div className="flex flex-col">
										<span className="text-sm font-medium">
											{isLoggedIn ? (isConnected ? 'Connected' : 'Reconnecting...') : 'Not connected'}
										</span>
										<span className="text-xs text-muted-foreground">
											{isLoggedIn ? 'WhatsApp bot is active' : 'No WhatsApp account linked'}
										</span>
									</div>
								</div>
								{isLoggedIn ? (
									<button
										onClick={() => setLogoutOpen(true)}
										className="text-xs px-3 py-1.5 rounded-lg bg-destructive/10 text-destructive font-medium hover:bg-destructive/20 transition-colors flex items-center gap-1.5"
									>
										<LogOut className="h-3.5 w-3.5" />
										Logout
									</button>
								) : (
									<button
										onClick={() => navigate('/login')}
										className="text-xs px-3 py-1.5 rounded-lg bg-primary/10 text-primary font-medium hover:bg-primary/20 transition-colors flex items-center gap-1.5"
									>
										<LogIn className="h-3.5 w-3.5" />
										Login
									</button>
								)}
							</div>
							<div className="px-4 py-4 space-y-3">
								<div className="flex items-start justify-between gap-4">
									<div className="flex items-start gap-3">
										<History className="h-4 w-4 mt-0.5 text-muted-foreground" />
										<div className="flex flex-col gap-0.5">
											<span className="text-sm font-medium">Chat history</span>
											<span className="text-xs text-muted-foreground">
												Add up to 50 older messages per chat. Existing messages are never removed.
											</span>
										</div>
									</div>
									<Button
										size="sm"
										onClick={startHistorySync}
										disabled={!isConnected || historyLoading || historyStatus?.state === 'running'}
									>
										{historyStatus?.state === 'running' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
										{historyStatus?.state === 'running' ? 'Syncing…' : 'Sync 50 per chat'}
									</Button>
								</div>
								{historyStatus && (
									<div className="space-y-2 text-xs text-muted-foreground">
										<div className="flex flex-wrap gap-x-4 gap-y-1">
											<span>Status: <b className="text-foreground capitalize">{historyStatus.state}</b></span>
											<span>Staged: {historyStatus.pendingMessages} messages in {historyStatus.pendingChats} chats</span>
											<span>Added: {historyStatus.messagesAdded}</span>
										</div>
										{historyStatus.state === 'running' && historyStatus.chatsTotal > 0 && (
											<div className="h-1.5 rounded-full bg-muted overflow-hidden">
												<div
													className="h-full bg-[#00a884] transition-all"
													style={{ width: `${Math.round(historyStatus.chatsProcessed / historyStatus.chatsTotal * 100)}%` }}
												/>
											</div>
										)}
						{(historyStatus.errors ?? []).length > 0 && (
							<p className="text-destructive">
								{(historyStatus.errors ?? []).length} chat(s) had errors. Last: {historyStatus.errors?.at(-1)?.message}
											</p>
										)}
									</div>
								)}
							</div>
						</div>
					</div>

					<div className="space-y-4">
						<h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">AI Configuration</h2>
						<div className="bg-card rounded-lg border divide-y overflow-hidden">
							<div className="flex items-center gap-3 px-4 py-3">
								<Sparkles className="h-4 w-4 text-muted-foreground" />
								<div className="flex flex-col">
									<span className="text-sm font-medium">Gemini + AI Server</span>
									<span className="text-xs text-muted-foreground">Connect the bot to Gemini and your AI backend</span>
								</div>
							</div>
							<div className="px-4 py-4 space-y-4">
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="gemini_api_key" className="text-xs font-medium">Gemini API Key</Label>
									<Input
										id="gemini_api_key"
										type="password"
										value={geminiApiKey}
										onChange={(e) => setGeminiApiKey(e.target.value)}
										placeholder={hasGeminiKey ? '••••••••' : 'Enter your Gemini API key'}
										autoComplete="new-password"
										disabled={settingsLoading}
									/>
									<p className="text-xs text-muted-foreground">Stored securely, leave blank to keep</p>
								</div>
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="ai_server_url" className="text-xs font-medium">AI Server URL</Label>
									<Input
										id="ai_server_url"
										type="url"
										value={aiServerUrl}
										onChange={(e) => setAiServerUrl(e.target.value)}
										placeholder="http://localhost:8981"
										disabled={settingsLoading}
									/>
								</div>
								<div className="flex justify-end">
									<Button size="sm" onClick={saveAiSettings} disabled={settingsLoading || savingAi}>
										{savingAi && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
										{savingAi ? 'Saving...' : 'Save'}
									</Button>
								</div>
							</div>
						</div>
					</div>

					<div className="space-y-4">
						<h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Call TTS</h2>
						<div className="bg-card rounded-lg border divide-y overflow-hidden">
							<div className="flex items-center gap-3 px-4 py-3">
								<Mic className="h-4 w-4 text-muted-foreground" />
								<div className="flex flex-col">
									<span className="text-sm font-medium">Text-to-Speech Provider</span>
									<span className="text-xs text-muted-foreground">Choose how incoming calls are spoken</span>
								</div>
							</div>
							<div className="px-4 py-4 space-y-4">
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="call_tts_provider" className="text-xs font-medium">Provider</Label>
									<Select
										value={ttsProvider === '' ? 'none' : ttsProvider}
										onValueChange={(v) => setTtsProvider(v === 'none' ? '' : v)}
										disabled={settingsLoading}
									>
										<SelectTrigger className="w-full">
											<SelectValue placeholder="Select a provider" />
										</SelectTrigger>
										<SelectContent>
											<SelectItem value="none">Disabled</SelectItem>
											<SelectItem value="fish">Fish Audio</SelectItem>
											<SelectItem value="edge">Edge TTS</SelectItem>
										</SelectContent>
									</Select>
								</div>
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="call_tts_default_voice" className="text-xs font-medium">Default Voice</Label>
									<Input
										id="call_tts_default_voice"
										value={ttsDefaultVoice}
										onChange={(e) => setTtsDefaultVoice(e.target.value)}
										placeholder="e.g. en-US-JennyNeural"
										disabled={settingsLoading}
									/>
								</div>
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="call_tts_fish_audio_key" className="text-xs font-medium">Fish Audio API Key</Label>
									<Input
										id="call_tts_fish_audio_key"
										type="password"
										value={fishAudioKey}
										onChange={(e) => setFishAudioKey(e.target.value)}
										placeholder={hasFishKey ? '••••••••' : 'Enter your Fish Audio API key'}
										autoComplete="new-password"
										disabled={settingsLoading}
									/>
									<p className="text-xs text-muted-foreground">Stored securely, leave blank to keep</p>
								</div>
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="call_tts_fish_audio_model" className="text-xs font-medium">Fish Audio Model</Label>
									<Input
										id="call_tts_fish_audio_model"
										value={fishAudioModel}
										onChange={(e) => setFishAudioModel(e.target.value)}
										placeholder="e.g. fishaudio/fish-speech-1.5"
										disabled={settingsLoading}
									/>
								</div>
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="call_tts_fish_audio_voice_id" className="text-xs font-medium">Fish Audio Voice ID</Label>
									<Input
										id="call_tts_fish_audio_voice_id"
										value={fishAudioVoiceId}
										onChange={(e) => setFishAudioVoiceId(e.target.value)}
										placeholder="Voice reference ID"
										disabled={settingsLoading}
									/>
								</div>
								<div className="flex justify-end">
									<Button size="sm" onClick={saveTtsSettings} disabled={settingsLoading || savingTts}>
										{savingTts && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
										{savingTts ? 'Saving...' : 'Save'}
									</Button>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>

			<AlertDialog open={logoutOpen} onOpenChange={setLogoutOpen}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<div className="flex items-center gap-2 text-destructive mb-2">
							<AlertCircle className="h-5 w-5" />
							<AlertDialogTitle>Confirm Logout</AlertDialogTitle>
						</div>
						<AlertDialogDescription>
							Are you sure you want to log out? You will need to scan the QR code again to reconnect.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel className="rounded-full">Cancel</AlertDialogCancel>
						<AlertDialogAction onClick={logout} className="bg-destructive text-destructive-foreground hover:bg-destructive/90 rounded-full">
							Log Out
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	)
}
