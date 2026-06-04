import { useState, useEffect } from 'react'
import { ArrowLeft, Save, Play, Code, AlertCircle, CheckCircle2, Terminal, Info, Zap, FileText, Bug } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { api, type Trigger } from '@/lib/api'
import { toast } from 'sonner'
import Editor from '@monaco-editor/react'
import { cn } from '@/lib/utils'

interface TriggerEditorPageProps {
        trigger: Trigger | null
        onBack: () => void
        onViewDocs: () => void
        isMobileView?: boolean
}

export function TriggerEditorPage({ trigger, onBack, onViewDocs, isMobileView }: TriggerEditorPageProps) {
        const [formData, setFormData] = useState<Partial<Trigger>>({
                name: '',
                pattern: '',
                script: `-- Available: sender, content, matches, send_text(), fetch()...\nlocal param = matches[1]\nprint("Simulating response for: " .. (param or "none"))\nsend_text(sender, "You said: " .. (param or "nothing"))`,
                is_active: true,
                priority: 0
        })

        const [testMessage, setTestMessage] = useState('')
        const [testResult, setTestResult] = useState<any>(null)
        const [isTesting, setIsTesting] = useState(false)
        const [isSaving, setIsSaving] = useState(false)
        const [activeTab, setActiveTab] = useState('editor')

        useEffect(() => {
                if (trigger) {
                        setFormData(trigger)
                }
        }, [trigger])

        const handleSave = async () => {
                if (!formData.name || !formData.pattern || !formData.script) {
                        toast.error('Please fill in all fields')
                        return
                }

                try {
                        setIsSaving(true)
                        if (formData.id) {
                                await api.updateTrigger(formData.id, formData)
                                toast.success('Trigger updated successfully')
                        } else {
                                await api.createTrigger(formData)
                                toast.success('Trigger created successfully')
                        }
                        onBack()
                } catch {
                        toast.error('Failed to save trigger')
                } finally {
                        setIsSaving(false)
                }
        }

        const handleTest = async () => {
                if (!formData.pattern || !formData.script || !testMessage) {
                        toast.error('Pattern, script, and test message are required')
                        return
                }

                try {
                        setIsTesting(true)
                        const result = await api.testTrigger({
                                pattern: formData.pattern,
                                script: formData.script,
                                message: testMessage
                        })
                        setTestResult(result)
                        if (isMobileView) {
                                setActiveTab('debugger')
                        }
                } catch (error: any) {
                        setTestResult({ error: error.message })
                } finally {
                        setIsTesting(false)
                }
        }

        const editorContent = (
                <div className='p-3 md:p-6 space-y-4 md:space-y-6 flex-1 flex flex-col min-h-0'>
                        <div className='grid grid-cols-2 md:grid-cols-3 gap-3 md:gap-6 shrink-0'>
                                <div className='col-span-2 md:col-span-1 space-y-1.5'>
                                        <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1'>Trigger Name</Label>
                                        <Input
                                                placeholder='e.g., Auto Reply Greeting'
                                                value={formData.name}
                                                onChange={e => setFormData(prev => ({ ...prev, name: e.target.value }))}
                                                className='rounded-xl h-9 md:h-11 border-border/40 bg-muted/30 focus-visible:ring-primary/20 text-xs md:text-sm'
                                        />
                                </div>
                                <div className='col-span-1 md:col-span-1 space-y-1.5'>
                                        <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1'>Regex Pattern</Label>
                                        <Input
                                                placeholder='^!test\s+(.*)'
                                                value={formData.pattern}
                                                onChange={e => setFormData(prev => ({ ...prev, pattern: e.target.value }))}
                                                className='rounded-xl h-9 md:h-11 border-border/40 bg-muted/30 font-mono focus-visible:ring-primary/20 text-xs md:text-sm'
                                        />
                                </div>
                                <div className='col-span-1 md:col-span-1 space-y-1.5'>
                                        <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1'>Priority</Label>
                                        <Input
                                                type='number'
                                                placeholder='0'
                                                value={formData.priority || 0}
                                                onChange={e => setFormData(prev => ({ ...prev, priority: parseInt(e.target.value) || 0 }))}
                                                className='rounded-xl h-9 md:h-11 border-border/40 bg-muted/30 focus-visible:ring-primary/20 text-xs md:text-sm'
                                        />
                                </div>
                        </div>
                        <div className='space-y-2 md:space-y-3 flex-1 flex flex-col min-h-0'>
                                <div className='flex items-center justify-between shrink-0'>
                                        <Label className='text-[10px] md:text-xs font-bold uppercase tracking-wider text-muted-foreground ml-1 flex items-center gap-2'>
                                                <Code className='h-3 w-3 text-primary' />
                                                Lua Script Logic
                                        </Label>
                                        <div className='flex gap-1 md:gap-2'>
                                                <Badge variant='outline' className='text-[8px] md:text-[9px] font-mono py-0 px-1 md:px-2'>matches[n]</Badge>
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
                                                                formatOnType: true,
                                                                scrollbar: {
                                                                    vertical: 'visible',
                                                                    horizontal: 'visible',
                                                                    useShadows: false,
                                                                    verticalScrollbarSize: 10,
                                                                    horizontalScrollbarSize: 10
                                                                }
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
                                        Trigger Debugger
                                </h2>
                                <div className='space-y-3'>
                                        <Label className='text-[10px] font-bold uppercase tracking-wider text-muted-foreground'>Simulate Message</Label>
                                        <div className='flex gap-2'>
                                                <Input
                                                        placeholder='Test message...'
                                                        value={testMessage}
                                                        onChange={e => setTestMessage(e.target.value)}
                                                        onKeyDown={e => e.key === 'Enter' && handleTest()}
                                                        className='rounded-xl bg-background border-border/40 h-10'
                                                />
                                                <Button
                                                        onClick={handleTest}
                                                        disabled={isTesting}
                                                        className='rounded-xl px-4 h-10'
                                                >
                                                        {isTesting ? <span className='w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin' /> : <Play className='h-4 w-4' />}
                                                </Button>
                                        </div>
                                </div>
                        </div>

                        <ScrollArea className='flex-1 p-4 md:p-6 min-h-0'>
                                {testResult ? (
                                        <div className='space-y-6'>
                                                <div className={cn(
                                                        'p-4 rounded-2xl flex items-center gap-3 border shadow-sm',
                                                        testResult.matched ? 'bg-green-500/5 border-green-500/20 text-green-600 dark:text-green-400' : 'bg-destructive/5 border-destructive/20 text-destructive'
                                                )}>
                                                        {testResult.matched ? <CheckCircle2 className='h-5 w-5' /> : <AlertCircle className='h-5 w-5' />}
                                                        <div>
                                                                <p className='text-sm font-bold'>{testResult.matched ? 'Pattern Matched' : 'No Match'}</p>
                                                                <p className='text-[10px] opacity-80'>{testResult.matched ? 'Script would be executed' : 'Check your regex pattern'}</p>
                                                        </div>
                                                </div>

                                                {testResult.matched && (
                                                        <>
                                                                <div className='space-y-2'>
                                                                        <Label className='text-[10px] font-bold uppercase tracking-wider text-muted-foreground flex items-center gap-2'>
                                                                                <Info className='h-3 w-3' /> Capture Groups
                                                                        </Label>
                                                                        <div className='p-3 rounded-xl bg-muted/50 border border-border/40 font-mono text-[10px] md:text-[11px] space-y-1.5'>
                                                                                {testResult.matches?.map((m: string, i: number) => (
                                                                                        <div key={i} className='flex gap-2 border-b border-border/10 pb-1 last:border-0 last:pb-0'>
                                                                                                <span className='text-muted-foreground w-6'>[{i}]</span>
                                                                                                <span className='text-foreground break-all'>{m || '""'}</span>
                                                                                        </div>
                                                                                ))}
                                                                        </div>
                                                                </div>

                                                                <div className='space-y-2'>
                                                                        <Label className='text-[10px] font-bold uppercase tracking-wider text-muted-foreground flex items-center gap-2'>
                                                                                <Terminal className='h-3 w-3' /> Execution Logs
                                                                        </Label>
                                                                        <div className='p-3 rounded-xl bg-black text-green-400 font-mono text-[10px] md:text-[11px] min-h-[100px] whitespace-pre-wrap leading-relaxed border border-white/5 shadow-inner'>
                                                                                {testResult.logs?.length > 0 ? testResult.logs.join('\n') : '> No logs recorded'}
                                                                                {testResult.error && <div className='text-destructive mt-2 font-bold'>ERROR: {testResult.error}</div>}
                                                                        </div>
                                                                </div>

                                                                <div className='space-y-2'>
                                                                        <Label className='text-[10px] font-bold uppercase tracking-wider text-muted-foreground flex items-center gap-2'>
                                                                                <Zap className='h-3 w-3' /> Simulated Actions
                                                                        </Label>
                                                                        <div className='space-y-2'>
                                                                                {testResult.actions?.length > 0 ? (
                                                                                        testResult.actions.map((a: string, i: number) => (
                                                                                                <div key={i} className='p-3 rounded-xl bg-primary/5 border border-primary/20 text-[10px] md:text-[11px] text-primary font-medium flex items-center gap-2'>
                                                                                                        <div className='w-1 h-1 rounded-full bg-primary' />
                                                                                                        {a}
                                                                                                </div>
                                                                                        ))
                                                                                ) : (
                                                                                        <div className='p-3 rounded-xl bg-muted/50 border border-border/40 text-[10px] md:text-[11px] text-muted-foreground italic text-center py-4'>
                                                                                                No messaging actions performed.
                                                                                        </div>
                                                                                )}
                                                                        </div>
                                                                </div>
                                                        </>
                                                )}
                                        </div>
                                ) : (
                                        <div className='h-full flex flex-col items-center justify-center text-center opacity-40 py-20'>
                                                <Bug className='h-12 w-12 mb-4 text-muted-foreground' />
                                                <p className='text-sm font-medium'>Ready to debug</p>
                                                <p className='text-xs max-w-[200px] mt-1 mx-auto'>Enter a test message above and click Play to simulate execution.</p>
                                        </div>
                                )}
                        </ScrollArea>
                </div>
        )

        return (
                <div className='flex flex-col h-full w-full bg-background overflow-hidden'>
                        <div className='p-4 md:p-6 border-b border-border/40 bg-muted/20 shrink-0 flex flex-col sm:flex-row sm:items-center justify-between gap-4'>
                                <div className='flex items-center gap-3 md:gap-4'>
                                        <Button variant='ghost' size='icon' onClick={onBack} className='rounded-full h-8 w-8 md:h-10 md:w-10'>
                                                <ArrowLeft className='h-5 w-5' />
                                        </Button>
                                        <div>
                                                <h1 className='text-lg md:text-xl font-bold tracking-tight'>
                                                        {formData.id ? 'Edit Trigger' : 'New Trigger'}
                                                </h1>
                                                <p className='text-[10px] md:text-xs text-muted-foreground hidden xs:block'>Configure regex pattern and Lua script execution.</p>
                                        </div>
                                </div>
                                <div className='flex items-center gap-2 sm:gap-3'>
                                        <Button variant='outline' onClick={onViewDocs} className='rounded-xl h-9 md:h-10 px-3 md:px-4 text-xs md:text-sm hidden md:flex'>
                                                <FileText className='mr-2 h-4 w-4' /> Documentation
                                        </Button>
                                        <Button variant='outline' onClick={onBack} className='rounded-xl h-9 md:h-10 px-3 md:px-4 text-xs md:text-sm flex-1 sm:flex-none'>Cancel</Button>
                                        <Button onClick={handleSave} disabled={isSaving} className='rounded-xl h-9 md:h-10 px-4 md:px-6 text-xs md:text-sm font-bold shadow-lg shadow-primary/20 flex-1 sm:flex-none'>
                                                {isSaving ? <span className='w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin mr-2' /> : <Save className='mr-2 h-4 w-4' />}
                                                Save <span className='hidden sm:inline'>Changes</span>
                                        </Button>
                                </div>
                        </div>

                        <div className='flex-1 flex flex-col md:flex-row overflow-hidden pb-16 md:pb-0'>
                                {isMobileView ? (
                                        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1 flex flex-col h-full overflow-hidden">
                                                <div className="px-4 pt-2 shrink-0">
                                                        <TabsList className="grid w-full grid-cols-2 rounded-xl h-10">
                                                                <TabsTrigger value="editor" className="rounded-lg text-xs font-bold uppercase tracking-wider">Editor</TabsTrigger>
                                                                <TabsTrigger value="debugger" className="rounded-lg text-xs font-bold uppercase tracking-wider flex items-center gap-2">
                                                                        Debugger
                                                                        {testResult && (
                                                                                <div className={cn("w-1.5 h-1.5 rounded-full", testResult.matched ? "bg-green-500" : "bg-destructive")} />
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
