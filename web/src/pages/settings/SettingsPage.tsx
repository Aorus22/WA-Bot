import { Check, Palette, LogOut, LogIn, Wifi, WifiOff, AlertCircle, Bot, Mic, Loader2, History, ChevronRight } from 'lucide-react'
import { themes } from '@/data/themes'
import { useAppTheme, setAppTheme, isGtkStyle, setGtkStyle, isWinUiStyle, setWinUiStyle } from '@/components/AppThemeProvider'
import { cn } from '@/lib/utils'
import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '@/contexts/AuthContext'
import { api, type HistorySyncStatus, type SettingsMap } from '@/lib/api'
import {
	SYNC_DEPTHS, type SyncDepthId, isActive, stateLabel, progressFraction, inProgressFraction, elapsedText, etaText, chatsText,
	pausedText, finishedChatsText, currentText, resultText, stagedText, errorText,
} from '@/lib/historySync'
import { useChatStore } from '@/stores/chatStore'
import { HistoryMediaSection } from './HistoryMediaSection'
import { BackupSection } from './BackupSection'
import { isMessageFormattingEnabled, setMessageFormattingEnabled } from '@/pages/chat/renderMd'
import { toast } from 'sonner'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@/components/ui/select'
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from '@/components/ui/dialog'
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
type AiProvider = 'gemini' | 'openai' | 'claude'

const AI_PROVIDERS: { id: AiProvider; label: string }[] = [
	{ id: 'gemini', label: 'Gemini' },
	{ id: 'openai', label: 'OpenAI-compatible API' },
	{ id: 'claude', label: 'Claude-compatible API' },
]

