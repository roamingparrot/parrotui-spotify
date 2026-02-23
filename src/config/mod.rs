use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

use crate::error::{Result, SpotError};

const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
const DEFAULT_TICK_RATE_MS: u64 = 250;
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub client_id: String,
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    #[serde(default = "default_tick_rate")]
    pub tick_rate_ms: u64,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
    #[serde(default = "default_device_name")]
    pub device_name: String,
    #[serde(default = "default_initial_volume")]
    pub initial_volume: u8,
}

fn default_redirect_uri() -> String {
    DEFAULT_REDIRECT_URI.to_string()
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
    50
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let path = path.unwrap_or_else(config_path);

        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let config: Config =
                toml::from_str(&raw).map_err(|e| SpotError::Config(format!("bad toml: {e}")))?;

            if !config.client_id.is_empty() {
                return Ok(config);
            }
        }

        // No config or empty client_id — run first-time setup
        let config = Self::first_run_setup(&path)?;
        Ok(config)
    }

    fn first_run_setup(path: &PathBuf) -> Result<Self> {
        eprintln!("Welcome to spotatui!\n");
        eprintln!("You need a Spotify Developer app to use this.");
        eprintln!("  1. Go to https://developer.spotify.com/dashboard");
        eprintln!("  2. Set redirect URI to: {DEFAULT_REDIRECT_URI}");
        eprintln!("  3. Create an app (select Web API + Web Playback SDK)");
        eprintln!("  4. Copy your Client ID\n");

        eprint!("Client ID: ");
        std::io::stderr().flush().ok();

        let mut client_id = String::new();
        std::io::stdin()
            .read_line(&mut client_id)
            .map_err(|e| SpotError::Config(format!("failed to read input: {e}")))?;
        let client_id = client_id.trim().to_string();

        if client_id.is_empty() {
            return Err(SpotError::Config("client_id cannot be empty".into()));
        }

        let config = Config {
            client_id,
            redirect_uri: default_redirect_uri(),
            tick_rate_ms: DEFAULT_TICK_RATE_MS,
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
            device_name: default_device_name(),
            initial_volume: default_initial_volume(),
        };

        // Save it
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let toml_str =
            toml::to_string_pretty(&config).map_err(|e| SpotError::Config(e.to_string()))?;
        std::fs::write(path, &toml_str)?;
        eprintln!("\nConfig saved to {}\n", path.display());

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
