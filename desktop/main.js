const { app, BrowserWindow, ipcMain, Menu, nativeImage, dialog } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fs = require('fs');

const DEFAULT_EXTERNAL_BACKEND_PORT = 8080;

function hasCliFlag(name) {
    return process.argv.some((arg) => arg === `--${name}` || arg === `-${name}`);
}

function getCliValue(name) {
    const longName = `--${name}`;
    const shortName = `-${name}`;

    for (let i = 0; i < process.argv.length; i += 1) {
        const arg = process.argv[i];
        if (arg.startsWith(`${longName}=`) || arg.startsWith(`${shortName}=`)) {
            return arg.slice(arg.indexOf('=') + 1);
        }
        if (arg === longName || arg === shortName) {
            return process.argv[i + 1] ?? '';
        }
    }

    return null;
}

function parseBackendPort(value) {
    if (value === null) {
        return DEFAULT_EXTERNAL_BACKEND_PORT;
    }
    if (!/^\d+$/.test(value)) {
        return null;
    }

    const port = Number.parseInt(value, 10);
    return port >= 1 && port <= 65535 ? port : null;
}

const noBackend = hasCliFlag('no-backend');
const externalBackendPort = noBackend ? parseBackendPort(getCliValue('port')) : null;

let mainWindow;
let backendProcess;

// Minimal menu with DevTools
const template = [
    {
        label: 'DevTools',
        accelerator: 'CmdOrCtrl+Shift+I',
        click: () => mainWindow?.webContents.toggleDevTools()
    }
];
Menu.setApplicationMenu(Menu.buildFromTemplate(template));

let backendPort = 0;

function startBackend() {
    const isDev = process.env.NODE_ENV === 'development';

    let backendPath;
    if (app.isPackaged) {
        backendPath = path.join(process.resourcesPath, 'be', 'wa-bot-backend');
        if (process.platform === 'win32') {
            backendPath += '.exe';
        }
    } else {
        backendPath = path.join(__dirname, '..', 'wa-bot-backend');
        if (process.platform === 'win32') {
            backendPath += '.exe';
        }
    }

    // Ensure data directories exist
    const userDataPath = app.getPath('userData');
    const dbPath = path.join(userDataPath, 'database');
    const mediaPath = path.join(userDataPath, 'media');

    if (!fs.existsSync(dbPath)) {
        fs.mkdirSync(dbPath, { recursive: true });
    }
    if (!fs.existsSync(mediaPath)) {
        fs.mkdirSync(mediaPath, { recursive: true });
    }

    console.log(`Starting backend: ${backendPath}`);

    const env = {
        ...process.env,
        PORT: ':0',
        ALLOWED_ORIGINS: '*',
        DB_PATH: dbPath,
        MEDIA_PATH: mediaPath,
        TZ: 'Asia/Jakarta',
    };

    backendProcess = spawn(backendPath, [], {
        env,
        cwd: userDataPath,
    });

    backendProcess.stdout.on('data', (data) => {
        const text = data.toString();
        console.log(`Backend: ${text}`);
        if (text.includes('BACKEND_PORT:')) {
            const port = parseInt(text.split('BACKEND_PORT:')[1].trim());
            if (!isNaN(port)) {
                backendPort = port;
                if (mainWindow) {
                    mainWindow.webContents.send('backend-ready', port);
                }
            }
        }
    });

    backendProcess.stderr.on('data', (data) => {
        console.error(`Backend Error: ${data}`);
    });

    backendProcess.on('close', (code) => {
        console.log(`Backend process exited with code ${code}`);
    });
}

// IPC Handlers
ipcMain.handle('get-backend-port', () => {
    return backendPort;
});

function createWindow() {
    const isWin = process.platform === 'win32';
    const isMac = process.platform === 'darwin';

    mainWindow = new BrowserWindow({
        width: 1200,
        height: 800,
        minWidth: 800,
        minHeight: 600,
        frame: false,
        transparent: false,
        backgroundColor: '#0a0a0b',
        ...( (isWin || isMac) ? { titleBarStyle: 'hidden' } : {} ),
        titleBarOverlay: isWin ? {
            color: '#00000000',
            symbolColor: '#94a3b8',
            height: 48
        } : false,
        autoHideMenuBar: false,
        webPreferences: {
            nodeIntegration: false,
            contextIsolation: true,
            preload: path.join(__dirname, 'preload.js')
        },
        title: "WA Bot Desktop",
        icon: nativeImage.createFromPath(path.join(__dirname, 'assets', 'icon.png')),
    });

    const isDev = process.env.NODE_ENV === 'development';

    if (isDev) {
        mainWindow.loadURL('http://localhost:5173');
    } else if (app.isPackaged) {
        mainWindow.loadFile(path.join(process.resourcesPath, 'fe', 'dist', 'index.html'));
    } else {
        mainWindow.loadFile(path.join(__dirname, '..', 'web', 'dist', 'index.html'));
    }

    mainWindow.on('closed', function () {
        mainWindow = null;
    });

    // DevTools accessible via Ctrl+Shift+I and right-click context menu
    // Debug logging
    mainWindow.webContents.on('did-finish-load', () => {
        console.log('[DEBUG] Page finished loading');
        if (backendPort > 0) {
            console.log(`[DEBUG] Sending backend-ready with port ${backendPort}`);
            mainWindow.webContents.send('backend-ready', backendPort);
        } else {
            console.log('[DEBUG] backendPort is 0, backend not ready yet');
        }
    });

    mainWindow.webContents.on('did-fail-load', (_event, errorCode, errorDesc, validatedURL) => {
        console.error(`[DEBUG] Page load FAILED: ${errorDesc} (code ${errorCode}) URL: ${validatedURL}`);
    });

    // Right-click context menu with Inspect Element
    mainWindow.webContents.on('context-menu', (_event, params) => {
        const { Menu } = require('electron');
        Menu.buildFromTemplate([{
            label: 'Inspect Element',
            click: () => mainWindow.webContents.inspectElement(params.x, params.y)
        }]).popup(mainWindow);
    });

    mainWindow.on('maximize', () => {
        mainWindow.webContents.send('window-state-change', 'maximized');
    });

    mainWindow.on('unmaximize', () => {
        mainWindow.webContents.send('window-state-change', 'restored');
    });
}

// IPC Handlers for Window Controls
ipcMain.on('window-minimize', () => {
    mainWindow.minimize();
});

ipcMain.on('window-maximize', () => {
    if (mainWindow.isMaximized()) {
        mainWindow.unmaximize();
    } else {
        mainWindow.maximize();
    }
});

ipcMain.on('window-close', () => {
    mainWindow.close();
});

ipcMain.handle('get-window-state', () => {
    return mainWindow ? (mainWindow.isMaximized() ? 'maximized' : 'restored') : 'restored';
});

app.on('ready', () => {
    if (noBackend) {
        if (externalBackendPort === null) {
            dialog.showErrorBox(
                'Invalid backend port',
                'Use --port with a value between 1 and 65535, for example: --no-backend --port 3090'
            );
            app.quit();
            return;
        }
        backendPort = externalBackendPort;
        console.log(`Backend manager disabled; connecting to existing backend on port ${backendPort}`);
    } else {
        startBackend();
    }
    createWindow();
});

app.on('window-all-closed', function () {
    if (process.platform !== 'darwin') {
        app.quit();
    }
});

app.on('activate', function () {
    if (mainWindow === null) {
        createWindow();
    }
});

app.on('will-quit', () => {
    if (backendProcess) {
        backendProcess.kill();
    }
});
