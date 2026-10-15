import { useEffect, useRef, useState } from "react"

const BARS = 32
const SAMPLE_MS = 70

/** Live input level of a microphone stream, drawn as scrolling bars (newest on the right). */
export function RecordingWaveform({ stream }: { stream: MediaStream | null }) {
	const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0))
	const levelRef = useRef(0)

	useEffect(() => {
		if (!stream) return
		const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext
		if (!AudioCtx) return
		const ctx: AudioContext = new AudioCtx()
		const analyser = ctx.createAnalyser()
		analyser.fftSize = 512
		ctx.createMediaStreamSource(stream).connect(analyser)
		const data = new Uint8Array(analyser.fftSize)

		let raf = 0
		const measure = () => {
			analyser.getByteTimeDomainData(data)
			let peak = 0
			for (const v of data) peak = Math.max(peak, Math.abs(v - 128))
			levelRef.current = Math.max(levelRef.current * 0.6, Math.min(1, peak / 80))
			raf = requestAnimationFrame(measure)
		}
		raf = requestAnimationFrame(measure)

		const sampler = window.setInterval(() => {
			setLevels((prev) => [...prev.slice(1), levelRef.current])
		}, SAMPLE_MS)

		return () => {
			cancelAnimationFrame(raf)
			clearInterval(sampler)
			ctx.close().catch(() => {})
			levelRef.current = 0
			setLevels(Array(BARS).fill(0))
		}
	}, [stream])

	return (
		<div className="flex items-center justify-end gap-[3px] h-8 flex-1 min-w-0 overflow-hidden" aria-hidden="true">
			{levels.map((level, i) => (
				<span
					key={i}
					className="w-[3px] shrink-0 rounded-full bg-primary transition-[height] duration-75"
					style={{ height: `${Math.max(12, level * 100)}%`, opacity: 0.4 + level * 0.6 }}
				/>
			))}
		</div>
	)
}
