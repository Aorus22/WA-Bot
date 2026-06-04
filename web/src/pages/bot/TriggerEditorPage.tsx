import { useState, useEffect, useRef } from 'react'
import { ArrowLeft, Save, Play, CheckCircle2, X, Sparkles, Loader2, Pencil, FileCode } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card'
import { api, type Trigger } from '@/lib/api'
import { toast } from 'sonner'
import Editor, { DiffEditor } from '@monaco-editor/react'
import { cn } from '@/lib/utils'
import { AIAssistant } from '@/components/AIAssistant'
import { useNavigate, useLocation } from "react-router-dom"
import { useIsMobile } from "@/hooks/use-mobile"

function formatTimeAgo(dateStr?: string): string {
	if (!dateStr) return ''
	const now = Date.now()
	const then = new Date(dateStr).getTime()
	const diffMs = now - then
	const diffSec = Math.floor(diffMs / 1000)
	const diffMin = Math.floor(diffSec / 60)
	const diffHour = Math.floor(diffMin / 60)
	const diffDay = Math.floor(diffHour / 24)
	const diffWeek = Math.floor(diffDay / 7)
	const diffMonth = Math.floor(diffDay / 30)

	if (diffSec < 60) return 'just now'
	if (diffMin < 60) return `${diffMin} minute${diffMin > 1 ? 's' : ''} ago`
	if (diffHour < 24) return `${diffHour} hour${diffHour > 1 ? 's' : ''} ago`
	if (diffDay < 7) return `${diffDay} day${diffDay > 1 ? 's' : ''} ago`
	if (diffWeek < 5) return `${diffWeek} week${diffWeek > 1 ? 's' : ''} ago`
	return `${diffMonth} month${diffMonth > 1 ? 's' : ''} ago`
}

