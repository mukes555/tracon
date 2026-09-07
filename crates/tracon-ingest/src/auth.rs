//! Shared-secret protection for the localhost ingest port.
//!
//! Any process on the machine, including a web page via a cross-origin POST,
//! can reach 127.0.0.1:48620. The token file is readable only by the user, so
//! a hook that presents it proves it runs as the user; the Host check refuses
//! DNS-rebinding style requests where a browser was pointed at a hostname
//! that happens to resolve to loopback.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub const TOKEN_HEADER: &str = "x-tracon-token";
const TOKEN_BYTES: usize = 32;

/// `~/.tracon`: home of the token and the plugin's spool file.
pub fn tracon_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".tracon"))
}

/// Read `dir/token`, creating a fresh random one (user-only permissions on
/// unix) when it is missing or unreadable as a token.
pub fn ensure_token(dir: &Path) -> anyhow::Result<String> {
    let path = dir.join("token");
    if let Some(existing) = read_existing_token(&path) {
        return Ok(existing);
    }

    std::fs::create_dir_all(dir)?;
    let token = random_hex_token()?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(token.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(token)
}

fn read_existing_token(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let token = content.trim();
    let is_hex = token.chars().all(|c| c.is_ascii_hexdigit());
    let well_formed = is_hex && token.len() == TOKEN_BYTES * 2;
    well_formed.then(|| token.to_string())
}

fn random_hex_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("no entropy source: {err}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub struct AuthConfig {
    pub token: String,
    pub port: u16,
}

/// Gate for the POST ingest routes. GETs stay open: they only ever return a
/// fixed informational string.
pub async fn require_local_token(
    State(auth): State<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() != Method::POST {
        return next.run(request).await;
    }
    let headers = request.headers();
    let host_ok = host_is_local(headers, auth.port);
    let token_ok = token_matches(headers, &auth.token);
    if !host_ok || !token_ok {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

fn host_is_local(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let loopback_ip = format!("127.0.0.1:{port}");
    let loopback_name = format!("localhost:{port}");
    host == loopback_ip || host.eq_ignore_ascii_case(&loopback_name)
}

fn token_matches(headers: &HeaderMap, expected: &str) -> bool {
    let Some(presented) = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    constant_time_eq(presented.trim().as_bytes(), expected.as_bytes())
}

/// Compare without short-circuiting on the first differing byte, so response
/// timing does not leak how much of a guessed token was right.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_token_creates_then_reuses_a_hex_token() {
        let dir = crate::test_support::temp_dir("auth");
        let first = ensure_token(&dir).unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));

        let second = ensure_token(&dir).unwrap();
        assert_eq!(first, second);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("token"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_token_file_is_regenerated() {
        let dir = crate::test_support::temp_dir("auth-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "not a token\n").unwrap();
        let token = ensure_token(&dir).unwrap();
        assert_eq!(token.len(), 64);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn constant_time_eq_compares_whole_slices() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
