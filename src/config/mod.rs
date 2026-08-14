use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{Result, SpotError};

const DEFAULT_TICK_RATE_MS: u64 = 50;
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_tick_rate")]
    pub tick_rate_ms: u64,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
    #[serde(default = "default_device_name")]
    pub device_name: String,
    #[serde(default = "default_initial_volume")]
    pub initial_volume: u8,
    #[serde(default)]
    pub theme: String,
    #[serde(default = "default_volume_step")]
    pub volume_step: u8,
    #[serde(default = "default_seek_step_secs")]
    pub seek_step_secs: u64,
    /// Streaming bitrate in kbps — 96, 160 or 320. Other values fall back to 160.
    #[serde(default = "default_bitrate")]
    pub bitrate: u16,
    #[serde(default)]
    pub normalisation: bool,
    #[serde(default = "default_audio_cache_mb")]
    pub audio_cache_mb: u64,
}

fn default_tick_rate() -> u64 {
    DEFAULT_TICK_RATE_MS
}
fn default_refresh_interval() -> u64 {
    DEFAULT_REFRESH_INTERVAL_SECS
}
fn default_device_name() -> String {
    "parrotui-spotify".to_string()
}
fn default_initial_volume() -> u8 {
    100
}
fn default_volume_step() -> u8 {
    5
}
fn default_seek_step_secs() -> u64 {
    5
}
fn default_bitrate() -> u16 {
    160
}
fn default_audio_cache_mb() -> u64 {
    50
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tick_rate_ms: default_tick_rate(),
            refresh_interval_secs: default_refresh_interval(),
            device_name: default_device_name(),
            initial_volume: default_initial_volume(),
            theme: String::new(),
            volume_step: default_volume_step(),
            seek_step_secs: default_seek_step_secs(),
            bitrate: default_bitrate(),
            normalisation: false,
            audio_cache_mb: default_audio_cache_mb(),
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let path = path.unwrap_or_else(config_path);

        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let config: Config =
                toml::from_str(&raw).map_err(|e| SpotError::Config(format!("bad toml: {e}")))?;
            return Ok(config);
        }

        // No config — write defaults
        let config = Config::default();
        config.save(&path)?;
        eprintln!("Config saved to {}", path.display());

        Ok(config)
    }

    /// Write the config back to disk. Serializing the whole struct discards any
    /// comments the user hand-wrote in the file.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let toml_str =
            toml::to_string_pretty(self).map_err(|e| SpotError::Config(e.to_string()))?;
        std::fs::write(path, &toml_str)?;
        Ok(())
    }

    pub fn tick_rate(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.tick_rate_ms)
    }

    pub fn seek_step_ms(&self) -> i64 {
        (self.seek_step_secs * 1000) as i64
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("could not determine config directory")
        .join("parrotui-spotify")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("could not determine cache directory")
        .join("parrotui-spotify")
}

pub fn token_cache_path() -> PathBuf {
    cache_dir().join("token.json")
}
