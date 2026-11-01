# Install and build

Omawser is a standalone Omarchy desktop application built with Tauri. It is not an Omarchy plugin or a bundled system application. The steps below install it for the current user without changing system packages or desktop defaults.

## Install

You need an Omarchy session, Git, Rust/Cargo, a C toolchain, and the Arch build dependencies: `base-devel`, `pkgconf`, `gtk3`, `webkit2gtk-4.1`, `libappindicator-gtk3`, and `librsvg`. Install any missing prerequisites using your usual Arch package workflow. Keep system WebKitGTK updated. If using rustup, make Cargo available on PATH.

Clone the source to the fixed location used by the launcher, then build the release binary:

```sh
mkdir -p "$HOME/.local/share/omawser"
git clone https://github.com/komagata/omawser.git "$HOME/.local/share/omawser/source"
cargo build --release --locked \
  --manifest-path "$HOME/.local/share/omawser/source/src-tauri/Cargo.toml" \
  --target-dir "$HOME/.local/share/omawser/source/src-tauri/target"
```

Install a small command wrapper and register the desktop entry:

```sh
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
cat > "$HOME/.local/bin/omawser" <<'EOF'
#!/bin/sh
exec "$HOME/.local/share/omawser/source/scripts/omawser" "$@"
EOF
chmod +x "$HOME/.local/bin/omawser"
cp "$HOME/.local/share/omawser/source/scripts/omawser.desktop" \
  "$HOME/.local/share/applications/omawser.desktop"
```

The wrapper uses the source-tree launcher so its NVIDIA Wayland compatibility handling remains active. Keep the source directory in place while using the installed command and desktop entry. No default browser or keybindings are changed.

## Launch

Run `omawser` from any directory, or search for **Omawser** in the app launcher. To open a URL directly:

```sh
omawser https://x.com/home
```

If the command is not found, add the local bin directory to PATH for the current shell and try again:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Enter an address with `Ctrl+L`; see [all browser controls](browser.md#keyboard-controls).

## Update

Close Omawser windows first. For an existing installation, update the source checkout and rebuild:

```sh
git -C "$HOME/.local/share/omawser/source" pull --ff-only
cargo build --release --locked \
  --manifest-path "$HOME/.local/share/omawser/source/src-tauri/Cargo.toml" \
  --target-dir "$HOME/.local/share/omawser/source/src-tauri/target"
```

Launch the app again to use the rebuilt version.

## Uninstall

Remove the command wrapper and desktop entry:

```sh
rm "$HOME/.local/bin/omawser" \
  "$HOME/.local/share/applications/omawser.desktop"
```

To remove the downloaded source and build files too, remove only its source directory:

```sh
rm -rf "$HOME/.local/share/omawser/source"
```

The WebKit profile is kept at `$XDG_DATA_HOME/omawser/webkit` (by default `~/.local/share/omawser/webkit`). Uninstalling does not remove it. Remove that profile separately only if you also want to delete saved website data. Omawser does not import another browser's profile. See [profile and security details](browser.md#profiles-and-security).

## Build from source

For development, clone the repository anywhere, build with Cargo, and run the source-tree launcher from that checkout:

```sh
git clone https://github.com/komagata/omawser.git
cd omawser
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target
./scripts/omawser
```

The explicit target directory places the binary where the launcher expects it, even if `CARGO_TARGET_DIR` is set globally. The launcher applies the [NVIDIA compatibility setting](browser.md#nvidia-on-wayland).
