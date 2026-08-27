# A Simple Autoclicker

A lightweight autoclicker for Linux and Windows. It can repeat mouse clicks or
recorded keyboard actions, stop after a duration or action count, target a fixed
screen position, save presets, and remain available from the system tray.

> **Platform status:** Linux uses GTK4/libadwaita and currently requires X11.
> Windows uses a native Win32 interface and supports 64-bit Windows 10 and 11.

## Features

- Left, middle, and right mouse clicking
- Record ordinary keys, function keys, modifiers, and combinations such as Ctrl+C
- Adjustable interval from 10 ms to 60 seconds
- Optional run-duration and exact action-count limits
- Fixed-position mouse clicking with a two-second capture delay
- Configurable F6–F12 global start/stop hotkey
- Named presets saved between launches
- System tray controls for showing, starting, stopping, and quitting the app
- Native interface on both GTK4/libadwaita (Linux) and Win32 (Windows)

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



## Limitations

- Linux requires X11; behavior through XWayland is not supported or guaranteed.
- The recorded action cannot be the same key as the global toggle hotkey.
- Another application may already own a selected global hotkey.
- The tray icon requires a desktop panel that supports StatusNotifier items.
- Unsigned Windows downloads can trigger a Microsoft Defender SmartScreen prompt.

m-specific input code inside `src/backend/` where possible.

## License

Licensed under the [MIT License](LICENSE).
