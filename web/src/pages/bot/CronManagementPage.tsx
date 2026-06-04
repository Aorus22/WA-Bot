import { useState, useEffect, useRef } from 'react'
import { Plus, Search, Download, Upload, MoreHorizontal, Trash2, Power, Eye, Play, Clock } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
	DropdownMenu,
	DropdownMenuTrigger,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu'
import { CronDeleteModal } from './CronDeleteModal'
import { CronDeleteAllModal } from './CronDeleteAllModal'
import { api, type CronJob } from '@/lib/api'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { useNavigate } from "react-router-dom"
import { useIsMobile } from "@/hooks/use-mobile"

function relativeTime(dateStr: string | undefined): string {
	if (!dateStr) return '—'
	const now = Date.now()
	const then = new Date(dateStr).getTime()
	if (isNaN(then)) return '—'
	const diff = now - then
	const seconds = Math.floor(diff / 1000)
	if (seconds < 5) return 'just now'
	if (seconds < 60) return `${seconds}s ago`
	const minutes = Math.floor(seconds / 60)
	if (minutes < 60) return `${minutes}m ago`
	const hours = Math.floor(minutes / 60)
	if (hours < 24) return `${hours}h ago`
	const days = Math.floor(hours / 24)
	if (days < 7) return `${days}d ago`
	if (days < 30) return `${Math.floor(days / 7)}w ago`
	if (days < 365) return `${Math.floor(days / 30)}mo ago`
	return `${Math.floor(days / 365)}y ago`
}

function getDescription(description: string | undefined): string {
	if (!description) return ''
	return description.length > 80 ? description.slice(0, 80) + '…' : description
}

