# Setup

## Requirements

Use an Omarchy session with Rust/Cargo, Git, a C toolchain, and the Arch build dependencies: `base-devel`, `pkgconf`, `gtk3`, `webkit2gtk-4.1`, `libappindicator-gtk3`, and `librsvg`. Keep system WebKitGTK updated. If using rustup, make Cargo available on PATH.

## Build and launch

```sh
git clone https://github.com/komagata/omawser.git
cd omawser
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target
./scripts/omawser
# Optional HTTP(S) destination:
./scripts/omawser https://x.com/home
```

The explicit target directory places the binary where the source-tree launcher expects it, even if `CARGO_TARGET_DIR` is set globally. The launcher applies the [NVIDIA compatibility setting](browser.md#nvidia-on-wayland). Enter a web address with `Ctrl+L`; see [all browser controls](browser.md#keyboard-controls).

Run the launcher from this checkout. Building does not place an executable on PATH or install a desktop entry. For manual developer integration, `scripts/omawser.desktop` is a template whose `Exec=omawser %u` requires a separately configured launcher on PATH. No default browser or keybindings are changed automatically.

## Update

Close browser windows to use the rebuilt version on the next launch, then run from the checkout:

```sh
git pull
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target
./scripts/omawser
```

## Browser data

Cookies and website data are stored in `$XDG_DATA_HOME/omawser/webkit` (default `~/.local/share/omawser/webkit`). Rebuilding or removing the source checkout does not delete this profile. The application does not move or import other browser profiles. See [profile and security details](browser.md#profiles-and-security).
