use redis::{AsyncCommands, Client};
use tiktoken_rs::cl100k_base;
use crate::types::ChatMessage;

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
        role: &str,
        session_id: &str,
        message: &str,
    ) -> redis::RedisResult<()> {
        let mut con = self.client.get_multiplexed_async_connection().await?;
    
        let tokens = count_tokens(message);
    
        let msg = ChatMessage {
            role: role.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
    
        let serialized = serde_json::to_string(&msg)
            .map_err(|e| redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "JSON serialization failed",
                e.to_string(),
            )))?;
    
        // store message
        con.rpush(session_id, serialized).await?;
    
        // track token count
        let token_key = format!("{}:tokens", session_id);
        con.incr::<&str, i64, i64>(&token_key, tokens as i64).await?;
    
        Ok(())
    }

    pub async fn get_all_messages(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Vec<ChatMessage>> {
        let mut con = self.client.get_multiplexed_async_connection().await?;
    
        let messages: Vec<String> = con.lrange(session_id, 0, -1).await?;
    
        let parsed: Vec<ChatMessage> = messages
            .into_iter()
            .filter_map(|msg| serde_json::from_str(&msg).ok())
            .collect();
    
        Ok(parsed)
    }
}
fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().unwrap();
    let tokens = bpe.encode_with_special_tokens(text);
    tokens.len()
}