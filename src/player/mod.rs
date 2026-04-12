use crate::api::{self, SpotifyClient};
use crate::error::{Result, SpotError};
use crate::playback::PlaybackEngine;
use crate::state::{App, ContentView, FocusPanel, Notification, SidebarItem};

const PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone)]
pub enum Action {
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

    // Data — fetched from Web API (async, spawned)
    RefreshPlayback,
    LoadPlaylists,
    #[allow(dead_code)]
    LoadLikedSongs,
    LoadMore,
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
    Failed {
        error: SpotError,
    },
}

pub type ResultTx = tokio::sync::mpsc::UnboundedSender<ActionResult>;

// ---------------------------------------------------------------------------
// Sync path — executes inline, never blocks
// ---------------------------------------------------------------------------

/// Handle an action synchronously if possible. Returns `Some(action)` for
/// actions that need async dispatch (API calls).
pub fn handle_sync(
    action: Action,
    app: &mut App,
    engine: &PlaybackEngine,
) -> Option<Action> {
    let result = match action {
        Action::TogglePlayPause => { return sync_result(toggle_play_pause(app, engine)); }
        Action::NextTrack => { return sync_result(engine.next()); }
        Action::PreviousTrack => { return sync_result(engine.prev()); }
        Action::VolumeUp => { return sync_result(adjust_volume(app, engine, 5)); }
        Action::VolumeDown => { return sync_result(adjust_volume(app, engine, -5)); }
        Action::SeekForward => { return sync_result(seek_relative(app, engine, 5000)); }
        Action::SeekBackward => { return sync_result(seek_relative(app, engine, -5000)); }
        Action::ToggleShuffle => { return sync_result(toggle_shuffle(app, engine)); }
        Action::CycleRepeat => { return sync_result(cycle_repeat(app, engine)); }
        Action::GoBack => { handle_go_back(app); return None; }

        // Select has a sync part (state change) + optional async part
        Action::Select => return handle_select_sync(app, engine),

        // Pure async — pass through
        other => other,
    };
    Some(result)
}

fn sync_result(r: Result<()>) -> Option<Action> {
    if let Err(e) = r {
        tracing::warn!(%e, "action failed");
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
                ContentView::LikedSongs { cursor, tracks, .. } => {
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
}

enum ContentSnapshot {
    Empty,
    PlaylistDetail { uri: String, cursor: usize },
    LikedSongs { uris: Vec<String>, cursor: usize },
}

fn snapshot(app: &App) -> AsyncContext {
    let content_snapshot = match &app.content {
        ContentView::Empty => ContentSnapshot::Empty,
        ContentView::PlaylistDetail { playlist_uri, cursor, .. } => {
            ContentSnapshot::PlaylistDetail {
                uri: playlist_uri.clone(),
                cursor: *cursor,
            }
        }
        ContentView::LikedSongs { tracks, cursor, .. } => {
            ContentSnapshot::LikedSongs {
                uris: tracks.iter().filter_map(|t| t.uri.clone()).collect(),
                cursor: *cursor,
            }
        }
    };
    AsyncContext {
        focus: app.focus,
        sidebar_item: app.current_sidebar_item().clone(),
        content_snapshot,
        sidebar_loaded: app.sidebar_items.len().saturating_sub(1) as u32,
        sidebar_can_load_more: app.sidebar_can_load_more(),
        content_can_load_more: app.content.can_load_more(),
        content_loaded: app.content.len() as u32,
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
                    match client.playlist_tracks(id, PAGE_SIZE, ctx.content_loaded).await {
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

async fn select_async(
    ctx: AsyncContext,
    client: &SpotifyClient,
    device_id: &str,
) -> ActionResult {
    match ctx.focus {
        FocusPanel::Sidebar => match ctx.sidebar_item {
            SidebarItem::LikedSongs => match client.liked_tracks(PAGE_SIZE, 0).await {
                Ok(page) => ActionResult::LikedSongs { page },
                Err(e) => ActionResult::Failed { error: e },
            },
            SidebarItem::Playlist(pl) => {
                match client.playlist_tracks(&pl.id, PAGE_SIZE, 0).await {
                    Ok(page) => ActionResult::PlaylistTracks {
                        name: pl.name,
                        uri: pl.uri,
                        page,
                    },
                    Err(e) => ActionResult::Failed { error: e },
                }
            }
        },
        FocusPanel::Content => {
            // The sync path already set optimistic state; just issue the play call.
            let result = match &ctx.content_snapshot {
                ContentSnapshot::Empty => return ActionResult::PlaybackStarted,
                ContentSnapshot::PlaylistDetail { uri, cursor } => {
                    client
                        .play_context_on(uri, Some(*cursor), device_id)
                        .await
                }
                ContentSnapshot::LikedSongs { uris, cursor } => {
                    if uris.is_empty() {
                        return ActionResult::PlaybackStarted;
                    }
                    client
                        .play_tracks_on(uris, Some(*cursor), device_id)
                        .await
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
            app.set_playlist_tracks(name, uri, page);
            app.focus = FocusPanel::Content;
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
        }
        ActionResult::MoreLikedSongs { page } => {
            app.append_liked_songs(page);
        }
        ActionResult::PlaybackState { state } => {
            if let Some(pb) = state {
                app.shuffle = pb.shuffle_state.unwrap_or(false);
                app.repeat = pb.repeat_mode();
                if let Some(dev) = &pb.device {
                    let v = dev.volume_percent.unwrap_or(app.volume);
                    app.volume = ((v + 2) / 5 * 5).min(100);
                }
                if app.now_playing_track.is_none()
                    && let Some(track) = pb.item
                {
                    app.progress
                        .start(pb.progress_ms.unwrap_or(0), track.duration_ms);
                    if !pb.is_playing {
                        app.progress.pause();
                    }
                    app.now_playing_track = Some(track);
                }
            }
        }
        ActionResult::PlaybackStarted => {}
        ActionResult::Failed { error } => {
            tracing::warn!(%error, "async action failed");
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
    app.focus = FocusPanel::Sidebar;
}
