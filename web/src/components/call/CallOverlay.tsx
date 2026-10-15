import { useEffect, useMemo, useRef, useState } from "react"
import { useCall } from "@/contexts/CallContext"
import { useCallMedia } from "@/hooks/useCallMedia"
import { Button } from "@/components/ui/button"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { VideoStage } from "@/components/call/VideoStage"
import { CallAudioEngine } from "@/lib/callAudio"
import { CameraSender, RemoteVideoRenderer } from "@/lib/callVideo"
import { PhoneOff, Mic, MicOff, Volume2, VolumeX, Video, VideoOff, Loader2 } from "lucide-react"
import { cn } from "@/lib/utils"

const ACTIVE_STATUSES = new Set(["preparing", "initiating", "ringing", "connecting", "connected", "ending"])

function initials(label: string) {
	const clean = label.replace(/@.*$/, "").replace(/[^a-zA-Z0-9]/g, " ").trim()
	if (!clean) return "WA"
	const parts = clean.split(/\s+/).filter(Boolean)
	if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
	return (parts[0][0] + parts[1][0]).toUpperCase()
}

function displayName(jid: string) {
	const s = jid.split("@")[0] ?? jid
	if (/^\d+$/.test(s) && s.length > 7) return s.replace(/(\d{3})(\d{3})(\d+)/, "$1 $2 $3")
	return s
}

function formatDuration(ms: number) {
	const total = Math.max(0, Math.floor(ms / 1000))
	const m = Math.floor(total / 60)
	const s = total % 60
	return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
}

