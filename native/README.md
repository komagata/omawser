# Experimental Qt comparison candidate

This is a preserved minimal Qt Widgets / Qt WebEngine experiment for historical
measurement. The current default launcher uses Tauri; Electron is a preserved fallback. The [measured comparison](../perf/PERFORMANCE.md) found this candidate slower to start and larger in PSS on this desktop.

Run these commands from the repository root, using the installed Qt packages (no
CMake downloads or npm dependencies):

```sh
cmake -S native -B native/build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build native/build -j2
native/build/omawser-qt --version
native/build/omawser-qt                    # X home
native/build/omawser-qt https://x.com/home
native/build/omawser-qt http://127.0.0.1:8000/fixture
```

The local Release build uses `-O3 -DNDEBUG`. C++17 and Qt 6.8 or later are required;
this environment has Qt / Qt WebEngine 6.11.2. `--version` prints the runtime Qt,
WebEngine, Chromium and Chromium security-patch versions and immediately exits.
It does not create QApplication, profiles, windows or network requests. The local
build reports Chromium 140.0.7339.225 with security patch 151.0.7922.71. These are
Qt's reported version values, not an assertion that the engine is Chromium 151.
The default user agent is retained, including its actual Chromium and QtWebEngine
tokens. The signed-out X login page was rendered on the real desktop; authenticated usage remains unverified. No version spoofing or injected website CSS is used.

The frameless window requests 1200×850, has a 1px theme accent frame, and contains
one WebEngine view. The application/desktop ID is `omawser-qt`; no desktop
entry or launcher is installed. The compositor can override size and placement.
The native address strip reserves 48px when open and disappears otherwise. The
website fills the remaining bounds. The experiment supports Ctrl+L for the address strip, Ctrl+N/W for windows,
F11, history, reload, and zoom. The main README describes the current Tauri
version, whose tab controls do not apply to this experiment. Shortcuts are
handled for WebEngine's focused child as well as the native input. Qt handles IME
composition; shortcut commands are suspended while preedit is active. Window
refocus returns focus to the page or the open address draft without replacing it.
Web-page context menus are disabled. Invalid addresses retain the input and focus; page
load failures open the input and allow Ctrl+R to retry the failed target.

Theme state is read from `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`
(default `~/.local/state`), falling back to `$XDG_CONFIG_HOME/omarchy/current/theme/colors.toml`
(default `~/.config`). Background, foreground, accent, muted, lighter_background,
and dark/light mode use the same defaults and parser rules as Electron. The
native palette and Qt color scheme follow mode; whether each website follows
`prefers-color-scheme` is an engine/site behavior to check. QFileSystemWatcher
watches existing ancestors, the file, and resolved symlink target ancestors. It
rearms after a 50ms event debounce, covering atomic file/directory replacements,
active-theme symlink switches and creation of missing paths. There is no polling
timer. If a watch cannot be registered, another surviving ancestor event can
recover it; there is no periodic retry.

## Profile and limits

A named persistent `omawser-qt` profile shares cookies across this candidate's
windows. Storage is `$XDG_DATA_HOME/omawser-qt` (default
`~/.local/share/omawser-qt`); cache is `$XDG_CACHE_HOME/omawser-qt` (default
`~/.cache/omawser-qt`). This never imports or reuses the Electron profile.
Session cookies also persist. No credentials or account actions are requested.

Only HTTP(S) main-frame navigation and redirects are accepted. Unknown schemes
are blocked and never launched externally. Permission requests and filesystem
access requests are denied; clipboard read/write access from JavaScript is
restricted. No WebChannel/native bridge is exposed. Sandbox, web security and
certificate checks retain Qt defaults. Website console messages are suppressed
because they can contain browsing URLs or private data; application code does
not log visited URLs. Qt/Chromium's own diagnostics can still print messages.

Experimental parity gaps: downloads are cancelled and site-created windows/popups
are denied (Ctrl+N still opens a separate candidate window with the shared
profile). OAuth relying on popups cannot work in this candidate. There are no
tabs, extensions, password manager, session restore or browser-wide search UI.
Authenticated X, Japanese IME on the desktop, video playback and OAuth have not
been verified. This candidate is not installed or selected as the default browser.

## Validation

```sh
ctest --test-dir native/build --output-on-failure
npm test
# Real desktop session, with loopback sockets allowed; temporary profile/theme only:
native/test-gui.sh
# Engine-neutral comparison, from repository root, after equal desktop conditions:
python3 perf/benchmark.py --config perf/electron-comparison.json --runs 3 --output /tmp/omawser-comparison.json
```

Core tests use QCoreApplication, without a display or WebEngine. They check URL
rejection/normalization, theme parsing, initially missing paths, fallback, atomic
file and directory replacement, symlink switching and target-file replacement.
The GUI runner creates fresh config/data/cache/state roots under
`/tmp/omawser-qt-test-*`, writes a fictional theme, and removes them on exit.
The GUI binary refuses existing data/cache/config files, paths outside that prefix,
symlinked roots and a missing or symlinked theme fixture. It runs a local HTTP
fixture and checks page focus, profile paths, frame/prompt bounds, actual key
presses on the focused widget, invalid/valid Enter, Esc, address draft refocus,
history, reload, connection-failure retry, zoom, fullscreen, theme replacement and denied file navigation
and popups. It does not contact X. Ctrl+N/W, Alt+Home, permission prompts, IME and
visual X rendering still require desktop checks.

The worker sandbox built Release and passed core / Electron tests, but cannot
access the real display or bind the loopback fixture. The coordinator subsequently passed the real GUI suite and measured both engines; see the [report](../perf/PERFORMANCE.md). Do not turn off the sandbox or weaken
foreground checks to obtain samples. Optional offscreen execution can diagnose
nonvisual test setup; it is not evidence of desktop or benchmark parity.

API references: [QWebEnginePage](https://doc.qt.io/qt-6/qwebenginepage.html),
[QWebEngineProfile](https://doc.qt.io/qt-6/qwebengineprofile.html),
[QFileSystemWatcher](https://doc.qt.io/qt-6/qfilesystemwatcher.html).
