import { X } from "lucide-react"
import type { LinkPreviewMeta } from "@/lib/api"
import { api } from "@/lib/api"

const hostOf = (url: string) => {
	try {
		return new URL(url).host
	} catch {
		return url
	}
}

/** Preview card shown above the composer while the draft holds a link. */
export function ComposerLinkPreview({ preview, onDismiss }: { preview: LinkPreviewMeta; onDismiss: () => void }) {
	return (
		<div className="flex items-center gap-3 p-2 bg-muted/50 border-l-4 border-primary rounded-r-lg">
			{preview.thumbnailUrl && (
				<img src={api.mediaURL(preview.thumbnailUrl)} alt="" className="h-14 w-14 object-cover rounded-md shrink-0" />
			)}
			<div className="flex-1 min-w-0">
				<p className="text-xs font-bold truncate">{preview.title || hostOf(preview.url)}</p>
				{preview.description && <p className="text-xs opacity-70 line-clamp-2">{preview.description}</p>}
				<p className="text-[10px] opacity-50 truncate">{hostOf(preview.url)}</p>
			</div>
			<button
				type="button"
				onClick={onDismiss}
				title="Remove link preview"
				aria-label="Remove link preview"
				className="p-1.5 rounded-button text-muted-foreground hover:text-foreground hover:bg-muted transition-colors shrink-0"
			>
				<X className="h-4 w-4" />
			</button>
		</div>
	)
}
