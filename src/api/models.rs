use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u32,
    #[allow(dead_code)]
    pub offset: u32,
    #[allow(dead_code)]
    pub limit: u32,
}

// -- Playback state --

#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub progress_ms: Option<u64>,
    pub item: Option<Track>,
    pub device: Option<Device>,
    pub shuffle_state: Option<bool>,
    pub repeat_state: Option<String>,
}

impl PlaybackState {
    pub fn repeat_mode(&self) -> RepeatMode {
        match self.repeat_state.as_deref() {
            Some("track") => RepeatMode::Track,
            Some("context") => RepeatMode::Context,
            _ => RepeatMode::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    Context,
    Track,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Context,
            Self::Context => Self::Track,
            Self::Track => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Context => "all",
            Self::Track => "one",
        }
    }
}

// -- Track --

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub name: String,
    pub uri: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub artists: Vec<Artist>,
    // Deserialized from API but not displayed yet
    #[allow(dead_code)]
    pub album: Option<Album>,
}

impl Track {
    pub fn artist_names(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    #[allow(dead_code)]
    pub name: String,
}

// -- Playlist --

#[derive(Debug, Clone, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub uri: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub tracks: Option<PlaylistTracksRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistTracksRef {
    #[allow(dead_code)]
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistItem {
    #[serde(default, deserialize_with = "deserialize_playlist_track")]
    pub track: Option<Track>,
}

/// Deserialize the `track` field of a playlist item, silently returning `None`
/// for entries that don't parse as a `Track` (podcast episodes, local files, etc.).
fn deserialize_playlist_track<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Track>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    Ok(value.and_then(|v| serde_json::from_value::<Track>(v).ok()))
}

// -- Saved tracks (liked songs) --

#[derive(Debug, Clone, Deserialize)]
pub struct SavedTrack {
    pub track: Track,
}

// -- Recently played --

#[derive(Debug, Clone, Deserialize)]
pub struct PlayHistory {
    #[allow(dead_code)]
    pub track: Track,
    #[allow(dead_code)]
    pub played_at: String,
    pub context: Option<PlayContext>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayContext {
    #[serde(rename = "type")]
    pub context_type: String,
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CursorPage<T> {
    pub items: Vec<T>,
}

// -- Device --

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    #[allow(dead_code)]
    pub id: Option<String>,
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub is_active: bool,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub device_type: String,
    pub volume_percent: Option<u8>,
}
