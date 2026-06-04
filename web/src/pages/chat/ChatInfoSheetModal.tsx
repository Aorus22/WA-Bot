import type { Chat, Message } from "@/lib/api"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { FileText, Image as ImageIcon, Link as LinkIcon, Video, File } from "lucide-react"

interface ChatInfoSheetModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	chat: Chat
	sharedMedia: Message[]
	sharedDocs: Message[]
	sharedLinks: Message[]
	getAvatarUrl: (target: Chat) => string | undefined
	getMediaUrl: (url: string | undefined) => string | undefined
	formatDate: (timestamp: number) => string
	onSelectImage: (url: string | null) => void
}

export function ChatInfoSheetModal({
	open,
	onOpenChange,
	chat,
	sharedMedia,
	sharedDocs,
	sharedLinks,
	getAvatarUrl,
	getMediaUrl,
	formatDate,
	onSelectImage,
}: ChatInfoSheetModalProps) {
	return (
		<Sheet open={open} onOpenChange={onOpenChange}>
			<SheetContent side="right" className="w-full sm:max-w-md p-0 flex flex-col h-full bg-background border-l border-border/40">
				<SheetHeader className="p-4 border-b border-border/40 bg-muted/20">
					<SheetTitle className="text-lg font-bold flex items-center gap-2">Chat Info</SheetTitle>
				</SheetHeader>

				<div className="flex flex-col items-center py-6 px-4 text-center border-b border-border/40 bg-muted/5">
					<Avatar className="h-20 w-20 border-2 border-background shadow-lg mb-3">
						<AvatarImage src={getAvatarUrl(chat)} />
						<AvatarFallback className="text-2xl bg-primary/10 text-primary font-bold">{chat.name.charAt(0).toUpperCase()}</AvatarFallback>
					</Avatar>
					<h2 className="text-base font-bold leading-tight">{chat.name}</h2>
					<p className="text-[11px] font-medium text-muted-foreground mt-1">{chat.id}</p>
				</div>

				<div className="flex-1 flex flex-col min-h-0 overflow-hidden">
					<Tabs defaultValue="media" className="flex-1 flex flex-col">
						<div className="px-4 pt-4">
							<TabsList className="w-full grid grid-cols-3 bg-muted/50 p-1">
								<TabsTrigger value="media" className="text-xs">Media</TabsTrigger>
								<TabsTrigger value="docs" className="text-xs">Docs</TabsTrigger>
								<TabsTrigger value="links" className="text-xs">Links</TabsTrigger>
							</TabsList>
						</div>

						<div className="flex-1 overflow-y-auto p-4 min-h-0">
							<TabsContent value="media" className="mt-0 outline-none h-full">
								{sharedMedia.length > 0 ? (
									<div className="grid grid-cols-3 gap-2">
										{sharedMedia.map(m => (
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
								) : (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground opacity-40">
										<ImageIcon className="h-12 w-12 mb-2" />
										<p className="text-sm">No media shared yet</p>
									</div>
								)}
							</TabsContent>

							<TabsContent value="docs" className="mt-0 outline-none h-full">
								{sharedDocs.length > 0 ? (
									<div className="space-y-3">
										{sharedDocs.map(m => (
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
								) : (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground opacity-40">
										<FileText className="h-12 w-12 mb-2" />
										<p className="text-sm">No documents shared yet</p>
									</div>
								)}
							</TabsContent>

							<TabsContent value="links" className="mt-0 outline-none h-full">
								{sharedLinks.length > 0 ? (
									<div className="space-y-3">
										{sharedLinks.map(m => {
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
								) : (
									<div className="flex flex-col items-center justify-center h-full py-12 text-muted-foreground opacity-40">
										<LinkIcon className="h-12 w-12 mb-2" />
										<p className="text-sm">No links shared yet</p>
									</div>
								)}
							</TabsContent>
						</div>
					</Tabs>
				</div>
			</SheetContent>
		</Sheet>
	)
}
