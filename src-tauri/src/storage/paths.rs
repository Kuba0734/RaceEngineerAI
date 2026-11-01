use std::path::PathBuf;

const APP_DIR: &str = "RaceEngineerAI";

/// `%APPDATA%\RaceEngineerAI` – settings, layouts, custom presets.
pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join(APP_DIR)
}

/// `%LOCALAPPDATA%\RaceEngineerAI` – logs, database, downloaded models, replays.
pub fn local_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join(APP_DIR)
}

pub fn logs_dir() -> PathBuf {
    local_dir().join("logs")
}

pub fn models_dir() -> PathBuf {
    local_dir().join("models")
}

pub fn replays_dir() -> PathBuf {
    local_dir().join("replays")
}

pub fn db_path() -> PathBuf {
    local_dir().join("sessions.sqlite")
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    for d in [config_dir(), local_dir(), logs_dir(), models_dir(), replays_dir()] {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}
