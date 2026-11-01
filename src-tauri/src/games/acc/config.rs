//! Sets up `Documents\Assetto Corsa Competizione\Config\broadcasting.json` (UTF-16 LE).
//! Always makes a backup first; the UI asks the user before calling [`apply`].

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

pub const DEFAULT_PORT: u16 = 9000;
pub const DEFAULT_PASSWORD: &str = "asd";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BroadcastingStatus {
    pub path: String,
    pub exists: bool,
    pub port: Option<u16>,
    pub password_set: bool,
    pub matches_app: bool,
}

pub fn config_path() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("Assetto Corsa Competizione").join("Config").join("broadcasting.json"))
}

fn decode(bytes: &[u8]) -> String {
    // UTF-16 LE with or without BOM; fall back to UTF-8 for hand-edited files.
    let looks_utf16 = bytes.starts_with(&[0xFF, 0xFE]) || (bytes.len() > 1 && bytes[1] == 0);
    if looks_utf16 {
        let start = if bytes.starts_with(&[0xFF, 0xFE]) { 2 } else { 0 };
        let units: Vec<u16> = bytes[start..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).into_owned()
    }
}

fn encode_utf16le(s: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

pub fn status(app_port: u16, app_password: &str) -> BroadcastingStatus {
    let path = config_path();
    let display = path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let parsed = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_str::<serde_json::Value>(decode(&b).trim_matches('\u{feff}')).ok());
    let port = parsed.as_ref().and_then(|v| v.get("updListenerPort")).and_then(|v| v.as_u64()).map(|p| p as u16);
    let pw = parsed.as_ref().and_then(|v| v.get("connectionPassword")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    BroadcastingStatus {
        path: display,
        exists: parsed.is_some(),
        port,
        password_set: !pw.is_empty(),
        matches_app: port == Some(app_port) && pw == app_password,
    }
}

/// Writes port + password, keeping other keys. Returns the backup path, if any.
pub fn apply(port: u16, password: &str) -> AppResult<Option<PathBuf>> {
    let path = config_path().ok_or_else(|| AppError::NotFound("documents folder".into()))?;
    let dir = path.parent().ok_or_else(|| AppError::Invalid("path".into()))?;
    if !dir.parent().is_some_and(|p| p.exists()) {
        return Err(AppError::NotFound("ACC documents folder – start ACC once first".into()));
    }
    std::fs::create_dir_all(dir)?;
    let mut value = serde_json::json!({});
    let mut backup = None;
    if let Ok(bytes) = std::fs::read(&path) {
        let ts = crate::storage::db::now_s();
        let b = path.with_file_name(format!("broadcasting.json.bak-{ts}"));
        std::fs::write(&b, &bytes)?;
        backup = Some(b);
        if let Ok(v) = serde_json::from_str(decode(&bytes).trim_matches('\u{feff}')) {
            value = v;
        }
    }
    if let Some(obj) = value.as_object_mut() {
        obj.insert("updListenerPort".into(), port.into());
        obj.insert("connectionPassword".into(), password.into());
        obj.entry("commandPassword").or_insert_with(|| "".into());
    }
    std::fs::write(&path, encode_utf16le(&serde_json::to_string_pretty(&value)?))?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_roundtrip() {
        let s = r#"{"updListenerPort": 9000, "connectionPassword": "asd", "commandPassword": ""}"#;
        let enc = encode_utf16le(s);
        assert_eq!(&enc[..2], &[0xFF, 0xFE]);
        assert_eq!(decode(&enc), s);
        assert_eq!(decode(s.as_bytes()), s);
    }
}
