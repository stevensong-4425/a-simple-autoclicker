use std::{env, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Action, ClickPosition, Hotkey};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Preset {
    pub name: String,
    pub action: Action,
    pub action_label: String,
    pub interval_ms: u64,
    pub duration_ms: u64,
    pub max_actions: u64,
    pub position: Option<ClickPosition>,
    pub hotkey: Hotkey,
}

#[cfg(test)]
mod tests {
    use super::{Preset, PresetStore};
    use crate::model::{Action, ClickPosition, Hotkey};

    #[test]
    fn preset_round_trips_every_setting() {
        let preset = Preset {
            name: "Ten minute target".into(),
            action: Action::RightClick,
            action_label: "Right mouse click".into(),
            interval_ms: 75,
            duration_ms: 600_000,
            max_actions: 250,
            position: Some(ClickPosition { x: 120, y: 340 }),
            hotkey: Hotkey::F9,
        };
        let encoded = serde_json::to_string(&preset).expect("serialize preset");
        let decoded: Preset = serde_json::from_str(&encoded).expect("deserialize preset");
        assert_eq!(decoded, preset);
    }

    #[test]
    fn deleting_without_a_selection_is_a_no_op() {
        let mut store = PresetStore::default();
        assert_eq!(store.delete(0), Ok(None));
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct PresetStore {
    presets: Vec<Preset>,
}

impl PresetStore {
    pub fn load() -> Self {
        let contents =
            fs::read_to_string(Self::path()).or_else(|_| fs::read_to_string(Self::legacy_path()));
        let Ok(contents) = contents else {
            return Self::default();
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    pub fn names(&self) -> Vec<&str> {
        self.presets
            .iter()
            .map(|preset| preset.name.as_str())
            .collect()
    }

    pub fn get(&self, index: usize) -> Option<Preset> {
        self.presets.get(index).cloned()
    }

    pub fn save(&mut self, preset: Preset) -> Result<(), String> {
        let previous = self.presets.clone();
        if let Some(existing) = self
            .presets
            .iter_mut()
            .find(|existing| existing.name == preset.name)
        {
            *existing = preset;
        } else {
            self.presets.push(preset);
            self.presets
                .sort_by_key(|preset| preset.name.to_lowercase());
        }

        if let Err(error) = self.persist() {
            self.presets = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn delete(&mut self, index: usize) -> Result<Option<Preset>, String> {
        if index >= self.presets.len() {
            return Ok(None);
        }

        let removed = self.presets.remove(index);
        if let Err(error) = self.persist() {
            self.presets.insert(index, removed);
            return Err(error);
        }
        Ok(Some(removed))
    }

    fn persist(&self) -> Result<(), String> {
        let path = Self::path();
        let parent = path
            .parent()
            .ok_or_else(|| "Invalid preset configuration path".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let contents = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        fs::write(path, contents).map_err(|error| error.to_string())
    }

    #[cfg(target_os = "windows")]
    fn path() -> PathBuf {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("A Simple Autoclicker")
            .join("presets.json")
    }

    #[cfg(not(target_os = "windows"))]
    fn path() -> PathBuf {
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("a-simple-autoclicker").join("presets.json")
    }

    #[cfg(target_os = "windows")]
    fn legacy_path() -> PathBuf {
        PathBuf::from("__no_legacy_windows_preset_file__")
    }

    #[cfg(not(target_os = "windows"))]
    fn legacy_path() -> PathBuf {
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("mint-autoclicker").join("presets.json")
    }
}
