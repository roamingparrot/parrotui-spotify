mod pkce;
mod token;

pub use token::{TokenData, TokenStore};

use crate::error::{Result, SpotError};

const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Spotify's internal client_id (same as the official desktop client / librespot).
/// Using this instead of a developer app client_id gives unrestricted Web API access
/// without requiring the user to register an app or get Extended Quota Mode approval.
pub const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

const REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";

const SCOPES: &str = "streaming \
                       user-read-playback-state \
                       user-modify-playback-state \
                       user-read-currently-playing \
                       user-read-recently-played \
                       playlist-read-private \
                       playlist-read-collaborative \
                       user-library-read";

/// Comma-separated scopes for keymaster token requests (Web API).
pub const WEB_API_SCOPES: &str = "streaming,\
                                   user-read-playback-state,\
                                   user-modify-playback-state,\
                                   user-read-currently-playing,\
                                   user-read-recently-played,\
                                   playlist-read-private,\
                                   playlist-read-collaborative,\
                                   user-library-read";

/// Check whether a token covers all the scopes this build requires.
/// Refreshing can't add new scopes, so a mismatch means we need fresh auth.
pub fn has_required_scopes(token: &TokenData) -> bool {
    let have: std::collections::HashSet<&str> = token
        .scope
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    SCOPES.split_whitespace().all(|s| have.contains(s))
}

/// Runs the full PKCE auth flow: opens browser, listens for callback, exchanges code.
pub async fn authenticate() -> Result<TokenData> {
    let (verifier, challenge) = pkce::generate();

    let auth_url = format!(
        "{AUTH_URL}?client_id={SPOTIFY_CLIENT_ID}&response_type=code\
         &redirect_uri={}&scope={}&code_challenge_method=S256\
         &code_challenge={challenge}",
        urlencoding(REDIRECT_URI),
        urlencoding(SCOPES),
    );

    tracing::info!("opening browser for auth");
    if open::that(&auth_url).is_err() {
        eprintln!("\nOpen this URL in your browser:\n\n  {auth_url}\n");
    }

    let code = listen_for_callback().await?;
    exchange_code(&code, &verifier).await
}

/// Refresh an existing token using the refresh_token grant.
pub async fn refresh(refresh_token: &str) -> Result<TokenData> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", SPOTIFY_CLIENT_ID),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(SpotError::TokenRefreshFailed(body));
    }

    let mut data: TokenData = resp.json().await?;
    if data.refresh_token.is_none() {
        data.refresh_token = Some(refresh_token.to_string());
    }
    data.stamp_expiry();
    Ok(data)
}

async fn exchange_code(code: &str, verifier: &str) -> Result<TokenData> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", SPOTIFY_CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(SpotError::Api {
            status: 0,
            message: format!("token exchange failed: {body}"),
        });
    }

    let mut data: TokenData = resp.json().await?;
    data.stamp_expiry();
    Ok(data)
}

/// Tiny HTTP server that catches the OAuth redirect on localhost.
async fn listen_for_callback() -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let parsed = url::Url::parse(REDIRECT_URI)?;
    let host = parsed.host_str().unwrap_or("127.0.0.1");
    let port = parsed.port().unwrap_or(8888);
    let bind = format!("{host}:{port}");

    let listener = TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "waiting for OAuth callback");

    let (mut stream, _) = listener.accept().await?;
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    let code = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|path| url::Url::parse(&format!("http://localhost{path}")).ok())
        .and_then(|url| {
            url.query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.to_string())
        })
        .ok_or_else(|| SpotError::Api {
            status: 0,
            message: "no code in callback".into(),
        })?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                    <html><body><h3>Got it! You can close this tab.</h3></body></html>";
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;

    Ok(code)
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
