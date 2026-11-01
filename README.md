# Omawser for Omarchy 4

Omawser is a keyboard-first native browser for Omarchy 4 Quattro, distributed as an Omarchy **service plugin**. The plugin adds headless launch IPC to the existing Omarchy shell. The independent Tauri 2 / Rust / system WebKitGTK browser owns its windows and profile; enabling the plugin creates no window, bar widget, or permanent UI.

Each window keeps live native WebViews in GTK tabs. One tab hides the tab strip and gives its space back to the website. Press **Ctrl+L** to enter an HTTP(S) address. New tabs start at X home. Theme colors follow Omarchy; website styling belongs to the website.

## Install

Use an Omarchy 4 Quattro session with `omarchy plugin` support. Install Rust/Cargo, a C toolchain and the Arch build dependencies separately: `base-devel`, `pkgconf`, `gtk3`, `webkit2gtk-4.1`, `libappindicator-gtk3`, and `librsvg`. Keep system WebKitGTK updated. If using rustup, make its Cargo available on PATH. Node/npm is not required for the Tauri application.

```sh
omarchy plugin add https://github.com/komagata/omawser.git
```

Adding clones and validates the repository, leaving the plugin **disabled** for review. Plugins run as your user inside the Omarchy shell. Review the code before enabling. Add does not install dependencies, compile the browser, run install hooks, or request system privileges.

Build explicitly in the installed checkout:

```sh
plugin_dir="$HOME/.config/omarchy/plugins/komagata.omawser"
cargo build --release --locked \
  --manifest-path "$plugin_dir/src-tauri/Cargo.toml" \
  --target-dir "$plugin_dir/src-tauri/target"
omarchy plugin enable komagata.omawser
omarchy-shell komagata.omawser status
omarchy-shell komagata.omawser open
```

The explicit target directory overrides a global `CARGO_TARGET_DIR` so the binary lands where the existing launcher expects it. Omarchy's plugin registry uses the HOME-based path above. The plugin is not a full application installer and does not add a desktop entry, change the default browser, or alter keybindings.

`open` returns `checking` while it checks the executable and launcher. Inspect `status` after the check: `lastAction: "launch-requested"` acknowledges a detached launch request, not a confirmed running window. A missing executable reports `state: "unbuilt"` with a build instruction. After building while enabled, use `omarchy-shell komagata.omawser refresh` to check again. `open` always checks again too. Requests during a check return `busy` and do not queue another launch. See [IPC and lifecycle](docs/plugin-design.md) for the complete contract.

Once the window opens, press **Ctrl+L**, type an address, then Enter.

## Keyboard controls

| Keys | Action |
| --- | --- |
| Ctrl+L / Escape | Show address strip / cancel it |
| Alt+Left / Alt+Right | Back / forward |
| Ctrl+R / F5 | Reload; retry a failed destination |
| Escape while loading | Stop loading |
| Ctrl+T / Ctrl+W | New tab / close tab (last tab closes window) |
| Ctrl+Shift+T | Reopen last closed URL (fresh page) |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+Page Down / Ctrl+Page Up | Next / previous tab |
| Ctrl+1 through Ctrl+8 / Ctrl+9 | Select tab position / last tab |
| Ctrl+N / Ctrl+Shift+W | New independent window / close window |
| F11 | Fullscreen |
| Ctrl+Plus / Ctrl+Equal / Ctrl+Minus / Ctrl+0 | Zoom in / in / out / reset |
| Alt+Home | X home |

The address strip accepts only HTTP(S). Tabs preserve live page state; reopening restores only the URL. [Native browser details](docs/browser.md) cover tab focus, links, downloads, theme watching and graphics compatibility.

For an optional launcher key, add a binding yourself to your existing Hyprland configuration, following its current syntax. For a Lua binding configuration, for example:

```lua
hl.bind("SUPER + B", hl.dsp.exec_cmd("omarchy-shell komagata.omawser open"))
```

Check for an existing binding before assigning that key. The plugin does not edit your configuration.

## Update

Updates pull source files; the ignored build output is not rebuilt. Close browser windows yourself if you want the new application version immediately, then:

```sh
omarchy plugin disable komagata.omawser
omarchy plugin update komagata.omawser
plugin_dir="$HOME/.config/omarchy/plugins/komagata.omawser"
cargo build --release --locked \
  --manifest-path "$plugin_dir/src-tauri/Cargo.toml" \
  --target-dir "$plugin_dir/src-tauri/target"
omarchy plugin enable komagata.omawser
omarchy-shell komagata.omawser status
```

If the build fails, leave the plugin disabled until the build succeeds. Enabling never opens the browser automatically. Disabling/reloading does not terminate a previously launched browser, so an existing window continues running its old executable until you close it.

## Disable or remove

```sh
omarchy plugin disable komagata.omawser
# Optional: delete the installed clone, including its local build output.
omarchy plugin remove komagata.omawser
```

Disabling unregisters this service's IPC and cancels pending checks. Removal deletes the installed clone and build output, not external browser data. Neither operation kills detached browser windows.

The application owns its persistent WebKit data and cache at `$XDG_DATA_HOME/omawser/webkit` (default `~/.local/share/omawser/webkit`), outside the clone. Cookies and website data remain after removal. The plugin does not read, move, or delete that profile. The retained Electron and experimental Qt implementations have separate profiles; nothing is imported automatically.

## Security and limits

The browser permits HTTP(S) navigation, denies native permission requests (camera, microphone, geolocation and notifications), and leaves TLS validation and the WebKit sandbox unchanged. Its Tauri capabilities are empty and no **Tauri plugins** or application commands are registered; the **Omarchy plugin** is a separate shell service. Remote pages have no Tauri bootstrap/message endpoint or asset/IPC protocol access. Downloads ask for a native save destination and overwrite confirmation.

Some popup/OAuth flows that require `window.opener` may fail. Extensions, a password manager and session restore are not implemented. Authenticated X, posting, Japanese IME, video/DRM and Google/Apple OAuth remain unverified. No runtime performance improvement is claimed. See [browser security](docs/browser.md#profiles-and-security), [plugin design and tests](docs/plugin-design.md), and [development and historical benchmarks](docs/development.md).
