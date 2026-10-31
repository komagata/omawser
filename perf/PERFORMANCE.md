# Omawser measurements — 2026-09-30

This report preserves measurements taken on 2026-09-30 under the former name "Oma Browser." Old command names, architecture and figures below describe the state at the time of measurement. See the [README](../README.md) for current instructions. **The current default is Tauri. These historical measurements compare Electron and Qt only; they make no performance claim about Tauri.**

At the time, the decision on this PC was to retain Electron. The C++ / Qt version worked, but in this same-page comparison its startup metric was slower and its PSS memory use was higher. The normal `oma-browser` launcher remained on Electron, with the theme-watching and initial-focus fixes applied. The Qt version was retained in `native/` for comparison. This was the historical Electron/Qt decision, not the current choice of default runtime.

The following are medians (minimum–maximum) from three runs per implementation. After implementation, builds and tests were complete, launch order was rotated as follows: "polling → updated → Qt," "updated → Qt → polling," and "Qt → polling → updated." All nine runs passed foreground, geometry and process-accounting validation.

| Implementation | Startup metric, ms | PSS, MiB | Idle CPU, % of one core | rAF interval, median / p95, ms | Process count |
| --- | ---: | ---: | ---: | ---: | ---: |
| Electron, polling (with common focus fix) | 565 (558–581) | 345.4 (345.4–346.1) | 0.66 (0.66–1.32) | 6.1 / 7.7 | 9 |
| Electron, updated | 522 (512–565) | 349.5 (346.8–350.3) | 0.99 (0.66–0.99) | 6.1 / 6.2 | 9 |
| C++ / Qt, Release prototype | 747 (735–771) | 458.6 (457.7–460.6) | 0.33 (0.33–0.33) | 16.7 / 16.7 | 5 |

**The startup metric is not time to visible content.** It runs from process launch to receipt of a notification sent after the local page creates its DOM and completes two `requestAnimationFrame` callbacks. It does not guarantee that window mapping or actual painting has completed.

PSS is a process total that reduces double-counting of shared memory; it does not include all dedicated GPU memory. Median RSS was 851.1 MiB for polling, 854.8 MiB for updated Electron, and 807.2 MiB for Qt. Because RSS counts shared regions repeatedly across processes, PSS was preferred when assessing memory use. Sharing with other applications running on the desktop also affects these values.

Idle CPU was observed for three seconds; one tick corresponds to approximately 0.33 percentage points. Qt had lower values, but this short observation alone cannot support a claim of substantial power savings. No large CPU or memory improvement was confirmed from the theme-watching change either.

The scrolling fixture was a local page with the same 240 cards, scrolled 4,000 px over two seconds. Electron's rAF interval was approximately 6.1 ms; Qt's was approximately 16.7 ms. These are JavaScript scheduling intervals, not counts of actually displayed frames or input-to-display latency. Even though Qt recorded zero long intervals, its threshold was relative to an interval equivalent to about 60 Hz, so this does not establish that it was smoother than Electron. These results do not measure actual X timeline performance.

This is a descriptive comparison of three runs each, not a statistical performance guarantee. The startup difference between the two Electron variants is not treated as a confirmed speedup. No startup or memory advantage was found for replacing Electron with Qt in this comparison.

## Changes applied at the time

- Replaced once-per-second theme-file reads with file, directory and symlink change notifications. The 50 ms debounce runs only after changes.
- Fixed focus returning to the hidden address UI when the native window first appeared. When the input is open, focus returns there while preserving its draft.
- Added a common harness to measure startup, PSS/RSS, idle CPU and rAF intervals using fresh profiles. It includes PID identity checks to avoid terminating processes outside the measurement.
- Implemented a C++17 / Qt Widgets / Qt WebEngine comparison version without permanent browser chrome. It retains square corners, a 1 px frame, a temporary Ctrl+L input and theme integration.

The polling baseline received only the same common focus fix, because the unmodified original lost page focus immediately after appearing and failed the foreground requirement. "Before" therefore means a control for comparing theme-watching approaches, not a completely untouched old version. An archive of the original source and the common focus patch are retained locally under `perf/results/2026-09-30/` and are not included in this repository.

