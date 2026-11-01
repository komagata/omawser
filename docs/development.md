# Development and historical implementations

The supported user flow is the [Omarchy 4 service plugin](../README.md). The Tauri application remains an independent process. Node/npm is not required for Tauri.

## Direct source run

From an Omarchy 4 source checkout with the [prerequisites](../README.md#install) installed:

```sh
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target
./scripts/omawser
# Optional HTTP(S) destination for direct development use:
./scripts/omawser https://x.com/home
```

The source launcher applies the [NVIDIA compatibility setting](browser.md#nvidia-on-wayland). Direct binary launches bypass it. `scripts/omawser.desktop` is only a template for a separately installed launcher on PATH; the plugin does not install it or set the default browser.

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
omabox run --net isolated -- python3 perf/benchmark.py --config perf/electron-comparison.json --runs 1 --output /tmp/omawser-smoke.json
# Comparison: three runs per engine, interleaved Electron/Qt in each round.
omabox run --net isolated -- python3 perf/benchmark.py --config perf/electron-comparison.json --runs 3 --output /tmp/omawser-comparison.json
# Tests (the HTTP integration tests need permission to bind loopback).
python3 -m unittest discover -s perf -p 'test_*.py'
```

Run the commands from the repository root. The supplied config compares the preserved Electron implementation (`electron43 .`) with the native Release binary (`./native/build/omawser-qt`); build it first using [native/README.md](../native/README.md). Paths are relative to the repository root, so the config remains usable after temporary workspaces are removed. Historical baseline reproduction belongs in the measured report. No Electron-specific API is involved. Optional `cwd` selects the command's working directory. `--order sequential` groups all runs per engine; default `interleaved` reduces time-order bias but does not eliminate it. Keep configuration/output files outside temporary sample profiles.

Each sample receives fresh temporary `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` and `XDG_DATA_HOME`. Startup ends at the first two-rAF ready POST, before native compositor mapping or paint; this remains a launch/navigation/HTTP proxy. After ready, the fixture waits for 500 ms of continuously focused, visible, same-geometry foreground state before clearing pre-measurement focus/visibility observations and starting its 500 ms rAF calibration. A focus, visibility or geometry change during calibration or later measured phases invalidates the sample; ready focus is retained as diagnostic metadata and is not by itself a rejection. The host `HOME`, `XDG_STATE_HOME`, display environment and sandbox are inherited; this app only reads the actual theme state. Arbitrary commands must likewise avoid writing host state or using host browser profiles. This is profile isolation, not a filesystem security boundary. No sandbox/GPU/web-security flags are altered. Cleanup sends signals only to the launched process group and identified descendants, with PID/start-time checks, and deletes its own profile. It never sends a broad process-name kill. The server binds `127.0.0.1` on an ephemeral port with an unpredictable per-sample path, caps POST bodies at 128 KiB and uses bounded socket/control timeouts. It closes after the sample.

Before measurement, use equal desktop conditions: same monitor and refresh rate, equal **page inner viewport and DPR**, same focus, no address prompt, no overlapping window, stable desktop workload and power policy. A Hyprland tile can override requested window sizes. The harness does not change compositor settings or window rules. `--viewport WIDTH HEIGHT DPR` makes the expected geometry explicit; otherwise the first foreground page establishes it. Each sample records ready/calibration/scroll metadata, userAgent, focus and visibility observations. A mismatch, hidden/unfocused page, focus loss, missing scroll, changing process tree or unreadable memory makes the sample invalid; invalid samples are retained in raw JSON and excluded from aggregates. Exit status is 1 when any sample is invalid. Ensure the content page actually holds focus rather than browser chrome.

Metrics in the raw `samples` array and per-label `aggregates`:

- **Startup, ms:** monotonic time immediately before process launch to server receipt of the page's ready POST, sent after DOM creation and two rAFs. Includes launch, navigation and loopback delivery. It is not a first-paint or user-perceived readiness measurement.
- **Settled RAM, KiB:** sums each distinct process in the launched parent/descendant tree (including orphaned children in its new session) once. PSS comes from `/proc/PID/smaps_rollup`; RSS comes from `VmRSS` in `/proc/PID/status` and is a kernel approximation. RSS double-counts shared pages across processes; PSS apportions them. Process count, partial-read flags and unavailable `/proc` stat PIDs are recorded. PSS/RSS are captured after idle CPU sampling, before scrolling. Transient exited processes are not part of that RAM snapshot.
- **Idle CPU, percent of one core:** summed `utime+stime` tick deltas divided by `SC_CLK_TCK` and actual elapsed seconds, multiplied by 100. 100% is one fully used core; multiple cores can exceed 100%. Default: 2 seconds settling after a 500 ms rAF calibration, then a 3 second CPU interval. There are only two `/proc` scans, with scan durations recorded; their midpoint times approximate per-process observation times. Tick quantization and scan skew limit short samples. PID reuse and tree changes invalidate the CPU comparison rather than silently counting different processes.
- **Scroll scheduling:** the page runs `scrollTo` from rAF for approximately 2 seconds at 2 CSS px/ms, recording actual distance, duration, every rAF interval, median and interpolated p95. A long interval exceeds 1.5 times the median of the preceding 500 ms calibration. There is one blocked control HTTP request during idle, no polling/rAF loop during the CPU interval. This is a **JavaScript rAF scheduling proxy**, not input-to-photon latency, compositor smoothness or X/timeline performance. It misses dropped rendered frames when JS still runs on schedule.

Aggregates report median/min/max over valid samples, with valid/invalid counts. Three samples are descriptive, not statistically strong evidence; inspect raw intervals, process completeness and logs before interpreting differences. Profiles are cold but the OS file cache is **warm/uncontrolled**; the harness neither drops caches nor calls these cold-launch measurements. Chromium version appears in userAgent; record the runtime (`electron43 --version`) and any Qt/WebEngine version separately when comparing builds. The startup focus repair is intended to keep the remote document focused when the native window first maps or regains focus, while leaving an open address prompt focused on its existing draft. The [2026-09-30 measured report](../perf/PERFORMANCE.md) records the validated desktop comparison and its limits; the historical control received the same focus fix.

## Experimental native candidate

[native/README.md](../native/README.md) describes the minimal C++/Qt WebEngine comparison candidate, Release build, independent persistent profile and isolated tests. It keeps the keyboard-only appearance and native address prompt. Downloads and site-created popups are denied in this experiment. The historical Electron/Qt [measured comparison](../perf/PERFORMANCE.md) found the Qt candidate slower to start and larger in PSS on that desktop. These figures do not measure the Tauri version.
