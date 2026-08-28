use std::mem::size_of;

use windows_sys::Win32::{
    Foundation::{GetLastError, POINT},
    UI::{
        Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
            KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
            MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
            MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
        },
        WindowsAndMessaging::{GetCursorPos, SetCursorPos},
    },
};

use crate::{
    backend::InputBackend,
    model::{Action, ClickPosition, KeyModifiers},
};

pub struct WindowsBackend;

impl InputBackend for WindowsBackend {
    fn perform(&mut self, action: Action, position: Option<ClickPosition>) -> Result<(), String> {
        if matches!(
            action,
            Action::LeftClick | Action::MiddleClick | Action::RightClick
        ) {
            if let Some(position) = position {
                if unsafe { SetCursorPos(position.x, position.y) } == 0 {
                    return Err(last_error("Could not move the pointer"));
                }
            }
        }

        match action {
            Action::LeftClick => send_mouse(MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            Action::MiddleClick => send_mouse(MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
            Action::RightClick => send_mouse(MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            Action::Key { keysym, modifiers } => {
                let virtual_key = u16::try_from(keysym)
                    .map_err(|_| "The recorded key is not valid on Windows".to_string())?;
                send_key(virtual_key, modifiers)
            }
        }
    }
}

pub fn pointer_position() -> Result<ClickPosition, String> {
    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(last_error("Could not read the pointer position"));
    }
    Ok(ClickPosition {
        x: point.x,
        y: point.y,
    })
}

fn send_mouse(down: u32, up: u32) -> Result<(), String> {
    send_inputs(&[mouse_input(down), mouse_input(up)])
}

fn send_key(key: u16, modifiers: KeyModifiers) -> Result<(), String> {
    if key == 0 || key > u8::MAX as u16 {
        return Err("The recorded key is not valid on Windows".into());
    }

    let modifier_keys = [
        (modifiers.control, 0x11),
        (modifiers.shift, 0x10),
        (modifiers.alt, 0x12),
        (modifiers.super_key, 0x5b),
    ];
    let mut inputs = Vec::with_capacity(10);
    for (enabled, virtual_key) in modifier_keys {
        if enabled {
            inputs.push(key_input(virtual_key, false));
        }
    }
    inputs.push(key_input(key, false));
    inputs.push(key_input(key, true));
    for (enabled, virtual_key) in modifier_keys.into_iter().rev() {
        if enabled {
            inputs.push(key_input(virtual_key, true));
        }
    }
    send_inputs(&inputs)
}

fn mouse_input(flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key_input(virtual_key: u16, released: bool) -> INPUT {
    let mut flags = if released { KEYEVENTF_KEYUP } else { 0 };
    if matches!(virtual_key, 0x21..=0x2e) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(last_error("Windows blocked the simulated input"))
    }
}

fn last_error(context: &str) -> String {
    let code = unsafe { GetLastError() };
    if code == 0 {
        context.to_string()
    } else {
        format!("{context} (Windows error {code})")
    }
}
