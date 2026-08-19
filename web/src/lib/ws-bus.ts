import type { WSMessage } from "@/hooks/use-websocket"

type Listener = (msg: WSMessage) => void

const listeners = new Set<Listener>()

export function emitWSMessage(msg: WSMessage): void {
	for (const l of listeners) l(msg)
}

export function subscribeWS(listener: Listener): () => void {
	listeners.add(listener)
	return () => {
		listeners.delete(listener)
	}
}
