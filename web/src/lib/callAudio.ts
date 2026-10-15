// Browser-side audio for a call: microphone capture (AudioWorklet -> 16 kHz
// mono 960-sample frames) and a small jitter-buffered player for the remote side.
// One AudioContext is shared by both directions.

import { MEDIA_SAMPLE_RATE, PcmFramer } from "@/lib/callMediaCodec"

const WORKLET_NAME = "wa-call-capture"
// Posts ~1024 mono samples at a time to keep the message rate low.
const WORKLET_SOURCE = `
class WaCallCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(1024);
    this.fill = 0;
  }
  process(inputs) {
    const ch = inputs[0] && inputs[0][0];
    if (ch) {
      for (let i = 0; i < ch.length; i++) {
        this.buf[this.fill++] = ch[i];
        if (this.fill === this.buf.length) {
          this.port.postMessage(this.buf);
          this.buf = new Float32Array(1024);
          this.fill = 0;
        }
      }
    }
    return true;
  }
}
registerProcessor("${WORKLET_NAME}", WaCallCapture);
`

type AudioContextCtor = typeof AudioContext

function getAudioContextCtor(): AudioContextCtor | null {
	if (typeof window === "undefined") return null
	return window.AudioContext ?? (window as unknown as { webkitAudioContext?: AudioContextCtor }).webkitAudioContext ?? null
}

export class MicrophoneError extends Error {
	readonly reason: "unsupported" | "denied" | "not_found" | "failed"
	constructor(reason: MicrophoneError["reason"], message: string) {
		super(message)
		this.name = "MicrophoneError"
		this.reason = reason
	}
}

function describeGetUserMediaError(err: unknown): MicrophoneError {
	const name = (err as { name?: string } | null)?.name
	if (name === "NotAllowedError" || name === "SecurityError" || name === "PermissionDeniedError") {
		return new MicrophoneError("denied", "Microphone blocked. Allow microphone access to talk in this call.")
	}
	if (name === "NotFoundError" || name === "OverconstrainedError" || name === "DevicesNotFoundError") {
		return new MicrophoneError("not_found", "No microphone found.")
	}
	return new MicrophoneError("failed", "Could not start the microphone.")
}

/**
 * Plays 16 kHz mono frames back-to-back through an AudioContext with a short
 * jitter buffer: starts after PRIME_FRAMES, re-primes after an underrun and
 * drops frames when more than MAX_BUFFER_SEC would be queued.
 */
export class RemoteAudioPlayer {
	static readonly PRIME_FRAMES = 2
	static readonly MAX_BUFFER_SEC = 0.2

	private readonly ctx: AudioContext
	private readonly gain: GainNode
	private pending: Float32Array[] = []
	private primed = false
	private nextTime = 0
	private sources = new Set<AudioBufferSourceNode>()

	constructor(ctx: AudioContext, destination: AudioNode = ctx.destination) {
		this.ctx = ctx
		this.gain = ctx.createGain()
		this.gain.gain.value = 1
		this.gain.connect(destination)
	}

	setVolume(volume: number): void {
		this.gain.gain.value = Math.max(0, Math.min(1, volume))
	}

	push(frame: Float32Array): void {
		if (frame.length === 0) return
		const now = this.ctx.currentTime
		if (this.primed && this.nextTime < now + 0.005) {
			// underrun: play what we have, but rebuild a small cushion first
			this.primed = false
		}
		if (!this.primed) {
			this.pending.push(frame)
			if (this.pending.length < RemoteAudioPlayer.PRIME_FRAMES) return
			this.primed = true
			this.nextTime = now + 0.03
			const queued = this.pending
			this.pending = []
			for (const f of queued) this.schedule(f)
			return
		}
		if (this.nextTime - now > RemoteAudioPlayer.MAX_BUFFER_SEC) return // too far behind: skip this frame
		this.schedule(frame)
	}

	private schedule(frame: Float32Array): void {
		const buffer = this.ctx.createBuffer(1, frame.length, MEDIA_SAMPLE_RATE)
		buffer.copyToChannel(frame as Float32Array<ArrayBuffer>, 0)
		const src = this.ctx.createBufferSource()
		src.buffer = buffer
		src.connect(this.gain)
		src.onended = () => {
			this.sources.delete(src)
			src.disconnect()
		}
		const when = Math.max(this.nextTime, this.ctx.currentTime)
		src.start(when)
		this.sources.add(src)
		this.nextTime = when + frame.length / MEDIA_SAMPLE_RATE
	}

	close(): void {
		this.pending = []
		this.primed = false
		for (const s of this.sources) {
			try {
				s.onended = null
				s.stop()
				s.disconnect()
			} catch {
				// already stopped
			}
		}
		this.sources.clear()
		try {
			this.gain.disconnect()
		} catch {
			// ignore
		}
	}
}

type Capture = {
	stop: () => void
}

/**
 * Owns the AudioContext, the remote-audio player and (once started) the
 * microphone capture. Call `close()` to release everything.
 */
export class CallAudioEngine {
	private ctx: AudioContext | null = null
	private player: RemoteAudioPlayer | null = null
	private stream: MediaStream | null = null
	private capture: Capture | null = null
	private closed = false
	private muted = false
	private onGesture: (() => void) | null = null

