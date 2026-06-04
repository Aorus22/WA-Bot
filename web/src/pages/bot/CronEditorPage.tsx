import { useState, useEffect } from 'react'
import { ArrowLeft, Save, Play, Code, Clock, Info, CheckCircle2, AlertCircle, Terminal, FileText, Activity, Calendar } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { api, type CronJob } from '@/lib/api'
import { toast } from 'sonner'
import Editor from '@monaco-editor/react'
import { cn } from '@/lib/utils'

interface CronEditorPageProps {
	job: CronJob | null
	onBack: () => void
    onViewDocs: () => void
	isMobileView?: boolean
}

export function CronEditorPage({ job, onBack, onViewDocs, isMobileView }: CronEditorPageProps) {
	const [formData, setFormData] = useState<Partial<CronJob>>({
		name: '',
		schedule: '0 * * * *',
		script: `-- Scheduled Task\nprint("Executing sequence: " .. os.date())\nsend_text("628123456789@s.whatsapp.net", "Automatic scheduled broadcast")`,
		is_active: true
	})

	const [isTesting, setIsTesting] = useState(false)
	const [isSaving, setIsSaving] = useState(false)
    const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null)
    const [activeTab, setActiveTab] = useState('editor')

	useEffect(() => {
		if (job) {
			setFormData(job)
		}
	}, [job])

	const handleSave = async () => {
		if (!formData.name || !formData.schedule || !formData.script) {
			toast.error('All configuration fields are required')
			return
		}

		try {
			setIsSaving(true)
			if (formData.id) {
				await api.updateCronJob(formData.id, formData)
				toast.success('Sequence configuration synchronized')
			} else {
				await api.createCronJob(formData)
				toast.success('New automated process established')
			}
			onBack()
		} catch {
			toast.error('Failed to synchronize configuration')
		} finally {
			setIsSaving(false)
		}
	}

	const handleTest = async () => {
		if (!formData.script) {
			toast.error('Script body required for execution')
			return
		}

		try {
			setIsTesting(true)
            setTestResult(null)
			await api.testCronJob(formData.script)
			toast.success('Manual trigger initialized')
            setTestResult({ success: true, message: 'Process started. Check system logs for output.' })
            if (isMobileView) setActiveTab('debugger')
		} catch (error: any) {
			toast.error('Initialization failed')
            setTestResult({ success: false, message: error.message })
		} finally {
			setIsTesting(false)
		}
	}

    const editorContent = (
        <div className='p-3 md:p-6 space-y-4 md:space-y-6 flex-1 flex flex-col min-h-0'>
            <div className='grid grid-cols-1 md:grid-cols-2 gap-3 md:gap-6 shrink-0'>
                <div className='space-y-1.5'>
                    <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1'>Process Name</Label>
                    <Input
                        placeholder='e.g., Daily Database Sync'
                        value={formData.name}
                        onChange={e => setFormData(prev => ({ ...prev, name: e.target.value }))}
                        className='rounded-xl h-9 md:h-11 border-border/40 bg-muted/30 focus-visible:ring-primary/20 text-xs md:text-sm'
                    />
                </div>
                <div className='space-y-1.5'>
                    <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1'>Temporal Schedule (Cron)</Label>
                    <div className='flex gap-2'>
                        <Input
                            placeholder='0 0 * * *'
                            value={formData.schedule}
                            onChange={e => setFormData(prev => ({ ...prev, schedule: e.target.value }))}
                            className='rounded-xl h-9 md:h-11 border-border/40 bg-muted/30 font-mono focus-visible:ring-primary/20 flex-1 text-xs md:text-sm'
                        />
                        <Button 
                            variant='outline' 
                            size='icon' 
                            className='rounded-xl h-9 md:h-11 w-9 md:w-11 shrink-0 border-border/40 bg-muted/20 hover:bg-primary/5 hover:text-primary'
                            onClick={() => window.open('https://crontab.guru/', '_blank')}
                        >
                            <Info className='h-3.5 w-3.5 md:h-4 md:w-4' />
                        </Button>
                    </div>
                </div>
            </div>

            <div className='space-y-2 md:space-y-3 flex-1 flex flex-col min-h-0'>
                <div className='flex items-center justify-between shrink-0'>
                    <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1 flex items-center gap-2'>
                        <Code className='h-3 w-3 text-primary' />
                        Execution Logic
                    </Label>
                    <div className='flex gap-1 md:gap-2'>
                        <Badge variant='outline' className='text-[8px] md:text-[9px] font-mono py-0 px-1 md:px-2'>os.date()</Badge>
                        <Badge variant='outline' className='text-[8px] md:text-[9px] font-mono py-0 px-1 md:px-2 hidden sm:inline-flex'>send_text()</Badge>
                    </div>
                </div>
                <div className='flex-1 md:h-full min-h-[350px] md:min-h-0 rounded-2xl border border-border/40 overflow-hidden bg-[#1e1e1e] shadow-inner relative'>
                    <div className='absolute inset-0'>
                        <Editor
                            height="100%"
                            defaultLanguage="lua"
                            theme="vs-dark"
                            value={formData.script}
                            onChange={(val) => setFormData(prev => ({ ...prev, script: val || '' }))}
                            loading={<div className="h-full w-full flex items-center justify-center text-muted-foreground text-[10px] font-mono animate-pulse">Initializing Engine...</div>}
                            options={{
                                minimap: { enabled: false },
                                fontSize: isMobileView ? 12 : 13,
                                lineNumbers: 'on',
                                roundedSelection: true,
                                scrollBeyondLastLine: false,
                                automaticLayout: true,
                                padding: { top: 12, bottom: 12 },
                                fixedOverflowWidgets: true,
                                wordWrap: 'on',
                                formatOnPaste: true,
                                formatOnType: true
                            }}
                        />
                    </div>
                </div>
            </div>
        </div>
    )

    const debuggerContent = (
        <div className={cn('bg-muted/10 flex flex-col h-full overflow-hidden shrink-0', isMobileView ? 'w-full' : 'w-[400px] lg:w-[450px]')}>
            <div className='p-4 md:p-6 border-b border-border/40 bg-muted/30 shrink-0'>       
                <h2 className='text-sm font-bold flex items-center gap-2 mb-4'>
                    <Terminal className='h-4 w-4 text-primary' />
                    Process Monitor
                </h2>
                <div className='space-y-3'>
                    <Label className='text-[10px] font-bold uppercase tracking-wider text-muted-foreground'>Manual Execution</Label>
                    <Button
                        onClick={handleTest}
                        disabled={isTesting}
                        className='rounded-xl w-full h-10 font-bold'
                    >
                        {isTesting ? <span className='w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin mr-2' /> : <Play className='h-4 w-4 mr-2' />}
                        Run Test Sequence
                    </Button>
                </div>
            </div>

            <ScrollArea className='flex-1 p-4 md:p-6 min-h-0'>
                {testResult ? (
                    <div className='space-y-6'>
                        <div className={cn(
                            'p-4 rounded-2xl flex items-center gap-3 border shadow-sm',
                            testResult.success ? 'bg-green-500/5 border-green-500/20 text-green-600 dark:text-green-400' : 'bg-destructive/5 border-destructive/20 text-destructive'    
                        )}>
                            {testResult.success ? <CheckCircle2 className='h-5 w-5' /> : <AlertCircle className='h-5 w-5' />}
                            <div>
                                <p className='text-sm font-bold'>{testResult.success ? 'Execution Success' : 'Execution Failed'}</p>
                                <p className='text-[10px] opacity-80'>{testResult.success ? 'Manual trigger processed' : 'Review script logic'}</p>
                            </div>
                        </div>

                        <div className='space-y-2'>
                            <Label className='text-[10px] font-bold uppercase tracking-wider text-muted-foreground flex items-center gap-2'>
                                <Activity className='h-3 w-3' /> Status Report
                            </Label>
                            <div className='p-4 rounded-xl bg-muted/50 border border-border/40 font-medium text-xs leading-relaxed'>
                                {testResult.message}
                            </div>
                        </div>

                        <div className='p-4 rounded-2xl bg-primary/5 border border-primary/10 space-y-3'>
                            <div className='flex items-center gap-2 text-primary'>
                                <Info className='h-4 w-4' />
                                <span className='text-xs font-bold uppercase tracking-widest'>Engine Notice</span>
                            </div>
                            <p className='text-[11px] leading-relaxed text-muted-foreground'>
                                Cron tests run in the background. Unlike triggers, they don't return live logs to this UI. Check the server console for print output.
                            </p>
                        </div>
                    </div>
                ) : (
                    <div className='h-full flex flex-col items-center justify-center text-center opacity-40 py-20'>
                        <Activity className='h-12 w-12 mb-4 text-muted-foreground' />  
                        <p className='text-sm font-medium'>Awaiting Initialization</p>     
                        <p className='text-xs max-w-[200px] mt-1 mx-auto'>Click "Run Test Sequence" to manually execute the Lua process.</p>
                    </div>
                )}
            </ScrollArea>
        </div>
    )

	return (
		<div className="flex flex-col h-full w-full bg-background overflow-hidden">
			{/* Unified Header */}
			<div className="p-4 md:p-6 border-b border-border/40 bg-muted/20 shrink-0 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
				<div className="flex items-center gap-3 md:gap-4">
					<Button 
						variant="ghost" 
						size="icon" 
						onClick={onBack}
						className="rounded-full h-8 w-8 md:h-10 md:w-10"
					>
						<ArrowLeft className="w-5 h-5" />
					</Button>
					<div className="flex flex-col">
						<div className="flex items-center gap-2">
							<h1 className="text-lg md:text-xl font-bold tracking-tight">
								{formData.id ? 'Task Configuration' : 'Establish Task'}
							</h1>
							<Badge className={cn(
                                "border-none rounded-full text-[10px] font-bold px-3 py-0.5",
                                formData.is_active 
                                    ? "bg-green-500/10 text-green-500 hover:bg-green-500/20" 
                                    : "bg-muted text-muted-foreground"
                            )}>
								{formData.is_active ? 'ACTIVE' : 'INACTIVE'}
							</Badge>
						</div>
                        <p className='text-[10px] md:text-xs text-muted-foreground hidden xs:block'>Define scheduled behavior and autonomous logic.</p>
					</div>
				</div>
				<div className="flex items-center gap-2 sm:gap-3">
                    <div className="flex items-center gap-2 mr-2 bg-muted/30 px-3 py-1.5 rounded-xl border border-border/40">
                        <Label htmlFor="active-toggle" className="text-[10px] font-bold uppercase cursor-pointer">Enabled</Label>
                        <input 
                            id="active-toggle"
                            type="checkbox" 
                            checked={formData.is_active}
                            onChange={e => setFormData(prev => ({ ...prev, is_active: e.target.checked }))}
                            className="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary"
                        />
                    </div>
                    <Button variant='outline' onClick={onViewDocs} className='rounded-xl h-9 md:h-10 px-3 md:px-4 text-xs md:text-sm hidden md:flex border-border/40'>
                        <FileText className='mr-2 h-4 w-4' /> Docs       
                    </Button>
                    <Button variant='outline' onClick={onBack} className='rounded-xl h-9 md:h-10 px-3 md:px-4 text-xs md:text-sm flex-1 sm:flex-none border-border/40'>Cancel</Button>
					<Button 
						onClick={handleSave} 
						disabled={isSaving}
						className="rounded-xl h-9 md:h-10 px-4 md:px-6 text-xs md:text-sm font-bold shadow-lg shadow-primary/20 flex-1 sm:flex-none"
					>
						{isSaving ? <span className='w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin mr-2' /> : <Save className="mr-2 h-4 w-4" />}
						Save <span className='hidden sm:inline'>Sequence</span>
					</Button>
				</div>
			</div>

			<div className='flex-1 flex flex-col md:flex-row overflow-hidden pb-16 md:pb-0'>  
                {isMobileView ? (
                    <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1 flex flex-col h-full overflow-hidden">
                        <div className="px-4 pt-2 shrink-0">
                            <TabsList className="grid w-full grid-cols-2 rounded-xl h-10">
                                <TabsTrigger value="editor" className="rounded-lg text-xs font-bold uppercase tracking-wider">Config</TabsTrigger>
                                <TabsTrigger value="debugger" className="rounded-lg text-xs font-bold uppercase tracking-wider flex items-center gap-2">
                                    Monitor
                                    {testResult && (
                                        <div className={cn("w-1.5 h-1.5 rounded-full", testResult.success ? "bg-green-500" : "bg-destructive")} />
                                    )}
                                </TabsTrigger>
                            </TabsList>
                        </div>
                        <TabsContent value="editor" className="flex-1 flex flex-col overflow-hidden m-0 p-0 outline-none">
                            <div className="flex-1 min-h-0">
                                {editorContent}
                            </div>
                        </TabsContent>
                        <TabsContent value="debugger" className="flex-1 flex flex-col overflow-hidden m-0 p-0 outline-none">
                            {debuggerContent}
                        </TabsContent>
                    </Tabs>
                ) : (
                    <>
                        <div className='flex-1 flex flex-col border-r border-border/40 overflow-hidden min-h-0'>
                            {editorContent}
                        </div>
                        {debuggerContent}
                    </>
                )}
            </div>
		</div>
	)
}
