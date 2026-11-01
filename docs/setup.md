# Install and setup

## Requirements

Use an Omarchy session with `omarchy plugin` support. Install Rust/Cargo, a C toolchain, and the Arch build dependencies: `base-devel`, `pkgconf`, `gtk3`, `webkit2gtk-4.1`, `libappindicator-gtk3`, and `librsvg`. Keep system WebKitGTK updated. If using rustup, make Cargo available on PATH. Node/npm is not required.

## Build and launch

Adding the plugin clones and validates the repository but leaves it disabled. It does not install dependencies or compile the browser. Review the code before enabling; plugins run as your user in the Omarchy shell.

Build from the installed checkout:

```sh
plugin_dir="$HOME/.config/omarchy/plugins/komagata.omawser"
cargo build --release --locked \
  --manifest-path "$plugin_dir/src-tauri/Cargo.toml" \
  --target-dir "$plugin_dir/src-tauri/target"
omarchy plugin enable komagata.omawser
omarchy-shell komagata.omawser status
omarchy-shell komagata.omawser open
```

The explicit target directory places the binary where the launcher expects it, even if `CARGO_TARGET_DIR` is set globally. Enabling does not open a window. `open` first checks readiness and returns `checking`; run `status` after the check. `lastAction: "launch-requested"` confirms dispatch only, not a visible window. If the binary is missing, build it and run `omarchy-shell komagata.omawser refresh`, or call `open` again. Requests during a check return `busy` and are not queued.

Enter a web address with `Ctrl+L`. The plugin does not add a desktop entry, set the default browser, or change keybindings. Add a launcher binding yourself if desired, after checking for a conflict.

## Update

Updating pulls source files but does not rebuild ignored output. Close browser windows yourself to use the new application version immediately, then:

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

If the build fails, leave the plugin disabled until it succeeds. Existing detached browser windows continue using their old executable until closed.

## Disable or remove

```sh
omarchy plugin disable komagata.omawser
# Optional: removes the installed checkout and its local build output.
omarchy plugin remove komagata.omawser
```

Disabling unregisters the service IPC and cancels pending checks. Neither disabling nor removal closes detached browser windows. Removal deletes the checkout and build output, not browser data. The persistent WebKit profile is `$XDG_DATA_HOME/omawser/webkit` (default `~/.local/share/omawser/webkit`); cookies and website data remain after removal. The plugin does not read, move, or delete that profile. Existing Electron and experimental Qt profiles are separate and are not imported.
