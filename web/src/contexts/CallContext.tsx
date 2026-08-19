import { createContext, useContext, useCallback, useEffect, useRef, useState, type ReactNode } from "react"
import { subscribeWS } from "@/lib/ws-bus"
import { api, type CallState, type CallType, type CallLog, type CallHistoryFilter } from "@/lib/api"
import type { WSMessage } from "@/hooks/use-websocket"

type VideoStatePayload = {
	kind: string
	id: string
	active: boolean
	orientation?: string
	raw?: unknown
	video_enabled: boolean
	remote_video_enabled: boolean
}

interface CallContextValue {
	activeCall: CallState | null
	incomingCall: CallState | null
	callHistory: CallLog[]
	peerAccepted: CallState | null
	videoState: VideoStatePayload | null
	videoUpgradeRequested: VideoStatePayload | null

	startCall: (target: string, type: CallType) => Promise<CallState>
	startGroupCall: (groupJid: string, participants: string[], type: CallType) => Promise<CallState>
	addParticipant: (id: string, targets: string[]) => Promise<void>
	ringParticipant: (id: string, target: string) => Promise<void>
	answerCall: (id: string) => Promise<void>
	rejectCall: (id: string) => Promise<void>
	hangupCall: (id: string) => Promise<void>
	getActiveCall: () => Promise<CallState | null>
	getHistory: (filter?: CallHistoryFilter) => Promise<CallLog[]>
	startVideo: (id: string) => Promise<void>
	acceptVideo: (id: string) => Promise<void>
	rejectVideo: (id: string) => Promise<void>
	stopVideo: (id: string) => Promise<void>
}

const CallContext = createContext<CallContextValue | null>(null)

const terminalStatuses = new Set(["ended", "rejected", "missed", "busy", "failed", "interrupted"])

function isCallStatePayload(v: unknown): v is CallState {
	return typeof v === "object" && v !== null && "id" in (v as Record<string, unknown>) && "status" in (v as Record<string, unknown>)
}

