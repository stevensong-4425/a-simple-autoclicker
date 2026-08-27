use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Action {
    LeftClick,
    MiddleClick,
    RightClick,
    Key {
        keysym: u64,
        modifiers: KeyModifiers,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClickPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Hotkey {
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl Hotkey {
    pub const ALL: [Self; 7] = [
        Self::F6,
        Self::F7,
        Self::F8,
        Self::F9,
        Self::F10,
        Self::F11,
        Self::F12,
    ];

    pub const LABELS: [&'static str; 7] = ["F6", "F7", "F8", "F9", "F10", "F11", "F12"];

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn keysym(self) -> u64 {
        match self {
            Self::F6 => 0xffc3,
            Self::F7 => 0xffc4,
            Self::F8 => 0xffc5,
            Self::F9 => 0xffc6,
            Self::F10 => 0xffc7,
            Self::F11 => 0xffc8,
            Self::F12 => 0xffc9,
        }
    }

    #[cfg(target_os = "windows")]
    pub fn virtual_key(self) -> u32 {
        match self {
            Self::F6 => 0x75,
            Self::F7 => 0x76,
            Self::F8 => 0x77,
            Self::F9 => 0x78,
            Self::F10 => 0x79,
            Self::F11 => 0x7a,
            Self::F12 => 0x7b,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Hotkey;

    #[test]
    fn every_hotkey_has_a_label_and_unique_keysym() {
        assert_eq!(Hotkey::ALL.len(), Hotkey::LABELS.len());
        let mut keysyms: Vec<_> = Hotkey::ALL.iter().map(|key| key.keysym()).collect();
        keysyms.sort_unstable();
        keysyms.dedup();
        assert_eq!(keysyms.len(), Hotkey::ALL.len());
    }
}
