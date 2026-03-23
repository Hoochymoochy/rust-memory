use std::sync::Arc;
use tokio::time::{Duration, interval};

pub struct Runner {
    redis: Arc<redis::RedisClient>,
    neo4j: Arc<neo4j::Neo4jClient>,
}

impl Runner {
    pub fn new(redis: Arc<redis::RedisClient>, neo4j: Arc<neo4j::Neo4jClient>) -> Self {
        Runner { redis, neo4j }
    }

    /// Check a single session and archive if oversized
    pub async fn check_and_archive_session(&self, session_id: &str, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let token_count = self.redis.get_token_count(session_id).await?;

        if token_count > 1000 {
            tracing::info!("Archiving session {} ({} tokens)", session_id, token_count);
            self.redis.archive_session(user_id, session_id).await?;
        }

        Ok(())
    }

    /// Background task: periodically check all active sessions
    pub async fn run_archive_monitor(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = interval(Duration::from_secs(10)); // Check every 10 seconds

        loop {
            interval.tick().await;

            // Get all active sessions
            let sessions = self.redis.get_active_sessions().await?;

            for session_id in sessions {
                // Get the user_id for this session
                if let Ok(user_id) = self.redis.get_session_user(&session_id).await {
                    // Check and archive if needed
                    if let Err(e) = self.check_and_archive_session(&session_id, &user_id).await {
                        tracing::error!("Failed to check session {}: {}", session_id, e);
                    }
                }
            }
        }
    }

    /// Background task: periodically migrate buffers to Neo4j
    pub async fn run_buffer_migration(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = interval(Duration::from_secs(3600)); // Every hour

        loop {
            interval.tick().await;

            tracing::info!("Running buffer migration to Neo4j...");

            // This would get all users and their old buffers
            // For now, just log
            if let Err(e) = self.migrate_old_buffers().await {
                tracing::error!("Buffer migration failed: {}", e);
            }
        }
    }

    /// Migrate old buffers (>30 days) from Redis to Neo4j
    async fn migrate_old_buffers(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement buffer migration logic
        // Get all buffers older than 30 days
        // Convert to nodes
        // Insert into Neo4j
        // Delete from Redis
        Ok(())
    }
}