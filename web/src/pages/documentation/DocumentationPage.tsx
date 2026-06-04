import { useState, useEffect } from 'react'
import { FileText, ChevronRight, MessageSquare, Bot, Database, Zap, Code, Shield, Menu, X } from 'lucide-react'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism'
import { cn } from '@/lib/utils'

export function DocumentationPage() {
	const [content, setContent] = useState<string>('Loading documentation...')
	const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false)

	useEffect(() => {
		fetch('/docs/api-reference.md')
			.then(res => res.text())
			.then(text => setContent(text))
			.catch(err => {
				console.error('Failed to load documentation:', err)
				setContent('# Error\nFailed to load documentation file.')
			})
	}, [])

	const scrollToSection = (id: string) => {
		const element = document.getElementById(id)
		if (element) {
			element.scrollIntoView({ behavior: 'smooth', block: 'start' })
			setIsMobileMenuOpen(false)
		}
	}

	const tocItems = [
		{ id: 'global-variables', label: 'Global Variables', icon: <Zap className='h-3.5 w-3.5' /> },
		{ id: 'messaging-functions', label: 'Messaging Functions', icon: <MessageSquare className='h-3.5 w-3.5' /> },
		{ id: 'utility-functions', label: 'Utility Functions', icon: <Code className='h-3.5 w-3.5' /> },
		{ id: 'state-management', label: 'State Management', icon: <Database className='h-3.5 w-3.5' /> },
		{ id: 'storage-and-system', label: 'Storage and System', icon: <Shield className='h-3.5 w-3.5' /> },
		{ id: 'example-weather-bot', label: 'Example Weather Bot', icon: <Bot className='h-3.5 w-3.5' /> },
	]

	return (
		<div className='flex h-full w-full bg-background overflow-hidden relative'>
			{isMobileMenuOpen && (
				<div
					className='fixed inset-0 bg-background/80 backdrop-blur-sm z-40 lg:hidden'
					onClick={() => setIsMobileMenuOpen(false)}
				/>
			)}

			<div className={cn(
				'fixed inset-y-0 left-0 z-50 w-64 border-r border-border/40 flex flex-col bg-background lg:bg-muted/10 shrink-0 h-full transition-transform duration-300 lg:translate-x-0 lg:static',
				isMobileMenuOpen ? 'translate-x-0' : '-translate-x-full'
			)}>
				<div className='p-6 border-b border-border/40 flex items-center justify-between'>
					<h2 className='text-sm font-bold flex items-center gap-2'>
						<FileText className='h-4 w-4 text-primary' />
						API Reference
					</h2>
					<Button variant='ghost' size='icon' className='lg:hidden -mr-2' onClick={() => setIsMobileMenuOpen(false)}>
						<X className='h-4 w-4' />
					</Button>
				</div>
				<ScrollArea className='flex-1'>
					<div className='p-4 space-y-1'>
						{tocItems.map(item => (
							<DocLink
								key={item.id}
								icon={item.icon}
								label={item.label}
								onClick={() => scrollToSection(item.id)}
							/>
						))}
					</div>
				</ScrollArea>
			</div>

			<div className='flex-1 flex flex-col min-w-0 bg-background h-full relative overflow-hidden'>
				<div className='lg:hidden p-4 border-b border-border/40 flex items-center bg-background/80 backdrop-blur-md sticky top-0 z-30 shrink-0'>
					<Button variant='ghost' size='icon' onClick={() => setIsMobileMenuOpen(true)}>
						<Menu className='h-5 w-5' />
					</Button>
					<span className='ml-3 text-sm font-bold'>Documentation</span>
				</div>

				<div className='flex-1 w-full overflow-y-auto'>
					<div className='max-w-4xl mx-auto p-6 md:p-12 pb-32'>
						<div className='prose prose-slate dark:prose-invert max-w-none
							prose-headings:scroll-mt-20 prose-headings:font-semibold prose-headings:tracking-tight      
							prose-h1:text-3xl md:prose-h1:text-4xl prose-h1:pb-4 prose-h1:border-b prose-h1:border-border/40 prose-h1:mb-8   
							prose-h2:text-xl md:prose-h2:text-2xl prose-h2:mt-12 prose-h2:pb-2 prose-h2:border-b prose-h2:border-border/40  
							prose-h3:text-lg md:prose-h3:text-xl prose-h3:mt-8
							prose-a:text-primary prose-a:no-underline hover:prose-a:underline
							prose-blockquote:border-l-4 prose-blockquote:border-border prose-blockquote:italic
							prose-table:block prose-table:overflow-x-auto prose-table:border prose-table:border-border/40
							prose-th:bg-muted/50 prose-th:p-2 prose-th:border prose-th:border-border/40
							prose-td:p-2 prose-td:border prose-td:border-border/40
							prose-img:rounded-2xl prose-hr:border-border/40'>

							<ReactMarkdown
								remarkPlugins={[remarkGfm]}
								components={{
									h2({ children }: any) {
										const id = String(children)
											.toLowerCase()
											.replace(/[^\w\s-]/g, '')
											.trim()
											.replace(/\s+/g, '-')
										return <h2 id={id}>{children}</h2>
									},
									code({ node, inline, className, children, ...props }: any) {
										const match = /language-(\w+)/.exec(className || '')
										return !inline && match ? (
											<div className='rounded-xl overflow-hidden my-6 border border-border/40 shadow-sm'>
												<SyntaxHighlighter
													style={vscDarkPlus}
													language={match[1]}
													PreTag="div"
													customStyle={{ margin: 0, padding: '1.5rem', fontSize: '13px' }}
													{...props}
												>
													{String(children).replace(/\n$/, '')}
												</SyntaxHighlighter>
											</div>
										) : (
											<code className={cn("bg-muted px-1.5 py-0.5 rounded-md font-mono text-[0.9em]", className)} {...props}>
												{children}
											</code>
										)
									}
								}}
							>
								{content}
							</ReactMarkdown>
						</div>
					</div>
				</div>
			</div>
		</div>
	)
}

function DocLink({ icon, label, onClick }: { icon: React.ReactNode, label: string, onClick?: () => void }) {
	return (
		<button
			onClick={(e) => {
				e.preventDefault()
				onClick?.()
			}}
			className='w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors group'
		>
			<div className='flex items-center gap-2'>
				{icon}
				{label}
			</div>
			<ChevronRight className='h-3 w-3 opacity-0 group-hover:opacity-100 transition-opacity' />
		</button>
	)
}
