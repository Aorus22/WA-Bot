const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electron', {
    minimize: () => ipcRenderer.send('window-minimize'),
    maximize: () => ipcRenderer.send('window-maximize'),
    close: () => ipcRenderer.send('window-close'),
    getWindowState: () => ipcRenderer.invoke('get-window-state'),
    onWindowStateChange: (callback) =>
        ipcRenderer.on('window-state-change', (_event, state) => callback(state)),
    getBackendPort: () => ipcRenderer.invoke('get-backend-port'),
    onBackendReady: (callback) =>
        ipcRenderer.on('backend-ready', (_event, port) => callback(port)),
    isElectron: true,
    platform: process.platform,
});

window.addEventListener('DOMContentLoaded', () => {
    console.log('WA Bot Desktop Preload Loaded');
});
