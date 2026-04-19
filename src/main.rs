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
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("parrotui-spotify {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    color_eyre::install()?;
    init_logging();

    let config = Config::load(None)?;

    // Web API token — obtained via PKCE OAuth with ncspot's client_id.
    let mut token_data = get_or_refresh_token().await?;
    tracing::info!(
        scopes = token_data.scope.as_deref().unwrap_or("(none)"),
        expires_in = token_data.expires_in,
        "oauth token ready"
    );

    let mut client = api::SpotifyClient::new(&token_data.access_token);
    let token_store = TokenStore::new();
    let _ = token_store.save(&token_data);

    // Streaming engine — uses separate credentials (spotify-player's client_id
    // via librespot-oauth, cached after first auth).
    eprintln!("Starting playback engine...");
    let engine = playback::PlaybackEngine::start(&config).await?;
    let mut player_events = engine.get_event_channel();
    eprintln!(
        "Device '{}' ready (id: {})",
        config.device_name, engine.device_id
    );

    let theme = ui::theme::Theme::from_name(&config.theme);
    let mut app = App::new(config.device_name.clone(), engine.device_id.clone(), config.initial_volume, theme);

    // Channel for async action results.
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<player::ActionResult>();

    // Fire initial data loads (non-blocking — results arrive via channel).
    player::dispatch_async(
        Action::LoadPlaylists,
        &app,
        client.clone(),
        engine.device_id.clone(),
        action_tx.clone(),
    );
    player::dispatch_async(
        Action::RefreshPlayback,
        &app,
        client.clone(),
        engine.device_id.clone(),
        action_tx.clone(),
    );

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = config.tick_rate();
    let refresh_interval = Duration::from_secs(config.refresh_interval_secs);
    let mut last_refresh = Instant::now();

    loop {
        app.clear_stale_notification();

        process_player_events(&mut app, &mut player_events);

        // Drain async action results (non-blocking).
        while let Ok(result) = action_rx.try_recv() {
            player::apply_result(&mut app, result);
        }

        // Replay any deferred action from a completed playback transfer.
        if let Some(replay) = app.pending_replay_action.take() {
            if let Some(async_action) = player::handle_sync(replay, &mut app, &engine) {
                player::dispatch_async(
                    async_action,
                    &app,
                    client.clone(),
                    engine.device_id.clone(),
                    action_tx.clone(),
                );
            }
        }

        // Background-load remaining tracks so the full playlist is in memory.
        if app.content.can_load_more() {
            player::dispatch_async(
                Action::LoadMore,
                &app,
                client.clone(),
                engine.device_id.clone(),
                action_tx.clone(),
            );
            app.content.set_loading();
        } else {
            app.pending_jump_to_bottom = false;
        }

        // Drain all queued input events — sync actions run inline, async ones
        // are spawned as background tasks via the channel.
        let mut had_input = false;
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                had_input = true;
                if let Some(action) = input::handle_key(&mut app, key)
                    && let Some(async_action) = player::handle_sync(action, &mut app, &engine)
                {
                    player::dispatch_async(
                        async_action,
                        &app,
                        client.clone(),
                        engine.device_id.clone(),
                        action_tx.clone(),
                    );
                }
            }
        }

        // Draw after processing input so state changes are visible immediately.
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if !app.running {
            break;
        }

        // Only sleep when idle — redraw immediately after input for snappy navigation.
        if !had_input {
            tokio::time::sleep(tick_rate).await;
        }

        let rate_ok = app
            .rate_limited_until
            .is_none_or(|until| Instant::now() >= until);

        if rate_ok && last_refresh.elapsed() >= refresh_interval {
            app.rate_limited_until = None;

            // Proactively refresh the OAuth token before it expires.
            if token_data.is_expired()
                && let Some(rt) = token_data.refresh_token.as_deref()
            {
                match auth::refresh(rt).await {
                    Ok(refreshed) => {
                        tracing::info!("oauth token refreshed");
                        client.set_token(&refreshed.access_token);
                        let _ = token_store.save(&refreshed);
                        token_data = refreshed;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "token refresh failed");
                    }
                }
            }

            // Retry loading playlists if the initial attempt failed.
            if app.sidebar_items.len() <= 1 {
                player::dispatch_async(
                    Action::LoadPlaylists,
                    &app,
                    client.clone(),
                    engine.device_id.clone(),
                    action_tx.clone(),
                );
            }

            player::dispatch_async(
                Action::RefreshPlayback,
                &app,
                client.clone(),
                engine.device_id.clone(),
                action_tx.clone(),
            );
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
/// Never awaits API calls — keeps the loop non-blocking.
fn process_player_events(
    app: &mut App,
    events: &mut librespot_playback::player::PlayerEventChannel,
) {
    while let Ok(event) = events.try_recv() {
        match event {
            PlayerEvent::Playing {
                position_ms,
                track_id,
                ..
            } => {
                if let Some(track) = &app.now_playing_track {
                    app.progress.start(position_ms as u64, track.duration_ms);
                } else {
                    let uri = track_id.to_uri();
                    if let Some(track) = app.find_track_by_uri(&uri) {
                        app.progress.start(position_ms as u64, track.duration_ms);
                        app.now_playing_track = Some(track);
                    } else {
                        app.progress.start(position_ms as u64, 0);
                    }
                }
            }
            PlayerEvent::Paused { position_ms, .. } => {
                let dur = app.progress.duration_ms;
                app.progress.start(position_ms as u64, dur);
                app.progress.pause();
            }
            PlayerEvent::Stopped { .. } => {
                app.progress.stop();
                app.now_playing_track = None;
            }
            PlayerEvent::EndOfTrack { .. } => {
                app.now_playing_track = None;
                app.progress.stop();
            }
            PlayerEvent::Loading { .. } => {}
            PlayerEvent::VolumeChanged { volume } => {
                let pct = ((volume as u32 * 100 + 32767) / 65535) as u8;
                app.volume = ((pct + 2) / 5 * 5).min(100);
            }
            PlayerEvent::ShuffleChanged { shuffle } => {
                app.shuffle = shuffle;
            }
            _ => {}
        }
    }
}

async fn get_or_refresh_token() -> color_eyre::Result<auth::TokenData> {
    let store = TokenStore::new();

    if let Ok(data) = store.load() {
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

    let file_appender = tracing_appender::rolling::daily(&log_dir, "parrotui-spotify.log");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("parrotui_spotify=debug,warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file_appender)
        .with_ansi(false)
        .init();
}
