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

use crate::config::Config;
use crate::error::{Result, SpotError};

pub struct PlaybackEngine {
    spirc: Spirc,
    player: Arc<Player>,
    #[allow(dead_code)]
    session: Session,
    pub device_id: String,
}

impl PlaybackEngine {
    pub async fn start(config: &Config, access_token: &str) -> Result<Self> {
        let device_id = device_id_from_name(&config.device_name);

        // Use default SessionConfig — the AP protocol requires Spotify's
        // internal KEYMASTER client_id, not our developer app's client_id.
        let session_config = SessionConfig {
            device_id: device_id.clone(),
            ..Default::default()
        };

        let cache_dir = crate::config::cache_dir();

        // Clear stale credentials from previous failed sessions.
        // Fresh OAuth token auth will produce new ones.
        let cred_dir = cache_dir.join("credentials");
        if cred_dir.exists() {
            let _ = std::fs::remove_dir_all(&cred_dir);
        }

        let cache = Cache::new(
            Some(cred_dir),
            Some(cache_dir.join("volume")),
            Some(cache_dir.join("audio")),
            Some(1024 * 1024 * 50),
        )
        .map_err(|e| SpotError::Session(format!("cache init: {e}")))?;

        let session = Session::new(session_config, Some(cache));
        let credentials = Credentials::with_access_token(access_token);

        // Don't call session.connect() here — Spirc::new() does it internally
        // after registering its message listeners. Calling it twice would fail
        // because the connection channel is a OnceLock.

        let player_config = PlayerConfig::default();
        let audio_format = AudioFormat::default();

        let backend_fn = audio_backend::find(Some("rodio".to_string()))
            .ok_or_else(|| SpotError::Playback("rodio audio backend not found".into()))?;

        let mixer_fn = mixer::find(Some("softvol"))
            .ok_or_else(|| SpotError::Playback("softvol mixer not found".into()))?;
        let mixer_instance = mixer_fn(MixerConfig::default())
            .map_err(|e| SpotError::Playback(format!("mixer init: {e}")))?;

        let vol_u16 = (config.initial_volume as u32 * 655) as u16;
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

        // Pass the same credentials for Spirc reconnection.
        // After session.connect(), the session holds reusable_credentials
        // bound to the KEYMASTER client_id which Login5 can use.
        let (spirc, spirc_task) = Spirc::new(
            connect_config,
            session.clone(),
            credentials,
            player.clone(),
            mixer_instance,
        )
        .await
        .map_err(|e| SpotError::Session(format!("spirc init: {e}")))?;

        tokio::spawn(spirc_task);

        tracing::info!(name = %config.device_name, "registered as Spotify Connect device");

        Ok(Self {
            spirc,
            player,
            session,
            device_id,
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
        self.spirc.prev().map_err(spirc_err)
    }

    pub fn set_volume(&self, percent: u8) -> Result<()> {
        let vol = (percent.min(100) as u32 * 655) as u16;
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

    pub fn shutdown(self) {
        let _ = self.spirc.shutdown();
    }

    #[allow(dead_code)]
    pub fn session(&self) -> &Session {
        &self.session
    }
}

fn spirc_err(e: librespot_core::Error) -> SpotError {
    SpotError::Playback(format!("spirc: {e}"))
}

fn device_id_from_name(name: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(name.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
