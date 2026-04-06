mod api;
mod auth;
mod config;
mod error;
mod input;
mod playback;
mod player;
mod state;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use librespot_playback::player::PlayerEvent;
use ratatui::prelude::*;

use crate::auth::TokenStore;
use crate::config::Config;
use crate::player::Action;
use crate::state::App;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    init_logging();

    let config = Config::load(None)?;

    let token_data = get_or_refresh_token().await?;
    tracing::info!(
        scopes = token_data.scope.as_deref().unwrap_or("(none)"),
        expires_in = token_data.expires_in,
        "oauth token ready"
    );

    // The PKCE token is only used to bootstrap the librespot session.
    // Web API calls use a keymaster token from the session instead,
    // which bypasses developer app restrictions (same access as official client).
    let mut client = api::SpotifyClient::new(&token_data.access_token);
    let token_store = TokenStore::new();
    let _ = token_store.save(&token_data);

    let token = token_data;

    eprintln!("Starting playback engine...");
    let mut engine = playback::PlaybackEngine::start(&config, &token.access_token).await?;
    let mut player_events = engine.get_event_channel();
    eprintln!("Device '{}' ready (id: {})", config.device_name, engine.device_id);

    // Get a Web API token from the librespot session via keymaster.
    // This token uses Spotify's internal client_id, not the developer app's,
    // so it has full API access regardless of app approval status.
    let web_api_scopes = auth::WEB_API_SCOPES;
    match engine.get_web_api_token(web_api_scopes).await {
        Ok(session_token) => {
            tracing::info!("using keymaster token for Web API");
            client.set_token(&session_token);
        }
        Err(e) => {
            tracing::warn!(%e, "keymaster token unavailable, falling back to OAuth token");
        }
    }

    let mut app = App::new(config.device_name.clone(), config.initial_volume);

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    player::handle_action(Action::LoadPlaylists, &mut app, &client, &mut engine).await;
    player::handle_action(Action::RefreshPlayback, &mut app, &client, &mut engine).await;

    let tick_rate = config.tick_rate();
    let refresh_interval = Duration::from_secs(config.refresh_interval_secs);
    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        app.clear_stale_notification();

        process_player_events(&mut app, &mut player_events, &client).await;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(action) = input::handle_key(&mut app, key) {
                    player::handle_action(action, &mut app, &client, &mut engine).await;
                }
            }
        }

        if !app.running {
            break;
        }

        if last_refresh.elapsed() >= refresh_interval {
            // Refresh the Web API token via keymaster (it handles caching/expiry internally).
            match engine.get_web_api_token(web_api_scopes).await {
                Ok(session_token) => {
                    client.set_token(&session_token);
                }
                Err(e) => {
                    tracing::warn!(%e, "keymaster token refresh failed");
                }
            }

            player::handle_action(Action::RefreshPlayback, &mut app, &client, &mut engine).await;
            last_refresh = Instant::now();
        }
    }

    engine.shutdown();
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    terminal.show_cursor()?;

    Ok(())
}

/// Process any pending librespot player events and update app state.
async fn process_player_events(
    app: &mut App,
    events: &mut librespot_playback::player::PlayerEventChannel,
    client: &api::SpotifyClient,
) {
    while let Ok(event) = events.try_recv() {
        match event {
            PlayerEvent::Playing { position_ms, track_id, .. } => {
                if let Some(track) = &app.now_playing_track {
                    // We already have track metadata (from optimistic set or prior fetch)
                    // — just update the position and keep the known duration.
                    app.progress.start(position_ms as u64, track.duration_ms);
                } else {
                    // No metadata yet. Try the local playlist data first (instant)
                    // before falling back to the Web API (which may lag behind Spirc).
                    let uri = track_id.to_uri();
                    if let Some(track) = app.find_track_by_uri(&uri) {
                        app.progress.start(position_ms as u64, track.duration_ms);
                        app.now_playing_track = Some(track);
                    } else {
                        app.progress.start(position_ms as u64, 0);
                        fetch_current_track(app, client).await;
                    }
                }
            }
            PlayerEvent::Paused { position_ms, .. } => {
                // Preserve existing duration — don't reset it.
                let dur = app.progress.duration_ms;
                app.progress.start(position_ms as u64, dur);
                app.progress.pause();
            }
            PlayerEvent::Stopped { .. } => {
                app.progress.stop();
                app.now_playing_track = None;
            }
            PlayerEvent::EndOfTrack { .. } => {
                // Spirc auto-advances. Clear track + progress so the bar
                // doesn't keep showing stale data while the next track loads.
                app.now_playing_track = None;
                app.progress.stop();
            }
            PlayerEvent::Loading { .. } => {
                // Don't clear now_playing_track here — if handle_select set it
                // optimistically, we want to keep showing it while the track loads.
                // It gets replaced when the Playing event arrives with new metadata,
                // or cleared on EndOfTrack/Stopped.
            }
            PlayerEvent::VolumeChanged { volume } => {
                app.volume = ((volume as u32 * 100 + 32767) / 65535) as u8;
            }
            PlayerEvent::ShuffleChanged { shuffle } => {
                app.shuffle = shuffle;
            }
            _ => {}
        }
    }
}

async fn fetch_current_track(app: &mut App, client: &api::SpotifyClient) {
    match client.current_playback().await {
        Ok(Some(pb)) => {
            if let Some(track) = pb.item {
                // Only update if we still don't have track info.
                // If the user selected a new track in the meantime, don't
                // overwrite with stale API data from a previous track.
                if app.now_playing_track.is_none() {
                    app.progress.duration_ms = track.duration_ms;
                    app.now_playing_track = Some(track);
                }
            }
        }
        Ok(None) => {}
        Err(e) => {
            tracing::debug!(%e, "failed to fetch current track");
        }
    }
}

async fn get_or_refresh_token() -> color_eyre::Result<auth::TokenData> {
    let store = TokenStore::new();

    if let Ok(data) = store.load() {
        // Refreshing can't add new scopes — force fresh auth if scopes changed.
        if !auth::has_required_scopes(&data) {
            tracing::info!("token missing required scopes, re-authenticating");
        } else if !data.is_expired() {
            return Ok(data);
        } else if let Some(rt) = data.refresh_token.clone() {
            match auth::refresh(&rt).await {
                Ok(refreshed) => return Ok(refreshed),
                Err(e) => {
                    tracing::warn!(%e, "refresh failed, starting fresh auth");
                }
            }
        }
    }

    let data = auth::authenticate().await?;
    store.save(&data)?;
    Ok(data)
}

fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let log_dir = config::cache_dir();
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "spotatui.log");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("spotatui=debug,warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file_appender)
        .with_ansi(false)
        .init();
}
