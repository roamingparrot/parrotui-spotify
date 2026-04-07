use crate::api::SpotifyClient;
use crate::error::{Result, SpotError};
use crate::playback::PlaybackEngine;
use crate::state::{App, ContentView, FocusPanel, Notification, SidebarItem};

const PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone)]
pub enum Action {
    // Playback — handled locally via Spirc
    TogglePlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBackward,
    ToggleShuffle,
    CycleRepeat,

    // Navigation
    Select,
    GoBack,

    // Data — still fetched from Web API
    RefreshPlayback,
    LoadPlaylists,
    #[allow(dead_code)]
    LoadLikedSongs,
    LoadMore,
}

pub async fn handle_action(
    action: Action,
    app: &mut App,
    client: &SpotifyClient,
    engine: &mut PlaybackEngine,
) {
    let result = match action {
        Action::TogglePlayPause => toggle_play_pause(app, engine),
        Action::NextTrack => engine.next(),
        Action::PreviousTrack => engine.prev(),
        Action::VolumeUp => adjust_volume(app, engine, 5),
        Action::VolumeDown => adjust_volume(app, engine, -5),
        Action::SeekForward => seek_relative(app, engine, 5000),
        Action::SeekBackward => seek_relative(app, engine, -5000),
        Action::ToggleShuffle => toggle_shuffle(app, engine),
        Action::CycleRepeat => cycle_repeat(app, engine),
        Action::Select => handle_select(app, client, engine).await,
        Action::GoBack => {
            handle_go_back(app);
            Ok(())
        }
        Action::RefreshPlayback => refresh_playback(app, client).await,
        Action::LoadPlaylists => load_playlists(app, client).await,
        Action::LoadLikedSongs => load_liked_songs(app, client).await,
        Action::LoadMore => load_more(app, client).await,
    };

    if let Err(e) = result {
        tracing::warn!(%e, "action failed");
        if let SpotError::RateLimited { retry_after_secs } = &e {
            let backoff = std::time::Duration::from_secs(*retry_after_secs);
            app.rate_limited_until = Some(std::time::Instant::now() + backoff);
        }
        app.notify_error(e);
    }
}

fn toggle_play_pause(app: &mut App, engine: &PlaybackEngine) -> Result<()> {
    if app.progress.is_playing() {
        engine.pause()?;
        app.progress.pause();
    } else {
        engine.play()?;
        app.progress.resume();
    }
    Ok(())
}

fn adjust_volume(app: &mut App, engine: &PlaybackEngine, delta: i16) -> Result<()> {
    let current = app.volume as i16;
    let next = (current + delta).clamp(0, 100) as u8;
    engine.set_volume(next)?;
    app.volume = next;
    Ok(())
}

fn seek_relative(app: &mut App, engine: &PlaybackEngine, delta_ms: i64) -> Result<()> {
    let pos = app.progress.position_ms() as i64;
    let dur = app.progress.duration_ms as i64;
    let target = (pos + delta_ms).clamp(0, dur.max(0)) as u64;
    engine.set_position(target as u32)?;
    app.progress.seek(target);
    Ok(())
}

fn toggle_shuffle(app: &mut App, engine: &PlaybackEngine) -> Result<()> {
    let new_state = !app.shuffle;
    engine.set_shuffle(new_state)?;
    app.shuffle = new_state;
    Ok(())
}

fn cycle_repeat(app: &mut App, engine: &PlaybackEngine) -> Result<()> {
    use crate::api::RepeatMode;
    let next = app.repeat.next();
    match next {
        RepeatMode::Off => {
            engine.set_repeat(false)?;
            engine.set_repeat_track(false)?;
        }
        RepeatMode::Context => {
            engine.set_repeat(true)?;
            engine.set_repeat_track(false)?;
        }
        RepeatMode::Track => {
            engine.set_repeat(true)?;
            engine.set_repeat_track(true)?;
        }
    }
    app.repeat = next;
    Ok(())
}

async fn refresh_playback(app: &mut App, client: &SpotifyClient) -> Result<()> {
    // Periodic sync with Spotify's view — catches external changes
    // (someone transferred playback away, changed shuffle, etc.)
    if let Some(pb) = client.current_playback().await? {
        app.shuffle = pb.shuffle_state.unwrap_or(false);
        app.repeat = pb.repeat_mode();
        if let Some(dev) = &pb.device {
            let v = dev.volume_percent.unwrap_or(app.volume);
            app.volume = ((v + 2) / 5 * 5).min(100);
        }
        // Update now-playing from API if we don't have local track info
        if app.now_playing_track.is_none() {
            if let Some(track) = pb.item {
                app.progress.start(pb.progress_ms.unwrap_or(0), track.duration_ms);
                if !pb.is_playing {
                    app.progress.pause();
                }
                app.now_playing_track = Some(track);
            }
        }
    }
    Ok(())
}

