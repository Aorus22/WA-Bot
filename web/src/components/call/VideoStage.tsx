import { useEffect, useRef, useSyncExternalStore } from "react"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { VideoOff, Maximize2 } from "lucide-react"
import { cn } from "@/lib/utils"
import type { RemoteVideoRenderer } from "@/lib/callVideo"

type Props = {
  callId: string
  localEnabled: boolean
  remoteEnabled: boolean
  targetLabel: string
  /** Live camera stream for the local picture-in-picture (owned by the parent). */
  localStream: MediaStream | null
  /** Paints incoming JPEG frames on the canvas this component attaches. */
  remoteRenderer: RemoteVideoRenderer
  className?: string
}

function initials(label: string) {
  const s = label.replace(/@.*$/, "").replace(/[^a-zA-Z0-9]/g, " ").trim()
  if (!s) return "WA"
  const parts = s.split(/\s+/).filter(Boolean)
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
  return (parts[0][0] + parts[1][0]).toUpperCase()
}

export function VideoStage({ localEnabled, remoteEnabled, targetLabel, localStream, remoteRenderer, className }: Props) {
  const remoteCanvasRef = useRef<HTMLCanvasElement>(null)
  const localVideoRef = useRef<HTMLVideoElement>(null)

  const hasRemoteFrames = useSyncExternalStore(
    (onChange) => remoteRenderer.subscribe(onChange),
    () => remoteRenderer.hasFrames,
  )

  // Local preview: the parent owns the camera (it also feeds the call), we only display it.
  useEffect(() => {
    const el = localVideoRef.current
    if (!el) return
    el.srcObject = localEnabled ? localStream : null
  }, [localEnabled, localStream])

  useEffect(() => {
    remoteRenderer.attach(remoteEnabled ? remoteCanvasRef.current : null)
    return () => remoteRenderer.attach(null)
  }, [remoteRenderer, remoteEnabled])

  // Simple animated placeholder for remote when enabled but no frames yet
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-2xl bg-zinc-950 border border-white/10",
        "flex flex-col",
        className,
      )}
    >
      {/* Remote stage */}
      <div className="relative flex-1 min-h-[280px] sm:min-h-[360px] bg-gradient-to-br from-zinc-900 via-zinc-950 to-black flex items-center justify-center overflow-hidden">
        {/* subtle mesh */}
        <div className="absolute inset-0 opacity-[0.08]" style={{ backgroundImage: `radial-gradient(circle at 1px 1px, white 1px, transparent 0)`, backgroundSize: "22px 22px" }} />

        {remoteEnabled ? (
          <>
            <canvas
              ref={remoteCanvasRef}
              className={cn("absolute inset-0 h-full w-full object-contain", !hasRemoteFrames && "opacity-0")}
            />

            {/* placeholder until frames arrive */}
            {!hasRemoteFrames && (
              <div className="relative z-10 flex flex-col items-center gap-4 px-6 text-center">
                <div className="h-20 w-20 rounded-full bg-white/10 backdrop-blur flex items-center justify-center border border-white/10">
                  <span className="text-xl font-semibold tracking-tight text-white">{initials(targetLabel)}</span>
                </div>
                <div className="space-y-1">
                  <p className="text-sm font-medium text-white/90">Remote camera on</p>
                  <p className="text-xs text-white/50">Waiting for video stream…</p>
                </div>
                <div className="flex items-center gap-1.5 mt-1">
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />
                  <span className="text-[11px] tracking-widest font-medium text-white/60 uppercase">Live</span>
                </div>
              </div>
            )}

            {/* shimmer bar */}
            <div className="absolute inset-x-0 bottom-0 h-[1px] bg-gradient-to-r from-transparent via-white/20 to-transparent" />
          </>
        ) : (
          <div className="relative z-10 flex flex-col items-center gap-5 py-10">
            <Avatar className="h-24 w-24 border-2 border-white/10 shadow-xl">
              <AvatarFallback className="bg-zinc-800 text-white text-2xl font-semibold">
                {initials(targetLabel)}
              </AvatarFallback>
            </Avatar>
            <div className="flex flex-col items-center gap-2">
              <p className="text-sm font-medium text-white">{targetLabel.replace(/@.*$/, "")}</p>
              <span className="inline-flex items-center gap-1.5 rounded-full bg-white/10 border border-white/10 px-2.5 py-1 text-[11px] font-medium text-white/70">
                <VideoOff className="h-3 w-3" /> Camera off
              </span>
            </div>
          </div>
        )}

        {/* expand hint */}
        <div className="absolute right-3 top-3 rounded-full bg-black/40 border border-white/10 p-1.5 text-white/60 backdrop-blur">
          <Maximize2 className="h-3.5 w-3.5" />
        </div>
      </div>

      {/* Local PiP */}
      <div className="absolute bottom-3 right-3 sm:bottom-4 sm:right-4">
        <div className="relative h-[112px] w-[88px] sm:h-[132px] sm:w-[96px] overflow-hidden rounded-xl border border-white/15 bg-zinc-900 shadow-2xl">
          {localEnabled ? (
            <video
              ref={localVideoRef}
              autoPlay
              playsInline
              muted
              className="h-full w-full object-cover scale-x-[-1]"
            />
          ) : (
            <div className="h-full w-full flex flex-col items-center justify-center gap-2 bg-zinc-800 p-2 text-center">
              <VideoOff className="h-5 w-5 text-white/40" />
              <span className="text-[10px] leading-tight text-white/50">Camera off</span>
            </div>
          )}
          <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/60 to-transparent px-2 py-1">
            <p className="text-[10px] font-medium text-white/80 truncate">You</p>
          </div>
          {localEnabled && <span className="absolute left-1.5 top-1.5 h-2 w-2 rounded-full bg-emerald-400 shadow shadow-emerald-400/50" />}
        </div>
      </div>
    </div>
  )
}
