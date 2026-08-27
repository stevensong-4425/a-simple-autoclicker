use std::{
    ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use libloading::Library;

use crate::{clicker::ClickEngine, model::Hotkey};

type Display = c_void;
type Window = c_ulong;
type OpenDisplay = unsafe extern "C" fn(*const c_char) -> *mut Display;
type DefaultRootWindow = unsafe extern "C" fn(*mut Display) -> Window;
type KeysymToKeycode = unsafe extern "C" fn(*mut Display, c_ulong) -> c_uint;
type GrabKey =
    unsafe extern "C" fn(*mut Display, c_int, c_uint, Window, c_int, c_int, c_int) -> c_int;
type UngrabKey = unsafe extern "C" fn(*mut Display, c_int, c_uint, Window) -> c_int;
type Pending = unsafe extern "C" fn(*mut Display) -> c_int;
type NextEvent = unsafe extern "C" fn(*mut Display, *mut XEvent) -> c_int;
type Flush = unsafe extern "C" fn(*mut Display) -> c_int;

const KEY_PRESS: c_int = 2;
const ANY_MODIFIER: c_uint = 1 << 15;
const GRAB_MODE_ASYNC: c_int = 1;

#[repr(C)]
union XEvent {
    event_type: c_int,
    pad: [c_long; 24],
}

enum Command {
    SetHotkey(Hotkey),
    SetEnabled(bool),
}

pub struct HotkeyManager {
    sender: mpsc::Sender<Command>,
}

impl HotkeyManager {
    pub fn start(engine: Arc<ClickEngine>, initial: Hotkey) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            if let Err(error) = run_hotkey_loop(engine, initial, receiver) {
                eprintln!("Global hotkey unavailable: {error}");
            }
        });
        Self { sender }
    }

    pub fn set_hotkey(&self, hotkey: Hotkey) {
        let _ = self.sender.send(Command::SetHotkey(hotkey));
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.sender.send(Command::SetEnabled(enabled));
    }
}

fn run_hotkey_loop(
    engine: Arc<ClickEngine>,
    initial: Hotkey,
    receiver: mpsc::Receiver<Command>,
) -> Result<(), String> {
    unsafe {
        let x11 = Library::new("libX11.so.6").map_err(|error| error.to_string())?;
        let open_display: OpenDisplay = *x11.get(b"XOpenDisplay\0").map_err(|e| e.to_string())?;
        let default_root: DefaultRootWindow = *x11
            .get(b"XDefaultRootWindow\0")
            .map_err(|e| e.to_string())?;
        let keysym_to_keycode: KeysymToKeycode =
            *x11.get(b"XKeysymToKeycode\0").map_err(|e| e.to_string())?;
        let grab_key: GrabKey = *x11.get(b"XGrabKey\0").map_err(|e| e.to_string())?;
        let ungrab_key: UngrabKey = *x11.get(b"XUngrabKey\0").map_err(|e| e.to_string())?;
        let pending: Pending = *x11.get(b"XPending\0").map_err(|e| e.to_string())?;
        let next_event: NextEvent = *x11.get(b"XNextEvent\0").map_err(|e| e.to_string())?;
        let flush: Flush = *x11.get(b"XFlush\0").map_err(|e| e.to_string())?;

        let display = open_display(std::ptr::null());
        if display.is_null() {
            return Err("Could not connect to the X11 display".into());
        }
        let root = default_root(display);
        let mut keycode = keysym_to_keycode(display, initial.keysym()) as c_int;
        let mut enabled = true;
        grab_key(
            display,
            keycode,
            ANY_MODIFIER,
            root,
            0,
            GRAB_MODE_ASYNC,
            GRAB_MODE_ASYNC,
        );
        flush(display);

        loop {
            while let Ok(command) = receiver.try_recv() {
                match command {
                    Command::SetHotkey(hotkey) => {
                        if enabled {
                            ungrab_key(display, keycode, ANY_MODIFIER, root);
                        }
                        keycode = keysym_to_keycode(display, hotkey.keysym()) as c_int;
                        if enabled {
                            grab_key(
                                display,
                                keycode,
                                ANY_MODIFIER,
                                root,
                                0,
                                GRAB_MODE_ASYNC,
                                GRAB_MODE_ASYNC,
                            );
                        }
                    }
                    Command::SetEnabled(should_enable) if should_enable != enabled => {
                        enabled = should_enable;
                        if enabled {
                            grab_key(
                                display,
                                keycode,
                                ANY_MODIFIER,
                                root,
                                0,
                                GRAB_MODE_ASYNC,
                                GRAB_MODE_ASYNC,
                            );
                        } else {
                            ungrab_key(display, keycode, ANY_MODIFIER, root);
                        }
                    }
                    Command::SetEnabled(_) => {}
                }
                flush(display);
            }

            while pending(display) > 0 {
                let mut event = XEvent { pad: [0; 24] };
                next_event(display, &mut event);
                if event.event_type == KEY_PRESS {
                    engine.toggle();
                }
            }
            thread::sleep(Duration::from_millis(15));
        }
    }
}
