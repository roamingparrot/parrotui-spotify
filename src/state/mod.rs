pub mod marquee;

pub use marquee::MarqueeState;

use crate::api::{Album, FullArtist, Page, Playlist, PlaylistItem, RepeatMode, SavedTrack, Track};
use crate::config::Config;
use crate::input::{Key, Keymap};
use crate::playback::ProgressTracker;
use crate::player::Action;
use crate::ui::theme::Theme;

// -- Search state --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTab {
    Tracks,
    Artists,
    Albums,
    Playlists,
}

impl SearchTab {
    pub const ALL: [SearchTab; 4] = [Self::Tracks, Self::Artists, Self::Albums, Self::Playlists];

    pub fn next(self) -> Self {
        match self {
            Self::Tracks => Self::Artists,
            Self::Artists => Self::Albums,
            Self::Albums => Self::Playlists,
            Self::Playlists => Self::Tracks,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Tracks => Self::Playlists,
            Self::Artists => Self::Tracks,
            Self::Albums => Self::Artists,
            Self::Playlists => Self::Albums,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Playlists => "Playlists",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub input_cursor: usize,
    pub input_active: bool,
    pub active_tab: SearchTab,
    pub sidebar_cursor: usize,
    pub tab_cursors: [usize; 4],
    pub tab_scroll_offsets: [usize; 4],
    pub result_tracks: Vec<Track>,
    pub result_artists: Vec<FullArtist>,
    pub result_albums: Vec<Album>,
    pub result_playlists: Vec<Playlist>,
    pub loading: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            input_cursor: 0,
            input_active: true,
            active_tab: SearchTab::Tracks,
            sidebar_cursor: 0,
            tab_cursors: [0; 4],
            tab_scroll_offsets: [0; 4],
            result_tracks: Vec::new(),
            result_artists: Vec::new(),
            result_albums: Vec::new(),
            result_playlists: Vec::new(),
            loading: false,
        }
    }

    pub fn current_tab_len(&self) -> usize {
        match self.active_tab {
            SearchTab::Tracks => self.result_tracks.len(),
            SearchTab::Artists => self.result_artists.len(),
            SearchTab::Albums => self.result_albums.len(),
            SearchTab::Playlists => self.result_playlists.len(),
        }
    }

    fn tab_index(&self) -> usize {
        self.active_tab as usize
    }

    pub fn current_cursor(&self) -> usize {
        self.tab_cursors[self.tab_index()]
    }

    pub fn current_cursor_mut(&mut self) -> &mut usize {
        let idx = self.tab_index();
        &mut self.tab_cursors[idx]
    }

    #[allow(dead_code)]
    pub fn current_scroll_offset_mut(&mut self) -> &mut usize {
        let idx = self.tab_index();
        &mut self.tab_scroll_offsets[idx]
    }

    pub fn move_down(&mut self) {
        let len = self.current_tab_len();
        if len == 0 {
            return;
        }
        let c = self.current_cursor_mut();
        if *c + 1 < len {
            *c += 1;
        }
    }

    pub fn move_up(&mut self) {
        let c = self.current_cursor_mut();
        *c = c.saturating_sub(1);
    }

    #[allow(dead_code)]
    pub fn switch_tab_forward(&mut self) {
        self.active_tab = self.active_tab.next();
    }

    #[allow(dead_code)]
    pub fn switch_tab_backward(&mut self) {
        self.active_tab = self.active_tab.prev();
    }

    pub fn push_char(&mut self, ch: char) {
        self.query.insert(self.input_cursor, ch);
        self.input_cursor += ch.len_utf8();
    }

