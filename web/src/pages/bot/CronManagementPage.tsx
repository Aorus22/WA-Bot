import { useState, useEffect, useRef } from 'react'
import { Plus, Trash2, Search, Code, Clock, Play, Download, Upload, MoreVertical, Calendar, Activity, FileText } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Switch } from '@/components/ui/switch'
import { 
    DropdownMenu, 
    DropdownMenuContent, 
    DropdownMenuItem, 
    DropdownMenuTrigger,
    DropdownMenuSeparator
} from '@/components/ui/dropdown-menu'
import { CronDeleteModal } from './CronDeleteModal'
import { CronDeleteAllModal } from './CronDeleteAllModal'
import { api, type CronJob } from '@/lib/api'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'

interface CronManagementPageProps {
	onEditCron: (cron: CronJob | null) => void
    onViewDocs: () => void
	isMobileView?: boolean
}

export function CronManagementPage({ onEditCron, onViewDocs, isMobileView }: CronManagementPageProps) {
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
            <div className='flex flex-col min-h-full w-full pb-20 md:pb-0'>
                <CronDeleteModal 
                    open={!!deleteId} 
                    onOpenChange={(open) => !open && setDeleteId(null)} 
                    onConfirm={handleDeleteConfirm} 
                />
                <CronDeleteAllModal 
                    open={isDeleteAllOpen} 
                    onOpenChange={setIsDeleteAllOpen} 
                    onConfirm={handleDeleteAllConfirm} 
                />

                <div className='p-4 md:p-8 border-b border-border/40 bg-muted/5'>
                    <div className='flex flex-col md:flex-row md:items-center justify-between gap-6'>
                        <div className='flex items-center gap-4'>
                            <div className='p-3 rounded-2xl bg-primary/10 text-primary shadow-sm'>
                                <Calendar className='h-6 w-6' />
                            </div>
                            <div>
                                <h1 className='text-2xl md:text-3xl font-bold tracking-tight text-foreground'>Cron Management</h1>
                                <p className='text-xs md:text-sm text-muted-foreground'>Automate tasks with scheduled Lua scripts.</p>
                            </div>
                        </div>
                        
                        <div className='flex flex-col sm:flex-row items-stretch sm:items-center gap-3'>
                            <div className='relative flex-1 sm:w-64'>
                                <Search className='absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground' />
                                <Input
                                    placeholder='Search jobs...'  
                                    value={searchQuery}
                                    onChange={e => setSearchQuery(e.target.value)}
                                    className='pl-9 rounded-xl border-border/40 bg-background focus-visible:ring-primary/20 h-10'
                                />
                            </div>

                            <div className='flex items-center gap-2 justify-end'>
                                <input type="file" ref={fileInputRef} onChange={handleFileImport} accept=".json" className="hidden" />
                                
                                <div className='flex items-center p-1 bg-muted/50 rounded-xl border border-border/40'>
                                    <Button
                                        variant='ghost'
                                        size='icon'
                                        onClick={() => setIsDeleteAllOpen(true)}
                                        className='h-8 w-8 rounded-lg text-destructive hover:text-destructive hover:bg-destructive/10'
                                        title='Delete ALL Jobs'
                                        disabled={jobs.length === 0}
                                    >
                                        <Trash2 className='h-4 w-4' />
                                    </Button>
                                    <Button
                                        variant='ghost'
                                        size='icon'
                                        onClick={handleExport}    
                                        className='h-8 w-8 rounded-lg'
                                        title='Export Jobs'   
                                    >
                                        <Download className='h-4 w-4' />
                                    </Button>
                                    <Button
                                        variant='ghost'
                                        size='icon'
                                        onClick={handleImportClick}
                                        className='h-8 w-8 rounded-lg'
                                        title='Import Jobs'   
                                    >
                                        <Upload className='h-4 w-4' />
                                    </Button>
                                </div>

                                <Button
                                    variant='outline'
                                    onClick={onViewDocs}
                                    className='rounded-xl h-10 px-4 border-border/40 hidden sm:flex'
                                >
                                    <FileText className='mr-2 h-4 w-4' /> Docs
                                </Button>

                                {!isMobileView && (
                                    <Button
                                        onClick={() => onEditCron(null)}
                                        className='rounded-xl px-6 h-10 font-bold shadow-lg shadow-primary/20'
                                    >
                                        <Plus className='mr-2 h-4 w-4' /> New Job
                                    </Button>
                                )}
                            </div>
                        </div>
                    </div>
                </div>

                <div className='p-4 md:p-8 w-full flex-1'>
                    {loading ? (
                        <div className='flex flex-col items-center justify-center py-24 gap-4 opacity-50'>
                            <div className='w-10 h-10 border-4 border-primary/30 border-t-primary rounded-full animate-spin' />
                            <p className='font-medium'>Syncing schedules...</p>
                        </div>
                    ) : filteredJobs.length === 0 ? (
                        <div className='flex flex-col items-center justify-center py-20 md:py-32 border-2 border-dashed border-border/40 rounded-[2rem] md:rounded-[3rem] text-center space-y-6 bg-muted/5'>
                            <div className='w-20 h-20 rounded-3xl bg-muted flex items-center justify-center text-muted-foreground'>
                                <Clock className='h-10 w-10' />
                            </div>
                            <div className='space-y-2 px-4'>
                                <h3 className='text-xl font-bold'>No active schedules</h3>
                                <p className='text-muted-foreground max-w-xs mx-auto'>Create your first automated task to begin autonomous operations.</p>
                            </div>
                            <Button onClick={() => onEditCron(null)} className='rounded-xl px-8 h-11 font-bold'>
                                <Plus className='mr-2 h-5 w-5' /> Initialize First Job
                            </Button>
                        </div>
                    ) : (
                        <div className={cn(
                            "grid gap-4 pb-10",
                            isMobileView ? "grid-cols-1" : "grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4"
                        )}>
                            {filteredJobs.map((job) => (
                                <Card key={job.id} className="group relative overflow-hidden border-border/40 bg-card/50 hover:bg-card hover:border-primary/30 transition-all rounded-[1.5rem]">
                                    <CardHeader className="pb-3">
                                        <div className="flex items-start justify-between gap-4">
                                            <div className="space-y-1 overflow-hidden">
                                                <CardTitle className="text-base font-bold truncate">
                                                    {job.name}
                                                </CardTitle>
                                                <Badge variant="outline" className="font-mono text-[10px] py-0 px-2 bg-muted/50 border-border/40 text-muted-foreground">
                                                    {job.schedule}
                                                </Badge>
                                            </div>
                                            <Switch 
                                                checked={job.is_active} 
                                                onCheckedChange={() => toggleStatus(job)}
                                                className="data-[state=checked]:bg-primary"
                                            />
                                        </div>
                                    </CardHeader>

                                    <CardContent className="pb-4">
                                        <div className="flex items-center gap-2 text-muted-foreground">
                                            <div className="p-1.5 rounded-lg bg-primary/5 text-primary">
                                                <Code className="w-3.5 h-3.5" />
                                            </div>
                                            <span className="text-xs font-medium">Lua Engine</span>
                                        </div>
                                        
                                        <div className="mt-4 flex items-center text-[10px] text-muted-foreground/70 font-bold uppercase tracking-wider">
                                            <Activity className="w-3 h-3 mr-1.5 text-primary/60" />
                                            Updated {new Date(job.updated_at || job.created_at || '').toLocaleDateString()}
                                        </div>
                                    </CardContent>

                                    <div className="flex items-center justify-between p-2 px-3 border-t bg-muted/30">
                                        <div className="flex gap-1">
                                            <Button 
                                                variant="ghost" 
                                                size="icon" 
                                                className="h-8 w-8 rounded-lg hover:bg-primary/10 hover:text-primary transition-colors"
                                                onClick={() => handleRunNow(job)}
                                                title="Run Now"
                                            >
                                                <Play className="w-4 h-4" />
                                            </Button>
                                            <Button 
                                                variant="ghost" 
                                                size="icon" 
                                                className="h-8 w-8 rounded-lg"
                                                onClick={() => onEditCron(job)}
                                                title="Edit Script"
                                            >
                                                <Code className="w-4 h-4" />
                                            </Button>
                                        </div>

                                        <DropdownMenu>
                                            <DropdownMenuTrigger asChild>
                                                <Button variant="ghost" size="icon" className="h-8 w-8 rounded-lg">
                                                    <MoreVertical className="w-4 h-4" />
                                                </Button>
                                            </DropdownMenuTrigger>
                                            <DropdownMenuContent align="end" className="rounded-xl border-border/40">
                                                <DropdownMenuItem onClick={() => onEditCron(job)} className="rounded-lg">
                                                    Edit Config
                                                </DropdownMenuItem>
                                                <DropdownMenuItem onClick={() => handleRunNow(job)} className="rounded-lg">
                                                    Manual Trigger
                                                </DropdownMenuItem>
                                                <DropdownMenuSeparator />
                                                <DropdownMenuItem 
                                                    onClick={() => setDeleteId(job.id)} 
                                                    className="text-destructive focus:text-destructive focus:bg-destructive/10 rounded-lg"
                                                >
                                                    <Trash2 className="w-4 h-4 mr-2" />
                                                    Delete Job
                                                </DropdownMenuItem>
                                            </DropdownMenuContent>
                                        </DropdownMenu>
                                    </div>
                                </Card>
                            ))}
                        </div>
                    )}
                </div>
            </div>
        </ScrollArea>
	)
}
