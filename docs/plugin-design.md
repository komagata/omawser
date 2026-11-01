# Omarchy service plugin design

The root manifest registers `komagata.omawser` as a single `service` entry point hosted by Omarchy. There is no bar widget, panel, overlay, standalone Quickshell instance, install hook or automatic browser launch. Omarchy add clones/validates the files; dependencies and a release build remain explicit user steps.

## Responsibilities

`plugin/Service.qml` checks the checkout-relative `src-tauri/target/release/omawser` and `scripts/omawser` for executable permission. It resolves file URLs relative to the QML file and decodes them before passing paths as argv values. It dispatches the existing compatibility launcher, which continues to own NVIDIA/Wayland environment handling. The launcher and native browser are unchanged.

The service uses argv arrays for both the executable probe and detached launch, with no constructed shell command. It accepts no URLs, commands or path arguments over IPC. The probe is asynchronous with a three-second watchdog. No output is accumulated. There is no polling, build, dependency installation, permission change or credential/profile access.

## IPC contract

Use `omarchy-shell komagata.omawser METHOD` while enabled:

| Method | Immediate response | Effect |
| --- | --- | --- |
| `status` | Bounded JSON with `state`, `message`, `busy`, `lastAction` | Inspect only; does not check files or launch |
| `refresh` | `checking`, or `busy` during another check | Probe executable readiness, without launching |
| `open` | `checking`, or `busy` during another check | Probe again, then dispatch one detached launch if ready |

Methods take no arguments. A rejected overlapping request is not queued. Each later accepted `open` can request another application process/window; this plugin does not deduplicate independent requests after the previous check finishes. It makes no single-instance guarantee.

States are `checking`, `unbuilt` (binary or launcher missing/not executable), `ready` (last check passed), and `failed` (probe did not finish within three seconds). `status` is a snapshot: `ready` does not mean a browser process is running, and files may change after the last check. A launch-time race is possible between the probe and detached dispatch. The launcher independently checks its binary too.

`lastAction` is `none`, `checking-refresh`, `checking-open`, `checked`, `open-rejected`, `check-failed` or `launch-requested`. Only `launch-requested` acknowledges dispatch, and it does not confirm process startup, WebKit initialization or a visible window. Read `status` after `open`; the immediate `checking` response is not a launch acknowledgement. A missing probe executable also eventually reaches `failed`. Repair the prerequisite and call `refresh` or `open` to retry.

## Lifecycle and ownership

Loading/enabling performs one readiness check, without launching. The manifest deliberately omits `keepLoaded`. Disable, removal and ordinary hot reload destroy the service, stop its probe/watchdog, release IPC, and cancel pending opens. The destruction guard discards late completion. Re-enabling creates a fresh service and checks again.

Detached application windows survive service disable/reload/removal. They own their own lifecycle and external WebKit profile; the plugin neither kills them nor reads/deletes their data. Removal owns the installed checkout/build output only. See the [update/removal instructions](setup.md#update). A previously running application keeps its old executable until closed.

## Tests

Static checks, from the checkout root:

```sh
omarchy plugin validate "$PWD"
qmllint -I /usr/share/omarchy/shell plugin/Service.qml
node --test test/*.test.cjs
python3 scripts/test_graphics_compat.py
python3 -m unittest discover -s perf -p 'test_*.py'
git diff --check
```

An optional portable plugin validator can add advisory checks; the installed Omarchy validator is authoritative for the target shell. QML linting needs Qt and Quickshell import/type metadata. These checks do not rebuild or exercise the native browser.

Run the functional suite only inside your own network-isolated omabox with its fresh HOME. Do not mount this plugin with `--plugin`: the test installs a temporary copy itself. Create a box from the repository root. The test places its probe fixture in the box-only `~/.local/bin`, which must precede `/usr/bin` on the shell PATH:

```sh
omabox up --new --net isolated
# Replace BOX with the exact box name printed above:
omabox -b BOX run -- python3 "$PWD/scripts/test_plugin.py"
omabox -b BOX down
```

The Python standard-library driver calls the real `omarchy-shell` IPC and plugin enable/disable/remove commands. It uses fictional executable and probe fixtures under the box HOME. The test copies the existing launcher but substitutes a browser that records its argv and waits, without WebViews or network. The fixture binary is a preinstalled symlink to a fake executable outside the watched clone, so simulated build/chmod changes exercise readiness without triggering Omarchy source hot reload. The QML entrypoint and launch argv still contain spaces and shell/URL metacharacters. It checks missing builds, refresh after a simulated build, unusual path decoding, exactly one launch per accepted request, stale readiness, busy overlap, a timed-out probe and recovery, cancellation on disable, re-enable without launch, detached lifetime, removal and preservation of fictional external profile data. Test fixtures are never part of the service's production command path. Tear down the box to stop fixture processes and discard test data.

Native graphics, focus, WebKit security and real application rendering remain separate [development checks](development.md#validation-and-current-limits); this suite makes no claims about physical input, IME, hardware or the user's desktop.
