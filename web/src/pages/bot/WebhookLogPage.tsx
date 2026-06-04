import { useState, useEffect } from 'react'
import { ArrowLeft, Trash2, ChevronDown, ChevronRight, Globe, Filter } from 'lucide-react'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { api, type WebhookLog, type Webhook } from '@/lib/api'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { useNavigate } from "react-router-dom"
import { useIsMobile } from "@/hooks/use-mobile"

const methodColors: Record<string, string> = {
    GET: 'bg-blue-500/10 text-blue-500 border-blue-500/20',
    POST: 'bg-green-500/10 text-green-500 border-green-500/20',
    PUT: 'bg-yellow-500/10 text-yellow-500 border-yellow-500/20',
    PATCH: 'bg-orange-500/10 text-orange-500 border-orange-500/20',
    DELETE: 'bg-red-500/10 text-red-500 border-red-500/20',
}

function statusColor(code: number): string {
    if (code >= 200 && code < 300) return 'bg-green-500/10 text-green-500 border-green-500/20'
    if (code >= 400 && code < 500) return 'bg-yellow-500/10 text-yellow-500 border-yellow-500/20'
    if (code >= 500) return 'bg-red-500/10 text-red-500 border-red-500/20'
    return 'bg-muted text-muted-foreground'
}

function formatTimestamp(unix: number): string {
    return new Date(unix * 1000).toLocaleString()
}

function formatBody(body: string): string {
    try {
        return JSON.stringify(JSON.parse(body), null, 2)
    } catch {
        return body
    }
}

