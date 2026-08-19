import { useEffect, useMemo, useRef, useState } from "react"
import { useCall } from "@/contexts/CallContext"
import { Button } from "@/components/ui/button"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Phone, PhoneOff, Video, Mic, X } from "lucide-react"
import { cn } from "@/lib/utils"

function initials(label: string) {
  const clean = label.replace(/@.*$/, "").replace(/[^a-zA-Z0-9]/g, " ").trim()
  if (!clean) return "WA"
  const parts = clean.split(/\s+/).filter(Boolean)
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
  return (parts[0][0] + parts[1][0]).toUpperCase()
}

function displayName(jid: string) {
  const s = jid.split("@")[0] ?? jid
  // pretty: if looks like phone, add spacing
  if (/^\d+$/.test(s) && s.length > 7) return s.replace(/(\d{3})(\d{3})(\d+)/, "$1 $2 $3")
  return s
}

export function IncomingCallOverlay() {
  const { incomingCall, activeCall, videoUpgradeRequested, answerCall, rejectCall, acceptVideo, rejectVideo } = useCall()
  const [answering, setAnswering] = useState(false)
  const [rejecting, setRejecting] = useState(false)
  const [videoBusy, setVideoBusy] = useState<"accept" | "reject" | null>(null)
  const audioRef = useRef<HTMLAudioElement | null>(null)

  // Only show when we have a ringing incoming call and not already in an active connected session that isn't this id
  const call = incomingCall
  // Hide if activeCall is already connected with same id but not ringing — after answer it clears incoming quickly
  const visible = useMemo(() => {
    if (!call) return false
    // backend may keep incomingCall even after answer for a moment; hide once activeCall status is connected/ending
    if (activeCall && activeCall.id === call.id && (activeCall.status === "connected" || activeCall.status === "connecting")) {
      // still show? no — CallOverlay takes over
      return false
    }
    return call.status === "ringing" && call.direction === "incoming"
  }, [call, activeCall])

  const isVideo = call?.type === "video" || call?.type === "group_video"
  const upgrade = videoUpgradeRequested && call && videoUpgradeRequested.id === call.id ? videoUpgradeRequested : null

  // ringtone: try to autoplay a subtle tone oscillator fallback + html audio
  useEffect(() => {
    if (!visible) {
      if (audioRef.current) {
        audioRef.current.pause()
        audioRef.current.remove()
        audioRef.current = null
      }
      return
    }
    // try HTML audio first (no file — use WebAudio beep loop so no 404)
    let ctx: AudioContext | null = null
    let interval: number | null = null
    let stopped = false

    const startBeep = async () => {
      try {
        ctx = new (window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)()
        if (ctx.state === "suspended") await ctx.resume()
        const loop = () => {
          if (stopped || !ctx) return
          const o = ctx.createOscillator()
          const g = ctx.createGain()
          o.type = "sine"
          o.frequency.value = 420
          g.gain.value = 0
          o.connect(g)
          g.connect(ctx.destination)
          const now = ctx.currentTime
          g.gain.setValueAtTime(0, now)
          g.gain.linearRampToValueAtTime(0.12, now + 0.02)
          g.gain.exponentialRampToValueAtTime(0.01, now + 0.55)
          o.start(now)
          o.stop(now + 0.6)
          // second beep
          setTimeout(() => {
            if (stopped || !ctx) return
            const o2 = ctx.createOscillator()
            const g2 = ctx.createGain()
            o2.type = "sine"
            o2.frequency.value = 420
            g2.gain.value = 0
            o2.connect(g2)
            g2.connect(ctx.destination)
            const n2 = ctx.currentTime
            g2.gain.setValueAtTime(0, n2)
            g2.gain.linearRampToValueAtTime(0.12, n2 + 0.02)
            g2.gain.exponentialRampToValueAtTime(0.01, n2 + 0.55)
            o2.start(n2)
            o2.stop(n2 + 0.6)
          }, 300)
        }
        loop()
        interval = window.setInterval(loop, 2000)
      } catch {
        // autoplay blocked — ignore
      }
    }
    startBeep()

    // also attempt HTML audio from public folder if present (optional)
    const el = new Audio()
    el.loop = true
    el.volume = 0.45
    // intentionally no src — will 404 silently if not present; we suppress error
    el.src = "/ringtone.mp3"
    el.play().catch(() => {})
    audioRef.current = el

    return () => {
      stopped = true
      if (interval) window.clearInterval(interval)
      if (ctx) {
        try { ctx.close() } catch { /* ignore */ }
      }
      el.pause()
      el.remove()
    }
  }, [visible])

  if (!visible || !call) return null

  const handleAccept = async () => {
    if (answering) return
    setAnswering(true)
    try { await answerCall(call.id) } finally { setAnswering(false) }
  }
  const handleReject = async () => {
    if (rejecting) return
    setRejecting(true)
    try { await rejectCall(call.id) } finally { setRejecting(false) }
  }

  const handleAcceptVideo = async () => {
    setVideoBusy("accept")
    try { await acceptVideo(call.id) } finally { setVideoBusy(null) }
  }
  const handleRejectVideo = async () => {
    setVideoBusy("reject")
    try { await rejectVideo(call.id) } finally { setVideoBusy(null) }
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Incoming call"
      className="fixed inset-0 z-[90] flex items-end sm:items-center justify-center p-3 sm:p-6"
    >
      {/* backdrop */}
      <div className="absolute inset-0 bg-[#0a0a0a]/70 backdrop-blur-[14px] sm:backdrop-blur-[10px]" />

      {/* card */}
      <div
        className={cn(
          "relative w-full max-w-[380px] overflow-hidden rounded-[28px] border border-white/10 bg-card shadow-[0_20px_80px_rgba(0,0,0,0.45),0_1px_0_rgba(255,255,255,0.06)_inset]",
          "animate-in fade-in slide-in-from-bottom-4 sm:slide-in-from-bottom-2 duration-300",
        )}
      >
        {/* top glow */}
        <div className="pointer-events-none absolute -top-24 left-1/2 h-56 w-[120%] -translate-x-1/2 rounded-full bg-gradient-to-b from-primary/15 via-primary/5 to-transparent blur-[1px]" />
        <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/15 to-transparent" />

        <div className="relative px-6 pt-7 pb-6 flex flex-col items-center text-center">
          {/* status pill */}
          <div className="mb-5 inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 backdrop-blur">
            <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse shadow shadow-emerald-500/30" />
            <span className="text-[11px] font-semibold tracking-widest uppercase text-foreground/70">Incoming {isVideo ? "video" : "voice"} call</span>
          </div>

          {/* avatar with pulse rings */}
          <div className="relative mb-4">
            <div className="absolute inset-0 rounded-full bg-primary/20 animate-ping [animation-duration:2.2s]" />
            <div className="absolute -inset-3 rounded-full border border-primary/15" />
            <div className="absolute -inset-6 rounded-full border border-primary/[0.08]" />
            <Avatar className="relative h-[92px] w-[92px] border-[3px] border-white/15 shadow-xl">
              <AvatarFallback className="bg-zinc-900 text-white text-[28px] font-semibold tracking-tight">
                {initials(call.target)}
              </AvatarFallback>
            </Avatar>
            <span className={cn(
              "absolute -bottom-1 -right-1 flex h-7 w-7 items-center justify-center rounded-full border-2 border-card shadow-lg",
              isVideo ? "bg-sky-500" : "bg-emerald-500"
            )}>
              {isVideo ? <Video className="h-3.5 w-3.5 text-white" /> : <Mic className="h-3.5 w-3.5 text-white" />}
            </span>
          </div>

          <h2 className="text-[22px] font-semibold tracking-tight leading-none">
            {displayName(call.target)}
          </h2>
          <p className="mt-1 text-xs text-muted-foreground truncate max-w-[22ch]">{call.target}</p>
          {call.group_jid && (
            <p className="mt-1 text-[11px] px-2 py-1 rounded-full bg-muted text-muted-foreground truncate max-w-full">Group • {call.group_jid.split("@")[0]}</p>
          )}

          <p className="mt-3 text-sm text-foreground/60 animate-pulse">Ringing…</p>

          {/* video upgrade banner */}
          {upgrade && (
            <div className="mt-5 w-full rounded-2xl border border-sky-500/20 bg-sky-500/10 px-3 py-3 text-left">
              <p className="text-xs font-semibold text-sky-700 dark:text-sky-300 flex items-center gap-1.5">
                <Video className="h-3.5 w-3.5" /> Caller wants to turn on video
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-sky-700/70 dark:text-sky-200/70">Accept to enable your camera, or keep it as voice.</p>
              <div className="mt-2.5 flex gap-2">
                <Button size="sm" className="flex-1 rounded-full bg-sky-600 hover:bg-sky-700 text-white h-8 text-xs" onClick={handleAcceptVideo} disabled={!!videoBusy}>
                  {videoBusy === "accept" ? "…" : "Accept video"}
                </Button>
                <Button size="sm" variant="outline" className="flex-1 rounded-full h-8 text-xs bg-white/60 dark:bg-white/5" onClick={handleRejectVideo} disabled={!!videoBusy}>
                  {videoBusy === "reject" ? "…" : "Keep voice"}
                </Button>
              </div>
            </div>
          )}

          {/* actions */}
          <div className="mt-7 flex w-full items-center justify-center gap-4">
            <div className="flex flex-col items-center gap-2">
              <Button
                aria-label="Reject call"
                onClick={handleReject}
                disabled={rejecting || answering}
                className={cn(
                  "h-[64px] w-[64px] rounded-full bg-red-500 hover:bg-red-600 text-white shadow-[0_8px_24px_rgba(239,68,68,0.4)]",
                  "active:scale-[0.98] transition-transform"
                )}
              >
                <PhoneOff className="h-6 w-6" />
              </Button>
              <span className="text-[11px] font-medium tracking-wide text-muted-foreground">Decline</span>
            </div>

            <div className="h-10 w-px bg-border/60 self-start mt-3" />

            <div className="flex flex-col items-center gap-2">
              <Button
                aria-label="Accept call"
                onClick={handleAccept}
                disabled={answering || rejecting}
                className={cn(
                  "h-[64px] w-[64px] rounded-full bg-emerald-500 hover:bg-emerald-600 text-white shadow-[0_8px_24px_rgba(16,185,129,0.45)]",
                  "active:scale-[0.98] transition-transform"
                )}
              >
                <Phone className="h-6 w-6" />
              </Button>
              <span className="text-[11px] font-medium tracking-wide text-muted-foreground">{answering ? "Connecting…" : "Accept"}</span>
            </div>
          </div>

          <p className="mt-4 text-[11px] text-muted-foreground/70">Swipe or tap to answer — stay on this screen</p>
        </div>

        {/* bottom handle for mobile */}
        <div className="flex justify-center pb-3 sm:hidden">
          <div className="h-1 w-10 rounded-full bg-foreground/15" />
        </div>
      </div>

      {/* close control for screen readers / escape */}
      <button
        aria-label="Dismiss"
        onClick={handleReject}
        className="absolute right-3 top-3 hidden sm:flex h-8 w-8 items-center justify-center rounded-full bg-white/10 text-white/70 hover:bg-white/15 hover:text-white border border-white/10 backdrop-blur"
      >
        <X className="h-4 w-4" />
      </button>
    </div>
  )
}
