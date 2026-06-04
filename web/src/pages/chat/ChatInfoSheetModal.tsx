import { useState, useEffect, useCallback, useRef } from "react"
import type { Chat, Message } from "@/lib/api"
import { api } from "@/lib/api"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { FileText, Image as ImageIcon, Link as LinkIcon, Video, File, Loader2 } from "lucide-react"

interface ChatInfoSheetModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	chat: Chat
	getAvatarUrl: (target: Chat) => string | undefined
	getMediaUrl: (url: string | undefined) => string | undefined
	formatDate: (timestamp: number) => string
	onSelectImage: (url: string | null) => void
}

export function ChatInfoSheetModal({
	open,
	onOpenChange,
	chat,
	getAvatarUrl,
	getMediaUrl,
	formatDate,
	onSelectImage,
}: ChatInfoSheetModalProps) {
	const [media, setMedia] = useState<Message[]>([])
	const [docs, setDocs] = useState<Message[]>([])
	const [links, setLinks] = useState<Message[]>([])

	const [hasMoreMedia, setHasMoreMedia] = useState(true)
	const [hasMoreDocs, setHasMoreDocs] = useState(true)
	const [hasMoreLinks, setHasMoreLinks] = useState(true)

	const [loadingMedia, setLoadingMedia] = useState(false)
	const [loadingDocs, setLoadingDocs] = useState(false)
	const [loadingLinks, setLoadingLinks] = useState(false)

	const loadingMediaRef = useRef(false)
	const loadingDocsRef = useRef(false)
	const loadingLinksRef = useRef(false)

	const mediaScrollRef = useRef<HTMLDivElement>(null)
	const docsScrollRef = useRef<HTMLDivElement>(null)
	const linksScrollRef = useRef<HTMLDivElement>(null)

	const fetchMedia = useCallback(async (before?: number) => {
		if (!chat?.id || loadingMediaRef.current) return
		loadingMediaRef.current = true
		setLoadingMedia(true)
		try {
			const data = await api.getChatMedia(chat.id, 30, before)
			if (data && data.length > 0) {
				setMedia(prev => before ? [...prev, ...data] : data)
				setHasMoreMedia(data.length === 30)
			} else {
				setHasMoreMedia(false)
			}
		} catch (error) {
			console.error("Failed to fetch media:", error)
		} finally {
			loadingMediaRef.current = false
			setLoadingMedia(false)
		}
	}, [chat?.id])

	const fetchDocs = useCallback(async (before?: number) => {
		if (!chat?.id || loadingDocsRef.current) return
		loadingDocsRef.current = true
		setLoadingDocs(true)
		try {
			const data = await api.getChatDocs(chat.id, 30, before)
			if (data && data.length > 0) {
				setDocs(prev => before ? [...prev, ...data] : data)
				setHasMoreDocs(data.length === 30)
			} else {
				setHasMoreDocs(false)
			}
		} catch (error) {
			console.error("Failed to fetch docs:", error)
		} finally {
			loadingDocsRef.current = false
			setLoadingDocs(false)
		}
	}, [chat?.id])

	const fetchLinks = useCallback(async (before?: number) => {
		if (!chat?.id || loadingLinksRef.current) return
		loadingLinksRef.current = true
		setLoadingLinks(true)
		try {
			const data = await api.getChatLinks(chat.id, 30, before)
			if (data && data.length > 0) {
				setLinks(prev => before ? [...prev, ...data] : data)
				setHasMoreLinks(data.length === 30)
			} else {
				setHasMoreLinks(false)
			}
		} catch (error) {
			console.error("Failed to fetch links:", error)
		} finally {
			loadingLinksRef.current = false
			setLoadingLinks(false)
		}
	}, [chat?.id])

	useEffect(() => {
		if (open && chat?.id) {
			setMedia([])
			setDocs([])
			setLinks([])
			setHasMoreMedia(true)
			setHasMoreDocs(true)
			setHasMoreLinks(true)
			fetchMedia()
			fetchDocs()
			fetchLinks()
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [open, chat?.id])

	const handleScrollMedia = useCallback((e: React.UIEvent<HTMLDivElement>) => {
		const target = e.currentTarget
		const isNearBottom = target.scrollHeight - target.scrollTop - target.clientHeight < 50
		if (isNearBottom && !loadingMediaRef.current && hasMoreMedia && media.length > 0) {
			const oldestItem = media[media.length - 1]
			if (oldestItem) {
				fetchMedia(oldestItem.timestamp)
			}
		}
	}, [hasMoreMedia, media, fetchMedia])

	const handleScrollDocs = useCallback((e: React.UIEvent<HTMLDivElement>) => {
		const target = e.currentTarget
		const isNearBottom = target.scrollHeight - target.scrollTop - target.clientHeight < 50
		if (isNearBottom && !loadingDocsRef.current && hasMoreDocs && docs.length > 0) {
			const oldestItem = docs[docs.length - 1]
			if (oldestItem) {
				fetchDocs(oldestItem.timestamp)
			}
		}
	}, [hasMoreDocs, docs, fetchDocs])

	const handleScrollLinks = useCallback((e: React.UIEvent<HTMLDivElement>) => {
		const target = e.currentTarget
		const isNearBottom = target.scrollHeight - target.scrollTop - target.clientHeight < 50
		if (isNearBottom && !loadingLinksRef.current && hasMoreLinks && links.length > 0) {
			const oldestItem = links[links.length - 1]
			if (oldestItem) {
				fetchLinks(oldestItem.timestamp)
			}
		}
	}, [hasMoreLinks, links, fetchLinks])

	return (
		<Sheet open={open} onOpenChange={onOpenChange}>
			<SheetContent side="right" className="w-full sm:max-w-md p-0 flex flex-col h-full bg-background border-l border-border/40 overflow-hidden">
				<SheetHeader className="p-4 border-b border-border/40 bg-muted/20 shrink-0">
					<SheetTitle className="text-lg font-bold flex items-center gap-2">Chat Info</SheetTitle>
				</SheetHeader>

				<div className="flex items-center justify-center gap-4 py-4 px-4 border-b border-border/40 bg-muted/5 shrink-0">
					<Avatar className="h-14 w-14 border-2 border-background shadow-lg flex-shrink-0">
						<AvatarImage src={getAvatarUrl(chat)} />
						<AvatarFallback className="text-xl bg-primary/10 text-primary font-bold">{chat.name.charAt(0).toUpperCase()}</AvatarFallback>
					</Avatar>
					<div className="flex flex-col min-w-0">
						<h2 className="text-base font-bold leading-tight truncate">{chat.name}</h2>
						<p className="text-[11px] font-medium text-muted-foreground truncate">{chat.id}</p>
					</div>
				</div>

				<div className="flex-1 flex flex-col min-h-0 overflow-hidden">
					<Tabs defaultValue="media" className="flex-1 flex flex-col h-full">
						<div className="px-4 pt-4 shrink-0">
							<TabsList className="w-full grid grid-cols-3 bg-muted/50 p-1">
								<TabsTrigger value="media" className="text-xs">Media</TabsTrigger>
								<TabsTrigger value="docs" className="text-xs">Docs</TabsTrigger>
								<TabsTrigger value="links" className="text-xs">Links</TabsTrigger>
							</TabsList>
						</div>

						<TabsContent value="media" className="flex-1 min-h-0 mt-0 outline-none overflow-hidden">
							<div 
								ref={mediaScrollRef} 
								className="h-full overflow-y-auto p-4"
								onScroll={handleScrollMedia}
							>
								{media.length > 0 ? (
									<div className="grid grid-cols-3 gap-2">
										{media.map(m => (
											<div key={m.id} className="aspect-square rounded-lg overflow-hidden border border-border/40 bg-muted/20 group relative">
												{m.type === "image" ? (
													<img
														src={getMediaUrl(m.mediaUrl)}
														className="w-full h-full object-cover cursor-pointer hover:scale-110 transition-transform duration-500"
														onClick={() => onSelectImage(getMediaUrl(m.mediaUrl) || null)}
													/>
												) : (
													<div
														className="w-full h-full flex items-center justify-center cursor-pointer group-hover:scale-110 transition-transform duration-500"
														onClick={() => window.open(getMediaUrl(m.mediaUrl), "_blank")}
													>
														<Video className="h-8 w-8 text-primary/40" />
													</div>
												)}
											</div>
										))}
									</div>
								) : loadingMedia ? (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground">
										<Loader2 className="h-8 w-8 animate-spin text-primary mb-2" />
										<p className="text-sm">Loading media...</p>
									</div>
								) : (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground opacity-40">
										<ImageIcon className="h-12 w-12 mb-2" />
										<p className="text-sm">No media shared yet</p>
									</div>
								)}
								{loadingMedia && media.length > 0 && (
									<div className="flex justify-center py-4">
										<Loader2 className="h-6 w-6 animate-spin text-primary" />
									</div>
								)}
							</div>
						</TabsContent>

						<TabsContent value="docs" className="flex-1 min-h-0 mt-0 outline-none overflow-hidden">
							<div 
								ref={docsScrollRef} 
								className="h-full overflow-y-auto p-4"
								onScroll={handleScrollDocs}
							>
								{docs.length > 0 ? (
									<div className="space-y-3">
										{docs.map(m => (
											<a
												key={m.id}
												href={getMediaUrl(m.mediaUrl)}
												target="_blank"
												rel="noopener noreferrer"
												className="flex items-center gap-3 p-3 rounded-xl border border-border/40 hover:bg-muted/50 transition-colors group"
											>
												<div className="w-10 h-10 bg-blue-500/10 rounded-lg flex items-center justify-center text-blue-600 group-hover:scale-110 transition-transform">
													<File className="h-5 w-5" />
												</div>
												<div className="flex-1 min-w-0">
													<p className="text-sm font-bold truncate">Document</p>
													<p className="text-[10px] text-muted-foreground">{formatDate(m.timestamp)}</p>
												</div>
											</a>
										))}
									</div>
								) : loadingDocs ? (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground">
										<Loader2 className="h-8 w-8 animate-spin text-primary mb-2" />
										<p className="text-sm">Loading documents...</p>
									</div>
								) : (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground opacity-40">
										<FileText className="h-12 w-12 mb-2" />
										<p className="text-sm">No documents shared yet</p>
									</div>
								)}
								{loadingDocs && docs.length > 0 && (
									<div className="flex justify-center py-4">
										<Loader2 className="h-6 w-6 animate-spin text-primary" />
									</div>
								)}
							</div>
						</TabsContent>

						<TabsContent value="links" className="flex-1 min-h-0 mt-0 outline-none overflow-hidden">
							<div 
								ref={linksScrollRef} 
								className="h-full overflow-y-auto p-4"
								onScroll={handleScrollLinks}
							>
								{links.length > 0 ? (
									<div className="space-y-3">
										{links.map(m => {
											const urls = m.content.match(/(https?:\/\/[^\s]+)/g)
											return urls?.map((url, i) => (
												<a
													key={`${m.id}-${i}`}
													href={url}
													target="_blank"
													rel="noopener noreferrer"
													className="flex items-center gap-3 p-3 rounded-xl border border-border/40 hover:bg-muted/50 transition-colors group"
												>
													<div className="w-10 h-10 bg-green-500/10 rounded-lg flex items-center justify-center text-green-600 group-hover:scale-110 transition-transform">
														<LinkIcon className="h-5 w-5" />
													</div>
													<div className="flex-1 min-w-0">
														<p className="text-sm font-medium text-blue-500 truncate">{url}</p>
														<p className="text-[10px] text-muted-foreground">{formatDate(m.timestamp)}</p>
													</div>
												</a>
											))
										})}
									</div>
								) : loadingLinks ? (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground">
										<Loader2 className="h-8 w-8 animate-spin text-primary mb-2" />
										<p className="text-sm">Loading links...</p>
									</div>
								) : (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground opacity-40">
										<LinkIcon className="h-12 w-12 mb-2" />
										<p className="text-sm">No links shared yet</p>
									</div>
								)}
								{loadingLinks && links.length > 0 && (
									<div className="flex justify-center py-4">
										<Loader2 className="h-6 w-6 animate-spin text-primary" />
									</div>
								)}
							</div>
						</TabsContent>
					</Tabs>
				</div>
			</SheetContent>
		</Sheet>
	)
}
