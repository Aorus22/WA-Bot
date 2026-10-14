import type { HistoryMediaKind, HistoryMediaStatus } from './api'

/** The kinds of media a download can be limited to. Video and documents are big, so they are off by default. */
export const MEDIA_KINDS = [
	{ id: 'image', label: 'Photos', byDefault: true },
	{ id: 'sticker', label: 'Stickers', byDefault: true },
	{ id: 'audio', label: 'Audio & voice notes', byDefault: true },
	{ id: 'video', label: 'Videos', byDefault: false },
	{ id: 'document', label: 'Documents', byDefault: false },
] as const satisfies readonly { id: HistoryMediaKind; label: string; byDefault: boolean }[]

export const defaultMediaKinds = (): HistoryMediaKind[] => MEDIA_KINDS.filter((k) => k.byDefault).map((k) => k.id)

export const isMediaActive = (state: string) => state === 'running' || state === 'cancelling'

export function mediaStateLabel(state: string): string {
	switch (state) {
		case 'running': return 'Downloading'
		case 'cancelling': return 'Stopping…'
		case 'completed': return 'Done'
		case 'partial': return 'Done, with failures'
		case 'cancelled': return 'Stopped'
		case 'interrupted': return 'Paused'
		case 'failed': return 'Failed'
		default: return 'Not run yet'
	}
}

/** "812 KB", "34.5 MB", "1.2 GB". */
export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`
	const units = ['KB', 'MB', 'GB', 'TB']
	let value = bytes / 1024
	let unit = 0
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024
		unit++
	}
	return `${value >= 100 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

/** Share of the files handled (downloaded, gone or failed), 0..1. */
export function mediaProgress(st: HistoryMediaStatus): { done: number; unavailable: number; failed: number } {
	if (st.total <= 0) return { done: 0, unavailable: 0, failed: 0 }
	return { done: st.done / st.total, unavailable: st.unavailable / st.total, failed: st.failed / st.total }
}

export function mediaHeadline(st: HistoryMediaStatus): string {
	const handled = st.done + st.unavailable + st.failed
	const parts = [mediaStateLabel(st.state)]
	if (st.total > 0) parts.push(`${handled} of ${st.total} files (${Math.round((handled / st.total) * 100)}%)`)
	return parts.join(' · ')
}

export function mediaResultText(st: HistoryMediaStatus): string {
	const parts = [`Downloaded ${st.done} files (${formatBytes(st.bytes)})`]
	if (st.unavailable > 0) parts.push(`${st.unavailable} no longer on WhatsApp's servers`)
	if (st.failed > 0) parts.push(`${st.failed} failed`)
	return parts.join(' · ')
}

export function mediaPausedText(st: HistoryMediaStatus): string | null {
	if (!st.resumable) return null
	const why = st.state === 'interrupted'
		? 'The app or backend was restarted while downloading.'
		: st.state === 'cancelled' ? 'The download was stopped.' : 'The download stopped early.'
	return `${why} What was downloaded is kept, and ${st.remaining} files are left. Resume continues where it left off.`
}

export function mediaErrorText(st: HistoryMediaStatus): string | null {
	const last = st.errors.at(-1)
	if (!last) return null
	return st.errors.length > 1 ? `${last.message} (and ${st.errors.length - 1} more)` : last.message
}