async fn load_playlists(app: &mut App, client: &SpotifyClient) -> Result<()> {
    let page = client.my_playlists(PAGE_SIZE, 0).await?;
    let total = page.total;
    let mut playlists = page.items;

    // Sort by recently played — best-effort; API failure just keeps default order
    match client.recently_played(50).await {
        Ok(recent) => {
            let mut order: Vec<String> = Vec::new();
            for item in &recent.items {
                if let Some(ctx) = &item.context {
                    if ctx.context_type == "playlist" && !order.contains(&ctx.uri) {
                        order.push(ctx.uri.clone());
                    }
                }
            }
            playlists.sort_by_key(|pl| match order.iter().position(|uri| *uri == pl.uri) {
                Some(pos) => (0, pos),
                None => (1, 0),
            });
        }
        Err(e) => {
            tracing::debug!(%e, "recently_played unavailable, using default order");
        }
    }

    app.set_sidebar_playlists(playlists, total);
    Ok(())
}

async fn load_liked_songs(app: &mut App, client: &SpotifyClient) -> Result<()> {
    let page = client.liked_tracks(PAGE_SIZE, 0).await?;
    app.set_liked_songs(page);
    Ok(())
}

async fn load_more(app: &mut App, client: &SpotifyClient) -> Result<()> {
    match app.focus {
        FocusPanel::Sidebar => {
            if !app.sidebar_can_load_more() {
                return Ok(());
            }
            let loaded = app.sidebar_items.len().saturating_sub(1) as u32;
            app.sidebar_loading = true;
            let page = client.my_playlists(PAGE_SIZE, loaded).await?;
            app.append_sidebar_playlists(page.items, page.total);
        }
        FocusPanel::Content => {
            if !app.content.can_load_more() {
                return Ok(());
            }
            let loaded = app.content.len() as u32;
            match &app.content {
                ContentView::Empty => {}
                ContentView::PlaylistDetail { playlist_uri, .. } => {
                    let id = playlist_uri.rsplit(':').next().unwrap_or(playlist_uri);
                    let page = client.playlist_tracks(id, PAGE_SIZE, loaded).await?;
                    app.append_playlist_tracks(page);
                }
                ContentView::LikedSongs { .. } => {
                    let page = client.liked_tracks(PAGE_SIZE, loaded).await?;
                    app.append_liked_songs(page);
                }
            }
        }
    }
    Ok(())
}

async fn handle_select(
    app: &mut App,
    client: &SpotifyClient,
    engine: &PlaybackEngine,
) -> Result<()> {
    match app.focus {
        FocusPanel::Sidebar => {
            let item = app.current_sidebar_item().clone();
            match item {
                SidebarItem::LikedSongs => load_liked_songs(app, client).await?,
                SidebarItem::Playlist(pl) => {
                    let page = client.playlist_tracks(&pl.id, PAGE_SIZE, 0).await?;
                    app.set_playlist_tracks(pl.name, pl.uri, page);
                }
            }
            app.focus = FocusPanel::Content;
        }
        FocusPanel::Content => {
            match &app.content {
                ContentView::Empty => {}
                ContentView::PlaylistDetail {
                    cursor,
                    playlist_uri,
                    tracks,
                    ..
                } => {
                    let uri = playlist_uri.clone();
                    let offset = *cursor;
                    // Start playback on *our* device via Web API
                    client
                        .play_context_on(&uri, Some(offset), &engine.device_id)
                        .await?;

                    // Optimistically set now-playing from cached track data
                    if let Some(track) = tracks.get(offset) {
                        app.progress.start(0, track.duration_ms);
                        app.now_playing_track = Some(track.clone());
                    }
                    app.notify(Notification::info("playing"));
                }
                ContentView::LikedSongs {
                    tracks, cursor, ..
                } => {
                    let uris: Vec<String> =
                        tracks.iter().filter_map(|t| t.uri.clone()).collect();
                    if !uris.is_empty() {
                        client
                            .play_tracks_on(&uris, Some(*cursor), &engine.device_id)
                            .await?;

                        if let Some(track) = tracks.get(*cursor) {
                            app.progress.start(0, track.duration_ms);
                            app.now_playing_track = Some(track.clone());
                        }
                        app.notify(Notification::info("playing"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_go_back(app: &mut App) {
    app.focus = FocusPanel::Sidebar;
}
