// Browser-side video for a call: camera -> JPEG frames for the backend, and a
// renderer that paints incoming JPEG frames (with rotation metadata) on a canvas.

const MAX_WIDTH = 640
const MAX_HEIGHT = 480
const FPS = 15
const START_QUALITY = 0.6
const MIN_QUALITY = 0.35
const MAX_FRAME_BYTES = 90_000

export class CameraError extends Error {
	constructor(message: string) {
		super(message)
		this.name = "CameraError"
	}
}

function describeCameraError(err: unknown): CameraError {
	const name = (err as { name?: string } | null)?.name
	if (name === "NotAllowedError" || name === "SecurityError" || name === "PermissionDeniedError") {
		return new CameraError("Camera blocked. Allow camera access to share video.")
	}
	if (name === "NotFoundError" || name === "OverconstrainedError" || name === "DevicesNotFoundError") {
		return new CameraError("No camera found.")
	}
	return new CameraError("Could not start the camera.")
}

/** Opens the camera and pushes ~15 fps JPEG frames (<= 640x480) to `send`. */
export class CameraSender {
	private stream: MediaStream | null = null
	private video: HTMLVideoElement | null = null
	private canvas: HTMLCanvasElement | null = null
	private timer: number | null = null
	private encoding = false
	private encodeStartedAt = 0
	private quality = START_QUALITY
	private stopped = false

	/** The live camera stream (for the local preview), or null before start/after stop. */
	get mediaStream(): MediaStream | null {
		return this.stream
	}

	async start(send: (jpeg: Uint8Array) => boolean): Promise<MediaStream> {
		if (!navigator.mediaDevices?.getUserMedia) {
			throw new CameraError("Camera is not available here (needs a secure context).")
		}
		let stream: MediaStream
		try {
			stream = await navigator.mediaDevices.getUserMedia({
				audio: false,
				video: {
					width: { ideal: MAX_WIDTH, max: MAX_WIDTH },
					height: { ideal: MAX_HEIGHT, max: MAX_HEIGHT },
					frameRate: { ideal: FPS, max: FPS },
				},
			})
		} catch (err) {
			throw describeCameraError(err)
		}
		if (this.stopped) {
			stream.getTracks().forEach((t) => t.stop())
			throw new CameraError("Camera stopped.")
		}
		this.stream = stream

		const video = document.createElement("video")
		video.muted = true
		video.playsInline = true
		video.srcObject = stream
		this.video = video
		this.canvas = document.createElement("canvas")
		video.play().catch(() => {})

		this.timer = window.setInterval(() => this.tick(send), Math.round(1000 / FPS))
		return stream
	}

	private tick(send: (jpeg: Uint8Array) => boolean): void {
		const video = this.video
		const canvas = this.canvas
		if (!video || !canvas || this.stopped) return
		// skip while a frame is still encoding, but never stay stuck if its callback is lost
		if (this.encoding && Date.now() - this.encodeStartedAt < 1000) return
		const vw = video.videoWidth
		const vh = video.videoHeight
		if (!vw || !vh || video.readyState < 2) return
		const scale = Math.min(1, MAX_WIDTH / vw, MAX_HEIGHT / vh)
		const w = Math.max(2, Math.round((vw * scale) / 2) * 2)
		const h = Math.max(2, Math.round((vh * scale) / 2) * 2)
		if (canvas.width !== w) canvas.width = w
		if (canvas.height !== h) canvas.height = h
		const ctx = canvas.getContext("2d")
		if (!ctx) return
		ctx.drawImage(video, 0, 0, w, h)
		this.encoding = true
		this.encodeStartedAt = Date.now()
		canvas.toBlob(
			(blob) => {
				if (!blob || this.stopped) {
					this.encoding = false
					return
				}
				blob
					.arrayBuffer()
					.then((buf) => {
						if (this.stopped) return
						if (buf.byteLength > MAX_FRAME_BYTES && this.quality > MIN_QUALITY) {
							this.quality = Math.max(MIN_QUALITY, this.quality - 0.05)
						} else if (buf.byteLength < MAX_FRAME_BYTES / 2 && this.quality < START_QUALITY) {
							this.quality = Math.min(START_QUALITY, this.quality + 0.02)
						}
						send(new Uint8Array(buf))
					})
					.finally(() => {
						this.encoding = false
					})
			},
			"image/jpeg",
			this.quality,
		)
	}

