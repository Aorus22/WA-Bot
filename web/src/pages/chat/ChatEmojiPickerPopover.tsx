import { useState, useEffect, memo } from "react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Smile } from "lucide-react"
import EmojiPicker, { Theme, EmojiStyle } from "emoji-picker-react"
import { useTheme } from "@/components/theme-provider"

const EmojiPickerComponent = memo(({ onEmojiSelect }: { onEmojiSelect: (emoji: string) => void }) => {
	const { theme } = useTheme()
	const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches)
	const [rendered, setRendered] = useState(false)

	useEffect(() => {
		const timer = setTimeout(() => setRendered(true), 200)
		return () => clearTimeout(timer)
	}, [])

	return (
		<div className="h-[400px] w-full transform-gpu flex items-center justify-center bg-background">
			{!rendered ? (
				<div className="w-6 h-6 border-2 border-primary/20 border-t-primary rounded-full animate-spin" />
			) : (
				<EmojiPicker
					onEmojiClick={(emojiData) => onEmojiSelect(emojiData.emoji)}
					theme={isDark ? Theme.DARK : Theme.LIGHT}
					emojiStyle={EmojiStyle.NATIVE}
					lazyLoadEmojis={true}
					width="100%"
					height="100%"
					searchPlaceholder="Search emojis..."
					previewConfig={{ showPreview: false }}
					skinTonesDisabled
				/>
			)}
		</div>
	)
})

EmojiPickerComponent.displayName = "EmojiPickerComponent"

interface ChatEmojiPickerPopoverProps {
	onEmojiSelect: (emoji: string) => void
}

export function ChatEmojiPickerPopover({ onEmojiSelect }: ChatEmojiPickerPopoverProps) {
	const [isOpen, setIsOpen] = useState(false)

	return (
		<Popover onOpenChange={setIsOpen}>
			<PopoverTrigger asChild>
				<Button variant="ghost" size="icon" className="rounded-full text-muted-foreground hover:bg-muted">
					<Smile className="h-5 w-5" />
				</Button>
			</PopoverTrigger>
			<PopoverContent
				className="w-[400px] p-0 overflow-hidden shadow-2xl border-border/50 animate-in fade-in zoom-in-95 duration-200 transform-gpu data-[state=closed]:duration-0 data-[state=closed]:animate-none"
				align="start"
				side="top"
			>
				{isOpen && <EmojiPickerComponent onEmojiSelect={onEmojiSelect} />}
			</PopoverContent>
		</Popover>
	)
}
