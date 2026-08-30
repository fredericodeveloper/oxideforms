//! Lightweight, stateless admin authentication.
//!
//! Instead of holding server-side sessions, we issue an HMAC-signed token in an
//! `HttpOnly` cookie. The signing key is derived from the admin password, so the
//! token stays valid across restarts but is useless to anyone who doesn't know
//! the password (they cannot forge the signature).

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

fn new_mac(key: &[u8]) -> HmacSha256 {
    // HMAC accepts keys of any length; this only ever fails if a future digest
    // constrained the key size, so `expect` is safe here.
    HmacSha256::new_from_slice(key).expect("HMAC key length")
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Derive a stable 32-byte signing key from the admin password.
pub fn signing_key(admin_password: &str) -> Vec<u8> {
    use sha2::Digest;
    Sha256::digest(admin_password.as_bytes()).to_vec()
}

/// Create an `expiry.hmac` token valid until `expiry` (unix seconds).
pub fn make_token(key: &[u8], expiry: i64) -> String {
    let mut mac = new_mac(key);
    mac.update(expiry.to_string().as_bytes());
    let tag = mac.finalize().into_bytes();
    format!("{}.{}", expiry, hex::encode(tag))
}

/// Verify a token: it must be correctly signed and not expired.
pub fn verify_token(key: &[u8], token: &str) -> bool {
    let Some((exp, tag)) = token.split_once('.') else {
        return false;
    };
    let Ok(expiry) = exp.parse::<i64>() else {
        return false;
    };
    if expiry <= now_secs() {
        return false;
    }
    let mut mac = new_mac(key);
    mac.update(exp.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    constant_time_eq(&expected.into_bytes(), &tag.to_ascii_lowercase().into_bytes())
}

/// Constant-time password check (avoids leaking a length/prefix timing signal).
pub fn check_password(stored: &str, provided: &str) -> bool {
    use sha2::Digest;
    let a = Sha256::digest(stored.as_bytes());
    let b = Sha256::digest(provided.as_bytes());
    constant_time_eq(&a, &b)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
