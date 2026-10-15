// Pure helpers for the call media protocol (docs/CALL-MEDIA-PROTOCOL.md):
// binary frame encode/decode, PCM conversion and a streaming 16 kHz resampler
// that cuts the microphone signal into fixed 960-sample (60 ms) frames.
// Nothing in here touches the DOM, so it can be unit-tested under Node.

export const FRAME_KIND_OUTGOING_AUDIO = 0x01
export const FRAME_KIND_OUTGOING_VIDEO = 0x02
export const FRAME_KIND_INCOMING_AUDIO = 0x03
export const FRAME_KIND_INCOMING_VIDEO = 0x04
export const FRAME_KIND_KEYFRAME = 0x05

export const MEDIA_SAMPLE_RATE = 16000
export const MEDIA_FRAME_SAMPLES = 960

export function encodeFrame(kind: number, metadata: Uint8Array | null, payload: Uint8Array): Uint8Array {
	const meta = metadata ?? new Uint8Array(0)
	if (meta.length > 65535) throw new Error("metadata too long")
	const out = new Uint8Array(3 + meta.length + payload.length)
	out[0] = kind
	out[1] = (meta.length >> 8) & 0xff
	out[2] = meta.length & 0xff
	out.set(meta, 3)
	out.set(payload, 3 + meta.length)
	return out
}

export function decodeFrame(
	data: ArrayBuffer | Uint8Array,
): { kind: number; metadata: Uint8Array; payload: Uint8Array } | null {
	const u8 = data instanceof Uint8Array ? data : new Uint8Array(data)
	if (u8.length < 3) return null
	const kind = u8[0]
	const metaLen = (u8[1] << 8) | u8[2]
	if (u8.length < 3 + metaLen) return null
	// subarray: no copy; the message buffer is owned by us and not reused
	return { kind, metadata: u8.subarray(3, 3 + metaLen), payload: u8.subarray(3 + metaLen) }
}

export function float32ToS16LE(floats: Float32Array): Uint8Array {
	const out = new Uint8Array(floats.length * 2)
	const view = new DataView(out.buffer)
	for (let i = 0; i < floats.length; i++) {
		let s = Math.max(-1, Math.min(1, floats[i]))
		s = s < 0 ? s * 0x8000 : s * 0x7fff
		view.setInt16(i * 2, Math.round(s), true)
	}
	return out
}

export function s16LEToFloat32(u8: Uint8Array): Float32Array {
	const len = Math.floor(u8.length / 2)
	const out = new Float32Array(len)
	const view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength)
	for (let i = 0; i < len; i++) {
		out[i] = view.getInt16(i * 2, true) / 0x8000
	}
	return out
}

/** Quarter turns clockwise (0..3) from the incoming video metadata. */
export function parseVideoMetadata(metadata: Uint8Array): { participantJid?: string; orientation: number } {
	let participantJid: string | undefined
	let orientation = 0
	if (metadata.length > 0) {
		try {
			const meta = JSON.parse(new TextDecoder().decode(metadata)) as { participant_jid?: string; orientation?: number }
			if (typeof meta.participant_jid === "string" && meta.participant_jid) participantJid = meta.participant_jid
			if (typeof meta.orientation === "number" && Number.isFinite(meta.orientation)) {
				orientation = ((Math.round(meta.orientation) % 4) + 4) % 4
			}
		} catch {
			// ignore bad metadata
		}
	}
	return { participantJid, orientation }
}

/**
 * Streaming resampler (windowed-sinc low-pass + interpolation) from an
 * arbitrary input rate to 16 kHz mono, emitting exactly 960-sample frames.
 */
export class PcmFramer {
	readonly inputRate: number
	readonly outputRate: number
	private readonly ratio: number // input samples per output sample
	private readonly cutoff: number // normalised low-pass cutoff relative to the input rate (<= 1)
	private readonly halfWidth: number // kernel half width in input samples
	private readonly passthrough: boolean
	private buf = new Float32Array(0)
	private bufStart = 0 // absolute input index of buf[0]
	private nextOut = 0 // index of next output sample
	private readonly frame: Float32Array
	private frameFill = 0

