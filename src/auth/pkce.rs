use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};

/// Returns (verifier, challenge) for PKCE S256.
pub fn generate() -> (String, String) {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..64).map(|_| rng.r#gen::<u8>()).collect();
    let verifier = URL_SAFE_NO_PAD.encode(&bytes);

    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest);

    (verifier, challenge)
}
