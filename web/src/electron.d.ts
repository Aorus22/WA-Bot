declare global {
  interface Window {
    electron?: {
      minimize: () => void;
      maximize: () => void;
      close: () => void;
      getWindowState: () => Promise<"maximized" | "restored">;
      onWindowStateChange: (callback: (state: "maximized" | "restored") => void) => void;
      getBackendPort: () => Promise<number>;
      onBackendReady: (callback: (port: number) => void) => void;
      isElectron: boolean;
      platform: string;
    };
    __BACKEND_PORT__?: number;
  }
}

export {}
