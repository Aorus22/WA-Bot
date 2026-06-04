import { useState, useEffect, memo } from "react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Sticker, Trash2 } from "lucide-react"
import { api } from "@/lib/api"
import { toast } from "sonner"

const StickerPicker = memo(({ onStickerSelect }: { onStickerSelect: (sticker: any) => void }) => {
	const [stickers, setStickers] = useState<any[]>([])
	const [loading, setLoading] = useState(true)
	const [rendered, setRendered] = useState(false)

	useEffect(() => {
		const timer = setTimeout(() => setRendered(true), 200)
		return () => clearTimeout(timer)
	}, [])

	const getApiBase = () => {
		const envUrl = import.meta.env.VITE_API_URL
		if (envUrl) return envUrl
		return typeof window !== "undefined" ? `${window.location.origin}/api` : ""
	}

	const getMediaUrl = (url: string | undefined): string | undefined => {
		if (!url) return undefined
		if (url.startsWith("http://") || url.startsWith("https://")) return url

		const apiBase = getApiBase()
		const parts = url.split("/")
		if (parts.length > 1) {
			const filename = parts.pop()!
			const path = parts.join("/")
			const encodedFilename = encodeURIComponent(filename)
			const fullUrl = `${apiBase}${path}/${encodedFilename}`
			return fullUrl
		}

		return `${apiBase}${url}`
	}

	const loadStickers = async () => {
		try {
			setLoading(true)
			const data = await api.getFavorites()
			setStickers(data)
		} catch (err) {
			console.error("Failed to load stickers:", err)
		} finally {
			setLoading(false)
		}
	}

	useEffect(() => {
		if (rendered) {
			loadStickers()
		}
	}, [rendered])

	const deleteSticker = async (e: React.MouseEvent, id: string) => {
		e.stopPropagation()
		try {
			await api.deleteFavorite(id)
			toast.success("Sticker removed from favorites")
			loadStickers()
		} catch (err) {
			toast.error("Failed to remove sticker")
		}
	}

	return (
		<div className="h-[300px] w-full overflow-y-auto p-3 scrollbar-none flex items-center justify-center bg-background">
			{!rendered || (loading && stickers.length === 0) ? (
				<div className="w-6 h-6 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
			) : stickers.length === 0 ? (
				<div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-2">
					<p className="text-sm">No favorite stickers found</p>
					<p className="text-[10px] text-center px-4">Click on a received sticker in chat to add it to your favorites.</p>
				</div>
			) : (
				<div className="grid grid-cols-4 gap-2 w-full content-start h-full">
					{stickers.map((sticker) => (
						<div key={sticker.id} className="relative group/sticker-item">
							<button
								onClick={() => onStickerSelect(sticker)}
								className="aspect-square w-full flex items-center justify-center hover:bg-muted rounded-xl transition-all active:scale-95 group relative p-1"
							>
								<img src={getMediaUrl(sticker.mediaUrl)} alt="Sticker" className="max-w-full max-h-full object-contain" />
							</button>
							<button
								onClick={(e) => deleteSticker(e, sticker.id)}
								className="absolute -top-1 -right-1 p-1 bg-destructive text-destructive-foreground rounded-full opacity-0 group-hover/sticker-item:opacity-100 transition-opacity shadow-sm z-10"
							>
								<Trash2 className="h-3 w-3" />
							</button>
						</div>
					))}
				</div>
			)}
		</div>
	)
})

StickerPicker.displayName = "StickerPicker"

interface ChatStickerPickerPopoverProps {
	onStickerSelect: (sticker: any) => void
}

export function ChatStickerPickerPopover({ onStickerSelect }: ChatStickerPickerPopoverProps) {
	return (
		<Popover>
			<PopoverTrigger asChild>
				<Button variant="ghost" size="icon" className="rounded-full text-muted-foreground hover:bg-muted">
					<Sticker className="h-5 w-5" />
				</Button>
			</PopoverTrigger>
			<PopoverContent className="w-[400px] p-0 overflow-hidden shadow-2xl border-border/50 animate-in fade-in zoom-in-95 duration-200 transform-gpu" align="start" side="top">
				<StickerPicker onStickerSelect={onStickerSelect} />
			</PopoverContent>
		</Popover>
	)
}
