mod progress;

pub use progress::ProgressTracker;

use std::sync::Arc;

use librespot_connect::{ConnectConfig, Spirc};
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::SessionConfig;
use librespot_core::session::Session;
use librespot_playback::audio_backend;
use librespot_playback::config::{AudioFormat, PlayerConfig};
use librespot_playback::mixer::{self, MixerConfig};
use librespot_playback::player::{Player, PlayerEventChannel};

use crate::auth::STREAMING_CLIENT_ID;
use crate::config::Config;
use crate::error::{Result, SpotError};

/// Redirect URI registered for the streaming client_id (spotify-player's).
const STREAMING_REDIRECT_URI: &str = "http://127.0.0.1:8989/login";

/// OAuth scopes required for streaming.
const STREAMING_SCOPES: [&str; 6] = [
    "streaming",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-library-read",
    "user-read-private",
];

pub struct PlaybackEngine {
    spirc: Spirc,
    player: Arc<Player>,
    task: tokio::task::JoinHandle<()>,
}

impl PlaybackEngine {
    pub async fn start(config: &Config) -> Result<Self> {
        let device_id = device_id_from_name(&config.device_name);

        let session_config = SessionConfig {
            device_id: device_id.clone(),
            client_id: STREAMING_CLIENT_ID.to_string(),
            ..Default::default()
        };

        let cache_dir = crate::config::cache_dir();
        let cred_cache_dir = cache_dir.join("credentials");

        let volume_dir = cache_dir.join("volume");
        let audio_dir = cache_dir.join("audio");
        let cache = Cache::new(
            Some(&cred_cache_dir),
            Some(&volume_dir),
            Some(&audio_dir),
            Some(1024 * 1024 * 50),
        )
        .map_err(|e| SpotError::Session(format!("cache init: {e}")))?;

        // Resolve streaming credentials: try cache first, then browser OAuth.
        let credentials = resolve_streaming_credentials(&cache, &cred_cache_dir)?;

        let session = Session::new(session_config, Some(cache));

        let player_config = PlayerConfig::default();
        let audio_format = AudioFormat::default();

        let backend_fn = audio_backend::find(Some("rodio".to_string()))
            .ok_or_else(|| SpotError::Playback("rodio audio backend not found".into()))?;

        let mixer_fn = mixer::find(Some("softvol"))
            .ok_or_else(|| SpotError::Playback("softvol mixer not found".into()))?;
        let mixer_instance = mixer_fn(MixerConfig::default())
            .map_err(|e| SpotError::Playback(format!("mixer init: {e}")))?;

        let vol_u16 = (config.initial_volume as u32 * 65535 / 100) as u16;
        mixer_instance.set_volume(vol_u16);

        let player = Player::new(
            player_config,
            session.clone(),
            mixer_instance.get_soft_volume(),
            move || backend_fn(None, audio_format),
        );

        let connect_config = ConnectConfig {
            name: config.device_name.clone(),
            device_type: librespot_core::config::DeviceType::Computer,
            initial_volume: vol_u16,
            ..Default::default()
        };

        // Spirc::new() connects the session and caches reusable credentials.
        let (spirc, spirc_task) = Spirc::new(
            connect_config,
            session.clone(),
            credentials,
            player.clone(),
            mixer_instance,
        )
        .await
        .map_err(|e| SpotError::Session(format!("spirc init: {e}")))?;

        let task = tokio::spawn(spirc_task);

        tracing::info!(name = %config.device_name, "registered as Spotify Connect device");

        Ok(Self {
            spirc,
            player,
            task,
        })
    }

    pub fn play(&self) -> Result<()> {
        self.spirc.play().map_err(spirc_err)
    }

    pub fn pause(&self) -> Result<()> {
        self.spirc.pause().map_err(spirc_err)
    }

    pub fn next(&self) -> Result<()> {
        self.spirc.next().map_err(spirc_err)
    }

    pub fn prev(&self) -> Result<()> {
        // Seek to start first so Spirc always goes to the previous track
        // instead of restarting the current one.
        let _ = self.spirc.set_position_ms(0);
        self.spirc.prev().map_err(spirc_err)
    }

    pub fn set_volume(&self, percent: u8) -> Result<()> {
        let vol = (percent.min(100) as u32 * 65535 / 100) as u16;
        self.spirc.set_volume(vol).map_err(spirc_err)
    }

    pub fn set_position(&self, position_ms: u32) -> Result<()> {
        self.spirc.set_position_ms(position_ms).map_err(spirc_err)
    }

    pub fn set_shuffle(&self, on: bool) -> Result<()> {
        self.spirc.shuffle(on).map_err(spirc_err)
    }

    pub fn set_repeat(&self, on: bool) -> Result<()> {
        self.spirc.repeat(on).map_err(spirc_err)
    }

    pub fn set_repeat_track(&self, on: bool) -> Result<()> {
        self.spirc.repeat_track(on).map_err(spirc_err)
    }

    pub fn get_event_channel(&self) -> PlayerEventChannel {
        self.player.get_player_event_channel()
    }

    /// Returns true if the Spirc background task has exited, which means the
    /// librespot session has dropped and any further commands will fail.
    pub fn is_spirc_dead(&self) -> bool {
        self.task.is_finished()
    }

    pub fn shutdown(self) {
        let _ = self.spirc.shutdown();
        self.task.abort();
    }
}

/// Get streaming credentials: use librespot cache first, then browser OAuth.
fn resolve_streaming_credentials(
    cache: &Cache,
    cred_cache_dir: &std::path::Path,
) -> Result<Credentials> {
    if let Some(cached) = cache.credentials() {
        tracing::info!("using cached streaming credentials");
        return Ok(cached);
    }

    tracing::info!("no cached streaming credentials, opening browser OAuth");
    request_streaming_oauth(cred_cache_dir)
}

/// Run librespot-oauth to get streaming credentials via browser.
fn request_streaming_oauth(cred_cache_dir: &std::path::Path) -> Result<Credentials> {
    eprintln!("Streaming authentication required — opening browser...");

    let client = librespot_oauth::OAuthClientBuilder::new(
        STREAMING_CLIENT_ID,
        STREAMING_REDIRECT_URI,
        STREAMING_SCOPES.to_vec(),
    )
    .open_in_browser()
    .build()
    .map_err(|e| SpotError::Session(format!("oauth client build: {e:?}")))?;

    let token = client
        .get_access_token()
        .map_err(|e| SpotError::Session(format!("streaming oauth failed: {e:?}")))?;

    // Clear stale cached credentials so librespot caches the fresh ones on connect.
    let cred_file = cred_cache_dir.join("credentials.json");
    if cred_file.exists() {
        let _ = std::fs::remove_file(&cred_file);
    }

    Ok(Credentials::with_access_token(token.access_token))
}

fn spirc_err(e: librespot_core::Error) -> SpotError {
    SpotError::Playback(format!("spirc: {e}"))
}

pub fn device_id_from_name(name: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(name.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
