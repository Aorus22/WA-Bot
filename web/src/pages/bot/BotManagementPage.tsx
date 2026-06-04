import { useState, useEffect, useRef } from 'react'
import { Bot, Plus, Trash2, Search, Code, Zap, FileText, Download, Upload } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle, CardFooter } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Switch } from '@/components/ui/switch'
import { TriggerDeleteModal } from './TriggerDeleteModal'
import { TriggerDeleteAllModal } from './TriggerDeleteAllModal'
import { api, type Trigger } from '@/lib/api'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'

interface BotManagementPageProps {
        onEditTrigger: (trigger: Trigger | null) => void
        onViewDocs: () => void
        isMobileView?: boolean
}

export function BotManagementPage({ onEditTrigger, onViewDocs, isMobileView }: BotManagementPageProps) {
        const [triggers, setTriggers] = useState<Trigger[]>([])
        const [loading, setLoading] = useState(true)
        const [searchQuery, setSearchQuery] = useState('')
        const [deleteId, setDeleteId] = useState<string | null>(null)
        const [isDeleteAllOpen, setIsDeleteAllOpen] = useState(false)
        const fileInputRef = useRef<HTMLInputElement>(null)

        useEffect(() => {
                loadTriggers()
        }, [])

        const loadTriggers = async () => {
                try {
                        setLoading(true)
                        const data = await api.getTriggers()
                        setTriggers(data || [])
                } catch (error) {
                        console.error('Failed to load triggers:', error)
                        toast.error('Failed to load triggers')
                } finally {
                        setLoading(false)
                }
        }

        const handleDelete = async () => {
                if (!deleteId) return
                try {
                        await api.deleteTrigger(deleteId)
                        toast.success('Trigger deleted')
                        setDeleteId(null)
                        loadTriggers()
                } catch (error) {
                        toast.error('Failed to delete trigger')
                }
        }

        const handleDeleteAll = async () => {
                try {
                        setLoading(true)
                        setIsDeleteAllOpen(false)
                        await api.deleteAllTriggers()
                        toast.success('All triggers deleted')
                        loadTriggers()
                } catch (error) {
                        console.error('Failed to delete all triggers:', error)
                        toast.error('Failed to delete all triggers')
                } finally {
                        setLoading(false)
                }
        }

        const toggleStatus = async (trigger: Trigger) => {
                try {
                        await api.updateTrigger(trigger.id, { ...trigger, is_active: !trigger.is_active })
                        loadTriggers()
                } catch (error) {
                        toast.error('Failed to update status')
                }
        }

        const handleExport = () => {
                if (triggers.length === 0) {
                        toast.error('No triggers to export')
                        return
                }

                const dataStr = JSON.stringify(triggers, null, 2)
                const dataUri = 'data:application/json;charset=utf-8,' + encodeURIComponent(dataStr)

                const exportFileDefaultName = `wa-bot-triggers-${new Date().toISOString().split('T')[0]}.json`

                const linkElement = document.createElement('a')
                linkElement.setAttribute('href', dataUri)
                linkElement.setAttribute('download', exportFileDefaultName)
                linkElement.click()

                toast.success('Triggers exported successfully')
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
                                const importedTriggers = JSON.parse(content)

                                if (!Array.isArray(importedTriggers)) {
                                        throw new Error('Invalid format: Expected an array of triggers')
                                }

                                setLoading(true)
                                let successCount = 0
                                let failCount = 0

                                for (const trigger of importedTriggers) {
                                        try {
                                                const { id, created_at, updated_at, ...cleanTrigger } = trigger
                                                await api.createTrigger(cleanTrigger)
                                                successCount++
                                        } catch (err) {
                                                console.error('Failed to import trigger:', trigger.name, err)
                                                failCount++
                                        }
                                }

                                toast.success(`Imported ${successCount} triggers successfully${failCount > 0 ? `. ${failCount} failed.` : ''}`)
                                loadTriggers()
                        } catch (error) {
                                console.error('Import error:', error)
                                toast.error('Failed to parse import file')
                        } finally {
                                setLoading(false)
                                if (e.target) e.target.value = ''
                        }
                }
                reader.readAsText(file)
        }

        const filteredTriggers = triggers.filter(t =>
                t.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
                t.pattern.toLowerCase().includes(searchQuery.toLowerCase())
        )

        return (
                <ScrollArea className='h-full w-full bg-background'>
                        <div className='flex flex-col min-h-full w-full pb-20 md:pb-0'>
                                <TriggerDeleteModal
                                        open={!!deleteId}
                                        onOpenChange={(open) => !open && setDeleteId(null)}
                                        onConfirm={handleDelete}
                                />

                                <TriggerDeleteAllModal open={isDeleteAllOpen} onOpenChange={setIsDeleteAllOpen} onConfirm={handleDeleteAll} />

                                <div className='p-4 md:p-8 border-b border-border/40 bg-muted/5'>
                                        <div className='flex flex-col md:flex-row md:items-center justify-between gap-6'>
                                                <div className='flex items-center gap-4'>
                                                        <div className='p-3 rounded-2xl bg-primary/10 text-primary shadow-sm'>
                                                                <Bot className='h-6 w-6' />
                                                        </div>
                                                        <div>
                                                                <h1 className='text-2xl md:text-3xl font-bold tracking-tight text-foreground'>Bot Management</h1>
                                                                <p className='text-xs md:text-sm text-muted-foreground'>Configure automated regex triggers and Lua scripts.</p>
                                                        </div>
                                                </div>
                                                <div className='flex flex-col sm:flex-row items-stretch sm:items-center gap-3'>
                                                        <div className='relative flex-1 sm:w-64'>
                                                                <Search className='absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground' />
                                                                <Input
                                                                        placeholder='Search triggers...'
                                                                        value={searchQuery}
                                                                        onChange={e => setSearchQuery(e.target.value)}
                                                                        className='pl-9 rounded-xl border-border/40 bg-background focus-visible:ring-primary/20 h-10'
                                                                />
                                                        </div>

                                                        <div className='flex items-center gap-2 justify-end'>
                                                                <div className='flex items-center p-1 bg-muted/50 rounded-xl border border-border/40'>
                                                                        <Button
                                                                                variant='ghost'
                                                                                size='icon'
                                                                                onClick={() => setIsDeleteAllOpen(true)}
                                                                                className='h-8 w-8 rounded-lg text-destructive hover:text-destructive hover:bg-destructive/10'
                                                                                title='Delete ALL Triggers'
                                                                                disabled={triggers.length === 0}
                                                                        >
                                                                                <Trash2 className='h-4 w-4' />
                                                                        </Button>
                                                                        <Button
                                                                                variant='ghost'
                                                                                size='icon'
                                                                                onClick={handleExport}
                                                                                className='h-8 w-8 rounded-lg'
                                                                                title='Export Triggers'
                                                                        >
                                                                                <Download className='h-4 w-4' />
                                                                        </Button>
                                                                        <Button
                                                                                variant='ghost'
                                                                                size='icon'
                                                                                onClick={handleImportClick}
                                                                                className='h-8 w-8 rounded-lg'
                                                                                title='Import Triggers'
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
                                                                                onClick={() => onEditTrigger(null)}
                                                                                className='rounded-xl px-6 h-10 font-bold shadow-lg shadow-primary/20'
                                                                        >
                                                                                <Plus className='mr-2 h-4 w-4' /> New Trigger
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
                                                        <p className='font-medium'>Loading triggers...</p>
                                                </div>
                                        ) : filteredTriggers.length === 0 ? (
                                                <div className='flex flex-col items-center justify-center py-20 md:py-32 border-2 border-dashed border-border/40 rounded-[2rem] md:rounded-[3rem] text-center space-y-6 bg-muted/5'>
                                                        <div className='w-20 h-20 rounded-3xl bg-muted flex items-center justify-center text-muted-foreground'>
                                                                <Zap className='h-10 w-10' />
                                                        </div>
                                                        <div className='space-y-2 px-4'>
                                                                <h3 className='text-xl font-bold'>No triggers found</h3>
                                                                <p className='text-sm text-muted-foreground max-w-[320px] mx-auto'>Create your first dynamic trigger to start automating your bot.</p>
                                                        </div>
                                                        <Button
                                                                onClick={() => onEditTrigger(null)}
                                                                className='rounded-xl px-8 shadow-lg shadow-primary/10'
                                                        >
                                                                <Plus className='mr-2 h-4 w-4' /> Create Trigger
                                                        </Button>
                                                </div>
                                        ) : (
                                                <div className='grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4 md:gap-6'>
                                                        {filteredTriggers.map((trigger) => (
                                                                <Card 
                                                                        key={trigger.id} 
                                                                        onClick={() => onEditTrigger(trigger)}
                                                                        className='rounded-[1.5rem] md:rounded-[2rem] border-border/40 hover:shadow-xl hover:shadow-primary/5 transition-all duration-300 flex flex-col bg-card/50 backdrop-blur-sm overflow-hidden cursor-pointer hover:bg-muted/30 group'
                                                                >
                                                                        <CardHeader className='p-5 md:p-6 pb-2 md:pb-4'>
                                                                                <div className='flex items-center justify-between mb-3'>
                                                                                        <Badge
                                                                                                variant='outline'
                                                                                                className={cn(
                                                                                                        'rounded-full px-3 py-0.5 border-none text-[10px] font-bold uppercase tracking-wider',
                                                                                                        trigger.is_active ? 'bg-green-500/10 text-green-600 dark:text-green-400' : 'bg-muted text-muted-foreground'
                                                                                                )}
                                                                                        >
                                                                                                {trigger.is_active ? 'Active' : 'Disabled'}
                                                                                        </Badge>
                                                                                        <Button
                                                                                                variant='ghost'
                                                                                                size='icon'
                                                                                                className='h-8 w-8 rounded-full text-destructive hover:text-destructive hover:bg-destructive/10 transition-colors'
                                                                                                onClick={(e) => {
                                                                                                        e.stopPropagation();
                                                                                                        setDeleteId(trigger.id);
                                                                                                }}
                                                                                        >
                                                                                                <Trash2 className='h-3.5 w-3.5' />
                                                                                        </Button>
                                                                                </div>
                                                                                <CardTitle className='text-lg md:text-xl truncate group-hover:text-primary transition-colors'>{trigger.name}</CardTitle>
                                                                                <div className='mt-2 font-mono text-[9px] md:text-[10px] truncate bg-muted/80 p-1.5 rounded-lg border border-border/40'>
                                                                                        <span className='text-primary mr-1'>regex:</span> {trigger.pattern}
                                                                                </div>
                                                                        </CardHeader>
                                                                        <CardContent className='p-5 md:p-6 pt-2 md:pt-2 pb-5 md:pb-6 flex-1'>
                                                                                <div className='p-3 md:p-4 rounded-xl md:rounded-2xl bg-muted/40 border border-border/50 h-full'>
                                                                                        <div className='flex items-center gap-2 mb-2'>
                                                                                                <Code className='h-3 w-3 text-primary' />
                                                                                                <span className='text-[9px] md:text-[10px] font-bold uppercase tracking-wider text-muted-foreground'>Lua Script Preview</span>
                                                                                        </div>
                                                                                        <p className='text-[10px] md:text-[11px] font-mono text-foreground/70 line-clamp-3 md:line-clamp-4 whitespace-pre-wrap italic leading-relaxed'>
                                                                                                {trigger.script}
                                                                                        </p>
                                                                                </div>
                                                                        </CardContent>
                                                                        <CardFooter className='border-t border-border/40 p-4 md:p-5 pt-3 md:pt-4 bg-muted/10'>
                                                                                <div className='flex items-center justify-between w-full'>
                                                                                        <Badge className={cn(
                                                                                                'rounded-full px-2.5 py-0.5 font-bold gap-1 text-[9px] md:text-[10px] shadow-sm',
                                                                                                trigger.priority > 0 ? 'bg-primary' : trigger.priority < 0 ? 'bg-orange-500' : 'bg-muted text-muted-foreground border-none'
                                                                                        )}>
                                                                                                <Zap className={cn('h-2.5 w-2.5', trigger.priority === 0 ? 'text-muted-foreground' : 'fill-current')} />
                                                                                                PRIORITY {trigger.priority}
                                                                                        </Badge>
                                                                                        <div onClick={(e) => e.stopPropagation()}>
                                                                                                <Switch
                                                                                                        checked={trigger.is_active}
                                                                                                        onCheckedChange={() => toggleStatus(trigger)}
                                                                                                        className='data-[state=checked]:bg-green-500'
                                                                                                />
                                                                                        </div>
                                                                                </div>
                                                                        </CardFooter>
                                                                </Card>
                                                        ))}
                                                </div>
                                        )}
                                </div>
                        </div>

                        {/* Floating Action Button for Mobile */}
                        {isMobileView && (
                                <Button
                                        onClick={() => onEditTrigger(null)}
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
