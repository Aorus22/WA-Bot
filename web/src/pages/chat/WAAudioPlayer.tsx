import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Play, Pause, Mic } from "lucide-react"
import { cn } from "@/lib/utils"

interface WAAudioPlayerProps {
	src: string
	isMe: boolean
	type: string
	messageId: string
	avatarUrl?: string
	durationHint?: number
}

const BAR_COUNT = 30
const WAVE_HEIGHT = 28

const formatTime = (seconds: number): string => {
	if (!Number.isFinite(seconds) || seconds < 0) return "0:00"
	const m = Math.floor(seconds / 60)
	const s = Math.floor(seconds % 60)
	return `${m}:${String(s).padStart(2, "0")}`
}

/** Deterministic FNV-1a hash so the same message always renders the same waveform. */
const hashId = (str: string): number => {
	let h = 2166136261
	for (let i = 0; i < str.length; i++) {
		h ^= str.charCodeAt(i)
		h = Math.imul(h, 16777619)
	}
	return h >>> 0
}

export const WAAudioPlayer = ({
	src,
	isMe,
	type,
	messageId,
	avatarUrl,
	durationHint,
}: WAAudioPlayerProps) => {
	const audioRef = useRef<HTMLAudioElement>(null)
	const waveformRef = useRef<HTMLDivElement>(null)
	const [isPlaying, setIsPlaying] = useState(false)
	const [currentTime, setCurrentTime] = useState(0)
	const [duration, setDuration] = useState(0)
	const [hasError, setHasError] = useState(false)
	const [imgError, setImgError] = useState(false)

	useEffect(() => setImgError(false), [avatarUrl])

	const bars = useMemo(() => {
		const seed = hashId(messageId || "audio")
		const out: number[] = []
		for (let i = 0; i < BAR_COUNT; i++) {
			const raw = Math.sin(seed * (i + 1) * 12.9898 + 4.77) * 43758.5453
			const frac = raw - Math.floor(raw)
			out.push(6 + Math.round(frac * (WAVE_HEIGHT - 8))) // 6..26px
		}
		return out
	}, [messageId])

	const totalDuration = duration || durationHint || 0
	const progress = totalDuration > 0 ? Math.min(1, currentTime / totalDuration) : 0

	const togglePlay = useCallback(async () => {
		const audio = audioRef.current
		if (!audio) return
		try {
			if (audio.paused) {
				if (audio.ended) {
					audio.currentTime = 0
					setCurrentTime(0)
				}
				await audio.play()
				setIsPlaying(true)
			} else {
				audio.pause()
				setIsPlaying(false)
			}
		} catch (err) {
			console.error("Audio playback failed:", err)
		}
	}, [])

	const handleSeek = useCallback(
		(clientX: number) => {
			const audio = audioRef.current
			const el = waveformRef.current
			if (!audio || !el) return
			const rect = el.getBoundingClientRect()
			if (rect.width === 0) return
			const frac = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width))
			audio.currentTime = frac * totalDuration
			setCurrentTime(audio.currentTime)
		},
		[totalDuration]
	)

	const handleKeyDown = useCallback(
		(e: React.KeyboardEvent) => {
			const audio = audioRef.current
			if (!audio) return
			const step = e.shiftKey ? 10 : 5
			if (e.key === "ArrowRight") {
				e.preventDefault()
				audio.currentTime = Math.min(audio.duration || totalDuration, audio.currentTime + step)
				setCurrentTime(audio.currentTime)
			} else if (e.key === "ArrowLeft") {
				e.preventDefault()
				audio.currentTime = Math.max(0, audio.currentTime - step)
				setCurrentTime(audio.currentTime)
			} else if (e.key === " " || e.key === "Enter") {
				e.preventDefault()
				togglePlay()
			}
		},
		[togglePlay, totalDuration]
	)

	// Smooth progress via rAF while playing; audio events cover the idle path.
	useEffect(() => {
		if (!isPlaying) return
		let raf = 0
		const tick = () => {
			const audio = audioRef.current
			if (audio) {
				setCurrentTime(audio.currentTime)
				if (audio.duration && Number.isFinite(audio.duration)) setDuration(audio.duration)
			}
			raf = requestAnimationFrame(tick)
		}
		raf = requestAnimationFrame(tick)
		return () => cancelAnimationFrame(raf)
	}, [isPlaying])

	const accentText = isMe
		? "text-[#00a884] dark:text-white"
		: "text-[#00a884] dark:text-primary"

	return (
		<div
			className={cn(
				"relative z-10 flex items-center gap-3 -mx-[14px] -mt-2 px-3.5 pt-3 pb-2",
				"min-w-[240px] sm:min-w-[300px] max-w-full select-none"
			)}
		>
			<button
				type="button"
				onClick={togglePlay}
				aria-label={isPlaying ? "Pause audio" : "Play audio"}
				className={cn(
					"h-11 w-11 flex-shrink-0 rounded-full flex items-center justify-center transition-all duration-200",
					"shadow-lg hover:scale-105 active:scale-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2",
					isMe
						? "bg-[#00a884] text-white shadow-[#00a884]/30 hover:bg-[#00a884]/90 focus-visible:ring-[#00a884]"
						: "bg-primary text-primary-foreground shadow-black/20 hover:bg-primary/90 focus-visible:ring-primary"
				)}
			>
				{isPlaying ? (
					<Pause className="h-5 w-5 fill-current" />
				) : (
					<Play className="h-5 w-5 fill-current translate-x-[1px]" />
				)}
			</button>

			<div className="flex-1 min-w-0 flex flex-col gap-1.5">
				<div
					ref={waveformRef}
					role="slider"
					aria-label="Audio progress"
					aria-valuemin={0}
					aria-valuemax={100}
					aria-valuenow={Math.round(progress * 100)}
					tabIndex={0}
					onKeyDown={handleKeyDown}
					onTouchStart={(e) => e.stopPropagation()}
					onTouchMove={(e) => e.stopPropagation()}
					onPointerDown={(e) => {
						e.currentTarget.setPointerCapture(e.pointerId)
						handleSeek(e.clientX)
					}}
					onPointerMove={(e) => {
						if (e.buttons === 1) handleSeek(e.clientX)
					}}
					className={cn(
						"relative h-7 cursor-pointer touch-none rounded-md flex items-center",
						"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40"
					)}
				>
					{/* Unplayed layer */}
					<div className={cn("absolute inset-0 flex items-center justify-between", accentText)}>
						{bars.map((h, i) => (
							<div key={i} style={{ height: h }} className="w-[3px] rounded-full bg-current opacity-30" />
						))}
					</div>
					{/* Played layer, clipped from the right to follow progress */}
					<div
						className={cn("absolute inset-0 flex items-center justify-between", accentText, isPlaying && "animate-pulse")}
						style={{ clipPath: `inset(0 ${(1 - progress) * 100}% 0 0)` }}
					>
						{bars.map((h, i) => (
							<div key={i} style={{ height: h }} className="w-[3px] rounded-full bg-current" />
						))}
					</div>
				</div>

				<div className="flex items-center gap-1.5 text-[11px] font-medium opacity-70 tabular-nums leading-none">
					{hasError ? (
						<span className="opacity-80">Playback unavailable</span>
					) : (
						<>
							{type === "ptt" && <Mic className="h-3 w-3" />}
							<span>{formatTime(currentTime)}</span>
							<span className="opacity-50">/</span>
							<span>{formatTime(totalDuration)}</span>
						</>
					)}
				</div>
			</div>

			<div
				className={cn(
					"h-8 w-8 flex-shrink-0 rounded-full ring-2 overflow-hidden",
					isMe ? "ring-[#00a884]/30" : "ring-border/60"
				)}
			>
				{avatarUrl && !imgError ? (
					<img src={avatarUrl} alt="" onError={() => setImgError(true)} className="h-full w-full object-cover" />
				) : (
					<div
						className={cn(
							"h-full w-full flex items-center justify-center",
							isMe ? "bg-[#00a884]/15 text-[#00a884]" : "bg-muted text-primary"
						)}
					>
						<Mic className="h-3.5 w-3.5" />
					</div>
				)}
			</div>

			<audio
				ref={audioRef}
				src={src}
				preload="metadata"
				className="hidden"
				onLoadedMetadata={(e) => {
					if (Number.isFinite(e.currentTarget.duration)) setDuration(e.currentTarget.duration)
				}}
				onTimeUpdate={(e) => setCurrentTime(e.currentTarget.currentTime)}
				onPlay={() => setIsPlaying(true)}
				onPause={() => setIsPlaying(false)}
				onEnded={() => {
					setIsPlaying(false)
					setCurrentTime(0)
				}}
				onError={() => setHasError(true)}
			/>
		</div>
	)
}
