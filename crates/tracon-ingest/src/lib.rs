pub mod apps;
pub mod flags;
pub mod intel;
pub mod retention;
pub mod spool;
pub mod tailer;
pub mod thread;

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;
use tracon_core::store::Store;

/// Fixed default port the Claude Code plugin points its HTTP hooks at.
pub const DEFAULT_PORT: u16 = 48620;

/// Serve the ingest API on localhost until the process exits.
pub async fn serve(store: Arc<Store>, port: u16) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    axum::serve(listener, router(store)).await?;
    Ok(())
}

const INGEST_INFO: &str = "Tracon ingest is running. Agents POST hook events here; \
there is nothing to see over GET. Open the Tracon app for the timeline.";

fn router(store: Arc<Store>) -> Router {
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
        .with_state(store)
}

/// Always answers 200 with an empty body: for Claude Code hooks that means
/// "success, no decision". A recorder must never block or slow the agent,
/// so malformed payloads and store errors are swallowed here on purpose.
async fn ingest(State(store): State<Arc<Store>>, Json(payload): Json<Value>) -> &'static str {
    if store.capture_paused() {
        return "";
    }
    for event in tracon_adapters::events_from_any_hook_payload(&payload) {
        let _ = store.insert(&event);
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
        let _ = store.insert(&event);
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn ingest_stores_hook_event_and_returns_200() {
        let store = Arc::new(Store::open_in_memory().unwrap());
        let app = router(store.clone());

        let body = serde_json::json!({
            "session_id": "s1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": "t1",
            "tool_input": {"command": "cargo build"}
        });
        let request = Request::post("/ingest")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(store.stats().unwrap().event_count, 1);
    }

    #[tokio::test]
    async fn ingest_routes_cursor_payloads_to_the_cursor_adapter() {
        let store = Arc::new(Store::open_in_memory().unwrap());
        let app = router(store.clone());

        let body = serde_json::json!({
            "conversation_id": "conv-9",
            "generation_id": "g1",
            "hook_event_name": "beforeShellExecution",
            "workspace_roots": ["/tmp/demo"],
            "command": "pnpm add zod"
        });
        let request = Request::post("/ingest")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let events = store.events_for_session("conv-9", 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].agent, "cursor");
        assert_eq!(store.stats().unwrap().package_count, 1);
    }
}
