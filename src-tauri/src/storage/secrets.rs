//! Groq API key in the Windows Credential Manager. The key never touches disk or logs.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "RaceEngineerAI";
const USER: &str = "groq_api_key";

pub fn init_store() {
    match windows_native_keyring_store::Store::new() {
        Ok(store) => keyring_core::set_default_store(store),
        Err(e) => tracing::error!("credential store unavailable: {e}"),
    }
}

fn entry() -> AppResult<keyring_core::Entry> {
    keyring_core::Entry::new(SERVICE, USER).map_err(|e| AppError::Secret(e.to_string()))
}

pub fn set_api_key(key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return delete_api_key();
    }
    entry()?.set_password(key).map_err(|e| AppError::Secret(e.to_string()))
}

pub fn get_api_key() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|k| !k.is_empty())
}

pub fn delete_api_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Secret(e.to_string())),
    }
}

/// Masked form for the UI, e.g. `gsk_…a1b2`.
pub fn masked_api_key() -> Option<String> {
    get_api_key().map(|k| {
        let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        let head: String = k.chars().take(4).collect();
        format!("{head}…{tail}")
    })
}
