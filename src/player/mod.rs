use std::time::Duration;

use crate::api::{self, SpotifyClient};
use crate::error::{Result, SpotError};
use crate::playback::PlaybackEngine;
use crate::state::{App, ContentView, FocusPanel, Notification, SearchTab, SidebarItem};

const PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone)]
pub enum Action {
    // Clipboard
    CopyLink,

    // Playback — handled locally via Spirc (sync)
    TogglePlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBackward,
    ToggleShuffle,
    CycleRepeat,

    // Navigation (sync)
    Select,
    GoBack,

    // Persist the config after a settings edit (sync)
    SaveConfig,

    // Data — fetched from Web API (async, spawned)
    RefreshPlayback,
    LoadPlaylists,
    #[allow(dead_code)]
    LoadLikedSongs,
    LoadMore,
    CheckDeviceHealth {
        generation: u64,
    },

    // Transfer playback to our device, then replay the deferred Spirc action
    TransferAndReplay(Box<Action>),

    // Search
    SubmitSearch {
        query: String,
    },
    SearchSelect,
    LoadAlbumTracks {
        album_id: String,
        album_name: String,
        album_uri: String,
        artist_name: String,
    },
    LoadArtistTopTracks {
        artist_id: String,
        artist_name: String,
        artist_uri: String,
    },
    LoadPlaylistFromSearch {
        playlist_id: String,
        playlist_name: String,
        playlist_uri: String,
    },
}

/// Results sent back from spawned async tasks.
pub enum ActionResult {
    Playlists {
        items: Vec<api::Playlist>,
        total: u32,
    },
    PlaylistTracks {
        name: String,
        uri: String,
        page: api::Page<api::PlaylistItem>,
    },
    LikedSongs {
        page: api::Page<api::SavedTrack>,
    },
    MoreSidebarPlaylists {
        items: Vec<api::Playlist>,
        total: u32,
    },
    MorePlaylistTracks {
        page: api::Page<api::PlaylistItem>,
    },
    MoreLikedSongs {
        page: api::Page<api::SavedTrack>,
    },
    PlaybackState {
        state: Option<api::PlaybackState>,
    },
    PlaybackStarted,
    TransferCompleted {
        deferred: Action,
    },
    TransferFailed {
        error: SpotError,
    },
    DeviceHealth {
        generation: u64,
        found: bool,
    },
    SearchResults {
        results: api::SearchResults,
    },
    AlbumTracks {
        album_name: String,
        album_uri: String,
        artist_name: String,
        tracks: Vec<api::Track>,
        total: u32,
    },
    ArtistTopTracksResult {
        artist_name: String,
        artist_uri: String,
        tracks: Vec<api::Track>,
    },
    Failed {
        error: SpotError,
    },
}

pub type ResultTx = tokio::sync::mpsc::UnboundedSender<ActionResult>;

// ---------------------------------------------------------------------------
// Sync path — executes inline, never blocks
// ---------------------------------------------------------------------------

fn is_spirc_command(action: &Action) -> bool {
    matches!(
        action,
        Action::TogglePlayPause
            | Action::NextTrack
            | Action::PreviousTrack
            | Action::VolumeUp
            | Action::VolumeDown
            | Action::SeekForward
            | Action::SeekBackward
            | Action::ToggleShuffle
            | Action::CycleRepeat
    )
}