	/** Creates the context + player. Safe to call without a user gesture; resumes on the next one. */
	open(): void {
		if (this.ctx || this.closed) return
		const Ctor = getAudioContextCtor()
		if (!Ctor) return
		const ctx = new Ctor({ latencyHint: "interactive" })
		this.ctx = ctx
		this.player = new RemoteAudioPlayer(ctx)
		const resume = () => {
			if (ctx.state === "suspended") ctx.resume().catch(() => {})
		}
		resume()
		this.onGesture = resume
		window.addEventListener("pointerdown", resume, true)
		window.addEventListener("keydown", resume, true)
		ctx.addEventListener("statechange", () => {
			if (ctx.state === "suspended" && !this.closed) resume()
		})
	}

	playRemote(frame: Float32Array): void {
		this.player?.push(frame)
	}

	setSpeakerOn(on: boolean): void {
		this.player?.setVolume(on ? 1 : 0)
	}

	setMuted(muted: boolean): void {
		this.muted = muted
		this.stream?.getAudioTracks().forEach((t) => {
			t.enabled = !muted
		})
	}

	/**
	 * Starts the microphone. `onFrame` receives 960-sample 16 kHz mono frames in
	 * real time. Rejects with a MicrophoneError.
	 */
	async startMicrophone(onFrame: (frame: Float32Array) => void): Promise<void> {
		if (this.capture || this.closed) return
		this.open()
		const ctx = this.ctx
		if (!ctx || !navigator.mediaDevices?.getUserMedia) {
			throw new MicrophoneError("unsupported", "Microphone is not available here (needs a secure context).")
		}
		let stream: MediaStream
		try {
			stream = await navigator.mediaDevices.getUserMedia({
				audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
				video: false,
			})
		} catch (err) {
			throw describeGetUserMediaError(err)
		}
		if (this.closed) {
			stream.getTracks().forEach((t) => t.stop())
			return
		}
		this.stream = stream
		stream.getAudioTracks().forEach((t) => {
			t.enabled = !this.muted
		})
		try {
			if (ctx.state === "suspended") await ctx.resume().catch(() => {})
			const framer = new PcmFramer(ctx.sampleRate)
			const feed = (samples: Float32Array) => {
				if (this.muted) return
				framer.push(samples, onFrame)
			}
			this.capture = await this.createCapture(ctx, stream, feed)
			if (this.closed) this.stopCapture()
		} catch (err) {
			stream.getTracks().forEach((t) => t.stop())
			this.stream = null
			throw err instanceof MicrophoneError ? err : new MicrophoneError("failed", "Could not start the microphone.")
		}
	}

	private async createCapture(
		ctx: AudioContext,
		stream: MediaStream,
		feed: (samples: Float32Array) => void,
	): Promise<Capture> {
		const source = ctx.createMediaStreamSource(stream)
		const sink = ctx.createGain() // silent: keeps the capture node pulled by the graph
		sink.gain.value = 0
		sink.connect(ctx.destination)

		if (ctx.audioWorklet && typeof AudioWorkletNode !== "undefined") {
			const loaded = await this.loadWorklet(ctx)
			if (loaded) {
				const node = new AudioWorkletNode(ctx, WORKLET_NAME, { numberOfInputs: 1, numberOfOutputs: 1, channelCount: 1, channelCountMode: "explicit" })
				node.port.onmessage = (e: MessageEvent<Float32Array>) => feed(e.data)
				source.connect(node)
				node.connect(sink)
				return {
					stop: () => {
						node.port.onmessage = null
						try {
							source.disconnect()
							node.disconnect()
							sink.disconnect()
						} catch {
							// ignore
						}
					},
				}
			}
		}

		// Fallback for engines where the worklet module cannot be loaded.
		const proc = ctx.createScriptProcessor(2048, 1, 1)
		proc.onaudioprocess = (e) => feed(new Float32Array(e.inputBuffer.getChannelData(0)))
		source.connect(proc)
		proc.connect(sink)
		return {
			stop: () => {
				proc.onaudioprocess = null
				try {
					source.disconnect()
					proc.disconnect()
					sink.disconnect()
				} catch {
					// ignore
				}
			},
		}
	}

	private async loadWorklet(ctx: AudioContext): Promise<boolean> {
		const urls: Array<{ url: string; revoke?: boolean }> = []
		try {
			urls.push({ url: URL.createObjectURL(new Blob([WORKLET_SOURCE], { type: "text/javascript" })), revoke: true })
		} catch {
			// no Blob URLs
		}
		urls.push({ url: `data:text/javascript;base64,${btoa(WORKLET_SOURCE)}` })
		for (const { url, revoke } of urls) {
			try {
				await ctx.audioWorklet.addModule(url)
				if (revoke) URL.revokeObjectURL(url)
				return true
			} catch {
				if (revoke) URL.revokeObjectURL(url)
			}
		}
		return false
	}

	private stopCapture(): void {
		this.capture?.stop()
		this.capture = null
	}

	close(): void {
		if (this.closed) return
		this.closed = true
		this.stopCapture()
		this.stream?.getTracks().forEach((t) => t.stop())
		this.stream = null
		this.player?.close()
		this.player = null
		if (this.onGesture) {
			window.removeEventListener("pointerdown", this.onGesture, true)
			window.removeEventListener("keydown", this.onGesture, true)
			this.onGesture = null
		}
		const ctx = this.ctx
		this.ctx = null
		if (ctx && ctx.state !== "closed") ctx.close().catch(() => {})
	}
}
