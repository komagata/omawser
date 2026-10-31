const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const waitFor = async (check, label) => {
  for (let i = 0; i < 100; i++) {
    if (await check()) return;
    await delay(50);
  }
  throw new Error(`Timed out: ${label}`);
};

module.exports = async ({app, win, refreshTheme}) => {
  const web = win.browserView.webContents;
  const ui = script => win.webContents.executeJavaScript(script);
  const key = async (contents, keyCode, modifiers = []) => {
    contents.sendInputEvent({type:'keyDown', keyCode, modifiers});
    if (keyCode === 'Enter') contents.sendInputEvent({type:'char', keyCode:'\r', modifiers});
    contents.sendInputEvent({type:'keyUp', keyCode, modifiers});
    await delay(80);
  };
  const type = value => win.webContents.insertText(value);
  const bounds = () => win.browserView.getBounds();
  const expectedBounds = top => { const [width, height] = win.getContentSize(); return {x:1,y:top,width:width-2,height:height-top-1}; };
  const settledBounds = async top => {
    await waitFor(() => {
      const actual = bounds();
      return Object.entries(expectedBounds(top)).every(([key, value]) => actual[key] === value);
    }, `settled bounds at ${top}`);
    assert.deepEqual(bounds(), expectedBounds(top));
  };
  let failRequests = true;
  let failedRequests = 0;
  const requests = new Map();
  const server = http.createServer((req, res) => {
    requests.set(req.url, (requests.get(req.url) || 0) + 1);
    if (req.url.startsWith('/retry') && failRequests) {
      failedRequests++;
      req.socket.destroy();
      return;
    }
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('Content-Type', 'text/html');
    res.end(`<title>Fixture ${req.url}</title><h1>Browser fixture ${req.url}</h1><script>window.addEventListener('keydown', e => { if (e.key === 'Escape') document.body.dataset.escape = 'seen' })</script>`);
  });
  const stateFile = path.join(process.env.XDG_STATE_HOME, 'omarchy/current/theme/colors.toml');
  let original;
  let exitCode = 0;
  try {
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    original = fs.readFileSync(stateFile, 'utf8');
    const base = `http://127.0.0.1:${server.address().port}`;
    const promptState = () => ui('({open:!document.querySelector("header").hidden,error:document.querySelector("#error").textContent,value:document.querySelector("#address").value})');
    const failedPrompt = async target => {
      await waitFor(async () => {
        const state = await promptState();
        return state.open && state.error.startsWith('Load failed:') && !web.isLoading();
      }, 'failed load prompt');
      assert.equal((await promptState()).value, target);
      await settledBounds(48);
    };
    const recovered = async target => {
      await waitFor(async () => !web.isLoading() && web.getURL() === target &&
        await web.executeJavaScript('document.title') === `Fixture ${new URL(target).pathname}`,
      'successful retry');
      await waitFor(async () => {
        const state = await promptState();
        return !state.open && !state.error && web.isFocused();
      }, 'retry dismisses error and focuses page');
      await settledBounds(1);
    };
    await waitFor(() => !win.webContents.isLoading(), 'shell ready');
    // Initial navigation failure must display the requested target, even without a prior page.
    const initialTarget = `${base}/retry-initial`;
    win.navigate(initialTarget);
    await failedPrompt(initialTarget);
    const failuresBeforeRetry = failedRequests;
    await key(win.webContents, 'R', ['control']);
    await waitFor(() => failedRequests > failuresBeforeRetry, 'repeated failing retry request');
    await failedPrompt(initialTarget);
    failRequests = false;
    await key(win.webContents, 'R', ['control']);
    await recovered(initialTarget);

    await web.loadURL(`${base}/one`);
    await waitFor(() => web.isFocused(), 'initial remote focus');
    assert.equal(await web.executeJavaScript('document.hasFocus()'), true);
    await settledBounds(1);
    assert.equal(await ui('document.querySelector("header").hidden'), true);
    assert.equal(await web.executeJavaScript('typeof require'), 'undefined');
    assert.equal(await web.executeJavaScript('typeof window.browser'), 'undefined');
    const prefs = web.getLastWebPreferences();
    assert.equal(prefs.sandbox, true);
    assert.equal(prefs.nodeIntegration, false);
    assert.equal(web.session.isPersistent(), true);

    await key(web, 'L', ['control']);
    await waitFor(async () => ui('!document.querySelector("header").hidden'), 'prompt visible');
    await settledBounds(48);
    assert.equal(await ui('document.activeElement.id'), 'address');
    assert.equal(await ui('document.querySelector("#address").selectionStart'), 0);
    assert.equal(await ui('document.querySelector("#address").selectionEnd'), `${base}/one`.length);
    await key(win.webContents, 'L', ['control']);
    assert.equal(await ui('document.querySelector("#address").selectionEnd'), `${base}/one`.length);
    await type('javascript:alert(1)');
    await key(win.webContents, 'Enter');
    await waitFor(async () => ui('!document.querySelector("#error").hidden'), 'invalid address error');
    assert.equal(web.getURL(), `${base}/one`);
    assert.equal((await promptState()).value, 'javascript:alert(1)');
    const invalidReloads = requests.get('/one');
    await key(win.webContents, 'R', ['control']);
    await waitFor(() => requests.get('/one') > invalidReloads && !web.isLoading(), 'reload with invalid input');
    assert.equal((await promptState()).value, 'javascript:alert(1)');
    assert.equal((await promptState()).open, true);
    assert.ok((await promptState()).error);
    await settledBounds(48);
    await key(win.webContents, 'L', ['control']);
    await key(win.webContents, 'Backspace');
    await key(win.webContents, 'Enter');
    assert.equal(await ui('!document.querySelector("#error").hidden'), true);
    await settledBounds(48);
    await key(win.webContents, 'Escape');
    await waitFor(() => web.isFocused(), 'focus restored after cancel');
    await settledBounds(1);
    await key(web, 'Escape');
    assert.equal(await web.executeJavaScript('document.body.dataset.escape'), 'seen');
    await key(web, 'L', ['control']);
    assert.equal(await ui('document.querySelector("#address").value'), `${base}/one`);
    await type(`${base}/two`);
    await key(win.webContents, 'Enter');
    await waitFor(() => web.getURL() === `${base}/two`, 'valid address navigation');
    await settledBounds(1);
    await waitFor(() => web.isFocused(), 'focus after navigation');
    await key(web, 'Left', ['alt']);
    await waitFor(() => web.getURL() === `${base}/one`, 'back');
    await key(web, 'Right', ['alt']);
    await waitFor(() => web.getURL() === `${base}/two`, 'forward');
    await web.executeJavaScript('document.body.dataset.marker = "old"');
    await key(web, 'R', ['control']);
    await waitFor(async () => (await web.executeJavaScript('document.body.dataset.marker')) === undefined, 'reload');
    await key(web, '=', ['control']);
    assert.ok(web.getZoomLevel() > 0);
    await key(web, '-', ['control']);
    assert.equal(web.getZoomLevel(), 0);
    await key(web, '-', ['control']);
    assert.ok(web.getZoomLevel() < 0);
    await key(web, '0', ['control']);
    assert.equal(web.getZoomLevel(), 0);
    await key(web, 'L', ['control']);
    fs.writeFileSync(stateFile, 'accent = "#ff1234"\nbackground = "#111111"');
    await waitFor(async () => (await ui('getComputedStyle(document.querySelector("#address")).borderTopColor')) === 'rgb(255, 18, 52)', 'prompt theme refresh');
    assert.equal(await ui('getComputedStyle(document.querySelector("#address")).borderRadius'), '0px');
    win.setSize(540, 700);
    await waitFor(() => { const [width, height] = win.getSize(); return width === 540 && height === 700; }, 'small window resize');
    await settledBounds(48);
    await key(win.webContents, 'Escape');
    await settledBounds(1);
    await key(web, 'L', ['control']);
    assert.equal(await ui('getComputedStyle(document.querySelector("#address")).borderTopColor'), 'rgb(255, 18, 52)');
    await key(win.webContents, 'Escape');
    win.setSize(1200, 850);
    await waitFor(() => { const [width, height] = win.getSize(); return width === 1200 && height === 850; }, 'large window resize');
    await settledBounds(1);
    // Failed navigation from an existing page also shows and retries the new target.
    failRequests = true;
    const nextTarget = `${base}/retry-next`;
    await key(web, 'L', ['control']);
    await type(nextTarget);
    await key(win.webContents, 'Enter');
    await failedPrompt(nextTarget);
    failRequests = false;
    await key(win.webContents, 'R', ['control']);
    await recovered(nextTarget);
    // A user-opened strip and draft must survive a successful reload.
    await key(web, 'L', ['control']);
    await type(`${base}/draft`);
    await waitFor(() => win.isFocused(), 'window focused before synthetic focus-handler regression');
    // Synthetic handler regression: exercise deferred focus handling while the prompt owns focus.
    // This is not a real compositor focus transition; compositor stay_focused rules may reject blur.
    win.emit('focus');
    await waitFor(async () => win.isFocused() && win.webContents.isFocused() && await ui('document.activeElement.id') === 'address', 'prompt focus restoration preserves address draft');
    assert.equal(await ui('document.querySelector("#address").value'), `${base}/draft`);
    const draftReloads = requests.get('/retry-next');
    await key(win.webContents, 'R', ['control']);
    await waitFor(() => requests.get('/retry-next') > draftReloads && !web.isLoading(), 'reload with ordinary draft');
    assert.equal((await promptState()).open, true);
    assert.equal((await promptState()).value, `${base}/draft`);
    assert.equal((await promptState()).error, '');
    await key(win.webContents, 'Escape');
    await waitFor(() => web.isFocused(), 'focus after draft cancel');
    console.log('SMOKE PASS: keyboard prompt, validation, bounds, focus, history, reload, failed-load recovery, drafts, zoom, theme, sandbox, profile');
    if (!process.argv.includes('--smoke-local')) {
      await web.loadURL('https://x.com/home');
      console.log('X PAGE:', JSON.stringify(await web.executeJavaScript('({url:location.href,title:document.title})')));
    }
  } catch (error) {
    console.error(error);
    exitCode = 1;
  } finally {
    try {
      if (original !== undefined) fs.writeFileSync(stateFile, original);
    } catch (error) {
      console.error(error);
      exitCode = 1;
    } finally {
      await new Promise(resolve => server.close(resolve));
      app.exit(exitCode);
    }
  }
};
