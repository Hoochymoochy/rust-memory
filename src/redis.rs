use redis::{AsyncCommands, Client};

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

    pub async fn add_message(
        &self,
        session_id: String,
        message: &str,
    ) -> redis::RedisResult<()> {
        let mut con = self.client.get_multiplexed_async_connection().await?;
        
        con.rpush(session_id, message).await
    }

    pub async fn get_all_messages(
        &self,
        session_id: String,
    ) -> redis::RedisResult<Vec<String>> {
        let mut con = self.client.get_multiplexed_async_connection().await?;
        
        con.lrange(session_id, 0, -1).await
    }
}