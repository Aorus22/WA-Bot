import { Button } from "@/components/ui/button"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { X } from "lucide-react"

interface ChatImageViewerModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	imageUrl: string | null
	onClose: () => void
}

export function ChatImageViewerModal({ open, onOpenChange, imageUrl, onClose }: ChatImageViewerModalProps) {
	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent showCloseButton={false} className="max-w-[95vw] max-h-[95vh] p-0 bg-transparent border-none shadow-none flex items-center justify-center overflow-hidden">
				{imageUrl && (
					<div className="relative w-full h-full flex items-center justify-center animate-in zoom-in-95 duration-300">
						<img src={imageUrl} alt="Full size" className="max-w-full max-h-[90vh] object-contain rounded-lg shadow-2xl" />
						<Button
							variant="ghost"
							size="icon"
							className="absolute top-2 right-2 rounded-full bg-black/20 hover:bg-black/40 text-white backdrop-blur-md"
							onClick={onClose}
						>
							<X className="h-5 w-5" />
						</Button>
					</div>
				)}
			</DialogContent>
		</Dialog>
	)
}
