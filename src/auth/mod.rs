mod pkce;
mod token;

pub use token::{TokenData, TokenStore};

use crate::config::Config;
use crate::error::{Result, SpotError};

const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

const SCOPES: &str = "streaming \
                       user-read-playback-state \
                       user-modify-playback-state \
                       user-read-currently-playing \
                       user-read-email \
                       user-read-private \
                       playlist-read-private \
                       playlist-read-collaborative \
                       user-library-read";

/// Runs the full PKCE auth flow: opens browser, listens for callback, exchanges code.
pub async fn authenticate(config: &Config) -> Result<TokenData> {
    let (verifier, challenge) = pkce::generate();

    let auth_url = format!(
        "{AUTH_URL}?client_id={}&response_type=code\
         &redirect_uri={}&scope={}&code_challenge_method=S256\
         &code_challenge={challenge}",
        config.client_id,
        urlencoding(&config.redirect_uri),
        urlencoding(SCOPES),
    );

    tracing::info!("opening browser for auth");
    if open::that(&auth_url).is_err() {
        // Terminals without xdg-open, etc. Just print the URL.
        eprintln!("\nOpen this URL in your browser:\n\n  {auth_url}\n");
    }

    let code = listen_for_callback(&config.redirect_uri).await?;
    exchange_code(config, &code, &verifier).await
}

/// Refresh an existing token using the refresh_token grant.
pub async fn refresh(config: &Config, refresh_token: &str) -> Result<TokenData> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &config.client_id),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(SpotError::TokenRefreshFailed(body));
    }

    let mut data: TokenData = resp.json().await?;
    // Spotify might not return a new refresh token — keep the old one.
    if data.refresh_token.is_none() {
        data.refresh_token = Some(refresh_token.to_string());
    }
    data.stamp_expiry();
    Ok(data)
}

async fn exchange_code(config: &Config, code: &str, verifier: &str) -> Result<TokenData> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &config.redirect_uri),
            ("client_id", &config.client_id),
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
async fn listen_for_callback(redirect_uri: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let parsed = url::Url::parse(redirect_uri)?;
    let host = parsed.host_str().unwrap_or("127.0.0.1");
    let port = parsed.port().unwrap_or(8888);
    let bind = format!("{host}:{port}");

    let listener = TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "waiting for OAuth callback");

    let (mut stream, _) = listener.accept().await?;
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the code out of "GET /callback?code=XXXX..."
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
