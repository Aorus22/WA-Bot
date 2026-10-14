import { useCallback, useEffect, useRef, useState } from 'react'
import { Image as ImageIcon, Loader2 } from 'lucide-react'
import { toast } from 'sonner'
import { api, type HistoryMediaKind, type HistoryMediaStatus } from '@/lib/api'
import { formatDuration } from '@/lib/historySync'
import {
	MEDIA_KINDS, defaultMediaKinds, isMediaActive, mediaHeadline, mediaResultText, mediaPausedText, mediaErrorText, mediaProgress,
} from '@/lib/historyMedia'
import { useChatStore } from '@/stores/chatStore'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'

/** Downloads the photos, stickers, ... of messages imported from history into the app's storage. */
export function HistoryMediaSection({ isConnected }: { isConnected: boolean }) {
	const [status, setStatus] = useState<HistoryMediaStatus | null>(null)
	const [loading, setLoading] = useState(true)
	const [kinds, setKinds] = useState<HistoryMediaKind[]>(defaultMediaKinds)
	const [retryUnavailable, setRetryUnavailable] = useState(false)
	const wasRunning = useRef(false)
	const active = status ? isMediaActive(status.state) : false

	const load = useCallback(async () => {
		try {
			const next = await api.getHistoryMediaStatus()
			setStatus(next)
			if (wasRunning.current && !isMediaActive(next.state)) {
				useChatStore.getState().invalidateMessages()
				if (next.state === 'failed') toast.error('Media download failed')
				else if (next.state === 'cancelled') toast(`Media download stopped: ${next.done} files downloaded`)
				else toast.success(`Downloaded ${next.done} files`)
			}
			wasRunning.current = isMediaActive(next.state)
		} catch {
			// The section stays usable and retries when the user clicks it.
		} finally {
			setLoading(false)
		}
	}, [])

	useEffect(() => {
		void load()
	}, [load])

	useEffect(() => {
		if (!active) return
		const timer = window.setInterval(() => void load(), 1000)
		return () => window.clearInterval(timer)
	}, [active, load])

	const start = async (resume: boolean) => {
		setLoading(true)
		try {
			const next = await api.startHistoryMedia(resume ? { resume: true } : { types: kinds, retryUnavailable })
			wasRunning.current = true
			setStatus(next)
			toast.success(resume ? 'Media download resumed' : 'Media download started')
		} catch (err) {
			const current = await api.getHistoryMediaStatus().catch(() => null)
			if (current && isMediaActive(current.state)) {
				setStatus(current)
				wasRunning.current = true
				toast('A media download is already running.')
			} else {
				toast.error(err instanceof Error ? err.message : 'Failed to start the media download')
				await load()
			}
		} finally {
			setLoading(false)
		}
	}

	const stop = async () => {
		try {
			setStatus(await api.cancelHistoryMedia())
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Could not stop the media download')
		}
	}

	const toggleKind = (kind: HistoryMediaKind, on: boolean) =>
		setKinds((current) => (on ? [...current, kind] : current.filter((k) => k !== kind)))

	const chosenCount = kinds.reduce((sum, kind) => sum + (status?.available[kind] ?? 0), 0)
	const progress = status ? mediaProgress(status) : null
	const shown = status && (status.startedAt != null || active)
	const paused = status ? mediaPausedText(status) : null
	const error = status ? mediaErrorText(status) : null
	const timing = status && shown
		? [
			`${active ? 'running for' : 'ran for'} ${formatDuration(Math.floor(status.elapsedMs / 1000))}`,
			active ? (status.etaMs != null ? `about ${formatDuration(Math.floor(status.etaMs / 1000))} left` : 'estimating time…') : null,
		].filter(Boolean).join(' · ')
		: null

	return (
		<div className="px-4 py-4 space-y-3">
			<div className="flex items-start justify-between gap-4">
				<div className="flex items-start gap-3">
					<ImageIcon className="h-4 w-4 mt-0.5 text-muted-foreground" />
					<div className="flex flex-col gap-0.5">
						<span className="text-sm font-medium">Chat media</span>
						<span className="text-xs text-muted-foreground">
							History messages only keep their text. Download their photos, stickers and files into the app so they open instantly.
						</span>
					</div>
				</div>
				{active ? (
					<Button size="sm" variant="outline" className="text-destructive" onClick={stop} disabled={status?.state === 'cancelling'}>
						{status?.state === 'cancelling' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
						{status?.state === 'cancelling' ? 'Stopping…' : 'Stop download'}
					</Button>
				) : status?.resumable ? (
					<div className="flex gap-2">
						<Button size="sm" variant="outline" onClick={() => start(false)} disabled={!isConnected || loading || kinds.length === 0}>
							Start over
						</Button>
						<Button size="sm" onClick={() => start(true)} disabled={!isConnected || loading}>
							Resume
						</Button>
					</div>
				) : (
					<Button size="sm" onClick={() => start(false)} disabled={!isConnected || loading || kinds.length === 0}>
						Download media
					</Button>
				)}
			</div>
			<div className="flex flex-wrap gap-1.5">
				{MEDIA_KINDS.map((kind) => {
					const on = kinds.includes(kind.id)
					const count = status?.available[kind.id]
					return (
						<button
							key={kind.id}
							type="button"
							disabled={active}
							onClick={() => toggleKind(kind.id, !on)}
							className={`text-xs px-2.5 py-1 rounded-md border transition-colors disabled:opacity-60 ${
								on ? 'border-primary text-primary bg-muted' : 'hover:bg-muted'
							}`}
						>
							{kind.label}{count != null ? ` · ${count}` : ''}
						</button>
					)
				})}
			</div>
			<p className="text-xs text-muted-foreground">
				{kinds.length === 0
					? 'Choose at least one kind of media.'
					: `${chosenCount} files to download. Videos and documents can be large, so they are off by default.`}
			</p>
			<div className="flex items-start gap-3">
				<Switch id="history_media_retry" checked={retryUnavailable} onCheckedChange={setRetryUnavailable} disabled={active} />
				<div className="flex flex-col gap-0.5">
					<Label htmlFor="history_media_retry" className="text-xs font-medium">Also retry media that was no longer available</Label>
					<span className="text-xs text-muted-foreground">
						WhatsApp deletes old media from its servers; files found to be gone are skipped unless this is on.
					</span>
				</div>
			</div>
			{shown && status && progress && (
				<div className="space-y-1.5 text-xs text-muted-foreground">
					<div className="font-medium text-foreground">{mediaHeadline(status)}</div>
					{(active || status.total > 0) && (
						<div
							role="progressbar"
							aria-valuemin={0}
							aria-valuemax={status.total}
							aria-valuenow={status.done + status.unavailable + status.failed}
							className="flex h-2 rounded-full bg-muted overflow-hidden"
						>
							<div className="h-full bg-[#00a884] transition-all" style={{ width: `${Math.round(progress.done * 1000) / 10}%` }} />
							<div className="h-full bg-muted-foreground/40 transition-all" style={{ width: `${Math.round(progress.unavailable * 1000) / 10}%` }} />
							<div className="h-full bg-destructive/70 transition-all" style={{ width: `${Math.round(progress.failed * 1000) / 10}%` }} />
						</div>
					)}
					{timing && <div>{timing}</div>}
					{active && status.current > 0 && <div>Now: downloading {status.current} files at once</div>}
					{paused && <p className="text-amber-600 dark:text-amber-400">{paused}</p>}
					<div>{mediaResultText(status)}</div>
					{error && <p className="text-destructive">{error}</p>}
				</div>
			)}
		</div>
	)
}