export function CallProvider({ children }: { children: ReactNode }) {
	const [activeCall, setActiveCall] = useState<CallState | null>(null)
	const [incomingCall, setIncomingCall] = useState<CallState | null>(null)
	const [callHistory, setCallHistory] = useState<CallLog[]>([])
	const [peerAccepted, setPeerAccepted] = useState<CallState | null>(null)
	const [videoState, setVideoState] = useState<VideoStatePayload | null>(null)
	const [videoUpgradeRequested, setVideoUpgradeRequested] = useState<VideoStatePayload | null>(null)
	const activeCallRef = useRef<CallState | null>(null)
	activeCallRef.current = activeCall

	const getActiveCall = useCallback(async (): Promise<CallState | null> => {
		const state = await api.getActiveCall()
		setActiveCall(state)
		if (state && state.direction === "incoming" && state.status === "ringing") {
			setIncomingCall(state)
		}
		return state
	}, [])

	const getHistory = useCallback(async (filter?: CallHistoryFilter): Promise<CallLog[]> => {
		const res = await api.getCallHistory(filter)
		setCallHistory(res.logs ?? [])
		return res.logs ?? []
	}, [])

	const startCall = useCallback(async (target: string, type: CallType): Promise<CallState> => {
		const state = await api.createCall({ target, type })
		setActiveCall(state)
		// optimistic: if outgoing, clear incoming
		if (state.direction === "outgoing") setIncomingCall(null)
		return state
	}, [])

	const startGroupCall = useCallback(async (groupJid: string, participants: string[], type: CallType): Promise<CallState> => {
		const state = await api.createGroupCall({ group_jid: groupJid, participants, type })
		setActiveCall(state)
		return state
	}, [])

	const addParticipant = useCallback(async (id: string, targets: string[]): Promise<void> => {
		await api.addCallParticipants(id, targets)
	}, [])

	const ringParticipant = useCallback(async (id: string, target: string): Promise<void> => {
		await api.ringCallParticipant(id, target)
	}, [])

	const answerCall = useCallback(async (id: string): Promise<void> => {
		await api.answerCall(id)
	}, [])

	const rejectCall = useCallback(async (id: string): Promise<void> => {
		await api.rejectCall(id)
	}, [])

	const hangupCall = useCallback(async (id: string): Promise<void> => {
		await api.hangupCall(id)
	}, [])

	const startVideo = useCallback(async (id: string): Promise<void> => {
		await api.startVideo(id)
	}, [])

	const acceptVideo = useCallback(async (id: string): Promise<void> => {
		await api.acceptVideo(id)
	}, [])

	const rejectVideo = useCallback(async (id: string): Promise<void> => {
		await api.rejectVideo(id)
	}, [])

	const stopVideo = useCallback(async (id: string): Promise<void> => {
		await api.stopVideo(id)
	}, [])

	// Initial fetch of active call on mount
	useEffect(() => {
		getActiveCall().catch(() => {
			// ignore — no active call or not logged in yet
		})
	}, [getActiveCall])

	// Subscribe to shared WS bus from AuthContext
	useEffect(() => {
		const unsub = subscribeWS((message: WSMessage) => {
			const t = message.type
			const p = message.payload as unknown

			switch (t) {
				case "call.incoming": {
					if (isCallStatePayload(p)) {
						setIncomingCall(p)
						setActiveCall(p)
					}
					break
				}
				case "call.state": {
					if (p == null) {
						// backend sends null when no active call
						break
					}
					if (isCallStatePayload(p)) {
						const st = p
						if (terminalStatuses.has(st.status)) {
							// terminal: clear active if it matches
							setActiveCall((prev) => (prev?.id === st.id ? null : prev))
							setIncomingCall((prev) => (prev?.id === st.id ? null : prev))
						} else {
							setActiveCall(st)
							if (st.status === "ringing" && st.direction === "incoming") {
								setIncomingCall(st)
							} else if (st.direction === "outgoing") {
								setIncomingCall(null)
							}
						}
					}
					break
				}
				case "call.ended": {
					// payload: { id, status, reason, state }
					const payload = p as { id?: string; status?: string; state?: CallState } | null
					if (payload?.state && isCallStatePayload(payload.state)) {
						const st = payload.state
						setActiveCall((prev) => (prev?.id === st.id || prev?.id === payload.id ? null : prev))
						setIncomingCall((prev) => (prev?.id === st.id || prev?.id === payload.id ? null : prev))
					} else if (payload?.id) {
						setActiveCall((prev) => (prev?.id === payload.id ? null : prev))
						setIncomingCall((prev) => (prev?.id === payload.id ? null : prev))
					} else {
						setActiveCall(null)
						setIncomingCall(null)
					}
					// refresh history lazily
					getHistory().catch(() => {})
					setPeerAccepted(null)
					break
				}
				case "call.peer_accepted": {
					if (isCallStatePayload(p)) {
						setPeerAccepted(p)
						setActiveCall(p)
					} else if (p && typeof p === "object") {
						setPeerAccepted(p as CallState)
					}
					break
				}
				case "call.ready": {
					if (isCallStatePayload(p)) {
						setActiveCall(p)
						setPeerAccepted(null)
						// clear incoming when connected
						setIncomingCall((prev) => (prev?.id === p.id ? null : prev))
					}
					break
				}
				case "call.video_state": {
					const vs = p as VideoStatePayload
					setVideoState(vs)
					// patch activeCall video flags if ids match
					if (vs?.id && activeCallRef.current?.id === vs.id) {
						setActiveCall((prev) =>
							prev ? { ...prev, video_enabled: vs.video_enabled, remote_video_enabled: vs.remote_video_enabled } : prev,
						)
					}
					break
				}
				case "call.video_upgrade_requested": {
					const vs = p as VideoStatePayload
					setVideoUpgradeRequested(vs)
					setVideoState(vs)
					if (vs?.id && activeCallRef.current?.id === vs.id) {
						setActiveCall((prev) =>
							prev ? { ...prev, video_enabled: vs.video_enabled, remote_video_enabled: vs.remote_video_enabled } : prev,
						)
					}
					break
				}
				case "call.group_state": {
					// payload: { id, participants: string[], state: CallState }
					const payload = p as { id?: string; participants?: string[]; state?: CallState } | null
					if (payload?.participants && payload.id && activeCallRef.current?.id === payload.id) {
						setActiveCall((prev) => (prev ? { ...prev, participants: payload.participants } : prev))
					} else if (payload?.state && isCallStatePayload(payload.state)) {
						setActiveCall(payload.state)
					}
					break
				}
				case "call.participant_join": {
					// payload: { id, target, state: CallState }
					const payload = p as { id?: string; target?: string; state?: CallState } | null
					if (payload?.state && isCallStatePayload(payload.state)) {
						setActiveCall(payload.state)
					} else if (payload?.target && payload.id && activeCallRef.current?.id === payload.id) {
						setActiveCall((prev) =>
							prev && !prev.participants?.includes(payload.target!)
								? { ...prev, participants: [...(prev.participants ?? []), payload.target!] }
								: prev,
						)
					}
					break
				}
				default:
					break
			}
		})
		return unsub
	}, [getHistory])

	const value: CallContextValue = {
		activeCall,
		incomingCall,
		callHistory,
		peerAccepted,
		videoState,
		videoUpgradeRequested,
		startCall,
		startGroupCall,
		addParticipant,
		ringParticipant,
		answerCall,
		rejectCall,
		hangupCall,
		getActiveCall,
		getHistory,
		startVideo,
		acceptVideo,
		rejectVideo,
		stopVideo,
	}

	return <CallContext.Provider value={value}>{children}</CallContext.Provider>
}

export function useCall(): CallContextValue {
	const ctx = useContext(CallContext)
	if (!ctx) throw new Error("useCall must be used within CallProvider")
	return ctx
}
