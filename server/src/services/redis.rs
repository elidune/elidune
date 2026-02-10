//! Redis service for managing 2FA codes and temporary data

use redis::{AsyncCommands, Client};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct RedisService {
    client: Client,
}

impl RedisService {
    /// Create a new Redis service
    pub async fn new(url: &str) -> AppResult<Self> {
        let client = Client::open(url)
            .map_err(|e| AppError::Internal(format!("Failed to create Redis client: {}", e)))?;
        
        // Test connection
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to Redis: {}", e)))?;
        
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Redis connection test failed: {}", e)))?;

        Ok(Self { client })
    }

    /// Store a 2FA code for a user with expiration (in seconds)
    pub async fn store_2fa_code(&self, user_id: i32, code: &str, expiration_seconds: u64) -> AppResult<()> {
        let mut conn = self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;
        
        let key = format!("2fa:email:{}", user_id);
        conn.set_ex::<_, _, ()>(&key, code, expiration_seconds)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to store 2FA code in Redis: {}", e)))?;
        
        Ok(())
    }

    /// Verify and consume a 2FA code for a user
    pub async fn verify_2fa_code(&self, user_id: i32, code: &str) -> AppResult<bool> {
        let mut conn = self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;
        
        let key = format!("2fa:email:{}", user_id);
        
        // Get the stored code
        let stored_code: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get 2FA code from Redis: {}", e)))?;
        
        match stored_code {
            Some(stored) if stored == code => {
                // Code matches, delete it (one-time use)
                let _: () = conn
                    .del(&key)
                    .await
                    .map_err(|e| AppError::Internal(format!("Failed to delete 2FA code from Redis: {}", e)))?;
                Ok(true)
            }
            Some(_) => Ok(false), // Code doesn't match
            None => Ok(false), // Code not found or expired
        }
    }

    /// Check if a 2FA code exists for a user (without consuming it)
    pub async fn has_2fa_code(&self, user_id: i32) -> AppResult<bool> {
        let mut conn = self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;
        
        let key = format!("2fa:email:{}", user_id);
        let exists: bool = conn
            .exists(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to check 2FA code in Redis: {}", e)))?;
        
        Ok(exists)
    }

    /// Store a trusted device for a user (90 days expiration)
    pub async fn store_trusted_device(&self, user_id: i32, device_id: &str) -> AppResult<()> {
        let mut conn = self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;
        
        // 90 days in seconds
        let expiration_seconds = 90 * 24 * 3600;
        let key = format!("trust_device:{}:{}", user_id, device_id);
        conn.set_ex::<_, _, ()>(&key, "1", expiration_seconds)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to store trusted device in Redis: {}", e)))?;
        
        Ok(())
    }

    /// Check if a device is trusted for a user
    pub async fn is_device_trusted(&self, user_id: i32, device_id: &str) -> AppResult<bool> {
        let mut conn = self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;
        
        let key = format!("trust_device:{}:{}", user_id, device_id);
        let exists: bool = conn
            .exists(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to check trusted device in Redis: {}", e)))?;
        
        Ok(exists)
    }

    /// Get a Redis connection (for advanced operations)
    pub async fn get_connection(&self) -> AppResult<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))
    }
}
