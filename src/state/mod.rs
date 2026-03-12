use crate::api::{Page, Playlist, PlaylistItem, RepeatMode, SavedTrack, Track};
use crate::playback::ProgressTracker;

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
}

impl ContentView {
    pub fn cursor(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::PlaylistDetail { cursor, .. } => *cursor,
            Self::LikedSongs { cursor, .. } => *cursor,
        }
    }

    pub fn cursor_mut(&mut self) -> &mut usize {
        match self {
            Self::Empty => unreachable!("cursor_mut on Empty"),
            Self::PlaylistDetail { cursor, .. } => cursor,
            Self::LikedSongs { cursor, .. } => cursor,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::PlaylistDetail { tracks, .. } => tracks.len(),
            Self::LikedSongs { tracks, .. } => tracks.len(),
        }
    }

    pub fn is_loading(&self) -> bool {
        match self {
            Self::Empty => false,
            Self::PlaylistDetail { loading, .. } => *loading,
            Self::LikedSongs { loading, .. } => *loading,
        }
    }

    pub fn total(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::PlaylistDetail { total, .. } => *total,
            Self::LikedSongs { total, .. } => *total,
        }
    }

    pub fn can_load_more(&self) -> bool {
        (self.len() as u32) < self.total() && !self.is_loading()
    }
}

pub struct App {
    pub running: bool,

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

    // gg combo state
    pub pending_g: bool,
}

impl App {
    pub fn new(device_name: String, initial_volume: u8) -> Self {
        Self {
            running: true,
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
            volume: initial_volume,
            shuffle: false,
            repeat: RepeatMode::Off,
            device_name,
            pending_g: false,
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
        if let Some(n) = &self.notification {
            if n.expired() {
                self.notification = None;
            }
        }
    }

    pub fn is_playing(&self) -> bool {
        self.progress.is_playing()
    }

    pub fn now_playing(&self) -> Option<&Track> {
        self.now_playing_track.as_ref()
    }

    /// Look up a track by Spotify URI in the currently-displayed playlist or liked songs.
    pub fn find_track_by_uri(&self, uri: &str) -> Option<Track> {
        let tracks = match &self.content {
            ContentView::Empty => return None,
            ContentView::PlaylistDetail { tracks, .. } => tracks,
            ContentView::LikedSongs { tracks, .. } => tracks,
        };
        tracks.iter().find(|t| t.uri.as_deref() == Some(uri)).cloned()
    }

    pub fn progress_ms(&self) -> u64 {
        self.progress.position_ms()
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
            FocusPanel::Sidebar => self.sidebar_cursor = 0,
            FocusPanel::Content => {
                if self.content.len() > 0 {
                    *self.content.cursor_mut() = 0;
                }
            }
        }
    }

    pub fn jump_to_bottom(&mut self) {
        match self.focus {
            FocusPanel::Sidebar => {
                self.sidebar_cursor = self.sidebar_items.len().saturating_sub(1);
            }
            FocusPanel::Content => {
                if self.content.len() > 0 {
                    let len = self.content.len();
                    *self.content.cursor_mut() = len.saturating_sub(1);
                }
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPanel::Sidebar => FocusPanel::Content,
            FocusPanel::Content => FocusPanel::Sidebar,
        };
    }

    pub fn set_sidebar_playlists(&mut self, playlists: Vec<Playlist>, total: u32) {
        self.sidebar_items = vec![SidebarItem::LikedSongs];
        self.sidebar_items
            .extend(playlists.into_iter().map(SidebarItem::Playlist));
        self.sidebar_total = total;
        self.sidebar_loading = false;
    }

    pub fn append_sidebar_playlists(&mut self, playlists: Vec<Playlist>, total: u32) {
        self.sidebar_items
            .extend(playlists.into_iter().map(SidebarItem::Playlist));
        self.sidebar_total = total;
        self.sidebar_loading = false;
    }

    pub fn set_playlist_tracks(
        &mut self,
        name: String,
        uri: String,
        page: Page<PlaylistItem>,
    ) {
        let tracks = page
            .items
            .into_iter()
            .filter_map(|pi| pi.track)
            .collect();
        self.content = ContentView::PlaylistDetail {
            playlist_name: name,
            playlist_uri: uri,
            tracks,
            cursor: 0,
            total: page.total,
            loading: false,
        };
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
}
