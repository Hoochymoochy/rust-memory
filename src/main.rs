use axum::{
    routing::{get, post},
    Router,
    extract::Json
};
use serde_derive::Deserialize;
mod llm;


#[derive(Deserialize)]
struct query {
    message: String
}

#[tokio::main]
async fn main(){
    dotenvy::dotenv().ok();
    let app = Router::new()
        .route("/", get(plain))
        .route("/memory", post(memory_extractor));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn plain() -> &'static str {
    "Hello, World!"
}

async fn memory_extractor(Json(payload): Json<query>){
    if let Err(e) = llm::extract_facts(&payload.message) {
        eprintln!("Error: {}", e);
    }
}

async fn event_store() {

    println!("The current time is {:?}.", std::time::SystemTime::now());
}

async fn state_store() -> &'static str {
    "Hello, World!"
}

async fn read_retrieval() -> &'static str {
    "Hello, World!"
}
