import { useCallback, useEffect, useRef, useState } from "react"

const FRAME_KIND_OUTGOING_AUDIO = 0x01
const FRAME_KIND_OUTGOING_VIDEO = 0x02
const FRAME_KIND_INCOMING_AUDIO = 0x03
const FRAME_KIND_INCOMING_VIDEO = 0x04
const FRAME_KIND_KEYFRAME = 0x05

function encodeFrame(kind: number, metadata: Uint8Array | null, payload: Uint8Array): Uint8Array {
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

function decodeFrame(data: ArrayBuffer | Uint8Array): { kind: number; metadata: Uint8Array; payload: Uint8Array } | null {
	const u8 = data instanceof Uint8Array ? data : new Uint8Array(data)
	if (u8.length < 3) return null
	const kind = u8[0]
	const metaLen = (u8[1] << 8) | u8[2]
	if (u8.length < 3 + metaLen) return null
	const metadata = u8.slice(3, 3 + metaLen)
	const payload = u8.slice(3 + metaLen)
	return { kind, metadata, payload }
}

function float32ToS16LE(floats: Float32Array): Uint8Array {
	const out = new Uint8Array(floats.length * 2)
	const view = new DataView(out.buffer)
	for (let i = 0; i < floats.length; i++) {
		let s = Math.max(-1, Math.min(1, floats[i]))
		s = s < 0 ? s * 0x8000 : s * 0x7fff
		view.setInt16(i * 2, s, true)
	}
	return out
}

function s16LEToFloat32(u8: Uint8Array): Float32Array {
	const len = Math.floor(u8.length / 2)
	const out = new Float32Array(len)
	const view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength)
	for (let i = 0; i < len; i++) {
		const s = view.getInt16(i * 2, true)
		out[i] = s / 0x8000
	}
	return out
}

function getMediaWsUrl(callId: string, token?: string): string {
	const envUrl = import.meta.env.VITE_WS_URL as string | undefined
	let base: string
	if (envUrl) {
		// envUrl is like ws://host/ws — replace suffix
		base = envUrl.replace(/\/ws\/?$/, `/ws/calls/${encodeURIComponent(callId)}/media`)
	} else if (typeof window !== "undefined") {
		const protocol = window.location.protocol === "https:" ? "wss:" : "ws:"
		const port = (window as unknown as { __BACKEND_PORT__?: number }).__BACKEND_PORT__
		if (port) base = `${protocol}//localhost:${port}/ws/calls/${encodeURIComponent(callId)}/media`
		else {
			const origin = window.location.origin
			if (origin && origin !== "null" && origin !== "file://") {
				base = `${protocol}//${window.location.host}/ws/calls/${encodeURIComponent(callId)}/media`
			} else {
				base = `${protocol}//localhost:8080/ws/calls/${encodeURIComponent(callId)}/media`
			}
		}
	} else {
		base = `/ws/calls/${encodeURIComponent(callId)}/media`
	}
	if (token) {
		const sep = base.includes("?") ? "&" : "?"
		return `${base}${sep}token=${encodeURIComponent(token)}`
	}
	return base
}

export type UseCallMediaOptions = {
	onIncomingAudio?: (frame: Float32Array) => void
	onIncomingVideo?: (accessUnit: Uint8Array, participantJid?: string) => void
	onKeyframeRequest?: () => void
}

export type UseCallMediaReturn = {
	isMediaConnected: boolean
	isMuted: boolean
	mediaStream: MediaStream | null
	connectMedia: (callId: string, token?: string) => void
	disconnectMedia: () => void
	setMuted: (muted: boolean) => void
	toggleMute: () => void
	sendAudioFrame: (frame: Float32Array) => boolean
	sendVideoFrame: (accessUnit: Uint8Array) => boolean
}

