import { createRoot } from "react-dom/client"

import "./index.css"
import App from "./App.tsx"
import { api, setBackendPort } from "./lib/api"
import { isElectron, getBackendPort, onBackendReady } from "./lib/desktop-ipc"

// ── Electron desktop mode: set up IPC before React renders ──
if (isElectron) {
	const updatePort = (port: number) => {
		setBackendPort(port)
		api.setBaseUrl(`http://localhost:${port}/api`)
	}
	getBackendPort().then((port) => {
		if (port) updatePort(port)
	})
	onBackendReady(updatePort)

	// Global error handlers prevent uncaught errors from crashing Electron
	window.onerror = (_message, _source, _lineno, _colno, _error) => {
		console.error("[Global Error]", { _message, _source, _lineno, _colno, _error })
		return false
	}
	window.onunhandledrejection = (event) => {
		console.error("[Unhandled Promise Rejection]", event.reason)
	}
}

createRoot(document.getElementById("root")!).render(
	<App />
)