/// Handle an action synchronously if possible. Returns `Some(action)` for
/// actions that need async dispatch (API calls).
pub fn handle_sync(action: Action, app: &mut App, engine: &PlaybackEngine) -> Option<Action> {
    if matches!(action, Action::CopyLink) {
        copy_link(app);
        return None;
    }

    // If we're not the active device and this is a Spirc command,
    // defer it behind a playback transfer.
    if !app.is_active_device && is_spirc_command(&action) {
        tracing::info!(?action, "device not active, will transfer and replay");
        app.notify(Notification::info("transferring playback..."));
        app.is_active_device = true; // optimistic — prevent duplicate transfers
        return Some(Action::TransferAndReplay(Box::new(action)));
    }

    let result = match action {
        Action::TogglePlayPause => {
            return sync_result(toggle_play_pause(app, engine), app);
        }
        Action::NextTrack => {
            if let Err(e) = engine.next() {
                tracing::warn!(%e, "action failed");
                return None;
            }
            return Some(Action::RefreshPlayback);
        }
        Action::PreviousTrack => {
            if let Err(e) = engine.prev() {
                tracing::warn!(%e, "action failed");
                return None;
            }
            return Some(Action::RefreshPlayback);
        }
        Action::VolumeUp => {
            let step = app.config.volume_step as i16;
            return sync_result(adjust_volume(app, engine, step), app);
        }
        Action::VolumeDown => {
            let step = app.config.volume_step as i16;
            return sync_result(adjust_volume(app, engine, -step), app);
        }
        Action::SeekForward => {
            let step = app.config.seek_step_ms();
            return sync_result(seek_relative(app, engine, step), app);
        }
        Action::SeekBackward => {
            let step = app.config.seek_step_ms();
            return sync_result(seek_relative(app, engine, -step), app);
        }
        Action::ToggleShuffle => {
            return sync_result(toggle_shuffle(app, engine), app);
        }
        Action::CycleRepeat => {
            return sync_result(cycle_repeat(app, engine), app);
        }
        Action::GoBack => {
            handle_go_back(app);
            return None;
        }

        Action::SaveConfig => {
            let path = crate::config::config_path();
            return sync_result(app.config.save(&path), app);
        }

        // Select has a sync part (state change) + optional async part
        Action::Select => return handle_select_sync(app, engine),

        // SearchSelect — inspect the active tab and dispatch the right async action
        Action::SearchSelect => return handle_search_select_sync(app),

        // Pure async — pass through
        other => other,
    };
    Some(result)
}

fn sync_result(r: Result<()>, app: &mut App) -> Option<Action> {
    if let Err(e) = r {
        tracing::warn!(%e, "action failed");
        app.notify_error(e);
    }
    None
}

