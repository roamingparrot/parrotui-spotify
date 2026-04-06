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
        let url = format!("{BASE}/me/player");
        tracing::debug!(%url, "GET playback state");
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;
        if resp.status().as_u16() == 204 {
            return Ok(None);
        }
        self.check_status_ref(&resp, &url)?;
        match resp.json().await {
            Ok(state) => Ok(Some(state)),
            Err(e) => {
                tracing::debug!(%e, "could not decode playback state");
                Ok(None)
            }
        }
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
        let resp = self
            .http
            .put(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        self.check_status_owned(resp, &url).await
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
        let resp = self
            .http
            .put(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        self.check_status_owned(resp, &url).await
    }

    // -- Playlists --

    pub async fn my_playlists(&self, limit: u32, offset: u32) -> Result<Page<Playlist>> {
        let url = format!("{BASE}/me/playlists?limit={limit}&offset={offset}");
        tracing::debug!(%url, "GET my playlists");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_status_ref(&resp, &url)?;
        let body = resp.text().await?;
        serde_json::from_str(&body).map_err(|e| {
            tracing::error!(%e, %body, "failed to parse playlists response");
            e.into()
        })
    }

    pub async fn playlist_tracks(
        &self,
        playlist_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<PlaylistItem>> {
        let url = format!("{BASE}/playlists/{playlist_id}/tracks?limit={limit}&offset={offset}");
        tracing::debug!(%url, %playlist_id, "GET playlist tracks");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if status.as_u16() == 429 {
            tracing::warn!(%url, "rate limited");
            return Err(SpotError::RateLimited {
                retry_after_secs: 1,
            });
        }
        if !status.is_success() {
            tracing::warn!(%url, status = status.as_u16(), %body, "playlist tracks request failed");
            return Err(SpotError::Api {
                status: status.as_u16(),
                message: body,
            });
        }
        tracing::debug!(%url, bytes = body.len(), "playlist tracks response OK");
        serde_json::from_str(&body).map_err(|e| {
            tracing::error!(%e, %body, "failed to parse playlist tracks response");
            e.into()
        })
    }

    // -- Recently played --

    pub async fn recently_played(&self, limit: u32) -> Result<CursorPage<PlayHistory>> {
        let url = format!("{BASE}/me/player/recently-played?limit={limit}");
        tracing::debug!(%url, "GET recently played");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_status_ref(&resp, &url)?;
        Ok(resp.json().await?)
    }

    // -- Library --

    pub async fn liked_tracks(&self, limit: u32, offset: u32) -> Result<Page<SavedTrack>> {
        let url = format!("{BASE}/me/tracks?limit={limit}&offset={offset}");
        tracing::debug!(%url, "GET liked tracks");
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_status_ref(&resp, &url)?;
        Ok(resp.json().await?)
    }

    // -- Internals --

    /// Check HTTP status on a borrowed response (body not yet consumed).
    fn check_status_ref(&self, resp: &reqwest::Response, url: &str) -> Result<()> {
        let status = resp.status().as_u16();
        if status == 429 {
            tracing::warn!(%url, "rate limited");
            return Err(SpotError::RateLimited {
                retry_after_secs: 1,
            });
        }
        if !resp.status().is_success() && status != 204 {
            tracing::warn!(%url, %status, "request failed (body not yet read)");
            return Err(SpotError::Api {
                status,
                message: format!("{url} → HTTP {status}"),
            });
        }
        Ok(())
    }

    /// Check HTTP status on an owned response, reading the body on error.
    async fn check_status_owned(&self, resp: reqwest::Response, url: &str) -> Result<()> {
        let status = resp.status().as_u16();
        if status == 429 {
            tracing::warn!(%url, "rate limited");
            return Err(SpotError::RateLimited {
                retry_after_secs: 1,
            });
        }
        if !resp.status().is_success() && status != 204 {
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!(%url, %status, %body, "request failed");
            return Err(SpotError::Api {
                status,
                message: format!("{url} → {body}"),
            });
        }
        Ok(())
    }
}
