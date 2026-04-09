use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}

fn default_tick_rate() -> u64 {
    DEFAULT_TICK_RATE_MS
}
fn default_refresh_interval() -> u64 {
    DEFAULT_REFRESH_INTERVAL_SECS
}
fn default_device_name() -> String {
    "spotatui".to_string()
}
fn default_initial_volume() -> u8 {
    100
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
        let config = Config {
            tick_rate_ms: DEFAULT_TICK_RATE_MS,
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
            device_name: default_device_name(),
            initial_volume: default_initial_volume(),
            theme: String::new(),
        };

        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let toml_str =
            toml::to_string_pretty(&config).map_err(|e| SpotError::Config(e.to_string()))?;
        std::fs::write(&path, &toml_str)?;
        eprintln!("Config saved to {}", path.display());

        Ok(config)
    }

    pub fn tick_rate(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.tick_rate_ms)
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("no XDG config dir")
        .join("spotatui")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("no XDG cache dir")
        .join("spotatui")
}

pub fn token_cache_path() -> PathBuf {
    cache_dir().join("token.json")
}
