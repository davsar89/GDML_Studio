// GDML Studio desktop shell: runs the backend, which also serves the built
// frontend, and shows it in a window. Save goes through preload.js.
const { app, BrowserWindow, dialog, ipcMain } = require('electron');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const net = require('node:net');
const path = require('node:path');

// Installed: everything sits in resources/ (see "extraResources" in package.json).
// `npm start`: the repo, after `npm run build:backend` and `npm run build:frontend`.
const root = app.isPackaged ? process.resourcesPath : path.join(__dirname, '..');
const backendName = process.platform === 'win32' ? 'gdml-studio-backend.exe' : 'gdml-studio-backend';
const exe = app.isPackaged ? path.join(root, backendName) : path.join(root, 'target', 'release', backendName);

// Where Chromium blocklists the GPU (VMs, remote desktops, WSL), WebGL is off
// and the 3D view stays blank; this lets it fall back to software rendering.
// "Unsafe" refers to untrusted web content; the window only shows our own page.
app.commandLine.appendSwitch('enable-unsafe-swiftshader');

let backend = null;
let quitting = false;
let stderrTail = '';

// The port is free when checked, not reserved: if another process takes it
// before the backend binds, the backend exits and the exit handler reports it.
function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

// If Electron itself crashes, Windows leaves this backend running until reboot.
// Upgrade path if that matters: have the backend exit when its stdin closes.
function startBackend(port) {
  backend = spawn(exe, [], {
    // The backend serves cwd/frontend/dist.
    cwd: root,
    env: { ...process.env, GDML_PORT: String(port) },
    // A console-subsystem child of a GUI app would otherwise open a console window.
    windowsHide: true,
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  backend.stderr.on('data', (chunk) => { stderrTail = (stderrTail + chunk).slice(-2000); });
  const fail = (reason) => {
    // A failed spawn can emit both 'error' and 'exit'; report once.
    if (quitting) return;
    quitting = true;
    dialog.showErrorBox('GDML Studio', `The backend stopped (${reason}).\n\n${stderrTail}`);
    app.quit();
  };
  backend.on('error', (e) => fail(e.message));
  backend.on('exit', (code) => fail(`exit code ${code}`));
}

async function waitForBackend(url) {
  for (let i = 0; i < 200; i++) {
    if (backend.exitCode !== null) return false;
    try {
      if ((await fetch(url)).ok) return true;
    } catch { /* not listening yet */ }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}

function createWindow(url) {
  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    backgroundColor: '#11151b',
    autoHideMenuBar: true,
    // The installed exe already carries it; `npm start` would show Electron's.
    icon: path.join(__dirname, 'icon.png'),
    webPreferences: { preload: path.join(__dirname, 'preload.js') },
  });
  // A file dropped on the window would otherwise replace the app with its text.
  win.webContents.on('will-navigate', (e) => e.preventDefault());
  // With unsaved edits the page objects to unloading (Toolbar.tsx). A browser
  // asks the user then; Electron would silently refuse to close. Ask here.
  win.webContents.on('will-prevent-unload', (e) => {
    const choice = dialog.showMessageBoxSync(win, {
      type: 'warning',
      title: 'GDML Studio',
      message: 'This document has unsaved changes.',
      detail: 'Leave anyway? Unsaved changes will be lost.',
      buttons: ['Leave', 'Cancel'],
      defaultId: 1,
      cancelId: 1,
    });
    if (choice === 0) e.preventDefault();
  });
  win.loadURL(url);
}

ipcMain.handle('save-file', async (event, content, filename) => {
  if (typeof content !== 'string' || typeof filename !== 'string') throw new Error('Invalid save request');
  const { canceled, filePath } = await dialog.showSaveDialog(BrowserWindow.fromWebContents(event.sender), {
    defaultPath: filename,
    filters: [{ name: 'GDML', extensions: ['gdml'] }, { name: 'All files', extensions: ['*'] }],
  });
  if (canceled || !filePath) return null;
  await fs.promises.writeFile(filePath, content, 'utf8');
  return path.basename(filePath);
});

app.on('will-quit', () => {
  quitting = true;
  backend?.kill();
});

app.whenReady().then(async () => {
  // GDML_PORT pins the port, e.g. to open the same session in a browser for debugging.
  const port = Number(process.env.GDML_PORT) || await freePort();
  const url = `http://127.0.0.1:${port}/`;
  startBackend(port);
  if (await waitForBackend(url)) createWindow(url);
  else if (backend.exitCode === null) {
    dialog.showErrorBox('GDML Studio', `The backend did not answer on ${url} within 20 s.\n\n${stderrTail}`);
    app.quit();
  }
});
