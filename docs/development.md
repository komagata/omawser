# Development

The standalone browser uses Rust, Tauri and native GTK/WebKitGTK controls. See [setup](setup.md) for prerequisites, the release build and the source-tree launcher.

## Validation and current limits

```sh
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --lib --test core --test tabs --test theme_watch
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
python3 scripts/test_graphics_compat.py
cargo build --release --offline --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target
cargo build --offline --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target --features gui-smoke
omabox run --net isolated -- env WEBKIT_DISABLE_DMABUF_RENDERER=1 python3 scripts/tauri-smoke.py --binary "$PWD/src-tauri/target/debug/omawser"
```

Build outside the desktop test environment, then run GUI checks in omabox. Ordinary builds omit the smoke driver; use the `gui-smoke` build above for this check. `--binary` skips Cargo inside the box, whose HOME is fresh. The wrapper creates isolated HOME/config/data/cache/state/download directories under `/tmp`, serves local HTTP fixtures inside the isolated network and runs full and second-process persistence phases. The opt-in test driver is absent from ordinary builds; smoke-only home actions use the loopback fixture instead of X. Logs are file-backed and each phase has a bounded deadline with cleanup of its own process group.

The smoke sends native GTK/GDK keys and buttons, tests tab lifecycle and wrapping/direct selection, per-tab drafts/live form/JavaScript/zoom, background links and errors, reopen restrictions, window creation/closure, native theme updates, profile location and absent Tauri endpoints. It retains history, retry, forbidden navigation/redirect, geolocation denial and native download checks. Save and cancel are exercised after the original tab closes and again after the original window closes; saved fixture bytes and restart cookies/localStorage are checked. This is synthetic native-event coverage, not a physical keyboard or IME test.

Independent omabox checks confirmed the one/two/one-tab layout and reclaimed height, actual tab/window shortcuts and modified link clicks, dark/light themes, and selected-tab visibility with 14 tabs at a 480px window width. These checks use local fictional pages and isolated profiles. Authenticated X, posting, Japanese IME, real hardware, video/DRM and Google/Apple OAuth are unverified. Extensions, a password manager and session restore are not implemented. No runtime performance improvement is claimed.
