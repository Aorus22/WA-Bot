import type { HistorySyncCurrent, HistorySyncRequest, HistorySyncStatus } from './api'

/** The depth choices of the chat-history card. */
export const SYNC_DEPTHS = [
	{ id: 'per50', label: '50 / chat', hint: 'Quick: up to 50 older messages in every chat.', request: { perChat: 50 } },
	{ id: 'per200', label: '200 / chat', hint: 'Up to 200 older messages in every chat.', request: { perChat: 200 } },
	{ id: 'per1000', label: '1000 / chat', hint: 'Up to 1000 older messages in every chat. Takes a while on many chats.', request: { perChat: 1000 } },
	{ id: 'days30', label: 'Last 30 days', hint: 'Every message from the last 30 days, however many that is.', request: { sinceDays: 30 } },
	{ id: 'days365', label: 'Last year', hint: 'Every message from the last year. Can take a long time.', request: { sinceDays: 365 } },
	{
		id: 'all',
		label: 'Everything',
		hint: 'Everything your phone will give, back to the first message of each chat. Keep the phone online; this can take hours.',
		request: { all: true },
	},
] as const satisfies readonly { id: string; label: string; hint: string; request: HistorySyncRequest }[]

export type SyncDepthId = (typeof SYNC_DEPTHS)[number]['id']

export const isActive = (state: string) => state === 'running' || state === 'cancelling'

export function stateLabel(state: string): string {
	switch (state) {
		case 'running': return 'Syncing'
		case 'cancelling': return 'Stopping…'
		case 'completed': return 'Done'
		case 'partial': return 'Done, with warnings'
		case 'cancelled': return 'Stopped'
		case 'interrupted': return 'Paused'
		case 'failed': return 'Failed'
		default: return 'Not run yet'
	}
}

/** Share of chats finished, 0..1. */
export function progressFraction(st: HistorySyncStatus): number {
	if (st.chatsTotal <= 0) return 0
	return Math.min(1, Math.max(0, st.chatsProcessed / st.chatsTotal))
}

/** Share of chats being worked on right now, 0..1 (drawn after the finished part of the bar). */
export function inProgressFraction(st: HistorySyncStatus): number {
	if (st.chatsTotal <= 0 || !isActive(st.state)) return 0
	const room = st.chatsTotal - st.chatsProcessed
	return Math.min(room, st.current.length) / st.chatsTotal
}

/** "45 s", "3 min", "1 h 20 min". */
export function formatDuration(secs: number): string {
	if (secs < 60) return `${secs} s`
	const mins = Math.round(secs / 60)
	if (mins < 60) return `${mins} min`
	return mins % 60 === 0 ? `${mins / 60} h` : `${Math.floor(mins / 60)} h ${mins % 60} min`
}

/** "running for 3 min" / "ran for 12 min": the time the sync actually ran. */
export function elapsedText(st: HistorySyncStatus): string | null {
	if (st.startedAt == null) return null
	const secs = Math.max(0, Math.floor(st.elapsedMs / 1000))
	return `${isActive(st.state) ? 'running for' : 'ran for'} ${formatDuration(secs)}`
}

/** "about 20 min left" while running; "estimating time…" until the server can tell. */
export function etaText(st: HistorySyncStatus): string | null {
	if (!isActive(st.state) || st.chatsTotal <= 0) return null
	return st.etaMs != null ? `about ${formatDuration(Math.floor(st.etaMs / 1000))} left` : 'estimating time…'
}

/** What the headline says about the chats: "368 of 999 chats (37%)". */
export function chatsText(st: HistorySyncStatus): string | null {
	if (st.chatsTotal <= 0) return null
	return `${st.chatsProcessed} of ${st.chatsTotal} chats (${Math.round(progressFraction(st) * 100)}%)`
}

/** What the "retry finished chats" option is about, with how many chats it concerns. */
export function finishedChatsText(st: HistorySyncStatus | null): string {
	const n = st?.chatsFinished ?? 0
	return n > 0
		? `${n} chats already reached the start of their history. They are skipped unless this is on.`
		: 'Chats that already reached the start of their history are skipped unless this is on.'
}

/** Why a sync stopped before the end, and what resuming does. */
export function pausedText(st: HistorySyncStatus): string | null {
	if (!st.resumable) return null
	const left = st.chatsTotal - st.chatsProcessed
	const remaining = left > 0 ? `${left} chats are left` : 'some chats are left'
	const why = st.state === 'interrupted'
		? 'The app or backend was restarted while syncing.'
		: st.state === 'cancelled' ? 'The sync was stopped.' : 'The sync stopped early.'
	return `${why} Nothing imported so far is lost, and ${remaining}. Resume continues where it left off.`
}

export function currentText(c: HistorySyncCurrent): string {
	const name = c.name.trim() || c.chatId
	const what = c.phase === 'asking-phone' ? 'asking your phone for older messages' : 'importing'
	return c.added > 0 ? `${name} — ${what}, +${c.added} so far` : `${name} — ${what}`
}

export function resultText(st: HistorySyncStatus): string {
	const parts = [`Added ${st.messagesAdded} messages in ${st.chatsWithNew} chats`]
	if (st.chatsExhausted > 0) parts.push(`${st.chatsExhausted} chats had nothing older`)
	if (st.chatsSkipped > 0) parts.push(`${st.chatsSkipped} skipped (already at their start)`)
	if (st.timeouts > 0) parts.push(`${st.timeouts} requests got no answer from the phone`)
	return parts.join(' · ')
}

export function stagedText(st: HistorySyncStatus): string | null {
	return st.pendingMessages > 0
		? `${st.pendingMessages} older messages in ${st.pendingChats} chats are downloaded but not imported yet — choose a deeper option to import them.`
		: null
}

export function errorText(st: HistorySyncStatus): string | null {
	const last = st.errors.at(-1)
	if (!last) return null
	const message = last.chatId ? `${last.chatId}: ${last.message}` : last.message
	return st.errors.length > 1 ? `${message} (and ${st.errors.length - 1} more)` : message
}
