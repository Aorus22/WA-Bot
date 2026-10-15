import type { DisappearingSeconds } from "@/lib/api"

export const DISAPPEARING_OPTIONS: { seconds: DisappearingSeconds; label: string }[] = [
	{ seconds: 0, label: "Off" },
	{ seconds: 86400, label: "24 hours" },
	{ seconds: 604800, label: "7 days" },
	{ seconds: 7776000, label: "90 days" },
]

export const disappearingTimerLabel = (seconds: number | undefined): string =>
	DISAPPEARING_OPTIONS.find(option => option.seconds === (seconds || 0))?.label ?? `${Math.round((seconds || 0) / 86400)} days`