	stop(): void {
		this.stopped = true
		if (this.timer !== null) window.clearInterval(this.timer)
		this.timer = null
		this.stream?.getTracks().forEach((t) => t.stop())
		this.stream = null
		if (this.video) {
			this.video.pause()
			this.video.srcObject = null
			this.video = null
		}
		this.canvas = null
	}
}

/** Draws incoming JPEG frames onto an attached canvas, keeping only the newest. */
export class RemoteVideoRenderer {
	private canvas: HTMLCanvasElement | null = null
	private latest: { jpeg: Uint8Array; orientation: number } | null = null
	private decoding = false
	private active = false
	private idleTimer: number | null = null
	private listeners = new Set<(active: boolean) => void>()

	attach(canvas: HTMLCanvasElement | null): void {
		this.canvas = canvas
		if (!canvas) this.setActive(false)
	}

	/** Notified when frames start/stop arriving (stop = no frame for ~2.5 s). */
	subscribe(fn: (active: boolean) => void): () => void {
		this.listeners.add(fn)
		return () => {
			this.listeners.delete(fn)
		}
	}

	get hasFrames(): boolean {
		return this.active
	}

	push(jpeg: Uint8Array, orientation: number): void {
		if (!this.canvas) return
		this.latest = { jpeg, orientation }
		if (!this.decoding) void this.drain()
	}

	private async drain(): Promise<void> {
		this.decoding = true
		try {
			while (this.latest) {
				const { jpeg, orientation } = this.latest
				this.latest = null
				let bitmap: ImageBitmap
				try {
					bitmap = await createImageBitmap(new Blob([jpeg as Uint8Array<ArrayBuffer>], { type: "image/jpeg" }))
				} catch {
					continue // corrupt frame: skip
				}
				const canvas = this.canvas
				if (!canvas) {
					bitmap.close()
					continue
				}
				const quarter = ((orientation % 4) + 4) % 4
				const swap = quarter % 2 === 1
				const cw = swap ? bitmap.height : bitmap.width
				const ch = swap ? bitmap.width : bitmap.height
				if (canvas.width !== cw) canvas.width = cw
				if (canvas.height !== ch) canvas.height = ch
				const ctx = canvas.getContext("2d")
				if (ctx) {
					ctx.save()
					ctx.clearRect(0, 0, cw, ch)
					ctx.translate(cw / 2, ch / 2)
					ctx.rotate((quarter * Math.PI) / 2)
					ctx.drawImage(bitmap, -bitmap.width / 2, -bitmap.height / 2)
					ctx.restore()
					this.markFrame()
				}
				bitmap.close()
			}
		} finally {
			this.decoding = false
		}
	}

	private markFrame(): void {
		this.setActive(true)
		if (this.idleTimer !== null) window.clearTimeout(this.idleTimer)
		this.idleTimer = window.setTimeout(() => this.setActive(false), 2500)
	}

	private setActive(active: boolean): void {
		if (!active && this.idleTimer !== null) {
			window.clearTimeout(this.idleTimer)
			this.idleTimer = null
		}
		if (this.active === active) return
		this.active = active
		this.listeners.forEach((fn) => fn(active))
	}

	/** Forget the current picture (e.g. remote camera turned off). */
	reset(): void {
		this.latest = null
		const canvas = this.canvas
		if (canvas) canvas.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height)
		this.setActive(false)
	}
}
