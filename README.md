# Omawser

A keyboard-first Omarchy themed browser built with **Tauri 2, Rust and the system WebKitGTK**. Each window owns live native WebViews in a native GTK tab notebook. The top tab strip appears only with two or more tabs; one tab gives that space back to the website. Each tab has its own temporary native address entry. No HTML toolbar, iframe pages or extra hidden renderer is used.

## Build and run (Linux)

Requires Rust/Cargo, a C toolchain, `pkg-config`, GTK 3, WebKitGTK 4.1, and the Tauri Linux build dependencies (on Arch: `base-devel`, `gtk3`, `webkit2gtk-4.1`, `libappindicator-gtk3`, `librsvg`). This checkout was compiled against GTK 3.24.52 and WebKitGTK 2.54.1. Keep the system WebKitGTK package updated. `Cargo.lock` pins the Rust dependency resolution, including Tauri 2.12.0.

```sh
git clone https://github.com/komagata/omawser.git
cd omawser
# If Cargo is installed through rustup, make it available on PATH:
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --manifest-path src-tauri/Cargo.toml
./src-tauri/target/release/omawser
./src-tauri/target/release/omawser https://x.com/home
# Source-tree launcher, also used by npm start:
./scripts/omawser https://x.com/home
```

The PATH setting above uses the standard rustup installation directory; adjust it if Cargo is installed elsewhere. Node/npm is not needed to build or run Tauri. `scripts/omawser` is the source-tree launcher; `scripts/omawser.desktop` is a desktop-entry template (`Exec=omawser %u`) for a launcher installed on PATH; these commands do not install it or change the default browser.

## NVIDIA on Wayland

On this development host, the unmodified WebKitGTK renderer exits with Wayland `Missing acquire timeline`; setting `__NV_DISABLE_EXPLICIT_SYNC=1` alone leaves a black page. Visible page rendering was verified with `WEBKIT_DISABLE_DMABUF_RENDERER=1`. The source-tree launcher applies that setting only for a Wayland session with an NVIDIA PCI GPU, unless `GDK_BACKEND=x11` is explicit. It preserves any existing value, including `0` or an empty value. Direct binary launches do not apply this workaround; use the launcher or explicitly prefix the command:

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

The persistent WebKit profile is `$XDG_DATA_HOME/omawser/webkit` (default `~/.local/share/omawser/webkit`). Existing local profiles are preserved by an offline directory rename during installation; the app does not move or import them automatically. Cookies and website data are separate from Electron, Chromium, and the experimental Qt profile; nothing is imported. **WebKit cache is co-located in that profile:** Tauri/Wry's Linux API supplies the same base directory for data and cache, so it cannot independently honor `$XDG_CACHE_HOME` for WebKit cache. No cache symlinks or patched dependencies are used. Theme config honors `$XDG_CONFIG_HOME`; this app writes no custom config file there.

Only HTTP(S) navigation is accepted, including redirects and popup destinations. Tauri capabilities are empty; no plugins or application commands are registered. Remote WebViews have the Tauri bootstrap scripts and script message endpoint removed, and the `tauri:` asset and `ipc:` protocols are replaced by unconditional rejection handlers. The asset protocol is disabled. Native WebKit hooks deny permission requests (including camera, microphone, geolocation and notifications); no remote URL is sent to a shell or external protocol opener. TLS validation, WebKit sandbox settings and the system user agent remain unchanged.

Downloads with HTTP(S) request and response URLs ask for a native save destination with overwrite confirmation. Cancelling writes no download. A single native handler per WebKit context resolves the live originating tab/window, so downloads still work after the first tab or window closes. If WebKit omits the origin, only a unique active window or the sole live window sharing that context can own the dialog; ambiguous or closed owners cancel. An approved destination stays fixed when tabs switch. Remote strings are shown as plain text, never GTK markup. Site-created popups requiring `window.opener`, including some OAuth flows, may not work.

The Linux integration owns the native navigation policy and download-started handlers. Tauri 2.12.0 / locked Wry 0.57.0 install early handlers that otherwise consume modified links or retain a closed tab for downloads. Omawser verifies the expected per-instance handler structure, replaces only those handlers, and refuses new remote tabs if it changes. Every tab, including reopened tabs, installs HTTP(S) policy, script removal and permission denial before loading remote content. Dependency upgrades must rerun the native security and download smoke.

