use redis::{AsyncCommands, Client};
use tiktoken_rs::cl100k_base;

pub struct RedisClient {
    client: Client,
}

impl RedisClient {
    pub async fn new() -> redis::RedisResult<Self> {
        let client = Client::open("redis://127.0.0.1/")?;
        
        let mut con = client.get_multiplexed_async_connection().await?;
        redis::cmd("PING").query_async::<_, ()>(&mut con).await?;
        
        tracing::info!("Connected to Redis");
        Ok(RedisClient { client })
    }
    
    #[allow(dependency_on_unit_never_type_fallback)]
    pub async fn add_message(
        &self,
        session_id: &str,
        message: &str,
    ) -> redis::RedisResult<()> {
        let mut con = self.client.get_multiplexed_async_connection().await?;
    
        let tokens = count_tokens(message);
    
        // store message
        con.rpush(&session_id, message).await?;
    
        // track token count
        let token_key = format!("{}:tokens", session_id);
        con.incr::<&str, i64, i64>(&token_key, tokens as i64).await?;
    
        Ok(())
    }

    pub async fn get_all_messages(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Vec<String>> {
        let mut con = self.client.get_multiplexed_async_connection().await?;
        
        con.lrange(session_id, 0, -1).await
    }
}
fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().unwrap();
    let tokens = bpe.encode_with_special_tokens(text);
    tokens.len()
}