export function WebhookLogPage() {
    const navigate = useNavigate()
    const isMobileView = useIsMobile()
    const [logs, setLogs] = useState<WebhookLog[]>([])
    const [loading, setLoading] = useState(true)
    const [total, setTotal] = useState(0)
    const [offset, setOffset] = useState(0)
    const [expandedId, setExpandedId] = useState<string | null>(null)
    const [webhooks, setWebhooks] = useState<Webhook[]>([])
    const [filterWebhookId, setFilterWebhookId] = useState<string>('')
    const [showFilter, setShowFilter] = useState(false)
    const limit = 50

    useEffect(() => {
        api.getWebhooks().then(data => setWebhooks(data || [])).catch(() => {})
    }, [])

    const loadLogs = async (newOffset = 0, append = false) => {
        try {
            const result = await api.getWebhookLogs({
                webhook_id: filterWebhookId || undefined,
                limit,
                offset: newOffset,
            })
            setLogs(append ? [...logs, ...result.logs] : result.logs)
            setTotal(result.total)
            setOffset(newOffset)
        } catch (error) {
            console.error('Failed to load logs:', error)
            toast.error('Failed to load logs')
        } finally {
            setLoading(false)
        }
    }

    useEffect(() => {
        setLoading(true)
        loadLogs()
    }, [filterWebhookId])

    const handleClearAll = async () => {
        try {
            await api.deleteAllWebhookLogs()
            setLogs([])
            setTotal(0)
            setOffset(0)
            toast.success('All logs cleared')
        } catch {
            toast.error('Failed to clear logs')
        }
    }

    const hasMore = logs.length < total

    return (
        <div className="flex flex-col h-full">
            <div className="border-b border-border/40 bg-background/80 backdrop-blur-sm">
                <div className="p-4 md:px-8 md:py-4">
                    <div className="flex items-center justify-between gap-4">
                        <div className="flex items-center gap-3">
                            <Button variant="ghost" size="icon" onClick={() => navigate("/webhooks")} className="h-9 w-9 rounded-xl">
                                <ArrowLeft className="h-5 w-5" />
                            </Button>
                            <div>
                                <h1 className="text-lg font-bold flex items-center gap-2">
                                    <Globe className="h-5 w-5 text-primary" />
                                    Webhook Request Logs
                                </h1>
                                {!isMobileView && (
                                    <p className="text-xs text-muted-foreground">View incoming webhook request history</p>
                                )}
                            </div>
                        </div>

                        <div className="flex items-center gap-2">
                            <div className="relative">
                                <Button
                                    variant="outline"
                                    size={isMobileView ? "icon" : "default"}
                                    onClick={() => setShowFilter(!showFilter)}
                                    className={cn("rounded-xl border-border/40", filterWebhookId && "border-primary/40 bg-primary/5")}
                                >
                                    <Filter className={cn("h-4 w-4", !isMobileView && "mr-2")} />
                                    {!isMobileView && "Filter"}
                                </Button>
                                {showFilter && (
                                    <div className="absolute right-0 top-full mt-2 z-50 bg-background border border-border/40 rounded-xl shadow-lg p-2 min-w-[200px]">
                                        <button
                                            className={cn("w-full text-left px-3 py-2 rounded-lg text-sm hover:bg-muted/50", !filterWebhookId && "bg-muted/50 font-medium")}
                                            onClick={() => { setFilterWebhookId(''); setShowFilter(false) }}
                                        >
                                            All Webhooks
                                        </button>
                                        {webhooks.map(w => (
                                            <button
                                                key={w.id}
                                                className={cn("w-full text-left px-3 py-2 rounded-lg text-sm hover:bg-muted/50 truncate", filterWebhookId === w.id && "bg-muted/50 font-medium")}
                                                onClick={() => { setFilterWebhookId(w.id); setShowFilter(false) }}
                                            >
                                                {w.name || w.path}
                                            </button>
                                        ))}
                                    </div>
                                )}
                            </div>
                            <Button
                                variant="outline"
                                size={isMobileView ? "icon" : "default"}
                                onClick={handleClearAll}
                                disabled={logs.length === 0}
                                className="rounded-xl border-border/40 text-destructive hover:text-destructive hover:bg-destructive/10"
                            >
                                <Trash2 className={cn("h-4 w-4", !isMobileView && "mr-2")} />
                                {!isMobileView && "Clear All"}
                            </Button>
                        </div>
                    </div>

                    {filterWebhookId && (
                        <div className="mt-2 flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">Filtered by:</span>
                            <Badge variant="outline" className="text-xs">
                                {webhooks.find(w => w.id === filterWebhookId)?.name || filterWebhookId}
                                <button className="ml-1 hover:text-destructive" onClick={() => setFilterWebhookId('')}>x</button>
                            </Badge>
                        </div>
                    )}
                </div>
            </div>

            <div className="flex-1 overflow-y-auto">
                <div className="p-4 md:p-8 space-y-3">
                    {loading ? (
                        <div className="flex flex-col items-center justify-center py-24 gap-4 opacity-50">
                            <div className="w-10 h-10 border-4 border-primary/30 border-t-primary rounded-full animate-spin" />
                            <p className="font-medium">Loading logs...</p>
                        </div>
                    ) : logs.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-20 md:py-32 border-2 border-dashed border-border/40 rounded-[2rem] md:rounded-[3rem] text-center space-y-6 bg-muted/5">
                            <div className="w-20 h-20 rounded-3xl bg-muted flex items-center justify-center text-muted-foreground">
                                <Globe className="h-10 w-10" />
                            </div>
                            <div className="space-y-2 px-4">
                                <h3 className="text-xl font-bold">No logs yet</h3>
                                <p className="text-muted-foreground max-w-xs mx-auto">Webhook requests will appear here as they come in.</p>
                            </div>
                        </div>
                    ) : (
                        <>
                            <p className="text-xs text-muted-foreground">{total} log{total !== 1 ? 's' : ''} found</p>
                            {logs.map(log => (
                                <Card key={log.id} className="border-border/40 overflow-hidden">
                                    <CardContent className="p-0">
                                        <button
                                            className="w-full p-3 md:p-4 flex items-center gap-3 text-left hover:bg-muted/30 transition-colors"
                                            onClick={() => setExpandedId(expandedId === log.id ? null : log.id)}
                                        >
                                            {expandedId === log.id ? (
                                                <ChevronDown className="h-4 w-4 text-muted-foreground shrink-0" />
                                            ) : (
                                                <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0" />
                                            )}

                                            <Badge variant="outline" className={cn("text-[10px] font-mono font-bold px-2 py-0.5", methodColors[log.method] || 'bg-muted text-muted-foreground')}>
                                                {log.method}
                                            </Badge>

                                            <span className="font-mono text-sm text-foreground/80 truncate">/{log.webhook_path}</span>

                                            <span className="text-xs text-muted-foreground shrink-0 hidden sm:inline">{log.source_ip}</span>

                                            {log.status_code > 0 && (
                                                <Badge variant="outline" className={cn("text-[10px] font-mono px-2 py-0.5 ml-auto shrink-0", statusColor(log.status_code))}>
                                                    {log.status_code}
                                                </Badge>
                                            )}

                                            <span className="text-[10px] text-muted-foreground shrink-0 ml-2 hidden md:inline">{formatTimestamp(log.created_at)}</span>
                                        </button>

                                        {expandedId === log.id && (
                                            <div className="px-3 md:px-4 pb-3 md:pb-4 space-y-3 border-t border-border/30 pt-3">
                                                <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
                                                    <div>
                                                        <span className="text-muted-foreground">Source</span>
                                                        <p className="font-mono mt-0.5">{log.source_ip}</p>
                                                    </div>
                                                    <div>
                                                        <span className="text-muted-foreground">Path</span>
                                                        <p className="font-mono mt-0.5">/{log.webhook_path}</p>
                                                    </div>
                                                    <div>
                                                        <span className="text-muted-foreground">Status</span>
                                                        <p className="font-mono mt-0.5">{log.status_code || 'N/A'}</p>
                                                    </div>
                                                    <div>
                                                        <span className="text-muted-foreground">Time</span>
                                                        <p className="font-mono mt-0.5">{formatTimestamp(log.created_at)}</p>
                                                    </div>
                                                </div>

                                                {log.query_params && (
                                                    <div>
                                                        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">Query Params</span>
                                                        <div className="mt-1 p-2 rounded-lg bg-muted/50 text-xs font-mono">{log.query_params}</div>
                                                    </div>
                                                )}

                                                {log.headers && (
                                                    <div>
                                                        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">Headers</span>
                                                        <div className="mt-1 p-3 rounded-xl bg-black text-green-400 font-mono text-[10px] md:text-[11px] whitespace-pre-wrap leading-relaxed border border-white/5 shadow-inner">
                                                            {formatBody(log.headers)}
                                                        </div>
                                                    </div>
                                                )}

                                                <div>
                                                    <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">Body</span>
                                                    <div className="mt-1 p-3 rounded-xl bg-black text-green-400 font-mono text-[10px] md:text-[11px] min-h-[60px] whitespace-pre-wrap leading-relaxed border border-white/5 shadow-inner">
                                                        {log.body ? formatBody(log.body) : '> No body'}
                                                    </div>
                                                </div>
                                            </div>
                                        )}
                                    </CardContent>
                                </Card>
                            ))}

                            {hasMore && (
                                <div className="flex justify-center py-4">
                                    <Button
                                        variant="outline"
                                        onClick={() => loadLogs(offset + limit, true)}
                                        className="rounded-xl border-border/40"
                                    >
                                        Load More
                                    </Button>
                                </div>
                            )}
                        </>
                    )}
                </div>
            </div>
        </div>
    )
}