    pub fn pop_char(&mut self) {
        if self.input_cursor > 0 {
            let prev = self.query[..self.input_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.drain(prev..self.input_cursor);
            self.input_cursor = prev;
        }
    }

    pub fn clear_results(&mut self) {
        self.result_tracks.clear();
        self.result_artists.clear();
        self.result_albums.clear();
        self.result_playlists.clear();
        self.tab_cursors = [0; 4];
        self.tab_scroll_offsets = [0; 4];
    }

    pub fn sidebar_move_down(&mut self) {
        if self.sidebar_cursor < 3 {
            self.sidebar_cursor += 1;
        }
    }

    pub fn sidebar_move_up(&mut self) {
        self.sidebar_cursor = self.sidebar_cursor.saturating_sub(1);
    }

    pub fn select_sidebar_item(&mut self) {
        self.active_tab = SearchTab::ALL[self.sidebar_cursor];
    }

    pub fn has_results(&self) -> bool {
        !self.result_tracks.is_empty()
            || !self.result_artists.is_empty()
            || !self.result_albums.is_empty()
            || !self.result_playlists.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    Sidebar,
    Content,
}

#[derive(Debug, Clone)]
pub enum SidebarItem {
    LikedSongs,
    Playlist(Playlist),
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub is_error: bool,
    pub ttl: std::time::Instant,
}

impl Notification {
    pub fn info(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_error: false,
            ttl: std::time::Instant::now() + std::time::Duration::from_secs(3),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_error: true,
            ttl: std::time::Instant::now() + std::time::Duration::from_secs(5),
        }
    }

    pub fn expired(&self) -> bool {
        std::time::Instant::now() >= self.ttl
    }
}

#[derive(Debug, Clone)]
pub enum ContentView {
    Empty,
    PlaylistDetail {
        playlist_name: String,
        playlist_uri: String,
        tracks: Vec<Track>,
        cursor: usize,
        total: u32,
        loading: bool,
    },
    LikedSongs {
        tracks: Vec<Track>,
        cursor: usize,
        total: u32,
        loading: bool,
    },
    AlbumDetail {
        album_name: String,
        #[allow(dead_code)]
        album_uri: String,
        artist_name: String,
        tracks: Vec<Track>,
        cursor: usize,
        total: u32,
        loading: bool,
    },
    ArtistTopTracks {
        artist_name: String,
        #[allow(dead_code)]
        artist_uri: String,
        tracks: Vec<Track>,
        cursor: usize,
    },
}

impl ContentView {
    pub fn cursor(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::PlaylistDetail { cursor, .. }
            | Self::LikedSongs { cursor, .. }
            | Self::AlbumDetail { cursor, .. }
            | Self::ArtistTopTracks { cursor, .. } => *cursor,
        }
    }

    pub fn cursor_mut(&mut self) -> &mut usize {
        match self {
            Self::Empty => unreachable!("cursor_mut on Empty"),
            Self::PlaylistDetail { cursor, .. }
            | Self::LikedSongs { cursor, .. }
            | Self::AlbumDetail { cursor, .. }
            | Self::ArtistTopTracks { cursor, .. } => cursor,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::PlaylistDetail { tracks, .. }
            | Self::LikedSongs { tracks, .. }
            | Self::AlbumDetail { tracks, .. }
            | Self::ArtistTopTracks { tracks, .. } => tracks.len(),
        }
    }

    pub fn is_loading(&self) -> bool {
        match self {
            Self::Empty | Self::ArtistTopTracks { .. } => false,
            Self::PlaylistDetail { loading, .. }
            | Self::LikedSongs { loading, .. }
            | Self::AlbumDetail { loading, .. } => *loading,
        }
    }

    pub fn total(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::PlaylistDetail { total, .. }
            | Self::LikedSongs { total, .. }
            | Self::AlbumDetail { total, .. } => *total,
            Self::ArtistTopTracks { tracks, .. } => tracks.len() as u32,
        }
    }

    pub fn can_load_more(&self) -> bool {
        (self.len() as u32) < self.total() && !self.is_loading()
    }

    pub fn set_loading(&mut self) {
        match self {
            Self::Empty | Self::ArtistTopTracks { .. } => {}
            Self::PlaylistDetail { loading, .. }
            | Self::LikedSongs { loading, .. }
            | Self::AlbumDetail { loading, .. } => *loading = true,
        }
    }

    pub fn tracks(&self) -> &[Track] {
        match self {
            Self::Empty => &[],
            Self::PlaylistDetail { tracks, .. }
            | Self::LikedSongs { tracks, .. }
            | Self::AlbumDetail { tracks, .. }
            | Self::ArtistTopTracks { tracks, .. } => tracks,
        }
    }
}

pub struct App {
    pub running: bool,
    pub config: Config,

    pub focus: FocusPanel,
    pub sidebar_cursor: usize,
    pub sidebar_items: Vec<SidebarItem>,
    pub sidebar_total: u32,
    pub sidebar_loading: bool,

    pub content: ContentView,
    pub show_help: bool,
    pub notification: Option<Notification>,

    // Playback — owned locally
    pub progress: ProgressTracker,
    pub now_playing_track: Option<Track>,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub device_name: String,
    pub device_id: String,
    pub is_active_device: bool,
    pub pending_replay_action: Option<Action>,

    // Keys pressed so far towards an unfinished chord (e.g. the first 'g' of gg)
    pub keymap: Keymap,
    pub pending_keys: Vec<Key>,

    // Marquee scroll state
    pub sidebar_marquee: MarqueeState,
    pub track_marquee: MarqueeState,

