pub mod apps;
pub mod auth;
pub mod flags;
pub mod health;
pub mod intel;
mod intel_names;
pub mod retention;
pub mod sink;
pub mod spool;
mod tail_read;
pub mod tailer;
pub mod thread;

#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod test_support;

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{middleware, Json, Router};
use serde_json::Value;
use tracon_core::store::Store;

pub use auth::{ensure_token, tracon_dir};
pub use health::{health, HealthSnapshot};

/// Fixed default port the Claude Code plugin points its HTTP hooks at.
pub const DEFAULT_PORT: u16 = 48620;

/// Serve the ingest API on localhost until the process exits. `token` is the
/// shared secret hooks must present (see `ensure_token`).
pub async fn serve(store: Arc<Store>, port: u16, token: String) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    axum::serve(listener, router(store, port, token)).await?;
    Ok(())
}

const INGEST_INFO: &str = "Tracon ingest is running. Agents POST hook events here; \
there is nothing to see over GET. Open the Tracon app for the timeline.";

fn router(store: Arc<Store>, port: u16, token: String) -> Router {
    let auth = Arc::new(auth::AuthConfig { token, port });
    Router::new()
        .route("/", get(|| async { INGEST_INFO }))
        .route("/health", get(|| async { "ok" }))
        .route("/ingest", post(ingest).get(|| async { INGEST_INFO }))
        // Gemini's payload shape overlaps Claude's, so it gets its own route
        // instead of shape detection.
        .route(
            "/ingest/gemini",
            post(ingest_gemini).get(|| async { INGEST_INFO }),
        )
        .layer(middleware::from_fn_with_state(
            auth,
            auth::require_local_token,
        ))
        .with_state(store)
}

/// Always answers 200 with an empty body: for Claude Code hooks that means
/// "success, no decision". A recorder must never block or slow the agent,
/// so malformed payloads and store errors are swallowed here on purpose
/// (they still show up in `health()`).
async fn ingest(State(store): State<Arc<Store>>, Json(payload): Json<Value>) -> &'static str {
    if store.capture_paused() {
        return "";
    }
    for event in tracon_adapters::events_from_any_hook_payload(&payload) {
        let _ = sink::insert_event(&store, event);
    }
    ""
}

async fn ingest_gemini(
    State(store): State<Arc<Store>>,
    Json(payload): Json<Value>,
) -> &'static str {
    if store.capture_paused() {
        return "";
    }
    for event in tracon_adapters::gemini::events_from_hook_payload(&payload) {
        let _ = sink::insert_event(&store, event);
    }
    ""
}
