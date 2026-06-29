import React, { useState, useRef, useEffect } from 'react'
import { Bot, Send, X, Loader2, Sparkles, MessageSquare } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
	Sheet,
	SheetContent,
	SheetHeader,
	SheetTitle,
} from '@/components/ui/sheet'
import { Textarea } from '@/components/ui/textarea'
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@/components/ui/select'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism'
import { cn } from '@/lib/utils'
import { api } from '@/lib/api'

const AI_MODELS = [
	{ value: 'gemma-3-27b-it', label: 'Gemma 3 27B' },
	{ value: 'gemma-4-31b-it', label: 'Gemma 4 31B' },
	{ value: 'gemma-4-26b-a4b-it', label: 'Gemma 4 26B A4B' },
	{ value: 'gemini-3.1-flash-lite-preview', label: 'Gemini 3.1 Flash Lite' },
] as const

interface Message {
	role: 'user' | 'assistant'
	content: string
}

interface AIAssistantProps {
	currentCode?: string
	onApplyCode?: (code: string) => void
}

export function AIAssistant({ currentCode, onApplyCode }: AIAssistantProps) {
	const [isOpen, setIsOpen] = useState(false)
	const [prompt, setPrompt] = useState('')
	const [messages, setMessages] = useState<Message[]>([])
	const [isLoading, setIsLoading] = useState(false)
	const [selectedModel, setSelectedModel] = useState('gemma-3-27b-it')
	const scrollRef = useRef<HTMLDivElement>(null)

	useEffect(() => {
		if (scrollRef.current) {
			scrollRef.current.scrollTop = scrollRef.current.scrollHeight
		}
	}, [messages])

	const handleSubmit = async (e?: React.FormEvent) => {
		e?.preventDefault()
		if (!prompt.trim() || isLoading) return

		const userMessage = prompt.trim()
		setPrompt('')
		setMessages(prev => [...prev, { role: 'user', content: userMessage }])
		setIsLoading(true)

		try {
			const data = await api.chatAssistant(userMessage, currentCode, selectedModel)
			setMessages(prev => [...prev, { role: 'assistant', content: data.answer }])

			// Auto-apply if code block found
			const codeMatch = data.answer.match(/```lua\n([\s\S]*?)```/)
			if (codeMatch && codeMatch[1]) {
				onApplyCode?.(codeMatch[1].trim())
			}
		} catch (error) {
			console.error('AI Error:', error)
			setMessages(prev => [...prev, { role: 'assistant', content: 'Sorry, an error occurred while connecting to the AI assistant.' }])
		} finally {
			setIsLoading(false)
		}
	}

	return (
		<>
			{/* Floating Button */}
			<div className='absolute bottom-6 right-6 z-40 group'>
				<div className='absolute inset-0 bg-primary/20 rounded-full blur-xl group-hover:bg-primary/40 transition-all duration-500 animate-pulse' />
				<Button
					onClick={() => setIsOpen(true)}
					size='icon'
					className='h-12 w-12 rounded-full shadow-2xl hover:scale-110 active:scale-90 transition-all duration-300 bg-primary text-primary-foreground relative border border-primary/20 overflow-hidden group'
				>
					<div className='absolute inset-0 bg-gradient-to-tr from-white/20 to-transparent opacity-0 group-hover:opacity-100 transition-opacity' />
					<Sparkles className='h-6 w-6 group-hover:rotate-12 transition-transform duration-300' />
				</Button>
			</div>

			<Sheet open={isOpen} onOpenChange={setIsOpen} modal={false}>
				<SheetContent
					side='right'
					hideOverlay={true}
					showCloseButton={false}
					className='w-[90vw] sm:w-[600px] sm:max-w-none p-0 flex flex-col h-full border-l border-border/40 shadow-2xl z-50 bg-background/95 backdrop-blur-md transition-all duration-300 overflow-hidden'
				>
					<SheetHeader className='p-4 border-b border-border/40 shrink-0 flex flex-row items-center justify-between space-y-0'>
						<div className='flex items-center gap-2'>
							<div className='p-2 bg-primary/10 rounded-lg'>
								<Bot className='h-5 w-5 text-primary' />
							</div>
							<SheetTitle className='text-base font-bold'>AI Assistant</SheetTitle>
						</div>
						<Button variant='ghost' size='icon' onClick={() => setIsOpen(false)} className='h-8 w-8 rounded-full'>
							<X className='h-4 w-4' />
						</Button>
					</SheetHeader>

					<div className='flex-1 min-h-0 flex flex-col w-full overflow-hidden'>
						<div
							ref={scrollRef}
							className='flex-1 overflow-y-auto overflow-x-hidden p-4 space-y-4 pb-12 w-full custom-scrollbar'
						>
							{messages.map((msg, i) => (
								<div
									key={i}
									className={cn(
										'flex flex-col w-full max-w-full min-w-0',
										msg.role === 'user' ? 'items-end' : 'items-start'
									)}
								>
									<div className={cn(
										'text-[10px] uppercase tracking-wider font-bold mb-1 px-1 opacity-50 flex items-center gap-1',
										msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'
									)}>
										{msg.role === 'user' ? <MessageSquare className='h-2.5 w-2.5' /> : <Bot className='h-2.5 w-2.5' />}
										{msg.role === 'user' ? 'You' : 'AI Assistant'}
									</div>
									<div
										className={cn(
											'p-3 rounded-2xl text-sm leading-relaxed border shadow-sm w-fit max-w-[85%] min-w-0 flex flex-col overflow-hidden',
											msg.role === 'user'
												? 'bg-primary text-primary-foreground border-primary rounded-tr-none'
												: 'bg-muted border-border/40 rounded-tl-none'
										)}
									>
										<div className="w-full min-w-0 overflow-x-auto scrollbar-none">
											<ReactMarkdown
												remarkPlugins={[remarkGfm]}
												components={{
													p: ({ children }) => <p className="mb-2 last:mb-0 leading-relaxed break-words whitespace-pre-wrap">{children}</p>,
													ul: ({ children }) => <ul className="list-disc pl-4 mb-2 space-y-1">{children}</ul>,
													ol: ({ children }) => <ol className="list-decimal pl-4 mb-2 space-y-1">{children}</ol>,
													li: ({ children }) => <li className="text-sm break-words">{children}</li>,
													code({ node, inline, className, children, ...props }: any) {
														const match = /language-(\w+)/.exec(className || '')
														const codeStr = String(children).replace(/\n$/, '')
														return !inline && match ? (
															<div className='rounded-lg my-2 border border-border/40 bg-black/50 group/code w-full min-w-0 flex flex-col overflow-hidden'>
																<div className='flex items-center justify-between px-3 py-1.5 bg-muted/50 border-b border-border/40 shrink-0'>
																	<span className='text-[10px] font-bold uppercase text-muted-foreground'>{match[1]}</span>
																	<Button
																		variant='ghost'
																		size='sm'
																		className='h-6 px-2 text-[10px] font-bold gap-1.5 hover:bg-primary/20 hover:text-primary rounded-md transition-all active:scale-95'
																		onClick={() => onApplyCode?.(codeStr)}
																	>
																		<Sparkles className='h-3 w-3' />
																		Apply Changes
																	</Button>
																</div>
																<div className="w-full overflow-x-auto scrollbar-thin scrollbar-thumb-primary/20">
																	<SyntaxHighlighter
																		style={vscDarkPlus}
																		language={match[1]}
																		PreTag="div"
																		customStyle={{
																			margin: 0,
																			padding: '1rem',
																			fontSize: '11px',
																			background: 'transparent',
																			width: 'max-content',
																			minWidth: '100%'
																		}}
																		{...props}
																	>
																		{codeStr}
																	</SyntaxHighlighter>
																</div>
															</div>
														) : (
															<code className={cn("bg-black/20 px-1 py-0.5 rounded font-mono text-[0.9em] break-all whitespace-pre-wrap", className)} {...props}>
																{children}
															</code>
														)
													}
												}}
											>
												{msg.content}
											</ReactMarkdown>
										</div>
									</div>
								</div>
							))}
							{isLoading && (
								<div className='flex flex-col items-start max-w-[90%] mr-auto'>
									<div className='text-[10px] uppercase tracking-wider font-bold mb-1 px-1 opacity-50 flex items-center gap-1'>
										<Bot className='h-2.5 w-2.5' />
										AI Assistant
									</div>
									<div className='bg-muted border border-border/40 p-3 rounded-2xl rounded-tl-none flex items-center gap-2'>
										<Loader2 className='h-4 w-4 animate-spin text-primary' />
										<span className='text-xs italic'>Thinking...</span>
									</div>
								</div>
							)}
						</div>

						<div className='p-4 border-t border-border/40 shrink-0 bg-background/50'>
							<form
								onSubmit={handleSubmit}
								className='relative flex items-end gap-2 bg-muted/30 border border-border/40 p-1 rounded-xl focus-within:ring-1 focus-within:ring-primary/30 transition-all'
							>
								<Select value={selectedModel} onValueChange={setSelectedModel}>
									<SelectTrigger className='h-9 w-auto min-w-[120px] max-w-[180px] shrink-0 border-0 bg-background/50 rounded-lg text-[11px] font-semibold ml-0.5 mb-0.5 focus:ring-0'>
										<SelectValue />
									</SelectTrigger>
									<SelectContent>
										{AI_MODELS.map(m => (
											<SelectItem key={m.value} value={m.value} className='text-xs'>
												{m.label}
											</SelectItem>
										))}
									</SelectContent>
								</Select>
								<Textarea
									placeholder='Ask something...'
									className='min-h-[44px] max-h-32 flex-1 resize-none border-0 focus-visible:ring-0 focus-visible:ring-offset-0 bg-transparent py-3 px-3 scrollbar-none text-sm'
									value={prompt}
									onChange={(e) => setPrompt(e.target.value)}
									onKeyDown={(e) => {
										if (e.key === 'Enter' && !e.shiftKey) {
											e.preventDefault()
											handleSubmit()
										}
									}}
								/>
								<Button
									type='submit'
									size='icon'
									className='h-9 w-9 shrink-0 rounded-lg mb-0.5 mr-0.5 shadow-sm active:scale-95 transition-transform'
									disabled={!prompt.trim() || isLoading}
								>
									{isLoading ? <Loader2 className='h-4 w-4 animate-spin' /> : <Send className='h-4 w-4' />}
								</Button>
							</form>
							<div className='mt-2 text-[10px] text-center text-muted-foreground italic'>
								Press Enter to send, Shift+Enter for new line
							</div>
						</div>
					</div>
				</SheetContent>
			</Sheet>
		</>
	)
}
