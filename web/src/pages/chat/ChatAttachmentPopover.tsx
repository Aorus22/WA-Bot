import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { FileText, Paperclip, Video } from "lucide-react"

interface ChatAttachmentPopoverProps {
	onPickMedia: () => void
	onPickDocument: () => void
	open?: boolean
	onOpenChange?: (open: boolean) => void
}

export function ChatAttachmentPopover({ onPickMedia, onPickDocument, open: controlledOpen, onOpenChange: controlledOnOpenChange }: ChatAttachmentPopoverProps) {
	const [internalOpen, setInternalOpen] = useState(false)
	const isOpen = controlledOpen !== undefined ? controlledOpen : internalOpen
	const setOpen = controlledOnOpenChange || setInternalOpen

	return (
		<Popover open={isOpen} onOpenChange={setOpen}>
			<PopoverTrigger asChild>
				<Button variant="ghost" size="icon" className="rounded-full text-muted-foreground hover:bg-muted">
					<Paperclip className="h-5 w-5" />
				</Button>
			</PopoverTrigger>
			<PopoverContent className="w-[200px] p-1" align="start" side="top">
				<div className="flex flex-col">
					<button onClick={onPickMedia} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors">
						<div className="w-8 h-8 bg-purple-500/10 rounded-full flex items-center justify-center text-purple-600">
							<Video className="h-4 w-4" />
						</div>
						<span className="font-medium">Photos & Videos</span>
					</button>
					<button onClick={onPickDocument} className="flex items-center gap-3 w-full p-2.5 hover:bg-muted rounded-lg text-sm transition-colors">
						<div className="w-8 h-8 bg-blue-500/10 rounded-full flex items-center justify-center text-blue-600">
							<FileText className="h-4 w-4" />
						</div>
						<span className="font-medium">Document</span>
					</button>
				</div>
			</PopoverContent>
		</Popover>
	)
}
