import { useEffect, useState, useRef } from "react"
import { X, Download, ExternalLink, Star } from "lucide-react"
import { TransformWrapper, TransformComponent } from "react-zoom-pan-pinch"

interface ChatImageViewerModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	imageUrl: string | null
	sourceRect: DOMRect | null
	onClose: () => void
	onFavorite?: (url: string) => void
}

export function ChatImageViewerModal({ open, onOpenChange: _onOpenChange, imageUrl, sourceRect: _sourceRect, onClose, onFavorite }: ChatImageViewerModalProps) {
	const [ready, setReady] = useState(false)
	const [closing, setClosing] = useState(false)
	const containerRef = useRef<HTMLDivElement>(null)

	useEffect(() => {
		if (open) {
			document.body.style.overflow = "hidden"
			requestAnimationFrame(() => requestAnimationFrame(() => setReady(true)))
		} else {
			document.body.style.overflow = ""
			setReady(false)
			setClosing(false)
		}
		return () => { document.body.style.overflow = "" }
	}, [open])

	const handleClose = () => {
		setClosing(true)
		setTimeout(() => onClose(), 250)
	}

	if (!open || !imageUrl) return null

	return (
		<div
			ref={containerRef}
			className="fixed inset-0 z-50"
			style={{ opacity: closing ? 0 : 1, transition: "opacity 250ms" }}
		>
			<div
				className="absolute inset-0"
				style={{
					backgroundColor: "rgba(0,0,0,0.85)",
					opacity: ready ? 1 : 0,
					transition: "opacity 300ms",
				}}
				onClick={handleClose}
			/>

			<div
				className="fixed inset-0 z-10 flex items-center justify-center"
				style={{
					opacity: closing ? 0 : 1,
					transition: "opacity 250ms",
				}}
			>
				<TransformWrapper
					initialScale={1}
					minScale={0.5}
					maxScale={5}
					centerOnInit
					doubleClick={{ mode: "toggle" }}
					wheel={{ step: 0.2 }}
					panning={{ velocityDisabled: true }}
				>
					<TransformComponent
						wrapperStyle={{ width: "100vw", height: "100vh" }}
						contentStyle={{ display: "flex", alignItems: "center", justifyContent: "center", width: "100vw", height: "100vh" }}
					>
						<img
							src={imageUrl}
							alt="Full size"
							style={{
								maxWidth: "95vw",
								maxHeight: "90vh",
								borderRadius: "12px",
								objectFit: "contain",
								boxShadow: "0 25px 50px -12px rgba(0,0,0,0.5)",
							}}
							draggable={false}
						/>
					</TransformComponent>
				</TransformWrapper>
			</div>

			{/* Actions bar */}
			<div
				className="fixed top-3 right-3 z-20 flex items-center gap-1.5"
				style={{
					opacity: ready && !closing ? 1 : 0,
					transition: "opacity 200ms",
				}}
			>
				{onFavorite && (
					<button
						onClick={() => onFavorite(imageUrl)}
						className="p-2 rounded-full bg-black/30 hover:bg-black/50 text-white backdrop-blur-md transition-colors"
						title="Add to favorites"
					>
						<Star className="h-4 w-4" />
					</button>
				)}
				<button
					onClick={() => window.open(imageUrl, '_blank')}
					className="p-2 rounded-full bg-black/30 hover:bg-black/50 text-white backdrop-blur-md transition-colors"
				>
					<ExternalLink className="h-4 w-4" />
				</button>
				<a
					href={imageUrl}
					download
					className="p-2 rounded-full bg-black/30 hover:bg-black/50 text-white backdrop-blur-md transition-colors"
					onClick={(e) => e.stopPropagation()}
				>
					<Download className="h-4 w-4" />
				</a>
				<button
					onClick={handleClose}
					className="p-2 rounded-full bg-black/30 hover:bg-black/50 text-white backdrop-blur-md transition-colors"
				>
					<X className="h-5 w-5" />
				</button>
			</div>
		</div>
	)
}
