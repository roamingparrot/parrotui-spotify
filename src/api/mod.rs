mod models;

pub use models::*;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::{Result, SpotError};

const BASE: &str = "https://api.spotify.com/v1";
const MAX_ATTEMPTS: u8 = 4;

// Rate-limit pacing: enforce minimum interval between API calls.
static API_PACING: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
const MIN_INTERVAL: Duration = Duration::from_millis(250);

async fn pace() {
    let lock = API_PACING.get_or_init(|| Mutex::new(None));
    let mut last = lock.lock().await;
    if let Some(prev) = *last {
        let elapsed = prev.elapsed();
        if elapsed < MIN_INTERVAL {
            tokio::time::sleep(MIN_INTERVAL - elapsed).await;
        }
    }
    *last = Some(Instant::now());
}

pub struct SpotifyClient {
    http: reqwest::Client,
    token: String,
}

impl SpotifyClient {
    pub fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_string(),
        }
    }

    pub fn set_token(&mut self, token: &str) {
        self.token = token.to_string();
    }

    // -- Core request method with pacing + retry --

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut attempt: u8 = 0;
        loop {
            pace().await;
            let resp = match self.http.get(url).bearer_auth(&self.token).send().await {
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
        loop {
            pace().await;
            let resp = match self.http.get(url).bearer_auth(&self.token).send().await {
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
        loop {
            pace().await;
            let resp = match self
                .http
                .put(url)
                .bearer_auth(&self.token)
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

    // -- Library --

    pub async fn liked_tracks(&self, limit: u32, offset: u32) -> Result<Page<SavedTrack>> {
        let url = format!("{BASE}/me/tracks?limit={limit}&offset={offset}");
        tracing::debug!(%url, "GET liked tracks");
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