## Validation and current limits

```sh
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --lib --test core --test tabs --test theme_watch
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo build --release --offline --locked --manifest-path src-tauri/Cargo.toml
cargo build --offline --locked --manifest-path src-tauri/Cargo.toml --features gui-smoke
omabox run --net isolated -- env WEBKIT_DISABLE_DMABUF_RENDERER=1 python3 scripts/tauri-smoke.py --binary "$PWD/src-tauri/target/debug/omawser"
```

Build on the host, then run GUI checks in omabox. `--binary` skips Cargo inside the box, whose HOME is fresh. The wrapper creates isolated HOME/config/data/cache/state/download directories under `/tmp`, serves local HTTP fixtures inside the isolated network and runs full and second-process persistence phases. The opt-in test driver is absent from ordinary builds; smoke-only home actions use the loopback fixture instead of X. Logs are file-backed and each phase has a bounded deadline with cleanup of its own process group.

The smoke sends native GTK/GDK keys and buttons, tests tab lifecycle and wrapping/direct selection, per-tab drafts/live form/JavaScript/zoom, background links and errors, reopen restrictions, window creation/closure, native theme updates, profile location and absent Tauri endpoints. It retains history, retry, forbidden navigation/redirect, geolocation denial and native download checks. Save and cancel are exercised after the original tab closes and again after the original window closes; saved fixture bytes and restart cookies/localStorage are checked. This is synthetic native-event coverage, not a physical keyboard or IME test.

Independent omabox checks confirmed the one/two/one-tab layout and reclaimed height, actual tab/window shortcuts and modified link clicks, dark/light themes, and selected-tab visibility with 14 tabs at a 480px window width. These checks use local fictional pages and isolated profiles. Authenticated X, posting, Japanese IME, real hardware, video/DRM and Google/Apple OAuth are unverified. Extensions, a password manager and session restore are not implemented. No runtime performance improvement is claimed.

## Preserved Electron fallback

Original Electron files, tests and its profile are retained unchanged:

```sh
npm run start:electron -- https://x.com/home
# equivalent, requires the system electron43 package:
electron43 . https://x.com/home
```

Electron uses `$XDG_CONFIG_HOME/omawser/Partitions/web` (default `~/.config/omawser/Partitions/web`). The separate `native/` Qt experiment is also preserved. The source-tree launcher does not change desktop launchers or the default browser.

## Historical Electron/Qt browser benchmarks

`perf/benchmark.py` uses Python 3's standard library on Linux. It serves the same local scrolling page for every browser; no app measurement hooks or external assets are used. Browser commands are JSON argv arrays (no shell interpretation), with the fixture HTTP URL appended as the final positional argument. The browser must open that URL in a **new process** and honor the isolated XDG directories. Do not use a wrapper that forwards to an existing browser/profile.

```sh
# Quick protocol/browser smoke: one fresh sample per engine.
python3 perf/benchmark.py --config perf/electron-comparison.json --runs 1 --output /tmp/omawser-smoke.json
# Comparison: three runs per engine, interleaved Electron/Qt in each round.
python3 perf/benchmark.py --config perf/electron-comparison.json --runs 3 --output /tmp/omawser-comparison.json
# Tests (the HTTP integration tests need permission to bind loopback).
python3 -m unittest discover -s perf -p 'test_*.py'
```

Run the commands from the repository root. The supplied config compares the preserved Electron implementation (`electron43 .`) with the native Release binary (`./native/build/omawser-qt`); build it first using [native/README.md](native/README.md). Paths are relative to the repository root, so the config remains usable after temporary workspaces are removed. Historical baseline reproduction belongs in the measured report. No Electron-specific API is involved. Optional `cwd` selects the command's working directory. `--order sequential` groups all runs per engine; default `interleaved` reduces time-order bias but does not eliminate it. Keep configuration/output files outside temporary sample profiles.