function formatTimestamp(date: Date): string {
	return date.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function parseJsonMessage(jsonStr: string): { message: string; error?: string } {
	try {
		const payload = JSON.parse(jsonStr)
		const body = payload?.message?.body ?? payload?.body ?? payload?.message ?? ''
		return { message: String(body) }
	} catch {
		return { message: '', error: 'Invalid JSON' }
	}
}

const DEFAULT_PAYLOAD = JSON.stringify({
	event: 'message.received',
	message: {
		from: '6281234567890',
		body: 'hello',
		timestamp: Math.floor(Date.now() / 1000),
	}
}, null, 2)

export function TriggerEditorPage() {
	const navigate = useNavigate()
	const isMobileView = useIsMobile()
	const location = useLocation()
	const triggerFromNav = (location.state as any)?.trigger as Trigger | undefined

	const handleBack = () => {
		if (window.history.length > 1) {
			navigate(-1)
		} else {
			navigate("/triggers")
		}
	}

	const [formData, setFormData] = useState<Partial<Trigger>>({
		name: '',
		pattern: '',
		script: `-- Available: sender, content, matches, send_text(), fetch()...\nlocal param = matches[1]\nprint("Simulating response for: " .. (param or "none"))\nsend_text(sender, "You said: " .. (param or "nothing"))`,
		priority: 0
	})
	const [isSaving, setIsSaving] = useState(false)
	const [isTesting, setIsTesting] = useState(false)
	const [proposedCode, setProposedCode] = useState<string | null>(null)
	const [isDebuggerOpen, setIsDebuggerOpen] = useState(false)
	const [jsonPayload, setJsonPayload] = useState(DEFAULT_PAYLOAD)
	const [consoleLogs, setConsoleLogs] = useState<Array<{ time: string; type: 'info' | 'success' | 'error' | 'invoke' | 'print'; message: string }>>([
		{ time: formatTimestamp(new Date()), type: 'info', message: 'Debugger initialized. Ready for execution.' }
	])
	const diffEditorRef = useRef<any>(null)
	const logsEndRef = useRef<HTMLDivElement>(null)

	useEffect(() => {
		if (triggerFromNav) {
			setFormData(triggerFromNav)
		}
	}, [triggerFromNav])

	useEffect(() => {
		logsEndRef.current?.scrollIntoView({ behavior: 'smooth' })
	}, [consoleLogs])

	const handleApplyAIChange = (newCode: string) => {
		setProposedCode(newCode)
	}

	const handleAcceptChange = () => {
		if (proposedCode) {
			setFormData(prev => ({ ...prev, script: proposedCode }))
			setProposedCode(null)
		}
	}

	const handleRejectChange = () => setProposedCode(null)

	const handleSave = async () => {
		if (!formData.name || !formData.pattern || !formData.script) {
			toast.error('Please fill in all required fields')
			return
		}
		try {
			setIsSaving(true)
			if (formData.id) {
				await api.updateTrigger(formData.id, formData as Trigger)
				toast.success('Trigger updated successfully')
			} else {
				await api.createTrigger(formData as Trigger)
				toast.success('Trigger created successfully')
			}
			handleBack()
		} catch (error) {
			toast.error('Failed to save trigger')
		} finally {
			setIsSaving(false)
		}
	}

	const handleTest = async () => {
		const { message, error: parseError } = parseJsonMessage(jsonPayload)
		if (parseError) {
			setConsoleLogs(prev => [...prev, {
				time: formatTimestamp(new Date()),
				type: 'error',
				message: 'Failed to parse JSON payload. Please check syntax.'
			}])
			toast.error('Invalid JSON payload')
			return
		}
		if (!formData.pattern || !formData.script || !message) {
			toast.error('Pattern, script, and test message are required')
			return
		}
		try {
			setIsTesting(true)
			setConsoleLogs(prev => [...prev, {
				time: formatTimestamp(new Date()),
				type: 'invoke',
				message: `Starting execution for pattern: ${formData.pattern}`
			}])

			const result = await api.testTrigger({
				pattern: formData.pattern,
				script: formData.script,
				message: message
			})

			if (result.logs?.length > 0) {
				result.logs.forEach((log: string) => {
					setConsoleLogs(prev => [...prev, {
						time: formatTimestamp(new Date()),
						type: 'print',
						message: log
					}])
				})
			}

			if (result.error) {
				setConsoleLogs(prev => [...prev, {
					time: formatTimestamp(new Date()),
					type: 'error',
					message: result.error
				}])
			}

			if (result.matched) {
				setConsoleLogs(prev => [...prev, {
					time: formatTimestamp(new Date()),
					type: 'success',
					message: `Execution finished. Pattern matched. Returns: true`
				}])
				if (result.matches?.length > 0) {
					setConsoleLogs(prev => [...prev, {
						time: formatTimestamp(new Date()),
						type: 'info',
						message: `Captured groups: [${result.matches.join(', ')}]`
					}])
				}
			} else {
				setConsoleLogs(prev => [...prev, {
					time: formatTimestamp(new Date()),
					type: 'error',
					message: 'No match — pattern did not match the message.'
				}])
			}
		} catch (error: any) {
			setConsoleLogs(prev => [...prev, {
				time: formatTimestamp(new Date()),
				type: 'error',
				message: error.message || 'Execution failed'
			}])
		} finally {
			setIsTesting(false)
		}
	}

	const toggleDebugger = () => {
		if (!isDebuggerOpen) {
			setConsoleLogs([{
				time: formatTimestamp(new Date()),
				type: 'info',
				message: 'Debugger initialized. Ready for execution.'
			}])
		}
		setIsDebuggerOpen(prev => !prev)
	}

	const handleClearLogs = () => {
		setConsoleLogs([])
	}

	const subtitle = formData.id
		? `ID: ${formData.id}${formData.updated_at ? ` \u2022 Last edited ${formatTimeAgo(formData.updated_at)}` : ''}`
		: 'New trigger'

	return (
		<div className="h-full w-full bg-background flex flex-col min-h-0">
			{/* ─── Page Header ─── */}
			<div className="shrink-0 px-4 md:px-8 pt-4 md:pt-5 pb-3 md:pb-4 border-b border-border/40 bg-muted/10">
				<div className="flex items-start gap-3 min-w-0">
					<Button
						variant="ghost"
						size="icon"
						onClick={handleBack}
						className="rounded-full h-8 w-8 shrink-0 mt-0.5 -ml-1.5 text-muted-foreground hover:text-foreground"
					>
						<ArrowLeft className="h-4 w-4" />
					</Button>
					<div className="min-w-0">
						<h1 className="text-xl md:text-2xl font-semibold tracking-tight truncate">
							{formData.name || 'Untitled Trigger'}
						</h1>
						<p className="text-xs md:text-sm text-muted-foreground mt-0.5 font-mono tabular-nums">
							{subtitle}
						</p>
					</div>
				</div>
			</div>

			{/* ─── Scrollable Content ─── */}
			<div className="flex-1 overflow-y-auto w-full min-h-0">
				<div className="mx-auto w-full max-w-6xl px-4 md:px-8 py-6 md:py-8">
					{/* ─── General Settings ─── */}
					<Card className="mb-6">
						<CardHeader className="border-b border-border/40 pb-5">
							<CardTitle className="text-base">General Settings</CardTitle>
							<CardDescription>Basic configuration for your event trigger.</CardDescription>
						</CardHeader>
						<CardContent className="space-y-5">
							<div className="grid grid-cols-1 md:grid-cols-2 gap-5">
								<div className="space-y-2">
									<Label className="text-xs font-medium">Trigger Name</Label>
									<Input
										placeholder="e.g., Auto Reply Greeting"
										value={formData.name}
										onChange={e => setFormData(prev => ({ ...prev, name: e.target.value }))}
										className="h-10"
									/>
								</div>
								<div className="space-y-2">
									<Label className="text-xs font-medium">Priority</Label>
									<Input
										type="number"
										placeholder="0"
										value={formData.priority || 0}
										onChange={e => setFormData(prev => ({ ...prev, priority: parseInt(e.target.value) || 0 }))}
										className="h-10"
									/>
								</div>
							</div>
							<div className="space-y-2">
								<Label className="text-xs font-medium">Description <span className="text-muted-foreground font-normal">(optional)</span></Label>
								<Textarea
									placeholder="Briefly describe what this trigger does..."
									value={formData.description || ''}
									onChange={e => setFormData(prev => ({ ...prev, description: e.target.value }))}
									className="resize-none min-h-[60px]"
									rows={2}
								/>
							</div>
							<div className="space-y-2">
								<Label className="text-xs font-medium">Regex Pattern</Label>
								<Input
									placeholder="^!test\s+(.*)"
									value={formData.pattern}
									onChange={e => setFormData(prev => ({ ...prev, pattern: e.target.value }))}
									className="h-10 font-mono"
								/>
							</div>
						</CardContent>
					</Card>

					{/* ─── Lua Script Editor ─── */}
					<div className="space-y-3">
						<div className="flex items-center gap-2">
							<FileCode className="h-4 w-4 text-muted-foreground" />
							<Label className="text-xs font-medium uppercase tracking-wider text-muted-foreground">Lua Script</Label>
						</div>
						<div className="rounded-xl border border-[#333] overflow-hidden bg-[#1e1e1e] shadow-lg shadow-black/20">
							{/* Editor Tab Header */}
							<div className="flex items-center gap-2.5 px-4 py-2.5 bg-[#252525] border-b border-[#2a2a2a] select-none">
								<FileCode className="h-4 w-4 text-[#888]" />
								<span className="font-mono text-xs text-[#ccc] tracking-wide">handler.lua</span>

							</div>

							{/* Editor / DiffEditor */}
							<div className="relative" style={{ height: isMobileView ? '400px' : '550px' }}>
								{proposedCode ? (
									<>
										{/* AI Suggestion Overlay */}
										<div className="absolute top-3 right-3 z-10 flex items-center gap-1.5 p-1.5 bg-[#2a2a2a]/90 backdrop-blur-xl border border-[#444] rounded-xl shadow-2xl animate-in fade-in slide-in-from-top-2 duration-300">
											<div className="flex items-center gap-1.5 px-2 border-r border-[#444] mr-1.5">
												<Sparkles className="h-3.5 w-3.5 text-yellow-400 animate-pulse" />
												<span className="text-[10px] font-bold uppercase tracking-wider text-[#aaa]">AI Suggestion</span>
											</div>
											<div className="flex items-center bg-[#333] rounded-lg p-0.5 border border-[#444]">
												<Button size="icon" variant="ghost" className="h-6 w-6 rounded-md hover:bg-[#444] text-[#aaa]" onClick={() => diffEditorRef.current?.goToDiff('previous')} title="Previous change">
													<ArrowLeft className="h-3 w-3" />
												</Button>
												<div className="w-px h-3 bg-[#444] mx-0.5" />
												<Button size="icon" variant="ghost" className="h-6 w-6 rounded-md hover:bg-[#444] text-[#aaa]" onClick={() => diffEditorRef.current?.goToDiff('next')} title="Next change">
													<Play className="h-3 w-3 rotate-90" />
												</Button>
											</div>
											<Button size="sm" variant="ghost" className="h-7 px-2.5 rounded-lg text-[11px] font-bold text-[#aaa] hover:text-red-400 hover:bg-red-500/10 transition-colors" onClick={handleRejectChange}>
												<X className="mr-1.5 h-3 w-3" /> Reject
											</Button>
											<Button size="sm" className="h-7 px-3.5 rounded-lg text-[11px] font-bold bg-primary text-primary-foreground shadow-lg shadow-primary/20 hover:scale-[1.02] active:scale-[0.98] transition-all" onClick={handleAcceptChange}>
												<CheckCircle2 className="mr-1.5 h-3 w-3" /> Accept
											</Button>
										</div>
										<DiffEditor
											height="100%"
											original={formData.script}
											modified={proposedCode}
											language="lua"
											theme="vs-dark"
											onMount={(editor) => {
												diffEditorRef.current = editor;
												editor.getModifiedEditor().updateOptions({ readOnly: false });
												editor.getModifiedEditor().onDidChangeModelContent(() => {
													setProposedCode(editor.getModifiedEditor().getValue());
												});
											}}
											options={{
												renderSideBySide: false,
												readOnly: false,
												fontSize: isMobileView ? 11 : 13,
												automaticLayout: true,
												minimap: { enabled: false },
												scrollBeyondLastLine: false,
												useInlineViewWhenSpaceIsLimited: true,
												renderOverviewRuler: false,
												padding: { top: 16, bottom: 16 },
											}}
										/>
									</>
								) : (
									<Editor
										height="100%"
										defaultLanguage="lua"
										theme="vs-dark"
										value={formData.script}
										onChange={(val) => setFormData(prev => ({ ...prev, script: val || '' }))}
										options={{
											minimap: { enabled: false },
											fontSize: isMobileView ? 12 : 13,
											lineNumbers: 'on',
											roundedSelection: true,
											scrollBeyondLastLine: false,
											automaticLayout: true,
											padding: { top: 16, bottom: 16 },
											fixedOverflowWidgets: true,
										}}
									/>
								)}
							</div>
						</div>
					</div>
				</div>
			</div>

			{/* ─── Footer Bar ─── */}
			<div className="shrink-0 bg-background border-t border-border/40 px-4 md:px-6 py-3 md:py-4">
				<div className="mx-auto max-w-6xl flex items-center justify-between">
					<Button
						variant="outline"
						size="sm"
						className="gap-2 h-9 rounded-lg text-xs font-medium"
						onClick={toggleDebugger}
					>
						<Pencil className="h-3.5 w-3.5" />
						Test / Debug
					</Button>
					<div className="flex items-center gap-3">
						<Button
							variant="ghost"
							size="sm"
							className="h-9 px-4 rounded-lg text-xs font-medium text-muted-foreground"
							onClick={handleBack}
						>
							Cancel
						</Button>
						<Button
							size="sm"
							className="h-9 px-5 rounded-lg text-xs font-semibold shadow-lg shadow-primary/20"
							onClick={handleSave}
							disabled={isSaving}
						>
							{isSaving ? (
								<Loader2 className="h-3.5 w-3.5 animate-spin mr-1.5" />
							) : (
								<Save className="h-3.5 w-3.5 mr-1.5" />
							)}
							Save Changes
						</Button>
					</div>
				</div>
			</div>

			{/* ─── Debugger Drawer ─── */}
			<div
				className={cn(
					"fixed bottom-0 left-0 right-0 z-50 flex flex-col bg-[#1e1e1e] border-t border-[#333] shadow-2xl transition-transform duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]",
					isDebuggerOpen ? "translate-y-0" : "translate-y-full"
				)}
				style={{ height: '45vh', minHeight: '300px' }}
			>
				{/* Drawer Header */}
				<div className="flex items-center justify-between px-4 md:px-6 py-2.5 bg-[#181818] border-b border-[#2a2a2a] shrink-0">
					<div className="flex items-center gap-2.5">
						<span className="relative flex h-2 w-2">
							<span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75" />
							<span className="relative inline-flex rounded-full h-2 w-2 bg-green-500" />
						</span>
						<span className="text-sm font-medium text-[#e0e0e0]">Edge Debugger Active</span>
					</div>
					<div className="flex items-center gap-2">
						<Button
							variant="ghost"
							size="xs"
							className="h-7 px-2.5 rounded-md text-[11px] text-[#888] hover:text-[#e0e0e0] hover:bg-[#333]"
							onClick={handleClearLogs}
						>
							Clear Logs
						</Button>
						<Button
							variant="ghost"
							size="icon"
							className="h-7 w-7 rounded-md text-[#666] hover:text-[#e0e0e0] hover:bg-[#333]"
							onClick={() => setIsDebuggerOpen(false)}
						>
							<X className="h-4 w-4" />
						</Button>
					</div>
				</div>

				{/* Drawer Body */}
				<div className="flex flex-1 overflow-hidden">
					{/* Left Pane: Payload Input */}
					<div className="flex flex-col flex-[0.8] border-r border-[#2a2a2a] min-w-0">
						<div className="px-4 py-2 text-[11px] font-mono uppercase tracking-[0.05em] text-[#888] bg-[#1a1a1a] border-b border-[#2a2a2a] shrink-0">
							Simulate Payload (JSON)
						</div>
						<div className="flex-1 p-4 overflow-hidden">
							<textarea
								className="w-full h-full bg-transparent text-green-400/90 font-mono text-xs md:text-sm resize-none outline-none scrollbar-thin scrollbar-thumb-[#444] placeholder:text-[#555]"
								value={jsonPayload}
								onChange={e => setJsonPayload(e.target.value)}
								spellCheck={false}
							/>
						</div>
						<div className="px-4 py-2.5 shrink-0 border-t border-[#2a2a2a]">
							<Button
								size="sm"
								className="h-8 px-4 rounded-md text-[11px] font-semibold gap-1.5 bg-[#e0e0e0] text-[#1a1a1a] hover:bg-white shadow-none"
								onClick={handleTest}
								disabled={isTesting}
							>
								{isTesting ? (
									<Loader2 className="h-3.5 w-3.5 animate-spin" />
								) : (
									<Play className="h-3.5 w-3.5 fill-current" />
								)}
								Run Execution
							</Button>
						</div>
					</div>

					{/* Right Pane: Console Logs */}
					<div className="flex flex-col flex-1 min-w-0">
						<div className="px-4 py-2 text-[11px] font-mono uppercase tracking-[0.05em] text-[#888] bg-[#1a1a1a] border-b border-[#2a2a2a] shrink-0">
							Console Logs
						</div>
						<div className="flex-1 p-4 overflow-y-auto font-mono text-xs md:text-sm leading-relaxed space-y-1.5 scrollbar-thin scrollbar-thumb-[#444]">
							{consoleLogs.length === 0 && (
								<div className="text-[#555] italic">No log entries.</div>
							)}
							{consoleLogs.map((log, i) => (
								<div key={i} className="flex gap-2.5 items-baseline">
									<span className="text-[#555] shrink-0 tabular-nums select-none min-w-[64px]">{log.time}</span>
									<span className={cn(
										'shrink-0 font-medium',
										log.type === 'info' && 'text-blue-400',
										log.type === 'success' && 'text-green-400',
										log.type === 'error' && 'text-red-400',
										log.type === 'invoke' && 'text-yellow-400',
										log.type === 'print' && 'text-cyan-300',
									)}>
										[{log.type}]
									</span>
									<span className="text-[#d0d0d0] break-words min-w-0">{log.message}</span>
								</div>
							))}
							{isTesting && (
								<div className="flex gap-2.5 items-center text-[#888]">
									<span className="min-w-[64px]">{formatTimestamp(new Date())}</span>
									<Loader2 className="h-3 w-3 animate-spin text-yellow-400" />
									<span className="italic">Executing...</span>
								</div>
							)}
							<div ref={logsEndRef} />
						</div>
					</div>
				</div>
			</div>

			{/* ─── Debugger Backdrop (click to close) ─── */}
			{isDebuggerOpen && (
				<div
					className="fixed inset-0 z-40 bg-black/30 backdrop-blur-[1px]"
					onClick={() => setIsDebuggerOpen(false)}
				/>
			)}

			{/* ─── AI Assistant ─── */}
			{isMobileView ? (
				<div className="fixed bottom-16 right-4 z-30">
					<AIAssistant currentCode={formData.script} onApplyCode={handleApplyAIChange} />
				</div>
			) : (
				<AIAssistant currentCode={formData.script} onApplyCode={handleApplyAIChange} />
			)}
		</div>
	)
}