export function CallOverlay() {
	const { activeCall, hangupCall, startVideo, stopVideo } = useCall()
	const [speakerOn, setSpeakerOn] = useState(true)
	const [ending, setEnding] = useState(false)
	const [cameraBusy, setCameraBusy] = useState(false)
	const [now, setNow] = useState(() => Date.now())
	const [audioError, setAudioError] = useState<string | null>(null)
	const [cameraError, setCameraError] = useState<string | null>(null)
	const [localStream, setLocalStream] = useState<MediaStream | null>(null)
	const [remoteRenderer] = useState(() => new RemoteVideoRenderer())
	const engineRef = useRef<CallAudioEngine | null>(null)
	const media = useCallMedia({
		onIncomingAudio: (frame) => engineRef.current?.playRemote(frame),
		onIncomingVideo: (jpeg, meta) => remoteRenderer.push(jpeg, meta.orientation),
	})
	const { connectMedia, disconnectMedia, sendAudioFrame, sendVideoFrame, isMuted } = media

	const call = activeCall
	const visible = useMemo(() => {
		if (!call) return false
		return ACTIVE_STATUSES.has(call.status)
	}, [call])

	const connected = call?.status === "connected" || call?.status === "connecting"
	const isVideo = call?.type === "video" || call?.type === "group_video"
	const isGroup = call?.type === "group_audio" || call?.type === "group_video"
	const cameraOn = call?.video_enabled || isVideo
	const remoteOn = call?.remote_video_enabled || false

	// timer
	useEffect(() => {
		if (!visible) return
		setNow(Date.now())
		const i = window.setInterval(() => setNow(Date.now()), 1000)
		return () => window.clearInterval(i)
	}, [visible])

	// Media session: audio engine (mic + playback) and the media socket live as long
	// as the call is answered/connecting for this call id.
	const callId = call?.id ?? null
	const sessionCallId = visible && connected ? callId : null
	useEffect(() => {
		if (!sessionCallId) return
		const engine = new CallAudioEngine()
		engine.open()
		engineRef.current = engine
		engine.startMicrophone(sendAudioFrame).catch((err: unknown) => {
			if (engineRef.current === engine) setAudioError(err instanceof Error ? err.message : "Could not start the microphone.")
		})
		connectMedia(sessionCallId)
		return () => {
			engine.close()
			if (engineRef.current === engine) engineRef.current = null
			disconnectMedia()
			setAudioError(null)
		}
	}, [sessionCallId, connectMedia, disconnectMedia, sendAudioFrame])

	useEffect(() => {
		engineRef.current?.setMuted(isMuted)
	}, [isMuted, sessionCallId])

	useEffect(() => {
		engineRef.current?.setSpeakerOn(speakerOn)
	}, [speakerOn, sessionCallId])

	// Camera: capture and send JPEG frames while video is on during a live call.
	const wantCamera = !!sessionCallId && !!cameraOn
	useEffect(() => {
		if (!wantCamera) return
		let cancelled = false
		const sender = new CameraSender()
		sender
			.start(sendVideoFrame)
			.then((stream) => {
				if (!cancelled) setLocalStream(stream)
			})
			.catch((err: unknown) => {
				if (!cancelled) setCameraError(err instanceof Error ? err.message : "Could not start the camera.")
			})
		return () => {
			cancelled = true
			sender.stop()
			setLocalStream(null)
			setCameraError(null)
		}
	}, [wantCamera, sendVideoFrame])

	// Forget the last remote picture when the remote camera goes off or the call ends.
	useEffect(() => {
		if (!remoteOn || !sessionCallId) remoteRenderer.reset()
	}, [remoteOn, sessionCallId, remoteRenderer])

	if (!visible || !call) return null

	const startTs = call.answered_at ?? call.started_at ?? now
	const duration = connected ? now - startTs : 0

	const statusLabel: Record<string, string> = {
		preparing: "Preparing…",
		initiating: "Calling…",
		ringing: "Ringing…",
		connecting: "Connecting…",
		connected: formatDuration(duration),
		ending: "Ending…",
	}

	const handleEnd = async () => {
		if (ending) return
		setEnding(true)
		try {
			await hangupCall(call.id)
		} finally {
			setEnding(false)
		}
	}

	const handleCamera = async () => {
		if (cameraBusy) return
		setCameraBusy(true)
		try {
			if (cameraOn) await stopVideo(call.id)
			else await startVideo(call.id)
		} finally {
			setCameraBusy(false)
		}
	}

	const showVideo = cameraOn || remoteOn

	return (
		<div
			role="dialog"
			aria-modal="true"
			aria-label="Active call"
			className="fixed inset-0 z-[90] flex flex-col"
		>
			{/* backdrop */}
			<div className="absolute inset-0 bg-[#0a0a0a]/85 backdrop-blur-[14px]" />

			<div className="relative flex h-full flex-col">
				{/* header */}
				<div className="flex items-center justify-between px-5 pt-5">
					<div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 backdrop-blur">
						<span className={cn("h-2 w-2 rounded-full animate-pulse", connected ? "bg-emerald-500" : "bg-amber-400")} />
						<span className="text-[11px] font-semibold tracking-widest uppercase text-foreground/70">
							{isVideo ? "Video call" : "Voice call"}
						</span>
					</div>
					{media.isMediaConnected && (
						<span className="flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-2.5 py-1 text-[10px] font-medium text-foreground/60 backdrop-blur">
							<span className="h-1.5 w-1.5 rounded-full bg-emerald-400" /> Media linked
						</span>
					)}
				</div>

				{(audioError || cameraError || media.mediaError) && (
					<div className="mx-auto mt-3 flex max-w-[90%] flex-wrap items-center justify-center gap-2 rounded-full border border-amber-400/30 bg-amber-500/15 px-3.5 py-1.5 text-xs font-medium text-amber-200 backdrop-blur">
						<span>{audioError ?? cameraError ?? media.mediaError}</span>
						{!audioError && !cameraError && media.mediaError && (
							<button
								type="button"
								className="rounded-full bg-white/10 px-2 py-0.5 text-[11px] hover:bg-white/20"
								onClick={() => connectMedia(call.id)}
							>
								Retry
							</button>
						)}
					</div>
				)}

				<div className="flex-1 min-h-0 p-4 sm:p-6 flex flex-col items-center justify-center gap-5">
					{showVideo ? (
						<div className="w-full max-w-4xl flex-1 min-h-0">
							<VideoStage
								callId={call.id}
								localEnabled={cameraOn}
								remoteEnabled={remoteOn}
								targetLabel={call.target}
								localStream={localStream}
								remoteRenderer={remoteRenderer}
								className="h-full"
							/>
						</div>
					) : isGroup ? (
						<div className="flex flex-col items-center text-center w-full max-w-md">
							<div className="relative mb-5">
								<div className="absolute inset-0 rounded-full bg-primary/20 animate-ping [animation-duration:2.4s]" />
								<Avatar className="relative h-24 w-24 border-[3px] border-white/15 shadow-xl">
									<AvatarFallback className="bg-zinc-900 text-white text-2xl font-semibold tracking-tight">
										{call.group_jid ? initials(call.group_jid) : "GRP"}
									</AvatarFallback>
								</Avatar>
							</div>
							<h2 className="text-2xl font-semibold tracking-tight leading-none">
								{call.group_jid ? displayName(call.group_jid) : "Group call"}
							</h2>
							<p className="mt-1 text-xs text-muted-foreground truncate max-w-[28ch]">
								{call.participants?.length ?? 0} participants
							</p>
							{call.participants && call.participants.length > 0 && (
								<div className="mt-4 flex flex-wrap items-center justify-center gap-2">
									{call.participants.map((p) => (
										<span
											key={p}
											className="inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] font-medium text-foreground/80 backdrop-blur"
										>
											<span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
											{displayName(p)}
										</span>
									))}
								</div>
							)}
						</div>
					) : (
						<div className="flex flex-col items-center text-center">
							<div className="relative mb-5">
								<div className="absolute inset-0 rounded-full bg-primary/20 animate-ping [animation-duration:2.4s]" />
								<Avatar className="relative h-28 w-28 border-[3px] border-white/15 shadow-xl">
									<AvatarFallback className="bg-zinc-900 text-white text-3xl font-semibold tracking-tight">
										{initials(call.target)}
									</AvatarFallback>
								</Avatar>
							</div>
							<h2 className="text-2xl font-semibold tracking-tight leading-none">{displayName(call.target)}</h2>
							<p className="mt-1 text-xs text-muted-foreground truncate max-w-[28ch]">{call.target}</p>
						</div>
					)}
				</div>

				{/* status + controls */}
				<div className="relative flex flex-col items-center gap-6 px-6 pb-8 pt-2">
					<div className="flex items-center gap-2 text-foreground/70">
						{connected ? (
							<span className="text-lg font-medium tabular-nums tracking-tight">{statusLabel[call.status]}</span>
						) : (
							<span className="flex items-center gap-2 text-sm font-medium">
								<Loader2 className="h-4 w-4 animate-spin" />
								{statusLabel[call.status] ?? call.status}
							</span>
						)}
					</div>

					<div className="flex items-end justify-center gap-5">
						<div className="flex flex-col items-center gap-2">
							<Button
								aria-label={media.isMuted ? "Unmute" : "Mute"}
								onClick={media.toggleMute}
								variant="outline"
								className={cn(
									"h-[60px] w-[60px] rounded-full border-white/15 bg-white/10 hover:bg-white/15 text-foreground",
									media.isMuted && "bg-amber-500/80 hover:bg-amber-500 border-transparent text-white"
								)}
							>
								{media.isMuted ? <MicOff className="h-5 w-5" /> : <Mic className="h-5 w-5" />}
							</Button>
							<span className="text-[11px] font-medium text-muted-foreground">{media.isMuted ? "Unmute" : "Mute"}</span>
						</div>

						<div className="flex flex-col items-center gap-2">
							<Button
								aria-label={speakerOn ? "Speaker off" : "Speaker on"}
								onClick={() => setSpeakerOn((v) => !v)}
								variant="outline"
								className={cn(
									"h-[60px] w-[60px] rounded-full border-white/15 bg-white/10 hover:bg-white/15 text-foreground",
									!speakerOn && "bg-foreground/20 hover:bg-foreground/20 border-transparent"
								)}
							>
								{speakerOn ? <Volume2 className="h-5 w-5" /> : <VolumeX className="h-5 w-5" />}
							</Button>
							<span className="text-[11px] font-medium text-muted-foreground">Speaker</span>
						</div>

						{!isVideo && (
							<div className="flex flex-col items-center gap-2">
								<Button
									aria-label={cameraOn ? "Stop video" : "Start video"}
									onClick={handleCamera}
									disabled={cameraBusy}
									variant="outline"
									className={cn(
										"h-[60px] w-[60px] rounded-full border-white/15 bg-white/10 hover:bg-white/15 text-foreground",
										cameraOn && "bg-sky-500/80 hover:bg-sky-500 border-transparent text-white"
									)}
								>
									{cameraBusy ? <Loader2 className="h-5 w-5 animate-spin" /> : cameraOn ? <VideoOff className="h-5 w-5" /> : <Video className="h-5 w-5" />}
								</Button>
								<span className="text-[11px] font-medium text-muted-foreground">{cameraOn ? "Stop video" : "Video"}</span>
							</div>
						)}

						<div className="flex flex-col items-center gap-2">
							<Button
								aria-label="End call"
								onClick={handleEnd}
								disabled={ending}
								className="h-[68px] w-[68px] rounded-full bg-red-500 hover:bg-red-600 text-white shadow-[0_8px_24px_rgba(239,68,68,0.4)] active:scale-[0.98] transition-transform"
							>
								{ending ? <Loader2 className="h-6 w-6 animate-spin" /> : <PhoneOff className="h-6 w-6" />}
							</Button>
							<span className="text-[11px] font-medium tracking-wide text-muted-foreground">End</span>
						</div>
					</div>
				</div>
			</div>
		</div>
	)
}
