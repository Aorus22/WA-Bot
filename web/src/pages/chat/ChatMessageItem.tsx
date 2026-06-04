import { useState, useRef, memo } from "react"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { cn } from "@/lib/utils"
import { FileText, MoreVertical, Reply, Edit3, Trash2, Star } from "lucide-react"

export const ChatMessageItem = memo(({
	message,
	isMe,
	isLastInSequence,
	isFirstInSequence,
	chat,
	repliedMsg,
	onReply,
	onEdit,
	onDelete,
	onStickerFavorite,
	onImageClick,
	formatTime,
	renderFormattedContent,
	getMediaUrl,
	getAvatarUrl,
	showFavoriteBtn,
	setShowFavoriteBtn,
}: any) => {
	const [swipeX, setSwipeX] = useState(0)
	const startX = useRef(0)
	const threshold = 60

	const handleTouchStart = (e: React.TouchEvent) => {
		startX.current = e.touches[0].clientX
	}

	const handleTouchMove = (e: React.TouchEvent) => {
		const currentX = e.touches[0].clientX
		const diff = currentX - startX.current
		if (diff > 0) {
			setSwipeX(Math.min(diff, threshold + 20))
		}
	}

	const handleTouchEnd = () => {
		if (swipeX >= threshold) {
			onReply()
		}
		setSwipeX(0)
	}

	const isPending = message.status === "pending"
	const isFailed = message.status === "failed"
	const isImage = message.type === "image"
	const isVideo = message.type === "video"
	const isSticker = message.type === "sticker"
	const isDocument = message.type === "document"
	const isMedia = message.mediaUrl && message.mediaUrl.length > 0
	const showSenderInfo = !isMe && chat?.isGroup && isFirstInSequence

	return (
		<div
			id={message.id}
			className={cn(
				"flex w-full group animate-in fade-in slide-in-from-bottom-2 duration-300 relative",
				isMe ? "justify-end" : "justify-start",
				isLastInSequence ? "mb-4" : "mb-1"
			)}
		>
			<div
				className="absolute left-0 top-1/2 -translate-y-1/2 transition-opacity duration-200"
				style={{
					opacity: swipeX / threshold,
					transform: `translateX(${swipeX - 40}px)`,
				}}
			>
				<div className="bg-primary/20 p-2 rounded-full">
					<Reply className="h-4 w-4 text-primary" />
				</div>
			</div>

			<div
			        className={cn(
			                "max-w-[85%] sm:max-w-[70%] flex relative gap-3 transition-transform duration-200 will-change-transform min-w-0",
			                isMe ? "flex-row-reverse" : "flex-row"
			        )}				style={{ transform: `translateX(${swipeX}px)` }}
				onTouchStart={handleTouchStart}
				onTouchMove={handleTouchMove}
				onTouchEnd={handleTouchEnd}
			>
				{!isMe && chat?.isGroup && (
					<div className="w-8 flex-shrink-0 mt-0">
						{isFirstInSequence && (
							<Avatar className="h-8 w-8 border-2 border-background shadow-sm">
								<AvatarImage src={getAvatarUrl(message.from)} />
								<AvatarFallback className="text-[10px] bg-primary/10 text-primary font-bold">
									{message.senderName?.charAt(0).toUpperCase() || "?"}
								</AvatarFallback>
							</Avatar>
						)}
					</div>
				)}

				<div className={cn("flex flex-col", isMe ? "items-end" : "items-start")}>
					{showSenderInfo && (
						<span className="text-[11px] font-bold mb-1 ml-1 text-primary/80">{message.senderName || message.from.split("@")[0]}</span>
					)}

					{message.isAutomatic && (
						<span
							className={cn(
								"text-[10px] font-bold mb-1 px-1 uppercase tracking-widest flex items-center gap-1",
								isMe ? "text-indigo-500 dark:text-indigo-400" : "text-blue-500"
							)}
						>
							<span className={cn("w-1 h-1 rounded-full", isMe ? "bg-indigo-500 dark:bg-indigo-400" : "bg-blue-500")} />
							Bot Response
						</span>
					)}

					{repliedMsg && (
						<div
						        className="mb-2 p-2 bg-black/5 dark:bg-white/5 rounded-lg border-l-4 border-primary text-[12px] opacity-80 cursor-pointer hover:opacity-100 transition-opacity relative z-10"
						        onClick={() => document.getElementById(repliedMsg.id)?.scrollIntoView({ behavior: "smooth", block: "center" })}
						>							<p className="font-bold text-primary">{repliedMsg.from === "me" ? "You" : repliedMsg.senderName || "Unknown"}</p>
							<p className="truncate opacity-70">{repliedMsg.content}</p>
						</div>
					)}

					{isSticker ? (
					        <div className="relative group/sticker cursor-pointer z-10" onClick={() => setShowFavoriteBtn(showFavoriteBtn === message.id ? null : message.id)}>							<div className="w-[160px] h-[160px] flex items-center justify-center">
								{isMedia ? (
									<img src={getMediaUrl(message.mediaUrl)} alt="Sticker" className="max-w-full max-h-full object-contain" />
								) : (
									<div className="w-full h-full bg-muted rounded-xl flex items-center justify-center text-[10px] text-muted-foreground uppercase tracking-widest">
										Sticker
									</div>
								)}
							</div>
							{showFavoriteBtn === message.id && (
								<button
									onClick={(e) => {
										e.stopPropagation()
										onStickerFavorite(message.mediaUrl)
									}}
									className={cn(
									        "absolute top-0 p-1.5 bg-background/90 backdrop-blur-md rounded-full shadow-lg transition-all animate-in zoom-in-50 z-20",										isMe ? "left-0" : "right-0"
									)}
								>
									<Star className="h-4 w-4 text-yellow-500 fill-yellow-500" />
								</button>
							)}
							<div className={cn("absolute bottom-1 right-1 px-1.5 py-0.5 rounded-full bg-black/30 backdrop-blur-sm text-[10px] text-white opacity-0 group-hover/sticker:opacity-100 transition-opacity", isMe ? "right-1" : "left-1")}>
								{formatTime(message.timestamp)}
							</div>
						</div>
					) : (
						<div
							className={cn(
								"px-3.5 py-2 rounded-2xl text-[14.5px] relative transition-all duration-200 shadow-[0_1px_0.5px_rgba(0,0,0,0.13)]",
								isMe
									? cn(
										"bg-[#dcf8c6] dark:bg-[#005c4b] text-[#303030] dark:text-[#e9edef]",
										message.isAutomatic && "bg-[#e8eaff] dark:bg-[#2e334d]",
										isLastInSequence ? "rounded-tr-none" : "",
										isPending && "opacity-70",
										isFailed && "bg-destructive text-destructive-foreground"
									)
									: cn(
										"bg-white dark:bg-[#202c33] text-[#303030] dark:text-[#e9edef]",
										message.isAutomatic && "bg-[#eef2ff] dark:bg-[#1e2235] border-l-[3px] border-indigo-500 dark:border-indigo-400",
										isLastInSequence ? "rounded-tl-none" : ""
									)
							)}
						>
							{isImage && isMedia && (
							        <div className="mb-2 -mx-1 -mt-1 rounded-lg overflow-hidden border border-black/5 dark:border-white/5 relative z-10">
							                <img										src={getMediaUrl(message.mediaUrl)}
										alt="Image"
										className="w-full max-w-[320px] h-auto object-cover hover:scale-[1.02] transition-transform duration-500 cursor-zoom-in"
										onClick={(e) => {
											e.stopPropagation()
											onImageClick(getMediaUrl(message.mediaUrl))
										}}
									/>
								</div>
							)}
							{isVideo && isMedia && (
							        <div className="mb-2 -mx-1 -mt-1 rounded-lg overflow-hidden border border-black/5 dark:border-white/5 bg-black/10 flex items-center justify-center aspect-video relative z-10">
							                <video src={getMediaUrl(message.mediaUrl)} className="w-full h-auto max-h-[300px]" controls />
							                </div>
							                )}
							                {isDocument && isMedia && (							        <a
							                href={getMediaUrl(message.mediaUrl)}
							                target="_blank"
							                rel="noopener noreferrer"
							                className="flex items-center gap-3 mb-1 p-2 bg-black/5 dark:bg-white/5 rounded-lg border border-black/5 dark:border-white/5 relative z-10"
							        >
									<div className="w-9 h-9 bg-primary/10 rounded flex items-center justify-center">
										<FileText className="h-5 w-5 text-primary" />
									</div>
									<div className="flex-1 min-w-0">
										<p className="text-xs font-bold truncate">Document</p>
										<p className="text-[10px] opacity-60">PDF • View</p>
									</div>
								</a>
							)}
							{message.content && !["[Image]", "[Video]", "[Sticker]"].includes(message.content) && (
							        <div className="break-words [word-break:break-word] leading-relaxed whitespace-pre-wrap relative z-10">{renderFormattedContent(message.content)}</div>
							)}							<div className={cn("flex items-center gap-1 mt-1 justify-end", "text-[10px] font-medium opacity-50 uppercase tracking-tight")}>
								<span>{formatTime(message.timestamp)}</span>
								{isMe && (
									<span className="flex items-center ml-0.5">
										{isPending ? (
											<div className="w-2.5 h-2.5 border border-current/30 border-t-current rounded-full animate-spin" />
										) : isFailed ? (
											"!"
										) : (
											<div className="flex items-center">
												{message.status === "sent" ? (
													<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
														<path d="M13.5 4.5L6.5 11.5L3 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
													</svg>
												) : (
													<div className="relative flex items-center">
														<svg
															width="16"
															height="16"
															viewBox="0 0 16 16"
															fill="none"
															xmlns="http://www.w3.org/2000/svg"
															className={cn(message.status === "read" ? "text-[#53bdeb]" : "text-current opacity-70")}
														>
															<path d="M13.5 4.5L6.5 11.5L3 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
														</svg>
														<svg
															width="16"
															height="16"
															viewBox="0 0 16 16"
															fill="none"
															xmlns="http://www.w3.org/2000/svg"
															className={cn("absolute left-[4px]", message.status === "read" ? "text-[#53bdeb]" : "text-current opacity-70")}
														>
															<path d="M13.5 4.5L6.5 11.5L3 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
														</svg>
													</div>
												)}
											</div>
										)
										}</span>
								)}

								{isMe && !isSticker && (
								        <div className="absolute top-1/2 -translate-y-1/2 -right-2 opacity-0 group-hover:opacity-100 transition-opacity hidden md:block z-20">										<Popover>
											<PopoverTrigger asChild>
												<Button variant="ghost" size="icon" className="h-6 w-6 text-muted-foreground hover:text-foreground bg-background/80 backdrop-blur-sm rounded-full shadow-sm">
													<MoreVertical className="h-3 w-3" />
												</Button>
											</PopoverTrigger>
											<PopoverContent className="w-auto p-1" align="end">
												<div className="flex flex-col">
													<button onClick={onReply} className="flex items-center gap-2 w-full p-2 hover:bg-muted rounded text-sm"><Reply className="h-4 w-4 text-primary" /><span>Reply</span></button>
													<button onClick={onEdit} className="flex items-center gap-2 w-full p-2 hover:bg-muted rounded text-sm"><Edit3 className="h-4 w-4 text-orange-500" /><span>Edit</span></button>
													<button onClick={onDelete} className="flex items-center gap-2 w-full p-2 hover:bg-muted rounded text-sm text-destructive"><Trash2 className="h-4 w-4" /><span>Delete</span></button>
												</div>
											</PopoverContent>
										</Popover>
									</div>
								)}

								<Popover>
									<PopoverTrigger asChild>
										<div className="absolute inset-0 z-0 cursor-context-menu" onContextMenu={(e) => { if (isSticker) return; e.preventDefault(); e.currentTarget.click(); }} />
									</PopoverTrigger>
									<PopoverContent className="w-48 p-1 shadow-xl border-border/40 backdrop-blur-xl bg-background/95" align={isMe ? "end" : "start"}>
										<div className="flex flex-col">
											<button onClick={onReply} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors"><Reply className="h-4 w-4 text-primary" /><span className="font-medium">Reply</span></button>
											{isMe && (
												<>
													<button onClick={onEdit} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors"><Edit3 className="h-4 w-4 text-orange-500" /><span className="font-medium">Edit</span></button>
													<button onClick={onDelete} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors text-destructive"><Trash2 className="h-4 w-4" /><span className="font-medium">Delete for everyone</span></button>
												</>
											)}
										</div>
									</PopoverContent>
								</Popover>

								{!isMe && !isSticker && (
								        <div className="absolute top-1/2 -translate-y-1/2 -left-2 opacity-0 group-hover:opacity-100 transition-opacity hidden md:block z-20">										<Button variant="ghost" size="icon" onClick={onReply} className="h-6 w-6 text-muted-foreground hover:text-primary bg-background/80 backdrop-blur-sm rounded-full shadow-sm"><Reply className="h-3 w-3" /></Button>
									</div>
								)}
							</div>
						</div>
					)}
				</div>
			</div>
		</div>
	)
})

ChatMessageItem.displayName = "ChatMessageItem"
