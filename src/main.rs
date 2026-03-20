use axum::{
    routing::{get, post},
    Router,
};




#[tokio::main]
async fn main(){
    let app = Router::new()
        .route("/", get(plain));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn plain() -> &'static str {
    "Hello, World!"
}

