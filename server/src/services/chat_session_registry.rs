//! In-flight chat generation cancellation tokens.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub struct ChatSessionRegistry {
    inner: Arc<Mutex<HashMap<i64, CancellationToken>>>,
}

impl ChatSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, conversation_id: i64) -> CancellationToken {
        let mut map = self.inner.lock().await;
        if let Some(existing) = map.get(&conversation_id) {
            existing.cancel();
        }
        let token = CancellationToken::new();
        map.insert(conversation_id, token.clone());
        token
    }

    pub async fn cancel(&self, conversation_id: i64) {
        let mut map = self.inner.lock().await;
        if let Some(token) = map.remove(&conversation_id) {
            token.cancel();
        }
    }

    pub async fn clear(&self, conversation_id: i64) {
        let mut map = self.inner.lock().await;
        map.remove(&conversation_id);
    }
}
