import { useCallback, useEffect, useRef, useState } from "react"
import { api } from "@/lib/api"
import {
	FRAME_KIND_INCOMING_AUDIO,
	FRAME_KIND_INCOMING_VIDEO,
	FRAME_KIND_KEYFRAME,
	FRAME_KIND_OUTGOING_AUDIO,
	FRAME_KIND_OUTGOING_VIDEO,
	buildMediaWsUrl,
	decodeFrame,
	encodeFrame,
	float32ToS16LE,
	getBackendWsOrigin,
	parseVideoMetadata,
	s16LEToFloat32,
} from "@/lib/callMediaCodec"

// A dropped media socket never hangs up the call: ask for a new token and retry.
const RETRY_DELAYS_MS = [700, 1800]
// A connection that stayed up this long counts as healthy (retry budget resets).
const HEALTHY_AFTER_MS = 5000
const MAX_AUDIO_BACKLOG_BYTES = 64 * 1024
const MAX_VIDEO_BACKLOG_BYTES = 512 * 1024

export type IncomingVideoMeta = { participantJid?: string; orientation: number }

type ConnectionHandlers = {
	onOpen: () => void
	onClosed: () => void
	onGaveUp: () => void
	onAudio: (frame: Float32Array) => void
	onVideo: (jpeg: Uint8Array, meta: IncomingVideoMeta) => void
}

/** One media WebSocket for one call, with token-per-connect and bounded reconnects. */
class MediaConnection {
	private ws: WebSocket | null = null
	private timer: number | null = null
	private failures = 0
	private openedAt = 0
	private closed = false
	private readonly callId: string
	private readonly handlers: ConnectionHandlers

	constructor(callId: string, handlers: ConnectionHandlers) {
		this.callId = callId
		this.handlers = handlers
	}

	start(): void {
		void this.connect()
	}

	private async connect(): Promise<void> {
		if (this.closed) return
		let url: string
		try {
			const session = await api.createCallMediaSession(this.callId)
			if (this.closed) return
			url = buildMediaWsUrl(getBackendWsOrigin(), session.ws_path, session.token)
		} catch {
			this.retryOrGiveUp()
			return
		}

		let ws: WebSocket
		try {
			ws = new WebSocket(url)
		} catch {
			this.retryOrGiveUp()
			return
		}
		ws.binaryType = "arraybuffer"
		this.ws = ws

		ws.onopen = () => {
			if (this.closed || this.ws !== ws) return
			this.openedAt = Date.now()
			this.handlers.onOpen()
		}
		ws.onerror = () => {
			// onclose follows and drives the retry
		}
		ws.onclose = () => {
			if (this.ws !== ws) return
			this.ws = null
			if (this.closed) return
			if (this.openedAt && Date.now() - this.openedAt >= HEALTHY_AFTER_MS) this.failures = 0
			this.openedAt = 0
			this.handlers.onClosed()
			this.retryOrGiveUp()
		}
		ws.onmessage = (event: MessageEvent) => {
			if (this.closed || this.ws !== ws || typeof event.data === "string") return
			const frame = decodeFrame(event.data as ArrayBuffer)
			if (!frame) return
			switch (frame.kind) {
				case FRAME_KIND_INCOMING_AUDIO:
					this.handlers.onAudio(s16LEToFloat32(frame.payload))
					break
				case FRAME_KIND_INCOMING_VIDEO:
					if (frame.payload.length > 0) this.handlers.onVideo(frame.payload, parseVideoMetadata(frame.metadata))
					break
				case FRAME_KIND_KEYFRAME:
					break // JPEG mode: nothing to do
				default:
					break
			}
		}
	}

	private retryOrGiveUp(): void {
		if (this.closed) return
		if (this.failures >= RETRY_DELAYS_MS.length) {
			this.handlers.onGaveUp()
			return
		}
		const delay = RETRY_DELAYS_MS[this.failures]
		this.failures++
		this.timer = window.setTimeout(() => {
			this.timer = null
			void this.connect()
		}, delay)
	}

	send(kind: number, payload: Uint8Array, maxBacklog: number): boolean {
		const ws = this.ws
		if (!ws || ws.readyState !== WebSocket.OPEN || ws.bufferedAmount > maxBacklog) return false
		try {
			ws.send(encodeFrame(kind, null, payload) as Uint8Array<ArrayBuffer>)
			return true
		} catch {
			return false
		}
	}

