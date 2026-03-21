use axum::{
    routing::{get, post},
    Router,
    extract::Json
};
mod llm;
mod neo4j;
mod types;
use crate::types::{Query};


#[tokio::main]
async fn main(){
    dotenvy::dotenv().ok();
    let app = Router::new()
        .route("/", get(memory_retrieval))
        .route("/memory", post(memory_extractor));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn memory_retrieval(Json(payload): Json<Query>){
    let result = llm::generate_scheme(&payload.message).await.unwrap();
    let result = neo4j::retrieve_facts(&result).await.unwrap();
    let result_clone = result.clone();
    println!("{:?}", result_clone);
}

async fn memory_extractor(Json(payload): Json<Query>){
    let result = llm::extract_facts(&payload.message).unwrap();
    let result_clone = result.clone();
    neo4j::store_data(result).await;
    println!("{:?}", result_clone);
}
