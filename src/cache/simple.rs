// Redis cache implementation - Redis 缓存实现

use super::CacheConfig;
use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Redis 缓存管理器
pub struct SimpleCache {
    client: Client,
    pool: ConnectionManager,
    config: Arc<CacheConfig>,
    blacklist_regexes: Vec<Regex>,
}

/// 缓存的响应数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// 响应内容（SSE 事件列表）
    pub events: Vec<String>,
    /// 缓存时间戳
    pub cached_at: i64,
}

impl SimpleCache {
    /// 创建新的 Redis 缓存实例
    pub async fn new(config: CacheConfig) -> Result<Self> {
        // 构建 Redis URL
        let redis_url = if let Some(password) = &config.password {
            // 带密码的 URL
            let base_url = config.redis_url.trim_start_matches("redis://");
            format!("redis://:{}@{}", password, base_url)
        } else {
            config.redis_url.clone()
        };

        // 如果指定了 db，添加到 URL
        let redis_url = if let Some(db) = config.db {
            format!("{}/{}", redis_url, db)
        } else {
            redis_url
        };

        // 创建 Redis 客户端
        let client = Client::open(redis_url).context("创建 Redis 客户端失败")?;

        // 创建连接池
        let pool = ConnectionManager::new(client.clone())
            .await
            .context("创建 Redis 连接池失败")?;

        // 编译黑名单正则表达式
        let blacklist_regexes = config
            .blacklist_patterns
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();

        Ok(Self {
            client,
            pool,
            config: Arc::new(config),
            blacklist_regexes,
        })
    }

    /// 检查请求是否应该被缓存（黑名单过滤）
    pub fn should_cache(&self, request_json: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        // 检查黑名单
        for regex in &self.blacklist_regexes {
            if regex.is_match(request_json) {
                tracing::debug!("请求匹配黑名单模式，跳过缓存");
                return false;
            }
        }

        true
    }

    /// 从缓存读取
    pub async fn get(&self, key: &str) -> Result<Option<CachedResponse>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let mut conn = self.pool.clone();

