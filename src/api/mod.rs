mod models;

pub use models::*;

use crate::error::{Result, SpotError};

const BASE: &str = "https://api.spotify.com/v1";

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

    // -- Playback state (read-only, used for metadata sync) --

    pub async fn current_playback(&self) -> Result<Option<PlaybackState>> {
        let resp = self.get("/me/player").await?;
        if resp.status().as_u16() == 204 {
            return Ok(None);
        }
        self.check_error(&resp)?;
        Ok(Some(resp.json().await?))
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
        let resp = self
            .http
            .put(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        self.check_status(resp).await
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
        let resp = self
            .http
            .put(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        self.check_status(resp).await
    }

    // -- Playlists --

    pub async fn my_playlists(&self, limit: u32, offset: u32) -> Result<Page<Playlist>> {
        let url = format!("{BASE}/me/playlists?limit={limit}&offset={offset}");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_error(&resp)?;
        Ok(resp.json().await?)
    }

    pub async fn playlist_tracks(
        &self,
        playlist_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<PlaylistItem>> {
        let url =
            format!("{BASE}/playlists/{playlist_id}/tracks?limit={limit}&offset={offset}&fields=items(track(name,uri,duration_ms,artists(name),album(name))),total,offset,limit");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_error(&resp)?;
        Ok(resp.json().await?)
    }

    // -- Recently played --

    pub async fn recently_played(&self, limit: u32) -> Result<CursorPage<PlayHistory>> {
        let url = format!("{BASE}/me/player/recently-played?limit={limit}");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_error(&resp)?;
        Ok(resp.json().await?)
    }

    // -- Library --

    pub async fn liked_tracks(&self, limit: u32, offset: u32) -> Result<Page<SavedTrack>> {
        let url = format!("{BASE}/me/tracks?limit={limit}&offset={offset}");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_error(&resp)?;
        Ok(resp.json().await?)
    }

    // -- Internals --

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        Ok(self
            .http
            .get(format!("{BASE}{path}"))
            .bearer_auth(&self.token)
            .send()
            .await?)
    }

    fn check_error(&self, resp: &reqwest::Response) -> Result<()> {
        let status = resp.status().as_u16();
        if status == 429 {
            return Err(SpotError::RateLimited {
                retry_after_secs: 1,
            });
        }
        Ok(())
    }

    async fn check_status(&self, resp: reqwest::Response) -> Result<()> {
        let status = resp.status().as_u16();
        if status == 429 {
            return Err(SpotError::RateLimited {
                retry_after_secs: 1,
            });
        }
        if !resp.status().is_success() && status != 204 {
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotError::Api {
                status,
                message: body,
            });
        }
        Ok(())
    }
}
