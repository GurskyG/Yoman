use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub journal_folder: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            journal_folder: None,
        }
    }
}

pub fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|err| {
        format!("Yoman could not find its settings folder ({err}). Journal files were not changed.")
    })?;
    Ok(dir.join("settings.json"))
}

pub fn load_settings(path: &Path) -> Result<Settings, String> {
    if !path.exists() {
        return Ok(Settings::default());
    }

    let bytes = fs::read(path).map_err(|err| {
        format!(
            "Yoman could not read its settings file ({err}). Choose the journal folder again. Journal files were not changed."
        )
    })?;

    serde_json::from_slice(&bytes).map_err(|_| {
        "Yoman could not understand its settings file. Choose the journal folder again. Journal files were not changed.".to_string()
    })
}

pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Yoman could not create its settings folder ({err}). The journal folder was not changed."
            )
        })?;
    }

    let json = serde_json::to_vec_pretty(settings).map_err(|err| {
        format!("Yoman could not prepare its settings to save ({err}). No files were changed.")
    })?;
    let temporary = path.with_added_extension("tmp");
    fs::write(&temporary, &json).map_err(|err| {
        format!("Yoman could not write its settings ({err}). The journal folder was not changed.")
    })?;
    fs::rename(&temporary, path).map_err(|err| {
        format!(
            "Yoman could not finish saving its settings ({err}). The journal folder was not changed."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("yoman-settings-{nanos}-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn missing_settings_mean_no_folder() {
        let path = temp_file();
        let settings = load_settings(&path).unwrap();
        assert_eq!(settings.journal_folder, None);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn settings_round_trip() {
        let path = temp_file();
        let settings = Settings {
            journal_folder: Some("/tmp/yoman-journal".to_string()),
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(load_settings(&path).unwrap(), settings);
        assert!(!path.with_added_extension("tmp").exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn corrupt_settings_are_reported_and_left_in_place() {
        let path = temp_file();
        fs::write(&path, b"{").unwrap();
        let err = load_settings(&path).unwrap_err();
        assert!(err.contains("could not understand"));
        assert_eq!(fs::read(&path).unwrap(), b"{");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
