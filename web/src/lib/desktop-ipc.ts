export const isElectron = typeof window !== "undefined" && !!window.electron
export const isDesktop = isElectron
export const platform = isElectron ? window.electron?.platform || "win32" : "web"

export async function minimizeWindow() {
  if (isElectron) {
    window.electron?.minimize()
  }
}

export async function maximizeWindow() {
  if (isElectron) {
    window.electron?.maximize()
  }
}

export async function closeWindow() {
  if (isElectron) {
    window.electron?.close()
  }
}

export async function getWindowState(): Promise<"maximized" | "restored"> {
  if (isElectron) {
    return (await window.electron?.getWindowState()) || "restored"
  }
  return "restored"
}

export function onWindowStateChange(
  callback: (state: "maximized" | "restored") => void
): () => void {
  if (isElectron) {
    window.electron?.onWindowStateChange(callback)
    return () => {}
  }
  return () => {}
}

export async function getBackendPort(): Promise<number> {
  if (isElectron) {
    return (await window.electron?.getBackendPort()) || 0
  }
  return 0
}

export function onBackendReady(callback: (port: number) => void): () => void {
  if (isElectron) {
    window.electron?.onBackendReady(callback)
    return () => {}
  }
  return () => {}
}
