mod pkce;
mod token;

pub use token::{TokenData, TokenStore};

use crate::error::{Result, SpotError};

const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// ncspot's shared client_id — registered for Web API with the right redirect URI.
/// Using a well-known community client_id avoids requiring users to register their
/// own Spotify developer app.
pub const NCSPOT_CLIENT_ID: &str = "d420a117a32841c2b3474932e49fb54b";

/// spotify-player's client_id — known to work with librespot for native streaming.
/// Used exclusively for Spirc/Connect credential acquisition.
pub const STREAMING_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

/// Redirect URI registered for the ncspot client_id.
const REDIRECT_URI: &str = "http://127.0.0.1:8989/login";

const SCOPES: &str = "streaming \
                       user-read-playback-state \
                       user-modify-playback-state \
                       user-read-currently-playing \
                       user-read-recently-played \
                       playlist-read-private \
                       playlist-read-collaborative \
                       user-library-read \
                       user-read-private";

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

/// Runs the full PKCE auth flow: binds the callback listener, opens the browser,
/// then waits for the redirect. Binding first avoids a race where the browser
/// tab loads faster than our listener — and running `open::that` on a blocking
/// thread prevents it from stalling the async runtime when `xdg-open` is slow.
pub async fn authenticate() -> Result<TokenData> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let (verifier, challenge) = pkce::generate();

    let auth_url = format!(
        "{AUTH_URL}?client_id={NCSPOT_CLIENT_ID}&response_type=code\
         &redirect_uri={}&scope={}&code_challenge_method=S256\
         &code_challenge={challenge}",
        urlencoding(REDIRECT_URI),
        urlencoding(SCOPES),
    );

    let parsed = url::Url::parse(REDIRECT_URI)?;
    let host = parsed.host_str().unwrap_or("127.0.0.1");
    let port = parsed.port().unwrap_or(8989);
    let bind = format!("{host}:{port}");

    let listener = TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "waiting for OAuth callback");

    // Spawn the browser on a blocking thread — `open::that` calls xdg-open
    // which can stall on NixOS with Flatpak browsers, and we don't want to
    // block the async runtime.
    let auth_url_for_browser = auth_url.clone();
    tokio::task::spawn_blocking(move || {
        tracing::info!("opening browser for auth");
        if open::that(&auth_url_for_browser).is_err() {
            eprintln!("\nOpen this URL in your browser:\n\n  {auth_url_for_browser}\n");
        }
    });

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
            ("client_id", NCSPOT_CLIENT_ID),
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
            ("client_id", NCSPOT_CLIENT_ID),
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

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