A separate, single diagnostic measurement by process role found that the hidden address-UI renderer used approximately 34 MiB of PSS. Replacing the application with Qt also changed the runtime and rendering architecture, so that amount did not translate directly into savings. This supplementary measurement used different geometry and is not included in the comparison table above.

## Environment and conditions

Intel Core i9-14900K, NVIDIA RTX 4060, 62 GiB RAM, and an Omarchy / Hyprland Wayland session. All runs used the same 1920×1080 monitor at 164.917 Hz and scale 1. The measurement window was 1200×850, with an inner page viewport of 1198×848 and DPR 1. A temporary runtime rule made only the measured windows float; it was disabled afterward. No configuration files were changed.

Each run used empty XDG config/cache/data directories. The OS file cache was warm and uncontrolled, so this was not a cold-boot comparison. Other applications were running on the normal desktop; no caches were dropped and no system tuning was performed. After the page's foreground state and geometry had remained stable for 500 ms, the harness performed 500 ms of rAF calibration, waited two seconds, measured CPU for three seconds, and then scrolled. The page ran neither rAF nor periodic HTTP polling during idle measurement.

Electron was 43.6.0 / Chromium 150.0.7871.250 / Node 24.20.0. Qt and Qt WebEngine were 6.11.2, built in Release mode with `-O3 -DNDEBUG`. Qt reported Chromium 140.0.7339.225 and security-patch version 151.0.7922.71. Chromium versions and rendering paths also differed, so the differences cannot all be attributed to JavaScript versus C++.

## Validation at the time and remaining limits

Three Electron unit tests, eight Python harness tests and an isolated GUI smoke on the actual desktop passed. Checks covered keyboard controls, input validation, retry after errors, draft preservation, zoom, inner viewport dimensions and theme-change notifications. An additional focus check on an empty, unloaded WebContentsView was not adopted because it had no document; initial focus on real pages was checked separately with an actual-desktop probe and the measurement page.

Qt Core / version tests and seven actual-desktop GUI tests passed. Checks covered address entry, cancellation, history, reload, zoom, fullscreen, retry after errors, theme replacement and rejection of local-file navigation. Ctrl+L sent from the actual desktop also displayed the temporary input. The X login page and address input were visually checked; screenshots are retained locally under `perf/results/2026-09-30/` and are not included in this repository.

The authenticated X timeline, posting, video, Google/Apple OAuth and actual Japanese IME input were unverified. The Qt prototype rejects downloads and site-created popups, so popup-based OAuth cannot work. It uses a separate profile from Electron, and no existing login information was migrated. The normal launcher target and existing user profile were preserved at the time.

## Reproduction and data

Run the following from the repository root. Close permanent browser chrome and match foreground state, inner page viewport and DPR. Normal tiling can change the dimensions; mismatched samples are invalid. The harness itself does not change desktop settings. These commands reproduce the historical Electron/Qt comparison, not a Tauri benchmark.

```sh
cmake -S native -B native/build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build native/build -j2
ctest --test-dir native/build --output-on-failure
native/test-gui.sh
python3 perf/benchmark.py --config perf/electron-comparison.json --runs 3 --viewport 1198 848 1 --output /tmp/oma-comparison.json
```

The final nine-run dataset (`final-three-way.json`) records raw per-frame intervals, PID identities, aggregates, logs and launch arguments. It is retained locally under `perf/results/2026-09-30/`, together with the preliminary comparison (`electron-final.json`) and early measurements rejected for geometry or focus conditions. These records are not included in this repository. Some preliminary runs overlapped brief test work, so the conclusions use the final comparison. Paths under `/tmp` in the raw data refer to isolated copies used for measurement, not the normal launcher target.

At the time, creating Electron's address-UI renderer only when needed was a candidate for further optimization. It would trade memory use against input-opening latency and focus complexity; it was neither implemented nor measured in this work. This historical suggestion does not describe the current Tauri architecture.