const AI_BASE_DEFAULTS: Record<Exclude<AiProvider, 'gemini'>, string> = {
	openai: 'https://api.openai.com/v1',
	claude: 'https://api.anthropic.com',
}

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
	const [themeOpen, setThemeOpen] = useState(false)
	const [gtkStyle, setGtkStyleState] = useState(isGtkStyle)
	const [winuiStyle, setWinUiStyleState] = useState(isWinUiStyle)
	const navigate = useNavigate()
	const { isLoggedIn, isConnected, logout } = useAuth()

	// ── AI + TTS settings ─────────────────────────────────────────────────────
	const [settingsLoading, setSettingsLoading] = useState(true)
	const [hasGeminiKey, setHasGeminiKey] = useState(false)
	const [hasFishKey, setHasFishKey] = useState(false)

	const [geminiApiKey, setGeminiApiKey] = useState('')
	const [aiServerUrl, setAiServerUrl] = useState('')
	const [aiProvider, setAiProvider] = useState<AiProvider>('gemini')
	const [aiBaseUrl, setAiBaseUrl] = useState('')
	const [aiModel, setAiModel] = useState('')
	const [aiApiKey, setAiApiKey] = useState('')
	const [hasAiKey, setHasAiKey] = useState(false)
	const [savingAi, setSavingAi] = useState(false)
	const [readReceipts, setReadReceipts] = useState(true)
	const [savingReceipts, setSavingReceipts] = useState(false)
	const [formatMessages, setFormatMessages] = useState(isMessageFormattingEnabled)
	const [contactNameMode, setContactNameMode] = useState<'profile' | 'saved'>('profile')
	const [savingNames, setSavingNames] = useState(false)

	const [ttsProvider, setTtsProvider] = useState('')
	const [ttsDefaultVoice, setTtsDefaultVoice] = useState('')
	const [fishAudioKey, setFishAudioKey] = useState('')
	const [fishAudioModel, setFishAudioModel] = useState('')
	const [fishAudioVoiceId, setFishAudioVoiceId] = useState('')
	const [savingTts, setSavingTts] = useState(false)
	const [historyStatus, setHistoryStatus] = useState<HistorySyncStatus | null>(null)
	const [historyLoading, setHistoryLoading] = useState(true)
	const historyWasRunning = useRef(false)
	const [historyDepth, setHistoryDepth] = useState<SyncDepthId>('per50')
	const [retryFinished, setRetryFinished] = useState(false)
	const historyActive = historyStatus ? isActive(historyStatus.state) : false

	const loadHistoryStatus = useCallback(async () => {
		try {
			const status = await api.getHistorySyncStatus()
			setHistoryStatus(status)
			if (historyWasRunning.current && !isActive(status.state)) {
				const chats = await api.getChats()
				useChatStore.getState().setChats(chats || [])
				useChatStore.getState().invalidateMessages()
				if (status.state === 'failed') toast.error('History sync failed')
				else if (status.state === 'cancelled') toast(`History sync stopped: ${status.messagesAdded} messages added`)
				else toast.success(`History sync added ${status.messagesAdded} messages`)
			}
			historyWasRunning.current = isActive(status.state)
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
		if (!historyActive) return
		const timer = window.setInterval(() => {
			void loadHistoryStatus()
		}, 1000)
		return () => window.clearInterval(timer)
	}, [historyActive, loadHistoryStatus])

	const startHistorySync = async (resume = false) => {
		setHistoryLoading(true)
		try {
			const depth = SYNC_DEPTHS.find((d) => d.id === historyDepth) ?? SYNC_DEPTHS[0]
			const status = resume ? await api.resumeHistorySync() : await api.startHistorySync({ ...depth.request, retryFinished })
			historyWasRunning.current = true
			setHistoryStatus(status)
			toast.success(resume ? 'History sync resumed' : 'History sync started')
		} catch (err) {
			// One may already be running (the server answers 409): follow it instead of failing.
			const current = await api.getHistorySyncStatus().catch(() => null)
			if (current && isActive(current.state)) {
				setHistoryStatus(current)
				historyWasRunning.current = true
				toast('A sync is already running. Stop it first to start a different depth.')
			} else {
				toast.error(err instanceof Error ? err.message : resume ? 'Failed to resume history sync' : 'Failed to start history sync')
				await loadHistoryStatus()
			}
		} finally {
			setHistoryLoading(false)
		}
	}

	const cancelHistorySync = async () => {
		try {
			setHistoryStatus(await api.cancelHistorySync())
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Could not stop the history sync')
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
				setHasAiKey(Boolean(res.hasAiKey))
				setAiProvider(res.ai_provider === 'openai' || res.ai_provider === 'claude' ? res.ai_provider : 'gemini')
				setAiBaseUrl(res.ai_base_url ?? '')
				setAiModel(res.ai_model ?? '')
				setTtsProvider(res.call_tts_provider ?? '')
				setTtsDefaultVoice(res.call_tts_default_voice ?? '')
				setFishAudioModel(res.call_tts_fish_audio_model ?? '')
				setFishAudioVoiceId(res.call_tts_fish_audio_voice_id ?? '')
				setReadReceipts((res.read_receipts ?? 'true') !== 'false')
				setContactNameMode(res.contact_name_mode === 'saved' ? 'saved' : 'profile')
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
			const data: SettingsMap = {
				ai_server_url: aiServerUrl.trim(),
				ai_provider: aiProvider,
				ai_base_url: aiBaseUrl.trim(),
				ai_model: aiModel.trim(),
			}
			// Only send the secrets if the user typed a new value: blank keeps the stored one.
			if (geminiApiKey.trim()) data.gemini_api_key = geminiApiKey.trim()
			if (aiApiKey.trim()) data.ai_api_key = aiApiKey.trim()
			const res = await api.updateSettings(data)
			setHasGeminiKey(Boolean(res.hasGeminiKey))
			setHasAiKey(Boolean(res.hasAiKey))
			setAiServerUrl(res.ai_server_url ?? '')
			setGeminiApiKey('')
			setAiApiKey('')
			toast.success('AI configuration saved')
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Failed to save AI configuration')
		} finally {
			setSavingAi(false)
		}
	}

	const toggleFormatMessages = (enabled: boolean) => {
		setMessageFormattingEnabled(enabled)
		setFormatMessages(enabled)
	}

	const chooseContactNameMode = async (mode: 'profile' | 'saved') => {
		if (mode === contactNameMode) return
		const previous = contactNameMode
		setContactNameMode(mode)
		setSavingNames(true)
		try {
			const res = await api.updateSettings({ contact_name_mode: mode })
			setContactNameMode(res.contact_name_mode === 'saved' ? 'saved' : 'profile')
		} catch (err) {
			setContactNameMode(previous)
			toast.error(err instanceof Error ? err.message : 'Failed to save setting')
		} finally {
			setSavingNames(false)
		}
	}

	const toggleReadReceipts = async (enabled: boolean) => {
		setSavingReceipts(true)
		try {
			const res = await api.updateSettings({ read_receipts: enabled ? 'true' : 'false' })
			setReadReceipts((res.read_receipts ?? (enabled ? 'true' : 'false')) !== 'false')
			toast.success(enabled ? 'Read receipts enabled' : 'Read receipts disabled')
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Failed to save setting')
		} finally {
			setSavingReceipts(false)
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

	const activeTheme = themes.find((t) => t.name === current) ?? themes[0]

	return (
		<>
			<div className="flex flex-col items-center justify-start min-h-full p-4 md:p-8 overflow-y-auto pt-12">
				<div data-settings-page className="w-full max-w-2xl space-y-8 pb-12">
					<h1 className="text-2xl font-bold tracking-tight">Settings</h1>

					<div className="space-y-4">
						<h2 data-settings-section-title className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Appearance</h2>
						<div className="bg-card rounded-group border divide-y overflow-hidden">
							<button
								type="button"
								onClick={() => setThemeOpen(true)}
								className="flex w-full items-center justify-between gap-4 px-4 py-3 text-left hover:bg-muted/50 transition-colors"
							>
								<div className="flex items-center gap-3">
									<Palette className="h-4 w-4 text-muted-foreground" />
									<div className="flex flex-col">
										<span className="text-sm font-medium">Color theme</span>
										<span className="text-xs text-muted-foreground">{activeTheme.label}</span>
									</div>
								</div>
								<div className="flex items-center gap-2">
									<div
										className="h-8 w-[72px] rounded-md border flex items-center gap-1 px-2"
										style={{ backgroundColor: activeTheme.colors.background }}
									>
										<div className="h-2 w-2 rounded-full" style={{ backgroundColor: activeTheme.colors.primary }} />
										<div className="h-2 w-2 rounded-full" style={{ backgroundColor: activeTheme.colors.accent }} />
										<div className="h-2 w-2 rounded-full" style={{ backgroundColor: activeTheme.colors.destructive }} />
									</div>
									<ChevronRight className="h-4 w-4 text-muted-foreground" />
								</div>
							</button>
							<div className="flex items-center gap-3 px-4 py-3">
								<Palette className="h-4 w-4 text-muted-foreground" />
								<div className="flex flex-col flex-1 min-w-0">
									<span className="text-sm font-medium">GTK style</span>
									<span className="text-xs text-muted-foreground">Flat, rounded shapes like a GNOME app. Works with any colour theme</span>
								</div>
								<Switch
									checked={gtkStyle}
									onCheckedChange={(v) => {
										setGtkStyleState(Boolean(v))
										if (v) setWinUiStyleState(false)
										setGtkStyle(Boolean(v))
									}}
								/>
							</div>
							<div className="flex items-center gap-3 px-4 py-3">
								<Palette className="h-4 w-4 text-muted-foreground" />
								<div className="flex flex-col flex-1 min-w-0">
									<span className="text-sm font-medium">WinUI style</span>
									<span className="text-xs text-muted-foreground">Windows 11 Fluent shapes and Acrylic. Works with any colour theme</span>
								</div>
								<Switch
									checked={winuiStyle}
									onCheckedChange={(v) => {
										setWinUiStyleState(Boolean(v))
										if (v) setGtkStyleState(false)
										setWinUiStyle(Boolean(v))
									}}
								/>
							</div>
						</div>
					</div>

					<div className="space-y-4">
						<h2 data-settings-section-title className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Account</h2>
						<div className="bg-card rounded-group border divide-y overflow-hidden">
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
							<div className="px-4 py-4 space-y-4">
								<div className="flex items-start justify-between gap-4">
									<div className="flex items-start gap-3">
										<History className="h-4 w-4 mt-0.5 text-muted-foreground" />
										<div className="flex flex-col gap-0.5">
											<span className="text-sm font-medium">Chat history</span>
											<span className="text-xs text-muted-foreground">
												Load older messages from your phone. Existing messages are never removed.
											</span>
										</div>
									</div>
									{historyActive ? (
										<Button
											size="sm"
											variant="outline"
											className="text-destructive"
											onClick={cancelHistorySync}
											disabled={historyStatus?.state === 'cancelling'}
										>
											{historyStatus?.state === 'cancelling' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
											{historyStatus?.state === 'cancelling' ? 'Stopping…' : 'Stop sync'}
										</Button>
									) : historyStatus?.resumable ? (
										<div className="flex gap-2">
											<Button size="sm" variant="outline" onClick={() => startHistorySync(false)} disabled={!isConnected || historyLoading}>
												Start over
											</Button>
											<Button size="sm" onClick={() => startHistorySync(true)} disabled={!isConnected || historyLoading}>
												Resume
											</Button>
										</div>
									) : (
										<Button size="sm" onClick={() => startHistorySync(false)} disabled={!isConnected || historyLoading}>
											Sync history
										</Button>
									)}
								</div>
								<div className="flex items-center justify-between gap-4">
									<div className="flex flex-col gap-0.5">
										<span className="text-xs font-medium">How much to load</span>
										<span className="text-xs text-muted-foreground">{SYNC_DEPTHS.find((d) => d.id === historyDepth)?.hint}</span>
									</div>
									<Select value={historyDepth} onValueChange={(v) => setHistoryDepth(v as SyncDepthId)} disabled={historyActive}>
										<SelectTrigger className="w-[168px] h-8 text-xs shrink-0">
											<SelectValue />
										</SelectTrigger>
										<SelectContent>
											{SYNC_DEPTHS.map((depth) => (
												<SelectItem key={depth.id} value={depth.id}>{depth.label}</SelectItem>
											))}
										</SelectContent>
									</Select>
								</div>
								<div className="flex items-center justify-between gap-4">
									<div className="flex flex-col gap-0.5">
										<Label htmlFor="history_retry_finished" className="text-xs font-medium">Retry synced chats</Label>
										<span className="text-xs text-muted-foreground">{finishedChatsText(historyStatus)}</span>
									</div>
									<Switch
										id="history_retry_finished"
										checked={retryFinished}
										onCheckedChange={setRetryFinished}
										disabled={historyActive}
									/>
								</div>
								{historyStatus && (historyStatus.startedAt != null || historyActive) && (() => {
									const done = Math.round(progressFraction(historyStatus) * 1000) / 10
									const working = Math.round(inProgressFraction(historyStatus) * 1000) / 10
									const headline = [stateLabel(historyStatus.state), chatsText(historyStatus)].filter(Boolean).join(' · ')
									const timing = [elapsedText(historyStatus), etaText(historyStatus)].filter(Boolean).join(' · ')
									const paused = pausedText(historyStatus)
									const staged = stagedText(historyStatus)
									const error = errorText(historyStatus)
									return (
										<div className="space-y-1.5 border-t pt-3 text-xs text-muted-foreground">
											{historyActive ? (
												<>
													<div className="font-medium text-foreground">{headline}</div>
													<div
														role="progressbar"
														aria-valuemin={0}
														aria-valuemax={historyStatus.chatsTotal}
														aria-valuenow={historyStatus.chatsProcessed}
														className="flex h-1.5 rounded-full bg-muted overflow-hidden"
													>
														<div className="h-full bg-[#00a884] transition-all" style={{ width: `${Math.max(done === 0 ? 1 : 0, done)}%` }} />
														<div className="h-full bg-[#00a884]/40 animate-pulse transition-all" style={{ width: `${working}%` }} />
													</div>
													{timing && <div>{timing}</div>}
													{historyStatus.current.slice(0, 3).map((c) => (
														<div key={c.chatId}>Now: {currentText(c)}</div>
													))}
													{historyStatus.current.length === 0 && historyStatus.chatsTotal > 0 && <div>Now: picking the next chat…</div>}
												</>
											) : null}
											{paused && <p className="text-amber-600 dark:text-amber-400">{paused}</p>}
											<div>{resultText(historyStatus)}</div>
											{staged && <div>{staged}</div>}
											{error && <p className="text-destructive">{error}</p>}
										</div>
									)
								})()}
							</div>
							<HistoryMediaSection isConnected={isConnected} />
							<BackupSection />
						</div>
					</div>

					<div className="space-y-4">
						<h2 data-settings-section-title className="text-sm font-medium text-muted-foreground uppercase tracking-wider">AI</h2>
						<div className="bg-card rounded-group border divide-y overflow-hidden">
							<div className="flex items-center justify-between gap-4 px-4 py-3">
								<div className="flex items-center gap-3">
									<Bot className="h-4 w-4 text-muted-foreground" />
									<div className="flex flex-col">
										<span className="text-sm font-medium">AI provider</span>
										<span className="text-xs text-muted-foreground">Used by scripts and the script assistant</span>
									</div>
								</div>
								<Select value={aiProvider} onValueChange={(v) => setAiProvider(v as AiProvider)} disabled={settingsLoading}>
									<SelectTrigger className="w-[200px] h-8 text-xs">
										<SelectValue />
									</SelectTrigger>
									<SelectContent>
										{AI_PROVIDERS.map((p) => (
											<SelectItem key={p.id} value={p.id}>{p.label}</SelectItem>
										))}
									</SelectContent>
								</Select>
							</div>
							<div className="px-4 py-4 space-y-4">
								{aiProvider === 'gemini' ? (
									<div className="flex flex-col gap-1.5">
										<Label htmlFor="gemini_api_key" className="text-xs font-medium">Gemini API key</Label>
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
								) : (
									<>
										<div className="flex flex-col gap-1.5">
											<Label htmlFor="ai_base_url" className="text-xs font-medium">API base URL</Label>
											<Input
												id="ai_base_url"
												type="url"
												value={aiBaseUrl}
												onChange={(e) => setAiBaseUrl(e.target.value)}
												placeholder={`${AI_BASE_DEFAULTS[aiProvider]} (default)`}
												disabled={settingsLoading}
											/>
										</div>
										<div className="flex flex-col gap-1.5">
											<Label htmlFor="ai_api_key" className="text-xs font-medium">API key</Label>
											<Input
												id="ai_api_key"
												type="password"
												value={aiApiKey}
												onChange={(e) => setAiApiKey(e.target.value)}
												placeholder={hasAiKey ? '••••••••' : 'Enter the API key'}
												autoComplete="new-password"
												disabled={settingsLoading}
											/>
											<p className="text-xs text-muted-foreground">Stored securely, leave blank to keep</p>
										</div>
										<div className="flex flex-col gap-1.5">
											<Label htmlFor="ai_model" className="text-xs font-medium">Model</Label>
											<Input
												id="ai_model"
												value={aiModel}
												onChange={(e) => setAiModel(e.target.value)}
												placeholder="Model name, as the provider spells it"
												disabled={settingsLoading}
											/>
										</div>
									</>
								)}
								<div className="flex flex-col gap-1.5">
									<Label htmlFor="ai_server_url" className="text-xs font-medium">AI server URL</Label>
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
						<h2 data-settings-section-title className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Messages</h2>
						<div className="bg-card rounded-group border divide-y overflow-hidden">
							<div className="flex items-center gap-3 px-4 py-3.5">
								<div className="flex flex-col flex-1 min-w-0">
									<span className="text-sm font-medium">Format message text</span>
									<span className="text-xs text-muted-foreground">Show *bold*, _italic_, ~strikethrough~ and `monospace` like WhatsApp.</span>
								</div>
								<Switch checked={formatMessages} onCheckedChange={(v) => toggleFormatMessages(Boolean(v))} />
							</div>
							<div className="px-4 py-4 space-y-2">
								<span className="text-sm font-medium">Contact names</span>
								{([
									['saved', 'Saved contact name', "The name from your phone's contacts. Contacts you haven't saved show their phone number, like WhatsApp."],
									['profile', 'Profile name', 'The name each person chose for themselves.'],
								] as const).map(([value, title, description]) => (
									<button
										key={value}
										type="button"
										disabled={savingNames}
										onClick={() => chooseContactNameMode(value)}
										className={cn(
											'flex w-full items-center gap-3 rounded-lg border px-4 py-3 text-left transition-colors hover:bg-muted/50',
											contactNameMode === value && 'border-primary',
										)}
									>
										<span className={cn('flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full border-2', contactNameMode === value ? 'border-primary' : 'border-muted-foreground')}>
											{contactNameMode === value && <span className="h-2 w-2 rounded-full bg-primary" />}
										</span>
										<span className="flex flex-col min-w-0">
											<span className="text-sm font-medium">{title}</span>
											<span className="text-xs text-muted-foreground">{description}</span>
										</span>
									</button>
								))}
							</div>
						</div>
					</div>

					<div className="space-y-4">
						<h2 data-settings-section-title className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Privacy</h2>
						<div className="bg-card rounded-group border overflow-hidden">
							<div className="flex items-center gap-3 px-4 py-3.5">
								<Check className="h-4 w-4 text-primary" />
								<div className="flex flex-col flex-1 min-w-0">
									<span className="text-sm font-medium">Read receipts</span>
									<span className="text-xs text-muted-foreground">Send blue ticks to WhatsApp when you open a chat. Off keeps reads local only.</span>
								</div>
								<Switch
									checked={readReceipts}
									onCheckedChange={(v) => toggleReadReceipts(Boolean(v))}
									disabled={savingReceipts}
								/>
							</div>
						</div>
					</div>

					<div className="space-y-4">
						<h2 data-settings-section-title className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Call TTS</h2>
						<div className="bg-card rounded-group border divide-y overflow-hidden">
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

			<Dialog open={themeOpen} onOpenChange={setThemeOpen}>
				<DialogContent className="max-w-3xl max-h-[85vh] flex flex-col gap-4">
					<DialogHeader>
						<div className="flex items-start justify-between gap-4 pr-8">
							<div className="flex flex-col gap-1">
								<DialogTitle>Choose a theme</DialogTitle>
								<DialogDescription>{filtered.length} themes</DialogDescription>
							</div>
							<div className="flex gap-1.5">
								{(['all', 'dark', 'light'] as const).map((m) => (
									<button
										key={m}
										type="button"
										onClick={() => setMode(m)}
										className={`text-xs px-2.5 py-1 rounded-md border transition-colors ${
											mode === m ? 'border-primary text-primary bg-muted' : 'hover:bg-muted'
										}`}
									>
										{m === 'all' ? 'All' : m === 'dark' ? 'Dark' : 'Light'}
									</button>
								))}
							</div>
						</div>
					</DialogHeader>
					<div className="overflow-y-auto min-h-0">
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
				</DialogContent>
			</Dialog>

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
						<AlertDialogCancel className="rounded-button">Cancel</AlertDialogCancel>
						<AlertDialogAction onClick={logout} className="bg-destructive text-destructive-foreground hover:bg-destructive/90 rounded-button">
							Log Out
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	)
}
