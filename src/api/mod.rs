mod models;

pub use models::*;

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::Mutex as AsyncMutex;

use crate::error::{Result, SpotError};

const BASE: &str = "https://api.spotify.com/v1";
const MAX_ATTEMPTS: u8 = 4;

// Rate-limit pacing: enforce minimum interval between API calls.
static API_PACING: OnceLock<AsyncMutex<Option<Instant>>> = OnceLock::new();
const MIN_INTERVAL: Duration = Duration::from_millis(250);

async fn pace() {
    let lock = API_PACING.get_or_init(|| AsyncMutex::new(None));
    let mut last = lock.lock().await;
    if let Some(prev) = *last {
        let elapsed = prev.elapsed();
        if elapsed < MIN_INTERVAL {
            tokio::time::sleep(MIN_INTERVAL - elapsed).await;
        }
    }
    *last = Some(Instant::now());
}

#[derive(Clone)]
pub struct SpotifyClient {
    http: reqwest::Client,
    token: Arc<Mutex<String>>,
}

impl SpotifyClient {
    pub fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: Arc::new(Mutex::new(token.to_string())),
        }
    }

    pub fn set_token(&self, token: &str) {
        *self.token.lock().unwrap() = token.to_string();
    }

    fn current_token(&self) -> String {
        self.token.lock().unwrap().clone()
    }

    // -- Core request method with pacing + retry --

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut attempt: u8 = 0;
        let mut retried_auth = false;
        loop {
            pace().await;
            let resp = match self
                .http
                .get(url)
                .bearer_auth(self.current_token())
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) if attempt + 1 < MAX_ATTEMPTS && is_transient(&e) => {
                    let backoff = 1 + u64::from(attempt);
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    attempt += 1;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            let status = resp.status().as_u16();
            if resp.status().is_success() {
                return Ok(resp.json().await?);
            }
            if status == 401 && !retried_auth {
                retried_auth = true;
                tracing::debug!(%url, "401 received, retrying with refreshed token");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            if status == 429 && attempt + 1 < MAX_ATTEMPTS {
                let retry = retry_after(&resp);
                let backoff = retry.max(1) + u64::from(attempt);
                tracing::warn!(%url, backoff, "rate limited, backing off");
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                attempt += 1;
                continue;
            }
            if status == 429 {
                return Err(SpotError::RateLimited {
                    retry_after_secs: retry_after(&resp),
                });
            }
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotError::Api {
                status,
                message: body,
            });
        }
    }

    async fn get_json_optional<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<Option<T>> {
        let mut attempt: u8 = 0;
        let mut retried_auth = false;
        loop {
            pace().await;
            let resp = match self
                .http
                .get(url)
                .bearer_auth(self.current_token())
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) if attempt + 1 < MAX_ATTEMPTS && is_transient(&e) => {
                    let backoff = 1 + u64::from(attempt);
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    attempt += 1;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            let status = resp.status().as_u16();
            if status == 204 {
                return Ok(None);
            }
            if resp.status().is_success() {
                return Ok(Some(resp.json().await?));
            }
            if status == 401 && !retried_auth {
                retried_auth = true;
                tracing::debug!(%url, "401 received, retrying with refreshed token");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            if status == 429 && attempt + 1 < MAX_ATTEMPTS {
                let retry = retry_after(&resp);
                let backoff = retry.max(1) + u64::from(attempt);
                tracing::warn!(%url, backoff, "rate limited, backing off");
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                attempt += 1;
                continue;
            }
            if status == 429 {
                return Err(SpotError::RateLimited {
                    retry_after_secs: retry_after(&resp),
                });
            }
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotError::Api {
                status,
                message: body,
            });
        }
    }

    async fn put_json(&self, url: &str, body: &serde_json::Value) -> Result<()> {
        let mut attempt: u8 = 0;
        let mut retried_auth = false;
        loop {
            pace().await;
            let resp = match self
                .http
                .put(url)
                .bearer_auth(self.current_token())
                .json(body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) if attempt + 1 < MAX_ATTEMPTS && is_transient(&e) => {
                    let backoff = 1 + u64::from(attempt);
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    attempt += 1;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            let status = resp.status().as_u16();
            if resp.status().is_success() || status == 204 {
                return Ok(());
            }
            if status == 401 && !retried_auth {
                retried_auth = true;
                tracing::debug!(%url, "401 received, retrying with refreshed token");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            if status == 429 && attempt + 1 < MAX_ATTEMPTS {
                let retry = retry_after(&resp);
                let backoff = retry.max(1) + u64::from(attempt);
                tracing::warn!(%url, backoff, "rate limited, backing off");
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                attempt += 1;
                continue;
            }
            if status == 429 {
                return Err(SpotError::RateLimited {
                    retry_after_secs: retry_after(&resp),
                });
            }
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotError::Api {
                status,
                message: body,
            });
        }
    }

    // -- Devices --

    pub async fn get_devices(&self) -> Result<DevicesResponse> {
        let url = format!("{BASE}/me/player/devices");
        tracing::debug!(%url, "GET devices");
        self.get_json(&url).await
    }

    // -- Playback state (read-only, used for metadata sync) --

    pub async fn current_playback(&self) -> Result<Option<PlaybackState>> {
        let url = format!("{BASE}/me/player");
        tracing::debug!(%url, "GET playback state");
        self.get_json_optional(&url).await
    }

    // -- Start playback on a specific device (targets our Spirc device) --

    pub async fn play_context_on(
        &self,
        context_uri: &str,
        offset: Option<usize>,
        device_id: &str,
    ) -> Result<()> {
        let body = if let Some(off) = offset {
            serde_json::json!({
                "context_uri": context_uri,
                "offset": { "position": off }
            })
        } else {
            serde_json::json!({ "context_uri": context_uri })
        };
        let mut url = format!("{BASE}/me/player/play");
        if !device_id.is_empty() {
            url.push_str(&format!("?device_id={device_id}"));
        }
        tracing::debug!(%url, %context_uri, "PUT play context");
        self.put_json(&url, &body).await
    }

    pub async fn play_tracks_on(
        &self,
        uris: &[String],
        offset: Option<usize>,
        device_id: &str,
    ) -> Result<()> {
        let body = if let Some(off) = offset {
            serde_json::json!({
                "uris": uris,
                "offset": { "position": off }
            })
        } else {
            serde_json::json!({ "uris": uris })
        };
        let mut url = format!("{BASE}/me/player/play");
        if !device_id.is_empty() {
            url.push_str(&format!("?device_id={device_id}"));
        }
        tracing::debug!(%url, track_count = uris.len(), "PUT play tracks");
        self.put_json(&url, &body).await
    }

    // -- Transfer playback --

    pub async fn transfer_playback(&self, device_id: &str) -> Result<()> {
        let url = format!("{BASE}/me/player");
        let body = serde_json::json!({
            "device_ids": [device_id]
        });
        tracing::debug!(%device_id, "PUT transfer playback");
        self.put_json(&url, &body).await
    }

    // -- Playlists --

    pub async fn my_playlists(&self, limit: u32, offset: u32) -> Result<Page<Playlist>> {
        let url = format!("{BASE}/me/playlists?limit={limit}&offset={offset}");
        tracing::debug!(%url, "GET my playlists");
        self.get_json(&url).await
    }

    pub async fn playlist_tracks(
        &self,
        playlist_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<PlaylistItem>> {
        let url = format!("{BASE}/playlists/{playlist_id}/tracks?limit={limit}&offset={offset}");
        tracing::debug!(%url, %playlist_id, "GET playlist tracks");
        self.get_json(&url).await
    }

    // -- Recently played --

    pub async fn recently_played(&self, limit: u32) -> Result<CursorPage<PlayHistory>> {
        let url = format!("{BASE}/me/player/recently-played?limit={limit}");
        tracing::debug!(%url, "GET recently played");
        self.get_json(&url).await
    }

    // -- Search --

    pub async fn search(&self, query: &str, limit: u32) -> Result<SearchResults> {
        let query_string = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("q", query)
            .append_pair("type", "track,artist,album,playlist")
            .append_pair("limit", &limit.to_string())
            .finish();
        let url = format!("{BASE}/search?{query_string}");
        tracing::debug!(%url, "GET search");
        self.get_json(&url).await
    }

    // -- Album tracks --

    pub async fn album_tracks(
        &self,
        album_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<AlbumTrack>> {
        let url = format!("{BASE}/albums/{album_id}/tracks?limit={limit}&offset={offset}");
        tracing::debug!(%url, %album_id, "GET album tracks");
        self.get_json(&url).await
    }

    // -- Artist top tracks --

    pub async fn artist_top_tracks(&self, artist_id: &str) -> Result<ArtistTopTracks> {
        let url = format!("{BASE}/artists/{artist_id}/top-tracks");
        tracing::debug!(%url, %artist_id, "GET artist top tracks");
        self.get_json(&url).await
    }

    // -- Library --

    pub async fn liked_tracks(&self, limit: u32, offset: u32) -> Result<Page<SavedTrack>> {
        let url = format!("{BASE}/me/tracks?limit={limit}&offset={offset}");
        tracing::debug!(%url, "GET liked tracks");
        self.get_json(&url).await
    }

    pub async fn my_albums(&self, limit: u32, offset: u32) -> Result<Page<SavedAlbum>> {
        let url = format!("{BASE}/me/albums?limit={limit}&offset={offset}");
        tracing::debug!(%url, "GET my albums");
        self.get_json(&url).await
    }
}

fn retry_after(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
}

fn is_transient(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}
