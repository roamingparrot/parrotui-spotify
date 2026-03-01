use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config;
use crate::error::{Result, SpotError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,

    /// When the token actually expires. We set this ourselves after receiving it.
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

impl TokenData {
    pub fn stamp_expiry(&mut self) {
        self.expires_at = Some(Utc::now() + Duration::seconds(self.expires_in));
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(at) => Utc::now() >= at - Duration::seconds(60), // 1 min buffer
            None => true,
        }
    }
}

/// Handles persisting tokens. Tries keyring first, falls back to a cache file.
pub struct TokenStore {
    service: String,
}

impl TokenStore {
    pub fn new() -> Self {
        Self {
            service: "spotatui".into(),
        }
    }

    pub fn save(&self, data: &TokenData) -> Result<()> {
        let json = serde_json::to_string(data)?;

        // Try keyring. On many headless or minimal systems this fails,
        // which is fine — we fall back to a file.
        match self.save_to_keyring(&json) {
            Ok(()) => {
                tracing::debug!("token saved to keyring");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("keyring unavailable ({e}), falling back to file");
            }
        }

        self.save_to_file(&json)
    }

    pub fn load(&self) -> Result<TokenData> {
        // keyring first
        if let Ok(json) = self.load_from_keyring() {
            let data: TokenData = serde_json::from_str(&json)?;
            return Ok(data);
        }

        // file fallback
        let path = config::token_cache_path();
        if path.exists() {
            let json = std::fs::read_to_string(&path)?;
            let data: TokenData = serde_json::from_str(&json)?;
            return Ok(data);
        }

        Err(SpotError::NotAuthenticated)
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        let _ = self.delete_from_keyring();
        let path = config::token_cache_path();
        let _ = std::fs::remove_file(path);
    }

    fn save_to_keyring(&self, json: &str) -> Result<()> {
        let entry = keyring::Entry::new(&self.service, "default")
            .map_err(|e| SpotError::Keyring(e.to_string()))?;
        entry
            .set_password(json)
            .map_err(|e| SpotError::Keyring(e.to_string()))
    }

    fn load_from_keyring(&self) -> Result<String> {
        let entry = keyring::Entry::new(&self.service, "default")
            .map_err(|e| SpotError::Keyring(e.to_string()))?;
        entry
            .get_password()
            .map_err(|e| SpotError::Keyring(e.to_string()))
    }

    fn delete_from_keyring(&self) -> Result<()> {
        let entry = keyring::Entry::new(&self.service, "default")
            .map_err(|e| SpotError::Keyring(e.to_string()))?;
        entry
            .delete_credential()
            .map_err(|e| SpotError::Keyring(e.to_string()))
    }

    fn save_to_file(&self, json: &str) -> Result<()> {
        let path = config::token_cache_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Restrictive permissions
        write_private(&path, json)?;
        tracing::debug!(path = %path.display(), "token saved to file (keyring unavailable)");
        Ok(())
    }
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    std::io::Write::write_all(&mut f, contents.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}
