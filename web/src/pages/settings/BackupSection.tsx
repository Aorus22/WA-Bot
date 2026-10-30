import { useCallback, useEffect, useRef, useState } from 'react'
import { DatabaseBackup, Loader2 } from 'lucide-react'
import { toast } from 'sonner'
import { api, type BackupEntry, type BackupStatus } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { BoxedList } from '@/components/ui/boxed-list'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const FOLDER_KEY = 'wa-bot-backup-folder'
/** Backups kept per account after each run. */
const KEEP_BACKUPS = 5

const STAGES: Record<string, string> = {
	snapshot: 'Checking the databases',
	chat: 'Saving chats',
	media: 'Saving media',
	manifest: 'Finishing',
}

const RESTORE_STAGES: Record<string, string> = {
	chat: 'Reading chats from the backup',
	merge: 'Adding missing chats and messages',
	media: 'Adding missing media',
}

function progressText(status: BackupStatus, restoring: boolean): string {
	const label = (restoring ? RESTORE_STAGES[status.stage ?? ''] : undefined) ?? STAGES[status.stage ?? ''] ?? 'Working'
	if (status.total > 0) return `${label} · ${status.done} of ${status.total}`
	return status.item ? `${label} · ${status.item}` : label
}

/** Copies the chats and media of every account into a folder on the machine running the backend. */
export function BackupSection() {
	const [folder, setFolder] = useState(() => localStorage.getItem(FOLDER_KEY) ?? '')
	const [status, setStatus] = useState<BackupStatus | null>(null)
	const [restoreStatus, setRestoreStatus] = useState<BackupStatus | null>(null)
	const [lastWasRestore, setLastWasRestore] = useState(false)
	const [backups, setBackups] = useState<BackupEntry[]>([])
	const wasRunning = useRef(false)
	const restoreWasRunning = useRef(false)
	const backingUp = status?.running ?? false
	const restoring = restoreStatus?.running ?? false
	const running = backingUp || restoring
	const shown = lastWasRestore ? restoreStatus : status

	const loadList = useCallback(async (target: string) => {
		if (!target.startsWith('/')) return setBackups([])
		try {
			setBackups(await api.listBackups(target))
		} catch {
			setBackups([])
		}
	}, [])

	const loadStatus = useCallback(async () => {
		try {
			const next = await api.getBackupStatus()
			setStatus(next)
			if (wasRunning.current && !next.running) {
				if (next.error) toast.error(`Backup failed: ${next.error}`)
				else toast.success('Backup finished')
				void loadList(localStorage.getItem(FOLDER_KEY) ?? '')
			}
			wasRunning.current = next.running
		} catch {
			// Retried on the next poll or click.
		}
	}, [loadList])

	const loadRestoreStatus = useCallback(async () => {
		try {
			const next = await api.getRestoreStatus()
			setRestoreStatus(next)
			if (next.running) setLastWasRestore(true)
			if (restoreWasRunning.current && !next.running) {
				if (next.error) toast.error(`Restore failed: ${next.error}`)
				else toast.success('Restore finished')
			}
			restoreWasRunning.current = next.running
		} catch {
			// Retried on the next poll or click.
		}
	}, [])

	useEffect(() => {
		void loadStatus()
		void loadRestoreStatus()
		void loadList(folder)
		// Only on mount; the folder reloads the list when it is saved.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [])

	useEffect(() => {
		if (!backingUp) return
		const timer = window.setInterval(() => void loadStatus(), 1000)
		return () => window.clearInterval(timer)
	}, [backingUp, loadStatus])

	useEffect(() => {
		if (!restoring) return
		const timer = window.setInterval(() => void loadRestoreStatus(), 1000)
		return () => window.clearInterval(timer)
	}, [restoring, loadRestoreStatus])

	const saveFolder = () => {
		const target = folder.trim()
		setFolder(target)
		localStorage.setItem(FOLDER_KEY, target)
		void loadList(target)
	}

	const start = async () => {
		const target = folder.trim()
		if (!target.startsWith('/')) {
			toast.error('Enter the full path of the backup folder, e.g. /home/you/wabot-backup')
			return
		}
		saveFolder()
		try {
			await api.startBackup(target, KEEP_BACKUPS)
			setLastWasRestore(false)
			wasRunning.current = true
			setStatus({ running: true, done: 0, total: 0 })
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Could not start the backup')
		}
	}

	const restore = async (b: BackupEntry) => {
		try {
			await api.startRestore(folder.trim(), b.account, b.id)
			setLastWasRestore(true)
			restoreWasRunning.current = true
			setRestoreStatus({ running: true, done: 0, total: 0 })
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'Could not start the restore')
		}
	}

	const newest = [...backups].sort((a, b) => b.createdAt - a.createdAt)

	return (
		<div className="px-4 py-4 space-y-4">
			<div className="flex items-start justify-between gap-4">
				<div className="flex items-start gap-3">
					<DatabaseBackup className="h-4 w-4 mt-0.5 text-muted-foreground" />
					<div className="flex flex-col gap-0.5">
						<span className="text-sm font-medium">Backup</span>
						<span className="text-xs text-muted-foreground">
							Copies your chats and media into a folder. Your WhatsApp login is not included.
						</span>
					</div>
				</div>
				<Button size="sm" onClick={start} disabled={running || !folder.trim()}>
					{running && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
					{restoring ? 'Restoring…' : backingUp ? 'Backing up…' : 'Back up now'}
				</Button>
			</div>
			<div className="flex flex-col gap-1.5">
				<Label htmlFor="backup_folder" className="text-xs font-medium">Backup folder</Label>
				<Input
					id="backup_folder"
					value={folder}
					onChange={(e) => setFolder(e.target.value)}
					onBlur={saveFolder}
					placeholder="/home/you/wabot-backup"
					disabled={running}
					className="h-8 text-xs"
				/>
				<span className="text-xs text-muted-foreground">
					The full path on the computer running WA Bot. The newest {KEEP_BACKUPS} backups are kept.
				</span>
			</div>
			{shown && (running || shown.error || shown.summary) && (
				<div className="space-y-1.5 border-t pt-3 text-xs text-muted-foreground">
					{shown.running ? (
						<>
							<div className="font-medium text-foreground">{restoring ? 'Restoring' : 'Backing up'} · {shown.percent ?? 0}%</div>
							<div
								role="progressbar"
								aria-valuemin={0}
								aria-valuemax={100}
								aria-valuenow={shown.percent ?? 0}
								className="flex h-1.5 rounded-full bg-muted overflow-hidden"
							>
								<div className="h-full bg-[#00a884] transition-all" style={{ width: `${Math.max(1, shown.percent ?? 0)}%` }} />
							</div>
							<div>{progressText(shown, restoring)}</div>
							{(shown.accountCount ?? 0) > 1 && <div>Account {shown.accountIndex} of {shown.accountCount}</div>}
						</>
					) : shown.error ? (
						<p className="text-destructive">{shown.error}</p>
					) : (
						<p className="whitespace-pre-line">{shown.summary}</p>
					)}
				</div>
			)}
			{newest.length > 0 && (
				<div className="space-y-1.5">
					<p className="text-xs font-medium">{newest.length} backups in this folder</p>
					<p className="text-xs text-muted-foreground">
						Restore adds what is missing from a backup. Nothing you have now is replaced.
					</p>
					<BoxedList className="flex flex-col space-y-0.5">
						{newest.slice(0, 5).map((b) => (
							<div key={`${b.account}/${b.id}`} className="flex items-center justify-between text-xs text-muted-foreground">
								<span>{new Date(b.createdAt).toLocaleString()} · {b.account}</span>
								<Button size="sm" variant="secondary" onClick={() => void restore(b)} disabled={running}>
									Restore
								</Button>
							</div>
						))}
					</BoxedList>
				</div>
			)}
		</div>
	)
}
