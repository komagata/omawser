const {app, BrowserWindow, WebContentsView, ipcMain, Menu, nativeTheme, dialog} = require('electron');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const {address, theme} = require('./core.cjs');
const {watchTheme} = require('./theme-watcher.cjs');
app.setName('Omawser');
if (process.argv.includes('--smoke') || process.argv.includes('--smoke-local')) {
  for (const [name, prefix] of [['XDG_CONFIG_HOME', 'config'], ['XDG_STATE_HOME', 'state']]) {
    const value = process.env[name];
    const resolved = value && fs.existsSync(value) && fs.realpathSync(value);
    if (!resolved || !resolved.startsWith(`/tmp/omawser-test-${prefix}-`) || path.resolve(value) !== resolved) {
      throw new Error(`--smoke requires an isolated ${name} under /tmp/omawser-test-${prefix}-*`);
    }
  }
  if (fs.readdirSync(process.env.XDG_CONFIG_HOME).length !== 0) {
    throw new Error('--smoke requires an empty isolated XDG_CONFIG_HOME');
  }
  const colors = path.join(process.env.XDG_STATE_HOME, 'omarchy/current/theme/colors.toml');
  if (!fs.existsSync(colors) || fs.realpathSync(colors) !== colors) {
    throw new Error('--smoke requires a regular isolated colors.toml fixture');
  }
}
app.setPath('userData', path.join(app.getPath('appData'), 'omawser'));
app.enableSandbox();
const windows = new Set();
const themePaths = [
  path.join(process.env.XDG_STATE_HOME || path.join(os.homedir(), '.local/state'), 'omarchy/current/theme/colors.toml'),
  path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'omarchy/current/theme/colors.toml')
];
let palette;
function refreshTheme() {
  let source = '';
  for (const file of themePaths) { try { source = fs.readFileSync(file, 'utf8'); break; } catch {} }
  const next = theme(source);
  if (JSON.stringify(next) === JSON.stringify(palette)) return;
  palette = next;
  nativeTheme.themeSource = palette.mode;
  for (const win of windows) { win.setBackgroundColor(palette.background); win.webContents.send('theme', palette); }
}
function createWindow(url = 'https://x.com/home') {
  const win = new BrowserWindow({width:1200, height:850, minWidth:480, minHeight:320, frame:false, title:'Omawser', backgroundColor:palette.background,
    webPreferences:{preload:path.join(__dirname, 'preload.cjs'), sandbox:true, contextIsolation:true, nodeIntegration:false}});
  const view = new WebContentsView({webPreferences:{partition:'persist:web', sandbox:true, contextIsolation:true, nodeIntegration:false}});
  const web = view.webContents;
  win.contentView.addChildView(view);
  windows.add(win);
  win.browserView = view;
  let promptOpen = false;
  let promptError = '';
  let loadErrorPrompt = false;
  let failedTarget = null;
  let loadFailed = false;
  const layout = () => { const [width,height] = win.getContentSize(); const top = promptOpen ? 48 : 1; view.setBounds({x:1,y:top,width:width-2,height:height-top-1}); };
  const sendPrompt = () => {
    const url = failedTarget || web.getURL();
    // The renderer preserves drafts on errors; prefill only a network-error target.
    if (promptOpen && loadErrorPrompt) win.webContents.send('prompt', {open:true, url, error:''});
    win.webContents.send('prompt', {open:promptOpen, url, error:promptError});
  };
  const showPrompt = (error = '', fromLoadError = false) => {
    promptOpen = true;
    promptError = error;
    loadErrorPrompt = fromLoadError;
    layout();
    sendPrompt();
    win.webContents.focus();
  };
  const hidePrompt = () => {
    promptOpen = false;
    promptError = '';
    loadErrorPrompt = false;
    layout();
    win.webContents.send('prompt', {open:false, url:web.getURL(), error:''});
    web.focus();
  };
  layout();
  win.on('resize', layout);
  win.on('focus', () => {
    setImmediate(() => {
      if (win.isDestroyed() || !win.isFocused()) return;
      if (promptOpen) win.webContents.focus();
      else if (!web.isDestroyed()) web.focus();
    });
  });
  win.on('closed', () => { windows.delete(win); if (!web.isDestroyed()) web.close(); });
  const navigate = input => {
    const target = address(input);
    if (!target) { showPrompt('Enter a valid http:// or https:// address.'); return; }
    failedTarget = null;
    hidePrompt();
    web.loadURL(target).catch(() => {});
  };
  web.setUserAgent(web.getUserAgent().replace(/\sElectron\/\S+/g, '').replace(/\somawser\/\S+/gi, ''));
  web.session.setPermissionRequestHandler((_web, _permission, callback) => callback(false));
  web.session.setPermissionCheckHandler(() => false);
  for (const event of ['will-navigate', 'will-redirect']) web.on(event, (e, target) => { if (!address(target)) e.preventDefault(); });
  web.setWindowOpenHandler(({url}) => { if (address(url)) createWindow(url); return {action:'deny'}; });
  web.on('did-start-navigation', (_e, _url, inPlace, mainFrame) => {
    if (mainFrame && !inPlace) loadFailed = false;
  });
  web.on('did-fail-load', (_e, code, description, url, mainFrame) => {
    if (mainFrame && code !== -3) {
      loadFailed = true;
      failedTarget = address(url);
      showPrompt(`Load failed: ${description}. Ctrl+R retry or enter another address.`, true);
    }
  });
  web.on('did-finish-load', () => {
    // Chromium can finish its error page too; that is not successful recovery.
    if (loadFailed) return;
    failedTarget = null;
    if (loadErrorPrompt) hidePrompt();
    else if (!promptOpen) web.focus();
  });
  web.on('page-title-updated', (_e, title) => win.setTitle(`${title} — Omawser`));
  const action = name => {
    if (name === 'back' && web.navigationHistory.canGoBack()) web.navigationHistory.goBack();
    if (name === 'forward' && web.navigationHistory.canGoForward()) web.navigationHistory.goForward();
    if (name === 'reload') {
      if (failedTarget) web.loadURL(failedTarget).catch(() => {});
      else web.reload();
    }
    if (name === 'stop') web.stop();
    if (name === 'address') showPrompt();
    if (name === 'home') navigate('https://x.com/home');
    if (name === 'zoom-in') web.setZoomLevel(web.getZoomLevel()+0.5);
    if (name === 'zoom-out') web.setZoomLevel(web.getZoomLevel()-0.5);
    if (name === 'zoom-reset') web.setZoomLevel(0);
  };
  win.navigate = navigate;
  win.action = action;
  const shortcuts = (event, input) => {
    if (input.type !== 'keyDown' || input.isComposing) return;
    const key = input.key.toLowerCase();
    if (input.control && !input.alt && !input.meta && !input.shift && ['l','r','n','w','0'].includes(key)) {
      event.preventDefault();
      if (key === 'l') action('address');
      if (key === 'r') action('reload');
      if (key === 'n') createWindow();
      if (key === 'w') win.close();
      if (key === '0') action('zoom-reset');
    } else if (input.control && !input.alt && !input.meta && ['+','=','-'].includes(key)) {
      event.preventDefault(); action(key === '-' ? 'zoom-out' : 'zoom-in');
    } else if (input.alt && !input.control && !input.meta && !input.shift && ['arrowleft','arrowright','home'].includes(key)) {
      event.preventDefault(); action(key === 'home' ? 'home' : key === 'arrowleft' ? 'back' : 'forward');
    } else if (!input.control && !input.alt && !input.meta && !input.shift && key === 'escape') {
      if (promptOpen) { event.preventDefault(); hidePrompt(); }
      else if (web.isLoading()) { event.preventDefault(); action('stop'); }
    } else if (!input.control && !input.alt && !input.meta && !input.shift && key === 'f5') { event.preventDefault(); action('reload'); }
    else if (!input.control && !input.alt && !input.meta && !input.shift && key === 'f11') { event.preventDefault(); win.setFullScreen(!win.isFullScreen()); }
  };
  web.on('before-input-event', shortcuts);
  win.webContents.on('before-input-event', shortcuts);
  win.webContents.on('will-navigate', e => e.preventDefault());
  win.webContents.setWindowOpenHandler(() => ({action:'deny'}));
  win.loadFile(path.join(__dirname, 'index.html'));
  win.webContents.on('did-finish-load', () => { win.webContents.send('theme', palette); sendPrompt(); if (!promptOpen) web.focus(); });
  if (url) navigate(url);
  return win;
}
ipcMain.on('command', (event, command, value) => {
  const win = [...windows].find(w => w.webContents === event.sender && event.senderFrame === w.webContents.mainFrame);
  if (!win) return;
  if (command === 'navigate') win.navigate(value);
  else win.action(command);
});
app.whenReady().then(async () => {
  Menu.setApplicationMenu(null);
  refreshTheme();
  const themeWatcher = watchTheme(themePaths, refreshTheme);
  app.once('will-quit', () => themeWatcher.close());
  const win = createWindow(process.argv.includes('--smoke-local') ? null : process.argv.find(a => /^https?:\/\//.test(a)));
  win.browserView.webContents.session.on('will-download', (_event, item) => {
    const filePath = dialog.showSaveDialogSync({defaultPath:path.join(app.getPath('downloads'), item.getFilename())});
    if (!filePath) item.cancel(); else item.setSavePath(filePath);
  });
  if (process.argv.includes('--smoke')) require('./test/smoke.cjs')({app,win,refreshTheme});
});
app.on('window-all-closed', () => app.quit());
