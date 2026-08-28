# A Simple Autoclicker

A lightweight autoclicker for Linux and Windows. It can repeat mouse clicks or
recorded keyboard actions, stop after a duration or action count, target a fixed
screen position, save presets, and remain available from the system tray.

> **Platform status:** Linux uses GTK4/libadwaita and currently requires X11.
> Windows uses a modern DPI-aware interface and supports 64-bit Windows 10 and 11.

## Features

- Left, middle, and right mouse clicking
- Record ordinary keys, function keys, modifiers, and combinations such as Ctrl+C
- Adjustable interval from 10 ms to 60 seconds
- Optional run-duration and exact action-count limits
- Fixed-position mouse clicking with a two-second capture delay
- Configurable F6–F12 global start/stop hotkey
- Named presets saved between launches
- System tray controls for showing, starting, stopping, and quitting the app
- Modern interface on both GTK4/libadwaita (Linux) and Windows

## Install on Windows

Download `A-Simple-Autoclicker-Setup-VERSION-x64.exe` from the latest GitHub
release and run it. The installer adds Start menu and optional desktop shortcuts.

The release also contains `A-Simple-Autoclicker-Windows-x64.zip` for portable
use. Extract it and run `a-simple-autoclicker.exe`; no installation is needed.

Windows may show a SmartScreen warning until published builds are digitally
signed and have established reputation. Only download releases from this
repository. Some programs running as administrator only accept simulated input
from an autoclicker that is also running as administrator.

## Install on Debian, Ubuntu, or Linux Mint

Download the latest `.deb` from the repository's **Releases** page, then install
it from the directory containing the download:

```bash
sudo apt install ./a-simple-autoclicker_VERSION_ARCH.deb
```

After installation, launch **A Simple Autoclicker** from the desktop application
menu or run:

```bash
a-simple-autoclicker
```

Remove it with:

```bash
sudo apt remove a-simple-autoclicker
```

## Build and run from source

### Linux

Install the development dependencies:

```bash
sudo apt update
sudo apt install build-essential cargo libadwaita-1-dev libgtk-4-dev \
  libx11-dev libxtst-dev pkg-config rustc
```

Clone or download this repository, enter its directory, and run:

```bash
cargo run
```

Build an optimized binary with:

```bash
cargo build --release
```

The resulting executable is `target/release/a-simple-autoclicker`.

### Windows

Install [Rust](https://rustup.rs/), clone the repository, open PowerShell in the
project directory, and run:

```powershell
.\scripts\build-windows.ps1
```

The portable ZIP is written to `dist\A-Simple-Autoclicker-Windows-x64.zip`.
Tagged GitHub builds also create a Setup executable automatically.

## Build a Debian package

The package builder compiles a release binary and creates an installable package
under `dist/`:

```bash
./scripts/build-deb.sh
sudo apt install ./dist/a-simple-autoclicker_0.1.2_amd64.deb
```

The script automatically reads the version from `Cargo.toml` and the architecture
from `dpkg`, so future versions use the appropriate output filename.

## Usage

1. Select a mouse action or record a keyboard action.
2. Choose the interval between actions.
3. Optionally enable a time limit, action-count limit, or fixed mouse position.
4. Select **Start clicking** or use the configured global hotkey.
5. Stop from the window, tray menu, or global hotkey.

When capturing a fixed position, select **Capture position…** and move the pointer
to the target before the two-second countdown ends.

Use conservative intervals while testing. Very short intervals can make other
applications difficult to control.

## Presets and configuration

Named presets include the repeated action, interval, limits, fixed position, and
global hotkey. On Linux they are stored at:

```text
~/.config/a-simple-autoclicker/presets.json
```

Presets created by older development builds under
`~/.config/mint-autoclicker/presets.json` are read automatically.

On Windows, presets are stored under
`%APPDATA%\A Simple Autoclicker\presets.json`.

## Project structure

- `src/app.rs` — GTK/libadwaita interface
- `src/windows_app.rs` — DPI-aware Windows interface, tray, and global hotkey
- `src/clicker.rs` — timing, action-count limits, and worker state
- `src/model.rs` — shared actions, hotkeys, modifiers, and positions
- `src/backend/x11.rs` — X11/XTest input simulation and pointer capture
- `src/backend/windows.rs` — Windows input simulation and pointer capture
- `src/hotkey.rs` — X11 global hotkey handling
- `src/presets.rs` — JSON preset persistence
- `src/tray.rs` — StatusNotifier system tray integration
- `scripts/build-deb.sh` — Debian package builder
- `scripts/build-windows.ps1` — local portable Windows builder
- `.github/workflows/windows-release.yml` — Windows installer/release automation

The platform interfaces and input backends are separated from the shared click
engine, models, limits, and preset storage. This keeps future Wayland support
independent from the working X11 and Windows implementations.

## Limitations

- Linux requires X11; behavior through XWayland is not supported or guaranteed.
- The recorded action cannot be the same key as the global toggle hotkey.
- Another application may already own a selected global hotkey.
- The tray icon requires a desktop panel that supports StatusNotifier items.
- Unsigned Windows downloads can trigger a Microsoft Defender SmartScreen prompt.

## Contributing

Issues and pull requests are welcome. Before submitting changes, run:

```bash
cargo test
cargo clippy -- -D warnings
```

Please keep platform-specific input code inside `src/backend/` where possible.

## License

Licensed under the [MIT License](LICENSE).