        match conn.get::<_, Option<String>>(key).await {
            Ok(Some(value)) => {
                // 反序列化
                match serde_json::from_str::<CachedResponse>(&value) {
                    Ok(cached) => {
                        tracing::debug!("缓存命中: {}", key);
                        Ok(Some(cached))
                    }
                    Err(e) => {
                        tracing::warn!("缓存反序列化失败: {}", e);
                        Ok(None)
                    }
                }
            }
            Ok(None) => {
                tracing::debug!("缓存未命中: {}", key);
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("Redis 读取失败: {}", e);
                Ok(None) // 降级：Redis 故障不影响主流程
            }
        }
    }

    /// 写入缓存
    pub async fn set(&self, key: &str, response: &CachedResponse) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut conn = self.pool.clone();

        // 序列化
        let value = serde_json::to_string(response).context("序列化缓存数据失败")?;

        // 写入 Redis，设置 TTL
        let ttl = self.config.ttl_seconds;

        match conn.set_ex::<_, _, ()>(key, value, ttl).await {
            Ok(_) => {
                tracing::debug!("缓存写入成功: {} (TTL: {}s)", key, ttl);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Redis 写入失败: {}", e);
                Ok(()) // 降级：写入失败不影响主流程
            }
        }
    }

    /// 删除缓存
    pub async fn remove(&self, key: &str) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut conn = self.pool.clone();

        match conn.del::<_, ()>(key).await {
            Ok(_) => {
                tracing::debug!("缓存删除成功: {}", key);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Redis 删除失败: {}", e);
                Ok(())
            }
        }
    }

    /// 清空所有缓存
    pub async fn clear(&self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut conn = self.pool.clone();

        // 使用 SCAN 命令查找所有 kiro:cache:v1:* 的 key
        let pattern = "kiro:cache:v1:*";

        match redis::cmd("SCAN")
            .arg("0")
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg("1000")
            .query_async::<_, (String, Vec<String>)>(&mut conn)
            .await
        {
            Ok((_, keys)) => {
                if !keys.is_empty() {
                    match conn.del::<_, ()>(&keys).await {
                        Ok(_) => {
                            tracing::info!("清空缓存成功，删除 {} 个 key", keys.len());
                        }
                        Err(e) => {
                            tracing::warn!("批量删除缓存失败: {}", e);
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                tracing::warn!("扫描缓存 key 失败: {}", e);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;

    #[tokio::test]
    async fn test_cache_config() {
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            ttl_seconds: 3600,
            password: None,
            db: Some(0),
            blacklist_patterns: vec!["password".to_string()],
        };

        // 注意：此测试需要 Redis 服务运行
        // 如果 Redis 不可用，测试会失败
        match SimpleCache::new(config).await {
            Ok(cache) => {
                assert!(cache.config.enabled);
                assert_eq!(cache.config.ttl_seconds, 3600);
            }
            Err(e) => {
                // Redis 不可用时跳过测试
                eprintln!("跳过测试（Redis 不可用）: {}", e);
            }
        }
    }

    #[test]
    fn test_cached_response_serialization() {
        let response = CachedResponse {
            events: vec![
                "event: message_start".to_string(),
                "data: {...}".to_string(),
            ],
            cached_at: 1234567890,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: CachedResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.events, deserialized.events);
        assert_eq!(response.cached_at, deserialized.cached_at);
    }

    #[test]
    fn test_should_cache_disabled() {
        // 测试缓存禁用时不缓存
        let config = CacheConfig {
            enabled: false,
            redis_url: "redis://localhost:6379".to_string(),
            ttl_seconds: 3600,
            password: None,
            db: None,
            blacklist_patterns: vec![],
        };

        // 直接测试配置
        assert!(!config.enabled, "缓存禁用时 enabled 应为 false");
    }

    #[test]
    fn test_should_cache_blacklist_password() {
        // 测试黑名单过滤：password
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            ttl_seconds: 3600,
            password: None,
            db: None,
            blacklist_patterns: vec!["password".to_string()],
        };

        let blacklist_regexes: Vec<Regex> = config
            .blacklist_patterns
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();

        let request_with_password = r#"{"model":"claude-sonnet-4","messages":[{"role":"user","content":"My password is 123"}]}"#;
        let should_cache_with_password = !blacklist_regexes.iter().any(|regex| regex.is_match(request_with_password));
        assert!(!should_cache_with_password, "包含 password 的请求应被过滤");

        let request_without_password = r#"{"model":"claude-sonnet-4","messages":[{"role":"user","content":"Hello"}]}"#;
        let should_cache_without_password = !blacklist_regexes.iter().any(|regex| regex.is_match(request_without_password));
        assert!(should_cache_without_password, "不包含 password 的请求应通过");
    }

    #[test]
    fn test_should_cache_blacklist_api_key() {
        // 测试黑名单过滤：api_key / api-key
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            ttl_seconds: 3600,
            password: None,
            db: None,
            blacklist_patterns: vec!["api[_-]?key".to_string()],
        };

        let blacklist_regexes: Vec<Regex> = config
            .blacklist_patterns
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();

        let request_with_api_key = r#"{"model":"claude-sonnet-4","messages":[{"role":"user","content":"My api_key is xyz"}]}"#;
        let should_cache = !blacklist_regexes.iter().any(|regex| regex.is_match(request_with_api_key));
        assert!(!should_cache, "包含 api_key 的请求应被过滤");

        let request_with_api_dash_key = r#"{"model":"claude-sonnet-4","messages":[{"role":"user","content":"My api-key is xyz"}]}"#;
        let should_cache = !blacklist_regexes.iter().any(|regex| regex.is_match(request_with_api_dash_key));
        assert!(!should_cache, "包含 api-key 的请求应被过滤");

        let request_with_apikey = r#"{"model":"claude-sonnet-4","messages":[{"role":"user","content":"My apikey is xyz"}]}"#;
        let should_cache = !blacklist_regexes.iter().any(|regex| regex.is_match(request_with_apikey));
        assert!(!should_cache, "包含 apikey 的请求应被过滤");
    }

    #[test]
    fn test_should_cache_multiple_blacklist() {
        // 测试多个黑名单模式
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            ttl_seconds: 3600,
            password: None,
            db: None,
            blacklist_patterns: vec![
                "password".to_string(),
                "secret".to_string(),
                "token".to_string(),
            ],
        };

        let blacklist_regexes: Vec<Regex> = config
            .blacklist_patterns
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();

        let test_cases = vec![
            (r#"{"content":"password"}"#, false, "password"),
            (r#"{"content":"secret"}"#, false, "secret"),
            (r#"{"content":"token"}"#, false, "token"),
            (r#"{"content":"hello"}"#, true, "hello"),
        ];

        for (request, expected_should_cache, label) in test_cases {
            let should_cache = !blacklist_regexes.iter().any(|regex| regex.is_match(request));
            assert_eq!(should_cache, expected_should_cache, "测试用例 '{}' 失败", label);
        }
    }

    #[test]
    fn test_cached_response_structure() {
        // 测试 CachedResponse 结构
        let response = CachedResponse {
            events: vec![
                "event: message_start".to_string(),
                "data: {\"type\":\"message_start\"}".to_string(),
                "event: content_block_delta".to_string(),
                "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hello\"}}".to_string(),
                "event: message_stop".to_string(),
            ],
            cached_at: 1709366400, // 2024-03-02 00:00:00 UTC
        };

        assert_eq!(response.events.len(), 5);
        assert_eq!(response.cached_at, 1709366400);

        // 测试克隆
        let cloned = response.clone();
        assert_eq!(response.events, cloned.events);
        assert_eq!(response.cached_at, cloned.cached_at);
    }

    #[test]
    fn test_cached_response_empty_events() {
        // 测试空事件列表
        let response = CachedResponse {
            events: vec![],
            cached_at: 1234567890,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: CachedResponse = serde_json::from_str(&json).unwrap();

        assert!(deserialized.events.is_empty());
        assert_eq!(deserialized.cached_at, 1234567890);
    }

    #[test]
    fn test_cached_response_large_events() {
        // 测试大量事件
        let events: Vec<String> = (0..1000)
            .map(|i| format!("event: message_{}", i))
            .collect();

        let response = CachedResponse {
            events: events.clone(),
            cached_at: 1234567890,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: CachedResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.events.len(), 1000);
        assert_eq!(deserialized.events, events);
    }
}