export function useCallMedia(options: UseCallMediaOptions = {}): UseCallMediaReturn {
	const [isMediaConnected, setIsMediaConnected] = useState(false)
	const [isMuted, setIsMuted] = useState(false)
	const [mediaStream] = useState<MediaStream | null>(null)

	const wsRef = useRef<WebSocket | null>(null)
	const isMutedRef = useRef(false)
	const onAudioRef = useRef(options.onIncomingAudio)
	const onVideoRef = useRef(options.onIncomingVideo)
	const onKeyframeRef = useRef(options.onKeyframeRequest)

	useEffect(() => {
		onAudioRef.current = options.onIncomingAudio
		onVideoRef.current = options.onIncomingVideo
		onKeyframeRef.current = options.onKeyframeRequest
	}, [options.onIncomingAudio, options.onIncomingVideo, options.onKeyframeRequest])

	const disconnectMedia = useCallback(() => {
		if (wsRef.current) {
			try {
				wsRef.current.close(1000, "disconnectMedia")
			} catch {
				// ignore
			}
			wsRef.current = null
		}
		setIsMediaConnected(false)
	}, [])

	const connectMedia = useCallback(
		(callId: string, token?: string) => {
			disconnectMedia()
			const url = getMediaWsUrl(callId, token)
			const ws = new WebSocket(url)
			ws.binaryType = "arraybuffer"
			wsRef.current = ws

			ws.onopen = () => {
				setIsMediaConnected(true)
			}

			ws.onclose = () => {
				if (wsRef.current === ws) {
					wsRef.current = null
					setIsMediaConnected(false)
				}
			}

			ws.onerror = () => {
				// let onclose handle state
			}

			ws.onmessage = (event: MessageEvent) => {
				const raw = event.data as ArrayBuffer | string
				if (typeof raw === "string") {
					// control JSON (unlikely for media path) — ignore
					return
				}
				const decoded = decodeFrame(raw as ArrayBuffer)
				if (!decoded) return
				const { kind, metadata, payload } = decoded
				switch (kind) {
					case FRAME_KIND_INCOMING_AUDIO: {
						if (onAudioRef.current) {
							const floats = s16LEToFloat32(payload)
							onAudioRef.current(floats)
						}
						break
					}
					case FRAME_KIND_INCOMING_VIDEO: {
						if (onVideoRef.current) {
							let participantJid: string | undefined
							if (metadata.length > 0) {
								try {
									const meta = JSON.parse(new TextDecoder().decode(metadata)) as { participant_jid?: string }
									participantJid = meta.participant_jid
								} catch {
									// ignore bad metadata
								}
							}
							onVideoRef.current(payload, participantJid)
						}
						break
					}
					case FRAME_KIND_KEYFRAME: {
						onKeyframeRef.current?.()
						break
					}
					default:
						break
				}
			}
		},
		[disconnectMedia],
	)

	const setMuted = useCallback((muted: boolean) => {
		isMutedRef.current = muted
		setIsMuted(muted)
	}, [])

	const toggleMute = useCallback(() => {
		setMuted(!isMutedRef.current)
	}, [setMuted])

	const sendAudioFrame = useCallback(
		(frame: Float32Array): boolean => {
			if (isMutedRef.current) return false
			const ws = wsRef.current
			if (!ws || ws.readyState !== WebSocket.OPEN) return false
			try {
				const pcm = float32ToS16LE(frame)
				const encoded = encodeFrame(FRAME_KIND_OUTGOING_AUDIO, null, pcm)
				ws.send(encoded)
				return true
			} catch {
				return false
			}
		},
		[],
	)

	const sendVideoFrame = useCallback((accessUnit: Uint8Array): boolean => {
		const ws = wsRef.current
		if (!ws || ws.readyState !== WebSocket.OPEN) return false
		try {
			const encoded = encodeFrame(FRAME_KIND_OUTGOING_VIDEO, null, accessUnit)
			ws.send(encoded)
			return true
		} catch {
			return false
		}
	}, [])

	useEffect(() => {
		return () => {
			disconnectMedia()
		}
	}, [disconnectMedia])

	return {
		isMediaConnected,
		isMuted,
		mediaStream,
		connectMedia,
		disconnectMedia,
		setMuted,
		toggleMute,
		sendAudioFrame,
		sendVideoFrame,
	}
}
