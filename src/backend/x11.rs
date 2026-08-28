use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

use libloading::Library;

use crate::{
    backend::InputBackend,
    model::{Action, ClickPosition, KeyModifiers},
};

type Display = c_void;
type OpenDisplay = unsafe extern "C" fn(*const c_char) -> *mut Display;
type CloseDisplay = unsafe extern "C" fn(*mut Display) -> c_int;
type Flush = unsafe extern "C" fn(*mut Display) -> c_int;
type KeysymToKeycode = unsafe extern "C" fn(*mut Display, c_ulong) -> c_uint;
type FakeButton = unsafe extern "C" fn(*mut Display, c_uint, c_int, c_ulong) -> c_int;
type FakeKey = unsafe extern "C" fn(*mut Display, c_uint, c_int, c_ulong) -> c_int;
type FakeMotion = unsafe extern "C" fn(*mut Display, c_int, c_int, c_int, c_ulong) -> c_int;
type DefaultRootWindow = unsafe extern "C" fn(*mut Display) -> c_ulong;
type QueryPointer = unsafe extern "C" fn(
    *mut Display,
    c_ulong,
    *mut c_ulong,
    *mut c_ulong,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_uint,
) -> c_int;

pub struct X11Backend {
    display: *mut Display,
    close_display: CloseDisplay,
    flush: Flush,
    keysym_to_keycode: KeysymToKeycode,
    fake_button: FakeButton,
    fake_key: FakeKey,
    fake_motion: FakeMotion,
    // Keep the dynamic libraries alive for as long as their function pointers are used.
    _x11: Library,
    _xtst: Library,
}

// The display connection is created and used only by the click worker thread.
unsafe impl Send for X11Backend {}

impl X11Backend {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let x11 = Library::new("libX11.so.6").map_err(|error| error.to_string())?;
            let xtst = Library::new("libXtst.so.6").map_err(|error| error.to_string())?;

            let open_display: OpenDisplay = *x11
                .get(b"XOpenDisplay\0")
                .map_err(|error| error.to_string())?;
            let close_display: CloseDisplay = *x11
                .get(b"XCloseDisplay\0")
                .map_err(|error| error.to_string())?;
            let flush: Flush = *x11.get(b"XFlush\0").map_err(|error| error.to_string())?;
            let keysym_to_keycode: KeysymToKeycode = *x11
                .get(b"XKeysymToKeycode\0")
                .map_err(|error| error.to_string())?;
            let fake_button: FakeButton = *xtst
                .get(b"XTestFakeButtonEvent\0")
                .map_err(|error| error.to_string())?;
            let fake_key: FakeKey = *xtst
                .get(b"XTestFakeKeyEvent\0")
                .map_err(|error| error.to_string())?;
            let fake_motion: FakeMotion = *xtst
                .get(b"XTestFakeMotionEvent\0")
                .map_err(|error| error.to_string())?;

            let display = open_display(std::ptr::null());
            if display.is_null() {
                return Err("Could not connect to the X11 display".into());
            }

            Ok(Self {
                display,
                close_display,
                flush,
                keysym_to_keycode,
                fake_button,
                fake_key,
                fake_motion,
                _x11: x11,
                _xtst: xtst,
            })
        }
    }

    fn click(&mut self, button: u32) -> Result<(), String> {
        let (pressed, released) = unsafe {
            let pressed = (self.fake_button)(self.display, button, 1, 0);
            let released = (self.fake_button)(self.display, button, 0, 0);
            (self.flush)(self.display);
            (pressed, released)
        };
        if pressed == 0 || released == 0 {
            return Err("X11 rejected the simulated mouse click".into());
        }
        Ok(())
    }

    fn move_to(&mut self, position: ClickPosition) -> Result<(), String> {
        let moved = unsafe {
            let moved = (self.fake_motion)(self.display, -1, position.x, position.y, 0);
            (self.flush)(self.display);
            moved
        };
        if moved == 0 {
            return Err("X11 rejected the pointer movement".into());
        }
        Ok(())
    }

    fn key_event(&mut self, keysym: u64, pressed: bool) -> Result<(), String> {
        let keycode = unsafe { (self.keysym_to_keycode)(self.display, keysym) };
        if keycode == 0 {
            return Err(format!("X11 could not map keysym {keysym:#x}"));
        }
        let sent = unsafe { (self.fake_key)(self.display, keycode, i32::from(pressed), 0) };
        if sent == 0 {
            return Err("X11 rejected the simulated key event".into());
        }
        Ok(())
    }

    fn key(&mut self, keysym: u64, modifiers: KeyModifiers) -> Result<(), String> {
        let modifier_keysyms = [
            (modifiers.shift, 0xffe1),
            (modifiers.control, 0xffe3),
            (modifiers.alt, 0xffe9),
            (modifiers.super_key, 0xffeb),
        ];

        for (enabled, modifier) in modifier_keysyms {
            if enabled {
                self.key_event(modifier, true)?;
            }
        }
        self.key_event(keysym, true)?;
        self.key_event(keysym, false)?;
        for (enabled, modifier) in modifier_keysyms.into_iter().rev() {
            if enabled {
                self.key_event(modifier, false)?;
            }
        }
        unsafe {
            (self.flush)(self.display);
        }
        Ok(())
    }
}

impl InputBackend for X11Backend {
    fn perform(&mut self, action: Action, position: Option<ClickPosition>) -> Result<(), String> {
        if matches!(
            action,
            Action::LeftClick | Action::MiddleClick | Action::RightClick
        ) {
            if let Some(position) = position {
                self.move_to(position)?;
            }
        }
        match action {
            Action::LeftClick => self.click(1),
            Action::MiddleClick => self.click(2),
            Action::RightClick => self.click(3),
            Action::Key { keysym, modifiers } => self.key(keysym, modifiers),
        }
    }
}

pub fn pointer_position() -> Result<ClickPosition, String> {
    unsafe {
        let x11 = Library::new("libX11.so.6").map_err(|error| error.to_string())?;
        let open_display: OpenDisplay = *x11
            .get(b"XOpenDisplay\0")
            .map_err(|error| error.to_string())?;
        let close_display: CloseDisplay = *x11
            .get(b"XCloseDisplay\0")
            .map_err(|error| error.to_string())?;
        let default_root: DefaultRootWindow = *x11
            .get(b"XDefaultRootWindow\0")
            .map_err(|error| error.to_string())?;
        let query_pointer: QueryPointer = *x11
            .get(b"XQueryPointer\0")
            .map_err(|error| error.to_string())?;

        let display = open_display(std::ptr::null());
        if display.is_null() {
            return Err("Could not connect to the X11 display".into());
        }
        let mut root = 0;
        let mut child = 0;
        let mut root_x = 0;
        let mut root_y = 0;
        let mut window_x = 0;
        let mut window_y = 0;
        let mut mask = 0;
        let found = query_pointer(
            display,
            default_root(display),
            &mut root,
            &mut child,
            &mut root_x,
            &mut root_y,
            &mut window_x,
            &mut window_y,
            &mut mask,
        );
        close_display(display);
        if found == 0 {
            return Err("X11 could not read the current pointer position".into());
        }
        Ok(ClickPosition {
            x: root_x,
            y: root_y,
        })
    }
}

impl Drop for X11Backend {
    fn drop(&mut self) {
        unsafe {
            (self.close_display)(self.display);
        }
    }
}