export function CronManagementPage() {
	const navigate = useNavigate()
	const isMobileView = useIsMobile()
	const [jobs, setJobs] = useState<CronJob[]>([])
	const [loading, setLoading] = useState(true)
	const [searchQuery, setSearchQuery] = useState('')
	const [deleteId, setDeleteId] = useState<string | null>(null)
	const [isDeleteAllOpen, setIsDeleteAllOpen] = useState(false)
	const fileInputRef = useRef<HTMLInputElement>(null)

	useEffect(() => {
		loadJobs()
	}, [])

	const loadJobs = async () => {
		try {
			setLoading(true)
			const data = await api.getCronJobs()
			setJobs(data || [])
		} catch (error) {
			console.error('Failed to load cron jobs:', error)
			toast.error('Failed to load cron jobs')
		} finally {
			setLoading(false)
		}
	}

	const handleDeleteConfirm = async () => {
		if (!deleteId) return
		try {
			await api.deleteCronJob(deleteId)
			toast.success('Cron job deleted')
			setDeleteId(null)
			loadJobs()
		} catch (error) {
			toast.error('Failed to delete cron job')
		}
	}

	const handleDeleteAllConfirm = async () => {
		try {
			setLoading(true)
			setIsDeleteAllOpen(false)
			await api.deleteAllCronJobs()
			toast.success('All scheduled jobs purged')
			loadJobs()
		} catch (error) {
			toast.error('Failed to purge jobs')
		} finally {
			setLoading(false)
		}
	}

	const toggleStatus = async (job: CronJob) => {
		try {
			await api.updateCronJob(job.id, { ...job, is_active: !job.is_active })
			loadJobs()
		} catch (error) {
			toast.error('Failed to update status')
		}
	}

	const handleRunNow = async (job: CronJob) => {
		try {
			await api.testCronJob(job.script)
			toast.success(`Job '${job.name}' started manually`)
		} catch (error) {
			toast.error('Manual execution failed')
		}
	}

	const handleExport = () => {
		if (jobs.length === 0) {
			toast.error('No jobs to export')
			return
		}
		const dataStr = JSON.stringify(jobs, null, 2)
		const dataUri = 'data:application/json;charset=utf-8,' + encodeURIComponent(dataStr)
		const exportFileDefaultName = `wa-bot-cron-jobs-${new Date().toISOString().split('T')[0]}.json`
		const linkElement = document.createElement('a')
		linkElement.setAttribute('href', dataUri)
		linkElement.setAttribute('download', exportFileDefaultName)
		linkElement.click()
		toast.success('Export successful')
	}

	const handleImportClick = () => {
		fileInputRef.current?.click()
	}

	const handleFileImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
		const file = e.target.files?.[0]
		if (!file) return
		const reader = new FileReader()
		reader.onload = async (event) => {
			try {
				const content = event.target?.result as string
				const importedJobs = JSON.parse(content)
				if (!Array.isArray(importedJobs)) throw new Error('Invalid format')

				setLoading(true)
				let count = 0
				for (const job of importedJobs) {
					try {
						const { id, created_at, updated_at, ...cleanJob } = job
						await api.createCronJob(cleanJob)
						count++
					} catch (err) { console.error(err) }
				}
				toast.success(`Imported ${count} jobs`)
				loadJobs()
			} catch (error) {
				toast.error('Import failed')
			} finally {
				setLoading(false)
				if (e.target) e.target.value = ''
			}
		}
		reader.readAsText(file)
	}

	const filteredJobs = jobs.filter(job =>
		job.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
		job.schedule.toLowerCase().includes(searchQuery.toLowerCase())
	)

	return (
		<ScrollArea className='h-full w-full bg-background'>
			<div className='flex flex-col min-h-full w-full pb-20 md:pb-8'>
				<CronDeleteModal
					open={!!deleteId}
					onOpenChange={(open) => !open && setDeleteId(null)}
					onConfirm={handleDeleteConfirm}
				/>
				<CronDeleteAllModal open={isDeleteAllOpen} onOpenChange={setIsDeleteAllOpen} onConfirm={handleDeleteAllConfirm} />

				{/* ── Page Header ── */}
				<div className='px-4 md:px-8 pt-6 md:pt-10 pb-4 md:pb-6'>
					<div className='flex flex-col md:flex-row md:items-end justify-between gap-4'>
						<div className='space-y-1'>
							<h1 className='text-3xl md:text-4xl font-semibold tracking-tight text-foreground'>
								Cron Jobs
							</h1>
							<p className='text-sm md:text-base text-muted-foreground'>
								Automate tasks with scheduled Lua scripts.
							</p>
						</div>
						<div className='flex items-center gap-2 shrink-0'>
							<div className='flex items-center p-0.5 bg-muted/50 rounded-lg border border-border/40 md:hidden'>
								<Button
									variant='ghost'
									size='icon'
									onClick={() => setIsDeleteAllOpen(true)}
									className='h-8 w-8 rounded-md text-destructive hover:text-destructive hover:bg-destructive/10'
									title='Delete ALL Jobs'
									disabled={jobs.length === 0}
								>
									<Trash2 className='h-4 w-4' />
								</Button>
								<Button
									variant='ghost'
									size='icon'
									onClick={handleExport}
									className='h-8 w-8 rounded-md'
									title='Export Jobs'
								>
									<Download className='h-4 w-4' />
								</Button>
								<Button
									variant='ghost'
									size='icon'
									onClick={handleImportClick}
									className='h-8 w-8 rounded-md'
									title='Import Jobs'
								>
									<Upload className='h-4 w-4' />
								</Button>
							</div>
							<Button
								variant='outline'
								onClick={() => navigate("/documentation")}
								className='rounded-lg h-9 px-4 border-border/40 text-sm'
							>
								Docs
							</Button>
							{!isMobileView && (
								<Button
									onClick={() => navigate("/cron/new")}
									className='rounded-lg px-5 h-9 font-medium shadow-sm'
								>
									<Plus className='mr-1.5 h-4 w-4' /> Create Job
								</Button>
							)}
						</div>
					</div>
				</div>

				{/* ── Search & Toolbar ── */}
				<div className='px-4 md:px-8 pb-4 md:pb-6'>
					<div className='flex items-center gap-3'>
						<div className='relative flex-1 max-w-sm'>
							<Search className='absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none' />
							<Input
								placeholder='Search jobs…'
								value={searchQuery}
								onChange={e => setSearchQuery(e.target.value)}
								className='pl-9 h-9 rounded-lg border-border/40 bg-background text-sm focus-visible:ring-primary/20'
							/>
						</div>
						<div className='hidden md:flex items-center p-0.5 bg-muted/50 rounded-lg border border-border/40'>
							<Button
								variant='ghost'
								size='icon'
								onClick={() => setIsDeleteAllOpen(true)}
								className='h-8 w-8 rounded-md text-destructive hover:text-destructive hover:bg-destructive/10'
								title='Delete ALL Jobs'
								disabled={jobs.length === 0}
							>
								<Trash2 className='h-4 w-4' />
							</Button>
							<Button
								variant='ghost'
								size='icon'
								onClick={handleExport}
								className='h-8 w-8 rounded-md'
								title='Export Jobs'
							>
								<Download className='h-4 w-4' />
							</Button>
							<Button
								variant='ghost'
								size='icon'
								onClick={handleImportClick}
								className='h-8 w-8 rounded-md'
								title='Import Jobs'
							>
								<Upload className='h-4 w-4' />
							</Button>
						</div>
					</div>
				</div>

				{/* ── Content ── */}
				<div className='px-4 md:px-8 w-full flex-1'>
					{loading ? (
						<div className='flex flex-col items-center justify-center py-24 gap-4 opacity-50'>
							<div className='w-10 h-10 border-4 border-primary/30 border-t-primary rounded-full animate-spin' />
							<p className='font-medium'>Loading cron jobs…</p>
						</div>
					) : filteredJobs.length === 0 ? (
						<div className='flex flex-col items-center justify-center py-20 md:py-32 border-2 border-dashed border-border/40 rounded-2xl text-center space-y-6 bg-muted/5'>
							<div className='w-16 h-16 rounded-2xl bg-muted flex items-center justify-center text-muted-foreground'>
								<Clock className='h-8 w-8' />
							</div>
							<div className='space-y-2 px-4'>
								<h3 className='text-xl font-semibold'>No cron jobs found</h3>
								<p className='text-muted-foreground max-w-xs mx-auto text-sm'>
									{searchQuery
										? 'No jobs match your search. Try a different query.'
										: 'Create your first cron job to start automating.'}
								</p>
							</div>
							{!searchQuery && (
								<Button
									onClick={() => navigate("/cron/new")}
									className='rounded-lg px-8 h-10 font-medium'
								>
									<Plus className='mr-2 h-4 w-4' /> Create Job
								</Button>
							)}
						</div>
					) : (
						<>
							{/* ── Desktop List Header ── */}
							<div className='hidden md:grid md:grid-cols-[2fr_1fr_1fr_1fr_auto] gap-4 px-6 py-3 border border-border/40 rounded-t-lg bg-muted/30 border-b-0'>
								<div className='text-xs font-medium text-muted-foreground uppercase tracking-wider'>Name</div>
								<div className='text-xs font-medium text-muted-foreground uppercase tracking-wider'>Schedule</div>
								<div className='text-xs font-medium text-muted-foreground uppercase tracking-wider'>Status</div>
								<div className='text-xs font-medium text-muted-foreground uppercase tracking-wider'>Last Edited</div>
								<div className='w-10' />
							</div>

							{/* ── List Items ── */}
							<div className='border border-border/40 rounded-lg md:rounded-t-none bg-card overflow-hidden'>
								{filteredJobs.map((job) => (
									<div
										key={job.id}
										onClick={() => navigate("/cron/" + job.id, { state: { job } })}
										className={cn(
											'group cursor-pointer transition-colors',
											'border-b border-border/40 last:border-b-0',
											'hover:bg-muted/20'
										)}
									>
										{/* Desktop grid layout */}
										<div className='hidden md:grid md:grid-cols-[2fr_1fr_1fr_1fr_auto] gap-4 px-6 py-4 items-center'>
											{/* Name column */}
											<div className='min-w-0'>
												<div className='font-medium text-sm text-foreground truncate group-hover:text-primary transition-colors'>
													{job.name}
												</div>
												<div className='text-xs text-muted-foreground truncate mt-0.5'>
													{getDescription(job.description)}
												</div>
											</div>

											{/* Schedule column */}
											<div>
												<span className='inline-flex items-center font-mono text-xs text-muted-foreground bg-muted/60 px-2 py-1 rounded-md border border-border/40 leading-none'>
													{job.schedule}
												</span>
											</div>

											{/* Status column */}
											<div>
												<button
													onClick={(e) => { e.stopPropagation(); toggleStatus(job) }}
													className={cn(
														'inline-flex items-center gap-2 text-xs font-medium transition-colors',
														job.is_active ? 'text-foreground' : 'text-muted-foreground',
														'hover:opacity-80'
													)}
													title={job.is_active ? 'Click to deactivate' : 'Click to activate'}
												>
													<span className={cn(
														'w-2 h-2 rounded-full shrink-0 transition-colors',
														job.is_active ? 'bg-green-500' : 'bg-muted-foreground/40'
													)} />
													{job.is_active ? 'Active' : 'Inactive'}
												</button>
											</div>

											{/* Last Edited column */}
											<div className='text-xs text-muted-foreground tabular-nums'>
												{relativeTime(job.updated_at)}
											</div>

											{/* Actions column */}
											<div onClick={(e) => e.stopPropagation()} className='flex justify-end'>
												<DropdownMenu>
													<DropdownMenuTrigger asChild>
														<Button
															variant='ghost'
															size='icon'
															className='h-8 w-8 rounded-md text-muted-foreground hover:text-foreground'
														>
															<MoreHorizontal className='h-4 w-4' />
														</Button>
													</DropdownMenuTrigger>
													<DropdownMenuContent align='end' className='w-44 rounded-lg'>
														<DropdownMenuItem
															onClick={() => navigate("/cron/" + job.id, { state: { job } })}
														>
															<Eye className='h-4 w-4' />
															Open
														</DropdownMenuItem>
														<DropdownMenuItem onClick={() => toggleStatus(job)}>
															<Power className='h-4 w-4' />
															{job.is_active ? 'Deactivate' : 'Activate'}
														</DropdownMenuItem>
														<DropdownMenuItem onClick={() => handleRunNow(job)}>
															<Play className='h-4 w-4' />
															Run Now
														</DropdownMenuItem>
														<DropdownMenuSeparator />
														<DropdownMenuItem
															onClick={() => setDeleteId(job.id)}
															variant='destructive'
														>
															<Trash2 className='h-4 w-4' />
															Delete
														</DropdownMenuItem>
													</DropdownMenuContent>
												</DropdownMenu>
											</div>
										</div>

										{/* Mobile card layout */}
										<div className='md:hidden p-4 space-y-3'>
											<div className='flex items-start justify-between gap-3'>
												<div className='min-w-0 flex-1'>
													<div className='font-medium text-sm text-foreground truncate'>
														{job.name}
													</div>
													<div className='text-xs text-muted-foreground line-clamp-1 mt-0.5'>
														{getDescription(job.description)}
													</div>
												</div>
												<div onClick={(e) => e.stopPropagation()}>
													<DropdownMenu>
														<DropdownMenuTrigger asChild>
															<Button
																variant='ghost'
																size='icon'
																className='h-8 w-8 rounded-md text-muted-foreground hover:text-foreground -mr-1'
															>
																<MoreHorizontal className='h-4 w-4' />
															</Button>
														</DropdownMenuTrigger>
														<DropdownMenuContent align='end' className='w-44 rounded-lg'>
															<DropdownMenuItem
																onClick={() => navigate("/cron/" + job.id, { state: { job } })}
															>
																<Eye className='h-4 w-4' />
																Open
															</DropdownMenuItem>
															<DropdownMenuItem onClick={() => toggleStatus(job)}>
																<Power className='h-4 w-4' />
																{job.is_active ? 'Deactivate' : 'Activate'}
															</DropdownMenuItem>
															<DropdownMenuItem onClick={() => handleRunNow(job)}>
																<Play className='h-4 w-4' />
																Run Now
															</DropdownMenuItem>
															<DropdownMenuSeparator />
															<DropdownMenuItem
																onClick={() => setDeleteId(job.id)}
																variant='destructive'
															>
																<Trash2 className='h-4 w-4' />
																Delete
															</DropdownMenuItem>
														</DropdownMenuContent>
													</DropdownMenu>
												</div>
											</div>
											<div className='flex items-center gap-3'>
												<span className='inline-flex items-center font-mono text-[11px] text-muted-foreground bg-muted/60 px-2 py-1 rounded-md border border-border/40 leading-none'>
													{job.schedule}
												</span>
												<button
													onClick={(e) => { e.stopPropagation(); toggleStatus(job) }}
													className={cn(
														'inline-flex items-center gap-1.5 text-xs font-medium transition-colors',
														job.is_active ? 'text-foreground' : 'text-muted-foreground'
													)}
												>
													<span className={cn(
														'w-1.5 h-1.5 rounded-full shrink-0 transition-colors',
														job.is_active ? 'bg-green-500' : 'bg-muted-foreground/40'
													)} />
													{job.is_active ? 'Active' : 'Inactive'}
												</button>
												<span className='text-[11px] text-muted-foreground tabular-nums ml-auto'>
													{relativeTime(job.updated_at)}
												</span>
											</div>
										</div>
									</div>
								))}
							</div>
						</>
					)}
				</div>
			</div>

			{/* Floating Action Button for Mobile */}
			{isMobileView && (
				<Button
					onClick={() => navigate("/cron/new")}
					className='fixed bottom-20 right-6 h-14 w-14 rounded-2xl shadow-2xl shadow-primary/40 z-40 p-0'
				>
					<Plus className='h-6 w-6' />
				</Button>
			)}

			<input
				type='file'
				ref={fileInputRef}
				onChange={handleFileImport}
				accept='.json'
				className='hidden'
			/>
		</ScrollArea>
	)
}
