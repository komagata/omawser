# Omawser

A minimal keyboard-first browser for Omarchy.

## Build

See [prerequisites and setup](docs/setup.md).

```sh
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target
```

## Run

```sh
./scripts/omawser
```

Use `Ctrl+L` to enter an address, `Ctrl+T` to open a tab, and `Ctrl+W` to close it.

See [all browser controls](docs/browser.md#keyboard-controls) and [developer checks](docs/development.md).