Each sample receives fresh temporary `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` and `XDG_DATA_HOME`. Startup ends at the first two-rAF ready POST, before native compositor mapping or paint; this remains a launch/navigation/HTTP proxy. After ready, the fixture waits for 500 ms of continuously focused, visible, same-geometry foreground state before clearing pre-measurement focus/visibility observations and starting its 500 ms rAF calibration. A focus, visibility or geometry change during calibration or later measured phases invalidates the sample; ready focus is retained as diagnostic metadata and is not by itself a rejection. The host `HOME`, `XDG_STATE_HOME`, display environment and sandbox are inherited; this app only reads the actual theme state. Arbitrary commands must likewise avoid writing host state or using host browser profiles. This is profile isolation, not a filesystem security boundary. No sandbox/GPU/web-security flags are altered. Cleanup sends signals only to the launched process group and identified descendants, with PID/start-time checks, and deletes its own profile. It never sends a broad process-name kill. The server binds `127.0.0.1` on an ephemeral port with an unpredictable per-sample path, caps POST bodies at 128 KiB and uses bounded socket/control timeouts. It closes after the sample.

Before measurement, use equal desktop conditions: same monitor and refresh rate, equal **page inner viewport and DPR**, same focus, no address prompt, no overlapping window, stable desktop workload and power policy. A Hyprland tile can override requested window sizes. The harness does not change compositor settings or window rules. `--viewport WIDTH HEIGHT DPR` makes the expected geometry explicit; otherwise the first foreground page establishes it. Each sample records ready/calibration/scroll metadata, userAgent, focus and visibility observations. A mismatch, hidden/unfocused page, focus loss, missing scroll, changing process tree or unreadable memory makes the sample invalid; invalid samples are retained in raw JSON and excluded from aggregates. Exit status is 1 when any sample is invalid. Ensure the content page actually holds focus rather than browser chrome.

Metrics in the raw `samples` array and per-label `aggregates`:

- **Startup, ms:** monotonic time immediately before process launch to server receipt of the page's ready POST, sent after DOM creation and two rAFs. Includes launch, navigation and loopback delivery. It is not a first-paint or user-perceived readiness measurement.
- **Settled RAM, KiB:** sums each distinct process in the launched parent/descendant tree (including orphaned children in its new session) once. PSS comes from `/proc/PID/smaps_rollup`; RSS comes from `VmRSS` in `/proc/PID/status` and is a kernel approximation. RSS double-counts shared pages across processes; PSS apportions them. Process count, partial-read flags and unavailable `/proc` stat PIDs are recorded. PSS/RSS are captured after idle CPU sampling, before scrolling. Transient exited processes are not part of that RAM snapshot.
- **Idle CPU, percent of one core:** summed `utime+stime` tick deltas divided by `SC_CLK_TCK` and actual elapsed seconds, multiplied by 100. 100% is one fully used core; multiple cores can exceed 100%. Default: 2 seconds settling after a 500 ms rAF calibration, then a 3 second CPU interval. There are only two `/proc` scans, with scan durations recorded; their midpoint times approximate per-process observation times. Tick quantization and scan skew limit short samples. PID reuse and tree changes invalidate the CPU comparison rather than silently counting different processes.
- **Scroll scheduling:** the page runs `scrollTo` from rAF for approximately 2 seconds at 2 CSS px/ms, recording actual distance, duration, every rAF interval, median and interpolated p95. A long interval exceeds 1.5 times the median of the preceding 500 ms calibration. There is one blocked control HTTP request during idle, no polling/rAF loop during the CPU interval. This is a **JavaScript rAF scheduling proxy**, not input-to-photon latency, compositor smoothness or X/timeline performance. It misses dropped rendered frames when JS still runs on schedule.

Aggregates report median/min/max over valid samples, with valid/invalid counts. Three samples are descriptive, not statistically strong evidence; inspect raw intervals, process completeness and logs before interpreting differences. Profiles are cold but the OS file cache is **warm/uncontrolled**; the harness neither drops caches nor calls these cold-launch measurements. Chromium version appears in userAgent; record the runtime (`electron43 --version`) and any Qt/WebEngine version separately when comparing builds. The startup focus repair is intended to keep the remote document focused when the native window first maps or regains focus, while leaving an open address prompt focused on its existing draft. The [2026-09-30 measured report](perf/PERFORMANCE.md) records the validated desktop comparison and its limits; the historical control received the same focus fix.

## Experimental native candidate

[native/README.md](native/README.md) describes the minimal C++/Qt WebEngine comparison candidate, Release build, independent persistent profile and isolated tests. It keeps the keyboard-only appearance and native address prompt. Downloads and site-created popups are denied in this experiment. The historical Electron/Qt [measured comparison](perf/PERFORMANCE.md) found the Qt candidate slower to start and larger in PSS on that desktop. These figures do not measure the Tauri version.
