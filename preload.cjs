const {contextBridge, ipcRenderer} = require('electron');
contextBridge.exposeInMainWorld('browser', {
  command:(name, value) => ipcRenderer.send('command', name, value),
  onTheme:callback => ipcRenderer.on('theme', (_event, value) => callback(value)),
  onPrompt:callback => ipcRenderer.on('prompt', (_event, value) => callback(value))
});