    // Viewport scroll offsets (persisted across frames so the list only
    // scrolls when the cursor leaves the visible area).
    pub sidebar_scroll_offset: usize,
    pub content_scroll_offset: usize,

    // Deferred jump — set when G is pressed but not all tracks are loaded yet.
    pub pending_jump_to_bottom: bool,

    // Rate-limit backoff — skip API calls until this instant
    pub rate_limited_until: Option<std::time::Instant>,

    // Device health check
    pub engine_generation: u64,
    pub last_player_event_at: Option<std::time::Instant>,
    pub device_restart_pending: bool,
    pub restart_failure_count: u8,

    // Theme
    pub theme: Theme,

    // Search
    pub search: Option<SearchState>,
    pub search_origin: bool,
}

impl App {
    pub fn new(config: Config, device_id: String) -> Self {
        let theme = Theme::from_name(&config.theme);
        let device_name = config.device_name.clone();
        let volume = config.initial_volume;
        Self {
            running: true,
            config,
            focus: FocusPanel::Sidebar,
            sidebar_cursor: 0,
            sidebar_items: vec![SidebarItem::LikedSongs],
            sidebar_total: 0,
            sidebar_loading: true,
            content: ContentView::Empty,
            show_help: false,
            notification: None,
            progress: ProgressTracker::new(),
            now_playing_track: None,
            volume,
            shuffle: false,
            repeat: RepeatMode::Off,
            device_name,
            device_id,
            is_active_device: false,
            pending_replay_action: None,
            keymap: Keymap::default(),
            pending_keys: Vec::new(),
            sidebar_marquee: MarqueeState::new(),
            track_marquee: MarqueeState::new(),
            sidebar_scroll_offset: 0,
            content_scroll_offset: 0,
            pending_jump_to_bottom: false,
            rate_limited_until: None,
            engine_generation: 0,
            last_player_event_at: None,
            device_restart_pending: false,
            restart_failure_count: 0,
            theme,
            search: None,
            search_origin: false,
        }
    }

    pub fn current_sidebar_item(&self) -> &SidebarItem {
        &self.sidebar_items[self.sidebar_cursor]
    }

    pub fn sidebar_can_load_more(&self) -> bool {
        let playlist_count = self.sidebar_items.len().saturating_sub(1) as u32;
        playlist_count < self.sidebar_total && !self.sidebar_loading
    }

    pub fn notify(&mut self, n: Notification) {
        self.notification = Some(n);
    }

    pub fn notify_error(&mut self, msg: impl std::fmt::Display) {
        self.notification = Some(Notification::error(msg.to_string()));
    }

    pub fn clear_stale_notification(&mut self) {
        if let Some(n) = &self.notification
            && n.expired()
        {
            self.notification = None;
        }
    }

    /// Look up a track by Spotify URI in the currently-displayed playlist or liked songs.
    pub fn find_track_by_uri(&self, uri: &str) -> Option<Track> {
        let tracks = match &self.content {
            ContentView::Empty => return None,
            ContentView::PlaylistDetail { tracks, .. }
            | ContentView::LikedSongs { tracks, .. }
            | ContentView::AlbumDetail { tracks, .. }
            | ContentView::ArtistTopTracks { tracks, .. } => tracks,
        };
        tracks
            .iter()
            .find(|t| t.uri.as_deref() == Some(uri))
            .cloned()
    }

    // -- Cursor movement --

    pub fn move_cursor_down(&mut self) {
        match self.focus {
            FocusPanel::Sidebar => {
                if self.sidebar_cursor + 1 < self.sidebar_items.len() {
                    self.sidebar_cursor += 1;
                }
            }
            FocusPanel::Content => {
                let len = self.content.len();
                if len == 0 {
                    return;
                }
                let c = self.content.cursor_mut();
                if *c + 1 < len {
                    *c += 1;
                }
            }
        }
    }

    pub fn move_cursor_up(&mut self) {
        match self.focus {
            FocusPanel::Sidebar => {
                self.sidebar_cursor = self.sidebar_cursor.saturating_sub(1);
            }
            FocusPanel::Content => {
                if self.content.len() == 0 {
                    return;
                }
                let c = self.content.cursor_mut();
                *c = c.saturating_sub(1);
            }
        }
    }

    pub fn jump_to_top(&mut self) {
        match self.focus {
            FocusPanel::Sidebar => {
                self.sidebar_cursor = 0;
                self.sidebar_scroll_offset = 0;
            }
            FocusPanel::Content => {
                if self.content.len() > 0 {
                    *self.content.cursor_mut() = 0;
                    self.content_scroll_offset = 0;
                }
            }
        }
    }