	close(): void {
		this.closed = true
		if (this.timer !== null) window.clearTimeout(this.timer)
		this.timer = null
		const ws = this.ws
		this.ws = null
		if (ws) {
			ws.onopen = ws.onclose = ws.onerror = ws.onmessage = null
			try {
				ws.close(1000, "disconnectMedia")
			} catch {
				// ignore
			}
		}
	}
}

export type UseCallMediaOptions = {
	onIncomingAudio?: (frame: Float32Array) => void
	onIncomingVideo?: (jpeg: Uint8Array, meta: IncomingVideoMeta) => void
}

export type UseCallMediaReturn = {
	isMediaConnected: boolean
	/** Set when the media socket could not be (re)established. */
	mediaError: string | null
	isMuted: boolean
	connectMedia: (callId: string) => void
	disconnectMedia: () => void
	setMuted: (muted: boolean) => void
	toggleMute: () => void
	/** 960-sample 16 kHz mono frame. Returns false when muted/not connected. */
	sendAudioFrame: (frame: Float32Array) => boolean
	/** One complete JPEG. Returns false when not connected or the socket is backed up. */
	sendVideoFrame: (jpeg: Uint8Array) => boolean
}

export function useCallMedia(options: UseCallMediaOptions = {}): UseCallMediaReturn {
	const [isMediaConnected, setIsMediaConnected] = useState(false)
	const [mediaError, setMediaError] = useState<string | null>(null)
	const [isMuted, setIsMuted] = useState(false)

	const connRef = useRef<MediaConnection | null>(null)
	const isMutedRef = useRef(false)
	const onAudioRef = useRef(options.onIncomingAudio)
	const onVideoRef = useRef(options.onIncomingVideo)

	useEffect(() => {
		onAudioRef.current = options.onIncomingAudio
		onVideoRef.current = options.onIncomingVideo
	}, [options.onIncomingAudio, options.onIncomingVideo])

	const disconnectMedia = useCallback(() => {
		connRef.current?.close()
		connRef.current = null
		isMutedRef.current = false // every call starts unmuted
		setIsMuted(false)
		setIsMediaConnected(false)
		setMediaError(null)
	}, [])

	const connectMedia = useCallback((callId: string) => {
		connRef.current?.close()
		setMediaError(null)
		const conn: MediaConnection = new MediaConnection(callId, {
			onOpen: () => {
				if (connRef.current !== conn) return
				setIsMediaConnected(true)
				setMediaError(null)
			},
			onClosed: () => {
				if (connRef.current === conn) setIsMediaConnected(false)
			},
			onGaveUp: () => {
				if (connRef.current !== conn) return
				setIsMediaConnected(false)
				setMediaError("Audio connection lost")
			},
			onAudio: (frame) => onAudioRef.current?.(frame),
			onVideo: (jpeg, meta) => onVideoRef.current?.(jpeg, meta),
		})
		connRef.current = conn
		conn.start()
	}, [])

	const setMuted = useCallback((muted: boolean) => {
		isMutedRef.current = muted
		setIsMuted(muted)
	}, [])

	const toggleMute = useCallback(() => {
		setMuted(!isMutedRef.current)
	}, [setMuted])

	const sendAudioFrame = useCallback((frame: Float32Array): boolean => {
		if (isMutedRef.current) return false
		return connRef.current?.send(FRAME_KIND_OUTGOING_AUDIO, float32ToS16LE(frame), MAX_AUDIO_BACKLOG_BYTES) ?? false
	}, [])

	const sendVideoFrame = useCallback((jpeg: Uint8Array): boolean => {
		return connRef.current?.send(FRAME_KIND_OUTGOING_VIDEO, jpeg, MAX_VIDEO_BACKLOG_BYTES) ?? false
	}, [])

	useEffect(() => {
		return () => {
			connRef.current?.close()
			connRef.current = null
		}
	}, [])

	return {
		isMediaConnected,
		mediaError,
		isMuted,
		connectMedia,
		disconnectMedia,
		setMuted,
		toggleMute,
		sendAudioFrame,
		sendVideoFrame,
	}
}
