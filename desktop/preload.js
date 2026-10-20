// The page's only bridge to the desktop: Save through a native dialog (main.js).
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('desktop', {
  saveFile: (content, filename) => ipcRenderer.invoke('save-file', content, filename),
});
