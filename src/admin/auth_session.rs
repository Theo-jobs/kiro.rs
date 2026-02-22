//! OIDC 认证会话管理
//!
//! 管理设备授权流程中的临时会话状态

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

/// 认证会话状态
#[derive(Clone)]
pub enum AuthSessionStatus {
    /// 等待用户完成浏览器授权
    Pending,
    /// 授权完成，已获取 token
    Completed {
        refresh_token: String,
        client_id: String,
        client_secret: String,
        region: String,
    },
    /// 授权失败
    Failed(String),
}

impl std::fmt::Debug for AuthSessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Completed { region, .. } => f
                .debug_struct("Completed")
                .field("region", region)
                .field("refresh_token", &"[REDACTED]")
                .field("client_id", &"[REDACTED]")
                .field("client_secret", &"[REDACTED]")
                .finish(),
            Self::Failed(msg) => f.debug_tuple("Failed").field(msg).finish(),
        }
    }
}

/// 单个认证会话
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub id: String,
    pub status: AuthSessionStatus,
    pub created_at: DateTime<Utc>,
    pub region: String,
    pub start_url: String,
    pub machine_id: String,
}

/// 最大并发会话数
const MAX_SESSIONS: usize = 20;

/// 认证会话存储
pub struct AuthSessionStore {
    sessions: Mutex<HashMap<String, AuthSession>>,
}

impl Default for AuthSessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// 插入新会话（超过上限时拒绝）
    pub fn insert(&self, session: AuthSession) -> Result<(), &'static str> {
        let mut sessions = self.sessions.lock();
        if sessions.len() >= MAX_SESSIONS {
            return Err("并发认证会话数已达上限");
        }
        sessions.insert(session.id.clone(), session);
        Ok(())
    }

    /// 获取会话状态
    pub fn get_status(&self, id: &str) -> Option<AuthSessionStatus> {
        let sessions = self.sessions.lock();
        sessions.get(id).map(|s| s.status.clone())
    }

    /// 获取完整会话
    pub fn get(&self, id: &str) -> Option<AuthSession> {
        let sessions = self.sessions.lock();
        sessions.get(id).cloned()
    }

    /// 更新会话状态
    pub fn update_status(&self, id: &str, status: AuthSessionStatus) {
        let mut sessions = self.sessions.lock();
        if let Some(session) = sessions.get_mut(id) {
            session.status = status;
        }
    }

    /// 移除会话
    pub fn remove(&self, id: &str) -> Option<AuthSession> {
        let mut sessions = self.sessions.lock();
        sessions.remove(id)
    }

    /// 清理过期会话（超过 10 分钟）
    pub fn cleanup_expired(&self) {
        let mut sessions = self.sessions.lock();
        let now = Utc::now();
        sessions.retain(|_, session| {
            let elapsed = now.signed_duration_since(session.created_at);
            elapsed.num_minutes() < 10
        });
    }
}

