use axum::{
    routing::{get, post},
    Router,
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use tracing_subscriber;

mod llm;
mod neo4j;
mod types;
mod redis;
mod runners;
use uuid::Uuid;

use crate::types::{Query, UserResponse};
use neo4j::Neo4jClient;

use std::sync::Arc;
use axum::extract::State;

struct AppState {
    redis: Arc<redis::RedisClient>,
}


#[tokio::main]
async fn main() {
    let redis_client = redis::RedisClient::new()
        .await
        .expect("Failed to connect to Redis");

    let neo4j_client = neo4j::Neo4jClient::new()
        .await
        .expect("Failed to connect to Neo4j");

    let state: Arc<AppState> = Arc::new(AppState {
        redis: Arc::new(redis_client),
    });

    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = Router::new()
        .route("/health", get(health_check))
        // user endpoints
        .route("/user", get(user_endpoint))
        .route("/user", post(user_endpoint))
        // memory endpoints
        .route("/memory", post(agent_endpoint))
        .route("/memory", get(agent_endpoint))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind to port 3000");
    
    tracing::info!("Memory server listening on 127.0.0.1:3000");
    
    axum::serve(listener, app)
        .await
        .expect("Server error");

    let runner = Arc::new(runners::new(redis_client, neo4j_client.clone()));


    // ===== BACKGROUND TASK 1: Archive Monitor =====
    tokio::spawn({
        let runner = runner.clone();
        async move {
            if let Err(e) = runner.run_archive_monitor().await {
                tracing::error!("Archive monitor crashed: {}", e);
            }
        }
    });

    // ===== BACKGROUND TASK 2: Buffer Migration =====
    tokio::spawn({
        let runner = runner.clone();
        async move {
            if let Err(e) = runner.run_buffer_migration().await {
                tracing::error!("Buffer migration crashed: {}", e);
            }
        }
    });
}

async fn health_check() -> &'static str {
    "OK"
}

async fn user_endpoint(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Query>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    if payload.id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "please provide a user id".to_string(),
        ));
    }

    let session_id = payload
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    state
        .redis
        .add_message(&payload.role, &session_id, &payload.message)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let messages = state
        .redis
        .get_all_messages(&session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;


    Ok(Json(UserResponse {
        session_id,
        messages,
    }))
}

// Stub implementations for your other handlers
async fn memory_extractor(
    State(_state): State<Arc<AppState>>,
) -> Result<String, (StatusCode, String)> {
    Ok("GET /memory endpoint".to_string())
}

async fn agent_endpoint(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<Query>,
) -> Result<String, (StatusCode, String)> {
    Ok("Agent response".to_string())
}