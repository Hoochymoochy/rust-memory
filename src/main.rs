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
use crate::types::{Query, ExtractionResult, ExtractedFact};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/memory", post(memory_extractor))
        .route("/query", post(memory_retrieval));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind to port 3000");
    
    tracing::info!("Memory server listening on 127.0.0.1:3000");
    
    axum::serve(listener, app)
        .await
        .expect("Server error");
}

async fn health_check() -> &'static str {
    "OK"
}

async fn memory_extractor(
    Json(payload): Json<Query>,
) -> Result<Json<ExtractionResult>, ApiError> {
    tracing::info!("Extracting facts from message: {}", &payload.message[..50.min(payload.message.len())]);
    
    let result = llm::extract_facts(&payload.message)
        .map_err(|e| ApiError::LlmError(e))?;
    
    let fact_count = result.facts.len();
    
    neo4j::store_data(result.clone())
        .await
        .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    
    tracing::info!("Successfully stored {} facts", fact_count);
    Ok(Json(result))
}

async fn memory_retrieval(
    Json(payload): Json<Query>,
) -> Result<Json<Vec<ExtractedFact>>, ApiError> {
    tracing::info!("Generating query for message: {}", &payload.message[..50.min(payload.message.len())]);
    
    let query_str = llm::generate_scheme(&payload.message)
        .await
        .map_err(|e| ApiError::LlmError(e))?;
    
    tracing::debug!("Generated query: {}", query_str);
    
    let facts = neo4j::retrieve_facts(&query_str)
        .await
        .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    
    tracing::info!("Retrieved {} facts", facts.len());
    Ok(Json(facts))
}

/// Custom error type for API responses
pub enum ApiError {
    LlmError(String),
    DatabaseError(String),
    ValidationError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::LlmError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("LLM Error: {}", msg),
            ),
            ApiError::DatabaseError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database Error: {}", msg),
            ),
            ApiError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                format!("Validation Error: {}", msg),
            ),
        };

        let body = serde_json::json!({
            "error": error_message,
            "status": status.as_u16(),
        });

        (status, Json(body)).into_response()
    }
}