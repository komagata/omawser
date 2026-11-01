# Native browser behavior

Omawser targets Omarchy 4. These details describe the independent Tauri application.

## NVIDIA on Wayland

Previous development verification on an NVIDIA Wayland host found that the unmodified WebKitGTK renderer exited with `Missing acquire timeline`; setting `__NV_DISABLE_EXPLICIT_SYNC=1` alone left a black page. Visible page rendering was verified there with `WEBKIT_DISABLE_DMABUF_RENDERER=1`. The source-tree launcher applies that setting only for a Wayland session with an NVIDIA PCI GPU, unless `GDK_BACKEND=x11` is explicit. It preserves any existing value, including `0` or an empty value. Direct binary launches do not apply this workaround; use the launcher or explicitly prefix the command:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./src-tauri/target/release/omawser
omabox run --net isolated -- env WEBKIT_DISABLE_DMABUF_RENDERER=1 python3 scripts/tauri-smoke.py --binary "$PWD/src-tauri/target/debug/omawser"
python3 scripts/test_graphics_compat.py
```

This changes WebKit's DMABUF rendering path, not sandbox or TLS settings, and does not modify global environment or desktop configuration. Its performance impact is unmeasured. See the [official Tauri Linux graphics guide](https://v2.tauri.app/develop/debug/linux-graphics/), [Wayland error #10702](https://github.com/tauri-apps/tauri/issues/10702), and [NVIDIA rendering report #14924](https://github.com/tauri-apps/tauri/issues/14924).

## Design and theme

The default frameless window is 1200×850 (minimum 480×320), with no app-drawn outer border. With one tab the website fills the window. With two or more, a compact square tab strip sits above the temporary address strip. The active tab has a thin underline; inactive text and hover states follow the theme foreground. Titles are plain text, truncated with full-title tooltips and accessible names. Native tab scrolling keeps the selected tab reachable in narrow windows. Click a tab to select it; its close control or middle-click closes it. Ctrl+L shows a temporary native address strip with a square 1px input border; the current address is selected. Enter accepts only HTTP(S) addresses. Invalid input stays visible with plain text feedback. Escape closes the strip and returns focus to the website.

Switching tabs keeps each live page, history, form contents, scroll position, zoom and address draft. Closing an inactive tab preserves the active page; closing the active tab selects its right neighbor, or its left neighbor at the end. Background load, error and title callbacks update their own tab without opening the active address strip or changing the window title.

A main-frame load failure also opens the strip. Ctrl+R retries the failed target even if the user has edited the draft. Repeated errors and native refocus preserve the draft; a successful retry hides the error strip. A manually opened strip stays open when an unrelated load completes. Native window key handling precedes page key handling, so a page cannot consume Ctrl+L. Ordinary typing and GTK/WebKit input methods retain their normal dispatch.

Colors come first from `$XDG_STATE_HOME/omarchy/current/theme/colors.toml` (default `~/.local/state`), then `$XDG_CONFIG_HOME/omarchy/current/theme/colors.toml` (default `~/.config`). Background, foreground, accent and light/dark preference follow that file. Nonrecursive ancestor directory watches cover atomic file/directory replacement, symlink switches, resolved targets and initially missing paths. Reads are debounced after filesystem events; there is no periodic theme polling. Failed watches do not crash the app and can be retried on another ancestor event. Watches stop when the application exits. Website styling remains under website control.

## Keyboard controls

| Keys | Action |
| --- | --- |
| Ctrl+L | Show address strip and select the current URL |
| Enter in address strip | Open an HTTP(S) address |
| Escape in address strip | Cancel and return focus to the page |
| Alt+Left / Alt+Right | Back / forward |
| Ctrl+R / F5 | Reload |
| Escape while loading | Stop loading |
| Ctrl+T | New active tab at X home, with its address selected |
| Ctrl+W | Close active tab; the last tab closes its window |
| Ctrl+Shift+W | Close window |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab, wrapping |
| Ctrl+Page Down / Ctrl+Page Up | Next / previous tab, wrapping |
| Ctrl+1 through Ctrl+8 | Select that position; unavailable positions do nothing |
| Ctrl+9 | Select last tab |
| Ctrl+Shift+T | Reopen the last closed URL as an active tab |
| Ctrl+N | New independent window |
| F11 | Toggle fullscreen |
| Ctrl+Plus / Ctrl+Equal / Ctrl+Minus | Zoom in / out |
| Ctrl+0 | Reset zoom |
| Alt+Home | Open X home |

When the address strip is closed and the page is idle, Escape reaches the website. The native window title follows the active page. HTTP(S) popup requests open foreground tabs in their originating window without an opener relationship. Ctrl+click and middle-click on links open background tabs and preserve the current page and focus. Ordinary links navigate the current tab. The native tabstrip is one keyboard focus group: arrows select neighboring tabs while keeping focus there; Tab enters the selected page or its open address entry. Ctrl+W closes the selected tab without visiting each close control.

Reopen keeps at most 20 closed HTTP(S) URLs per window in memory. It opens a fresh page: closed history, forms and scroll position are not restored. Closing the window discards its reopen stack. Ctrl+N creates an independent window with its own tabs and drafts; windows share the persistent WebKit profile.

## Profiles and security

The persistent WebKit profile is `$XDG_DATA_HOME/omawser/webkit` (default `~/.local/share/omawser/webkit`). The plugin leaves existing profiles in place; the application does not move or import them automatically. Cookies and website data are separate from Electron, Chromium, and the experimental Qt profile; nothing is imported. **WebKit cache is co-located in that profile:** Tauri/Wry's Linux API supplies the same base directory for data and cache, so it cannot independently honor `$XDG_CACHE_HOME` for WebKit cache. No cache symlinks or patched dependencies are used. Theme config honors `$XDG_CONFIG_HOME`; this app writes no custom config file there.

Only HTTP(S) navigation is accepted, including redirects and popup destinations. Tauri capabilities are empty; no Tauri plugins or application commands are registered. The Omarchy service plugin is separate from the native application. Remote WebViews have the Tauri bootstrap scripts and script message endpoint removed, and the `tauri:` asset and `ipc:` protocols are replaced by unconditional rejection handlers. The asset protocol is disabled. Native WebKit hooks deny permission requests (including camera, microphone, geolocation and notifications); no remote URL is sent to a shell or external protocol opener. TLS validation, WebKit sandbox settings and the system user agent remain unchanged.

Downloads with HTTP(S) request and response URLs ask for a native save destination with overwrite confirmation. Cancelling writes no download. A single native handler per WebKit context resolves the live originating tab/window, so downloads still work after the first tab or window closes. If WebKit omits the origin, only a unique active window or the sole live window sharing that context can own the dialog; ambiguous or closed owners cancel. An approved destination stays fixed when tabs switch. Remote strings are shown as plain text, never GTK markup. Site-created popups requiring `window.opener`, including some OAuth flows, may not work.

The Linux integration owns the native navigation policy and download-started handlers. Tauri 2.12.0 / locked Wry 0.57.0 install early handlers that otherwise consume modified links or retain a closed tab for downloads. Omawser verifies the expected per-instance handler structure, replaces only those handlers, and refuses new remote tabs if it changes. Every tab, including reopened tabs, installs HTTP(S) policy, script removal and permission denial before loading remote content. Dependency upgrades must rerun the native security and download smoke.