	constructor(inputRate: number, outputRate = MEDIA_SAMPLE_RATE, frameSamples = MEDIA_FRAME_SAMPLES, lobes = 6) {
		if (!(inputRate > 0)) throw new Error("invalid input rate")
		this.inputRate = inputRate
		this.outputRate = outputRate
		this.ratio = inputRate / outputRate
		this.passthrough = inputRate === outputRate
		// Anti-alias slightly under Nyquist of the lower of the two rates.
		this.cutoff = Math.min(1, outputRate / inputRate) * 0.95
		this.halfWidth = Math.ceil(lobes / this.cutoff)
		this.frame = new Float32Array(frameSamples)
	}

	/** Feed input samples; `onFrame` is invoked with a fresh Float32Array per full frame. */
	push(input: Float32Array, onFrame: (frame: Float32Array) => void): void {
		if (input.length === 0) return
		if (this.passthrough) {
			this.fill(input, onFrame)
			return
		}
		// append
		const merged = new Float32Array(this.buf.length + input.length)
		merged.set(this.buf, 0)
		merged.set(input, this.buf.length)
		this.buf = merged
		const totalIn = this.bufStart + this.buf.length

		const out = new Float32Array(Math.ceil(input.length / this.ratio) + 4)
		let n = 0
		for (;;) {
			const t = this.nextOut * this.ratio
			const i0 = Math.floor(t)
			if (i0 + this.halfWidth >= totalIn) break // need more input
			out[n++] = this.interpolate(t, i0)
			this.nextOut++
		}
		// drop input that can no longer be referenced
		const keepFrom = Math.floor(this.nextOut * this.ratio) - this.halfWidth
		const drop = Math.min(keepFrom - this.bufStart, this.buf.length)
		if (drop > 0) {
			this.buf = this.buf.slice(drop)
			this.bufStart += drop
		}
		if (n > 0) this.fill(out.subarray(0, n), onFrame)
	}

	/** Discard any partial frame and filter history (e.g. after a long pause). */
	reset(): void {
		this.buf = new Float32Array(0)
		this.bufStart = 0
		this.nextOut = 0
		this.frameFill = 0
	}

	private interpolate(t: number, i0: number): number {
		const fc = this.cutoff
		const hw = this.halfWidth
		let acc = 0
		let wsum = 0
		for (let i = i0 - hw + 1; i <= i0 + hw; i++) {
			const d = t - i
			const x = i - this.bufStart
			// samples before the stream start are treated as silence but still weighted
			const sample = x >= 0 && x < this.buf.length ? this.buf[x] : 0
			const arg = Math.PI * fc * d
			const sinc = Math.abs(arg) < 1e-9 ? 1 : Math.sin(arg) / arg
			const win = 0.5 + 0.5 * Math.cos((Math.PI * d) / (hw + 1)) // Hann over +-(hw+1)
			const w = sinc * win
			acc += sample * w
			wsum += w
		}
		return wsum !== 0 ? acc / wsum : 0
	}

	private fill(samples: Float32Array, onFrame: (frame: Float32Array) => void): void {
		let off = 0
		while (off < samples.length) {
			const take = Math.min(this.frame.length - this.frameFill, samples.length - off)
			this.frame.set(samples.subarray(off, off + take), this.frameFill)
			this.frameFill += take
			off += take
			if (this.frameFill === this.frame.length) {
				onFrame(this.frame.slice())
				this.frameFill = 0
			}
		}
	}
}

/** Origin (ws://host:port) of the backend, using the same rules as the JSON /ws hub. */
export function getBackendWsOrigin(): string {
	const envUrl = import.meta.env.VITE_WS_URL as string | undefined
	if (envUrl) return envUrl.replace(/\/ws\/?$/, "")
	if (typeof window !== "undefined") {
		const protocol = window.location.protocol === "https:" ? "wss:" : "ws:"
		const port = (window as unknown as { __BACKEND_PORT__?: number }).__BACKEND_PORT__
		if (port) return `${protocol}//localhost:${port}`
		const origin = window.location.origin
		if (origin && origin !== "null" && origin !== "file://") return `${protocol}//${window.location.host}`
		return `${protocol}//localhost:8080`
	}
	return ""
}

export function buildMediaWsUrl(origin: string, wsPath: string, token: string): string {
	const path = wsPath.startsWith("/") ? wsPath : `/${wsPath}`
	const sep = path.includes("?") ? "&" : "?"
	return `${origin}${path}${sep}token=${encodeURIComponent(token)}&video=jpeg`
}
