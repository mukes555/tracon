use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use tower::ServiceExt;
use tracon_core::store::Store;

use crate::auth::TOKEN_HEADER;
use crate::{router, DEFAULT_PORT};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app(store: &Arc<Store>) -> Router {
    router(store.clone(), DEFAULT_PORT, TOKEN.to_string())
}

fn claude_body() -> String {
    serde_json::json!({
        "session_id": "s1",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_use_id": "t1",
        "tool_input": {"command": "cargo build"}
    })
    .to_string()
}

fn post(path: &str, host: &str, token: Option<&str>, body: String) -> Request<Body> {
    let mut request = Request::post(path)
        .header("content-type", "application/json")
        .header("host", host);
    if let Some(token) = token {
        request = request.header(TOKEN_HEADER, token);
    }
    request.body(Body::from(body)).unwrap()
}

#[tokio::test]
async fn ingest_stores_hook_event_and_returns_200() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let request = post("/ingest", "127.0.0.1:48620", Some(TOKEN), claude_body());

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(store.stats().unwrap().event_count, 1);
}

#[tokio::test]
async fn localhost_host_header_is_accepted() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let request = post("/ingest", "localhost:48620", Some(TOKEN), claude_body());

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(store.stats().unwrap().event_count, 1);
}

#[tokio::test]
async fn missing_token_is_forbidden() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let request = post("/ingest", "127.0.0.1:48620", None, claude_body());

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(store.stats().unwrap().event_count, 0);
}

#[tokio::test]
async fn wrong_token_is_forbidden() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let wrong = "f".repeat(64);
    let request = post(
        "/ingest/gemini",
        "127.0.0.1:48620",
        Some(&wrong),
        claude_body(),
    );

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(store.stats().unwrap().event_count, 0);
}

#[tokio::test]
async fn foreign_host_header_is_forbidden() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let request = post("/ingest", "evil.example:48620", Some(TOKEN), claude_body());

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(store.stats().unwrap().event_count, 0);
}

#[tokio::test]
async fn get_routes_stay_open_without_token() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let request = Request::get("/health").body(Body::empty()).unwrap();

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn ingest_routes_cursor_payloads_to_the_cursor_adapter() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let body = serde_json::json!({
        "conversation_id": "conv-9",
        "generation_id": "g1",
        "hook_event_name": "beforeShellExecution",
        "workspace_roots": ["/tmp/demo"],
        "command": "pnpm add zod"
    })
    .to_string();
    let request = post("/ingest", "127.0.0.1:48620", Some(TOKEN), body);

    let response = app(&store).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let events = store.events_for_session("conv-9", 10).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].agent, "cursor");
    assert_eq!(store.stats().unwrap().package_count, 1);
}