/// The sync portion of Select — changes focus/state immediately, returns
/// an async Action if an API call is needed.
fn handle_select_sync(app: &mut App, _engine: &PlaybackEngine) -> Option<Action> {
    match app.focus {
        FocusPanel::Sidebar => {
            // Will need to load tracks — return async action.
            // The dispatch_async function reads current sidebar item.
            Some(Action::Select)
        }
        FocusPanel::Content => {
            // Start playback optimistically — set progress immediately.
            match &app.content {
                ContentView::Empty => None,
                ContentView::PlaylistDetail { cursor, tracks, .. } => {
                    if let Some(track) = tracks.get(*cursor) {
                        app.progress.start(0, track.duration_ms);
                        app.now_playing_track = Some(track.clone());
                    }
                    app.notify(Notification::info("playing"));
                    // Still need to issue the play API call
                    Some(Action::Select)
                }
                ContentView::LikedSongs { cursor, tracks, .. }
                | ContentView::AlbumDetail { cursor, tracks, .. }
                | ContentView::ArtistTopTracks { cursor, tracks, .. } => {
                    if let Some(track) = tracks.get(*cursor) {
                        app.progress.start(0, track.duration_ms);
                        app.now_playing_track = Some(track.clone());
                    }
                    app.notify(Notification::info("playing"));
                    Some(Action::Select)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Async dispatch — spawns a tokio task, sends result through channel
// ---------------------------------------------------------------------------

/// Snapshot of app state needed by async tasks (avoids borrowing App).
struct AsyncContext {
    focus: FocusPanel,
    sidebar_item: SidebarItem,
    content_snapshot: ContentSnapshot,
    sidebar_loaded: u32,
    sidebar_can_load_more: bool,
    content_can_load_more: bool,
    content_loaded: u32,
    search_snapshot: Option<SearchSnapshot>,
}

enum ContentSnapshot {
    Empty,
    PlaylistDetail { uri: String, cursor: usize },
    LikedSongs { uris: Vec<String>, cursor: usize },
}

struct SearchSnapshot {
    track_uris: Vec<String>,
    track_cursor: usize,
}

fn snapshot(app: &App) -> AsyncContext {
    let content_snapshot = match &app.content {
        ContentView::Empty => ContentSnapshot::Empty,
        ContentView::PlaylistDetail {
            playlist_uri,
            cursor,
            ..
        } => ContentSnapshot::PlaylistDetail {
            uri: playlist_uri.clone(),
            cursor: *cursor,
        },
        ContentView::LikedSongs { tracks, cursor, .. }
        | ContentView::AlbumDetail { tracks, cursor, .. }
        | ContentView::ArtistTopTracks { tracks, cursor, .. } => ContentSnapshot::LikedSongs {
            uris: tracks.iter().filter_map(|t| t.uri.clone()).collect(),
            cursor: *cursor,
        },
    };
    let search_snapshot = app.search.as_ref().map(|s| SearchSnapshot {
        track_uris: s
            .result_tracks
            .iter()
            .filter_map(|t| t.uri.clone())
            .collect(),
        track_cursor: s.current_cursor(),
    });
    AsyncContext {
        focus: app.focus,
        sidebar_item: app.current_sidebar_item().clone(),
        content_snapshot,
        sidebar_loaded: app.sidebar_items.len().saturating_sub(1) as u32,
        sidebar_can_load_more: app.sidebar_can_load_more(),
        content_can_load_more: app.content.can_load_more(),
        content_loaded: app.content.len() as u32,
        search_snapshot,
    }
}

pub fn dispatch_async(
    action: Action,
    app: &App,
    client: SpotifyClient,
    device_id: String,
    tx: ResultTx,
) {
    let ctx = snapshot(app);
    tokio::spawn(async move {
        let result = run_async(action, ctx, &client, &device_id).await;
        let _ = tx.send(result);
    });
}

async fn run_async(
    action: Action,
    ctx: AsyncContext,
    client: &SpotifyClient,
    device_id: &str,
) -> ActionResult {
    match action {
        Action::RefreshPlayback => match client.current_playback().await {
            Ok(state) => ActionResult::PlaybackState { state },
            Err(e) => ActionResult::Failed { error: e },
        },

        Action::LoadPlaylists => match load_playlists_async(client).await {
            Ok((items, total)) => ActionResult::Playlists { items, total },
            Err(e) => ActionResult::Failed { error: e },
        },

        Action::LoadLikedSongs => match client.liked_tracks(PAGE_SIZE, 0).await {
            Ok(page) => ActionResult::LikedSongs { page },
            Err(e) => ActionResult::Failed { error: e },
        },

        Action::LoadMore => load_more_async(ctx, client).await,

        Action::Select => select_async(ctx, client, device_id).await,

        Action::TransferAndReplay(deferred) => {
            match client.transfer_playback(device_id).await {
                Ok(()) => {
                    // Give librespot time to receive the Connect handoff.
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    ActionResult::TransferCompleted {
                        deferred: *deferred,
                    }
                }
                Err(e) => ActionResult::TransferFailed { error: e },
            }
        }

        Action::CheckDeviceHealth { generation } => match client.get_devices().await {
            Ok(resp) => {
                let found = resp
                    .devices
                    .iter()
                    .any(|d| d.id.as_deref() == Some(device_id));
                ActionResult::DeviceHealth { generation, found }
            }
            Err(e) => {
                tracing::warn!(%e, "device health check failed");
                ActionResult::DeviceHealth {
                    generation,
                    found: true,
                }
            }
        },

        Action::SubmitSearch { query } => match client.search(&query, 20).await {
            Ok(results) => ActionResult::SearchResults { results },
            Err(e) => ActionResult::Failed { error: e },
        },

        Action::SearchSelect => {
            if let Some(snap) = &ctx.search_snapshot {
                if snap.track_uris.is_empty() {
                    return ActionResult::PlaybackStarted;
                }
                match client
                    .play_tracks_on(&snap.track_uris, Some(snap.track_cursor), device_id)
                    .await
                {
                    Ok(()) => ActionResult::PlaybackStarted,
                    Err(e) => ActionResult::Failed { error: e },
                }
            } else {
                ActionResult::PlaybackStarted
            }
        }

        Action::LoadAlbumTracks {
            album_id,
            album_name,
            album_uri,
            artist_name,
        } => match client.album_tracks(&album_id, 50, 0).await {
            Ok(page) => {
                let tracks: Vec<api::Track> = page
                    .items
                    .into_iter()
                    .map(|at| api::Track {
                        name: at.name,
                        uri: at.uri,
                        duration_ms: at.duration_ms,
                        artists: at.artists,
                    })
                    .collect();
                ActionResult::AlbumTracks {
                    album_name,
                    album_uri,
                    artist_name,
                    total: page.total,
                    tracks,
                }
            }
            Err(e) => ActionResult::Failed { error: e },
        },

        Action::LoadArtistTopTracks {
            artist_id,
            artist_name,
            artist_uri,
        } => match client.artist_top_tracks(&artist_id).await {
            Ok(resp) => ActionResult::ArtistTopTracksResult {
                artist_name,
                artist_uri,
                tracks: resp.tracks,
            },
            Err(e) => ActionResult::Failed { error: e },
        },

        Action::LoadPlaylistFromSearch {
            playlist_id,
            playlist_name,
            playlist_uri,
        } => match client.playlist_tracks(&playlist_id, PAGE_SIZE, 0).await {
            Ok(page) => ActionResult::PlaylistTracks {
                name: playlist_name,
                uri: playlist_uri,
                page,
            },
            Err(e) => ActionResult::Failed { error: e },
        },

        // Sync actions should never reach here
        _ => ActionResult::Failed {
            error: SpotError::Playback("unexpected async dispatch".into()),
        },
    }
}

async fn load_playlists_async(client: &SpotifyClient) -> Result<(Vec<api::Playlist>, u32)> {
    let page = client.my_playlists(PAGE_SIZE, 0).await?;
    let total = page.total;
    let mut playlists = page.items;

    // Sort by recently played — best-effort
    match client.recently_played(50).await {
        Ok(recent) => {
            let mut order: Vec<String> = Vec::new();
            for item in &recent.items {
                if let Some(ctx) = &item.context
                    && ctx.context_type == "playlist"
                    && !order.contains(&ctx.uri)
                {
                    order.push(ctx.uri.clone());
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

    Ok((playlists, total))
}

async fn load_more_async(ctx: AsyncContext, client: &SpotifyClient) -> ActionResult {
    match ctx.focus {
        FocusPanel::Sidebar => {
            if !ctx.sidebar_can_load_more {
                return ActionResult::PlaybackStarted; // no-op
            }
            match client.my_playlists(PAGE_SIZE, ctx.sidebar_loaded).await {
                Ok(page) => ActionResult::MoreSidebarPlaylists {
                    items: page.items,
                    total: page.total,
                },
                Err(e) => ActionResult::Failed { error: e },
            }
        }
        FocusPanel::Content => {
            if !ctx.content_can_load_more {
                return ActionResult::PlaybackStarted; // no-op
            }
            match &ctx.content_snapshot {
                ContentSnapshot::Empty => ActionResult::PlaybackStarted,
                ContentSnapshot::PlaylistDetail { uri, .. } => {
                    let id = uri.rsplit(':').next().unwrap_or(uri);
                    match client
                        .playlist_tracks(id, PAGE_SIZE, ctx.content_loaded)
                        .await
                    {
                        Ok(page) => ActionResult::MorePlaylistTracks { page },
                        Err(e) => ActionResult::Failed { error: e },
                    }
                }
                ContentSnapshot::LikedSongs { .. } => {
                    match client.liked_tracks(PAGE_SIZE, ctx.content_loaded).await {
                        Ok(page) => ActionResult::MoreLikedSongs { page },
                        Err(e) => ActionResult::Failed { error: e },
                    }
                }
            }
        }
    }
}

async fn select_async(ctx: AsyncContext, client: &SpotifyClient, device_id: &str) -> ActionResult {
    match ctx.focus {
        FocusPanel::Sidebar => match ctx.sidebar_item {
            SidebarItem::LikedSongs => match client.liked_tracks(PAGE_SIZE, 0).await {
                Ok(page) => ActionResult::LikedSongs { page },
                Err(e) => ActionResult::Failed { error: e },
            },
            SidebarItem::Playlist(pl) => match client.playlist_tracks(&pl.id, PAGE_SIZE, 0).await {
                Ok(page) => ActionResult::PlaylistTracks {
                    name: pl.name,
                    uri: pl.uri,
                    page,
                },
                Err(e) => ActionResult::Failed { error: e },
            },
        },
        FocusPanel::Content => {
            // The sync path already set optimistic state; just issue the play call.
            let result = match &ctx.content_snapshot {
                ContentSnapshot::Empty => return ActionResult::PlaybackStarted,
                ContentSnapshot::PlaylistDetail { uri, cursor } => {
                    client.play_context_on(uri, Some(*cursor), device_id).await
                }
                ContentSnapshot::LikedSongs { uris, cursor } => {
                    if uris.is_empty() {
                        return ActionResult::PlaybackStarted;
                    }
                    client.play_tracks_on(uris, Some(*cursor), device_id).await
                }
            };
            match result {
                Ok(()) => ActionResult::PlaybackStarted,
                Err(e) => ActionResult::Failed { error: e },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Apply results — called from main loop after draining the channel
// ---------------------------------------------------------------------------

pub fn apply_result(app: &mut App, result: ActionResult) {
    match result {
        ActionResult::Playlists { items, total } => {
            app.set_sidebar_playlists(items, total);
        }
        ActionResult::PlaylistTracks { name, uri, page } => {
            let from_search = app.search.is_some();
            app.set_playlist_tracks(name, uri, page);
            app.focus = FocusPanel::Content;
            if from_search {
                app.search_origin = true;
            }
        }
        ActionResult::LikedSongs { page } => {
            app.set_liked_songs(page);
            app.focus = FocusPanel::Content;
        }
        ActionResult::MoreSidebarPlaylists { items, total } => {
            app.append_sidebar_playlists(items, total);
        }
        ActionResult::MorePlaylistTracks { page } => {
            app.append_playlist_tracks(page);
            if app.pending_jump_to_bottom {
                app.jump_to_bottom();
            }
        }
        ActionResult::MoreLikedSongs { page } => {
            app.append_liked_songs(page);
            if app.pending_jump_to_bottom {
                app.jump_to_bottom();
            }
        }
        ActionResult::PlaybackState { state } => {
            if let Some(pb) = state {
                app.shuffle = pb.shuffle_state.unwrap_or(false);
                app.repeat = pb.repeat_mode();

                // Check if playback is on a different device.
                let our_device = pb
                    .device
                    .as_ref()
                    .and_then(|d| d.id.as_deref())
                    .is_some_and(|id| id == app.device_id);

                if let Some(dev) = &pb.device {
                    let v = dev.volume_percent.unwrap_or(app.volume);
                    app.volume = snap_volume(v, app.config.volume_step);
                    app.is_active_device = dev.id.as_deref() == Some(&app.device_id);
                }

                if let Some(track) = pb.item {
                    let is_different = app
                        .now_playing_track
                        .as_ref()
                        .is_none_or(|current| current.uri != track.uri);
                    if is_different || !our_device {
                        app.progress
                            .start(pb.progress_ms.unwrap_or(0), track.duration_ms);
                        app.now_playing_track = Some(track);
                    }
                    if !pb.is_playing {
                        app.progress.pause();
                    } else if !app.progress.is_playing() {
                        app.progress.resume();
                    }
                } else {
                    app.now_playing_track = None;
                    app.progress.stop();
                }
            }
        }
        ActionResult::PlaybackStarted => {}
        ActionResult::TransferCompleted { deferred } => {
            app.is_active_device = true;
            app.pending_replay_action = Some(deferred);
        }
        ActionResult::TransferFailed { error } => {
            app.is_active_device = false;
            tracing::warn!(%error, "playback transfer failed");
            app.notify_error(format!("transfer failed: {error}"));
            if let SpotError::RateLimited { retry_after_secs } = &error {
                let backoff = std::time::Duration::from_secs(*retry_after_secs);
                app.rate_limited_until = Some(std::time::Instant::now() + backoff);
            }
        }
        ActionResult::SearchResults { results } => {
            if let Some(search) = &mut app.search {
                search.loading = false;
                search.input_active = false;
                if let Some(page) = results.tracks {
                    search.result_tracks = page.items;
                }
                if let Some(page) = results.artists {
                    search.result_artists = page.items;
                }
                if let Some(page) = results.albums {
                    search.result_albums = page.items;
                }
                if let Some(page) = results.playlists {
                    search.result_playlists = page.items;
                }
                search.tab_cursors = [0; 4];
                search.tab_scroll_offsets = [0; 4];
                for tab in SearchTab::ALL {
                    let has = match tab {
                        SearchTab::Tracks => !search.result_tracks.is_empty(),
                        SearchTab::Artists => !search.result_artists.is_empty(),
                        SearchTab::Albums => !search.result_albums.is_empty(),
                        SearchTab::Playlists => !search.result_playlists.is_empty(),
                    };
                    if has {
                        search.active_tab = tab;
                        search.sidebar_cursor = tab as usize;
                        break;
                    }
                }
            }
            app.focus = FocusPanel::Sidebar;
        }
        ActionResult::AlbumTracks {
            album_name,
            album_uri,
            artist_name,
            tracks,
            total,
        } => {
            app.set_album_detail(album_name, album_uri, artist_name, tracks, total);
            app.focus = FocusPanel::Content;
        }
        ActionResult::ArtistTopTracksResult {
            artist_name,
            artist_uri,
            tracks,
        } => {
            app.set_artist_top_tracks(artist_name, artist_uri, tracks);
            app.focus = FocusPanel::Content;
        }
        ActionResult::DeviceHealth { generation, found } => {
            if generation != app.engine_generation {
                tracing::debug!(
                    result_gen = generation,
                    current_gen = app.engine_generation,
                    "discarding stale health check result"
                );
                return;
            }
            if !found {
                tracing::warn!("device not found in Spotify devices list, requesting restart");
                app.device_restart_pending = true;
                app.notify(Notification::error("device disconnected, reconnecting..."));
            } else {
                app.restart_failure_count = 0;
            }
        }
        ActionResult::Failed { error } => {
            tracing::warn!(%error, "async action failed");
            if let SpotError::Api {
                status: 404,
                ref message,
            } = error
                && (message.contains("Device not found") || message.contains("device"))
            {
                tracing::warn!("404 device not found, requesting restart");
                app.device_restart_pending = true;
            }
            if let SpotError::RateLimited { retry_after_secs } = &error {
                let backoff = std::time::Duration::from_secs(*retry_after_secs);
                app.rate_limited_until = Some(std::time::Instant::now() + backoff);
            }
            app.notify_error(error);
        }
    }
}

// ---------------------------------------------------------------------------
// Sync helpers
// ---------------------------------------------------------------------------

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

/// Round a volume reported by librespot or the Web API onto the configured
/// step grid. Volume round-trips through a u16 scale, so what comes back is
/// rarely the exact percentage we sent.
pub fn snap_volume(pct: u8, step: u8) -> u8 {
    let step = step.max(1) as u16;
    let snapped = (pct as u16 + step / 2) / step * step;
    snapped.min(100) as u8
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

fn handle_go_back(app: &mut App) {
    if app.search_origin {
        app.search_origin = false;
        app.content = ContentView::Empty;
        if let Some(search) = &mut app.search {
            search.input_active = false;
        }
    } else {
        app.focus = FocusPanel::Sidebar;
    }
}

fn handle_search_select_sync(app: &mut App) -> Option<Action> {
    let search = app.search.as_ref()?;
    let tab = search.active_tab;
    let cursor = search.current_cursor();

    match tab {
        SearchTab::Tracks => {
            if let Some(track) = search.result_tracks.get(cursor) {
                app.progress.start(0, track.duration_ms);
                app.now_playing_track = Some(track.clone());
                app.notify(Notification::info("playing"));
            }
            Some(Action::SearchSelect)
        }
        SearchTab::Albums => {
            let album = search.result_albums.get(cursor)?;
            Some(Action::LoadAlbumTracks {
                album_id: album.id.clone(),
                album_name: album.name.clone(),
                album_uri: album.uri.clone(),
                artist_name: album.artist_names(),
            })
        }
        SearchTab::Artists => {
            let artist = search.result_artists.get(cursor)?;
            Some(Action::LoadArtistTopTracks {
                artist_id: artist.id.clone(),
                artist_name: artist.name.clone(),
                artist_uri: artist.uri.clone(),
            })
        }
        SearchTab::Playlists => {
            let pl = search.result_playlists.get(cursor)?;
            Some(Action::LoadPlaylistFromSearch {
                playlist_id: pl.id.clone(),
                playlist_name: pl.name.clone(),
                playlist_uri: pl.uri.clone(),
            })
        }
    }
}

fn copy_link(app: &mut App) {
    let uri = match app.focus {
        FocusPanel::Sidebar => match app.current_sidebar_item() {
            SidebarItem::LikedSongs => None,
            SidebarItem::Playlist(pl) => Some(pl.uri.clone()),
        },
        FocusPanel::Content => {
            let tracks = app.content.tracks();
            let cursor = app.content.cursor();
            tracks.get(cursor).and_then(|t| t.uri.clone())
        }
    };

    let Some(uri) = uri else {
        app.notify(Notification::info("nothing to copy"));
        return;
    };

    let url = spotify_uri_to_url(&uri).unwrap_or(uri);

    if set_clipboard(&url) {
        app.notify(Notification::info(format!("copied: {url}")));
    } else {
        app.notify(Notification::error("clipboard write failed"));
    }
}

fn spotify_uri_to_url(uri: &str) -> Option<String> {
    // spotify:track:ID → https://open.spotify.com/track/ID
    let mut parts = uri.splitn(3, ':');
    let prefix = parts.next()?;
    let kind = parts.next()?;
    let id = parts.next()?;
    if prefix != "spotify" {
        return None;
    }
    Some(format!("https://open.spotify.com/{kind}/{id}"))
}

/// Write text to the system clipboard via OSC 52 escape sequence.
fn set_clipboard(text: &str) -> bool {
    use base64::Engine;
    use std::io::Write;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{encoded}\x1b\\").is_ok() && out.flush().is_ok()
}