    pub fn jump_to_bottom(&mut self) {
        match self.focus {
            FocusPanel::Sidebar => {
                self.sidebar_cursor = self.sidebar_items.len().saturating_sub(1);
                self.sidebar_scroll_offset = 0;
            }
            FocusPanel::Content => {
                if self.content.len() > 0 {
                    let len = self.content.len();
                    *self.content.cursor_mut() = len.saturating_sub(1);
                    self.content_scroll_offset = 0;
                }
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        match self.focus {
            FocusPanel::Sidebar => {
                self.sidebar_marquee.reset();
                self.focus = FocusPanel::Content;
            }
            FocusPanel::Content => {
                self.focus = FocusPanel::Sidebar;
            }
        }
    }

    pub fn set_sidebar_playlists(&mut self, playlists: Vec<Playlist>, total: u32) {
        self.sidebar_items = vec![SidebarItem::LikedSongs];
        self.sidebar_items
            .extend(playlists.into_iter().map(SidebarItem::Playlist));
        self.sidebar_total = total;
        self.sidebar_loading = false;
        self.sidebar_scroll_offset = 0;
    }

    pub fn append_sidebar_playlists(&mut self, playlists: Vec<Playlist>, total: u32) {
        self.sidebar_items
            .extend(playlists.into_iter().map(SidebarItem::Playlist));
        self.sidebar_total = total;
        self.sidebar_loading = false;
    }

    pub fn set_playlist_tracks(&mut self, name: String, uri: String, page: Page<PlaylistItem>) {
        let tracks = page.items.into_iter().filter_map(|pi| pi.track).collect();
        self.content = ContentView::PlaylistDetail {
            playlist_name: name,
            playlist_uri: uri,
            tracks,
            cursor: 0,
            total: page.total,
            loading: false,
        };
        self.pending_jump_to_bottom = false;
        self.content_scroll_offset = 0;
    }

    pub fn append_playlist_tracks(&mut self, page: Page<PlaylistItem>) {
        if let ContentView::PlaylistDetail {
            tracks,
            total,
            loading,
            ..
        } = &mut self.content
        {
            let new: Vec<Track> = page.items.into_iter().filter_map(|pi| pi.track).collect();
            tracks.extend(new);
            *total = page.total;
            *loading = false;
        }
    }

    pub fn set_liked_songs(&mut self, page: Page<SavedTrack>) {
        let tracks = page.items.into_iter().map(|st| st.track).collect();
        self.content = ContentView::LikedSongs {
            tracks,
            cursor: 0,
            total: page.total,
            loading: false,
        };
        self.pending_jump_to_bottom = false;
        self.content_scroll_offset = 0;
    }

    pub fn append_liked_songs(&mut self, page: Page<SavedTrack>) {
        if let ContentView::LikedSongs {
            tracks,
            total,
            loading,
            ..
        } = &mut self.content
        {
            let new: Vec<Track> = page.items.into_iter().map(|st| st.track).collect();
            tracks.extend(new);
            *total = page.total;
            *loading = false;
        }
    }

    // -- Search helpers --

    pub fn open_search(&mut self) {
        self.search = Some(SearchState::new());
        // A drill-down view may still be marked as search-origin; clear it so
        // the new search modal captures input.
        self.search_origin = false;
    }

    pub fn close_search(&mut self) {
        // Returning home: drop the search modal and put focus back on the
        // sidebar. Search results live in `self.search` (not `self.content`),
        // so leaving focus on Content would strand the cursor on whatever the
        // content panel holds — an empty home screen or a stale drill-down.
        if self.search_origin {
            self.content = ContentView::Empty;
        }
        self.search = None;
        self.search_origin = false;
        self.focus = FocusPanel::Sidebar;
    }

    pub fn set_album_detail(
        &mut self,
        name: String,
        uri: String,
        artist: String,
        tracks: Vec<Track>,
        total: u32,
    ) {
        self.content = ContentView::AlbumDetail {
            album_name: name,
            album_uri: uri,
            artist_name: artist,
            tracks,
            cursor: 0,
            total,
            loading: false,
        };
        self.content_scroll_offset = 0;
        self.search_origin = true;
    }

    pub fn set_artist_top_tracks(&mut self, name: String, uri: String, tracks: Vec<Track>) {
        self.content = ContentView::ArtistTopTracks {
            artist_name: name,
            artist_uri: uri,
            tracks,
            cursor: 0,
        };
        self.content_scroll_offset = 0;
        self.search_origin = true;
    }
}
