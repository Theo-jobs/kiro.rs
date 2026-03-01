// Cache configuration - Redis 缓存配置

use serde::{Deserialize, Serialize};

/// Redis 缓存配置结构
///
/// # 示例
/// ```json
/// {
///   "enabled": true,
///   "redisUrl": "redis://localhost:6379",
///   "ttlSeconds": 3600,
///   "password": null,
///   "db": 0,
///   "blacklistPatterns": ["password", "api[_-]?key", "secret"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfig {
    /// 是否启用缓存
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Redis 连接 URL
    /// 格式：redis://[username:password@]host:port[/db]
    #[serde(default = "default_redis_url")]
    pub redis_url: String,

    /// 缓存过期时间（秒）
    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: u64,

    /// Redis 密码（可选）
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Redis 数据库编号（可选，默认 0）
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db: Option<i64>,

    /// 敏感内容黑名单（正则表达式）
    /// 匹配到的请求不会被缓存
    #[serde(default = "default_blacklist_patterns")]
    pub blacklist_patterns: Vec<String>,
}

fn default_enabled() -> bool {
    false
}

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_ttl_seconds() -> u64 {
    3600 // 1 小时
}

fn default_blacklist_patterns() -> Vec<String> {
    vec![
        "password".to_string(),
        "api[_-]?key".to_string(),
        "secret".to_string(),
    ]
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            redis_url: default_redis_url(),
            ttl_seconds: default_ttl_seconds(),
            password: None,
            db: None,
            blacklist_patterns: default_blacklist_patterns(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CacheConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.ttl_seconds, 3600);
        assert_eq!(config.blacklist_patterns.len(), 3);
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://127.0.0.1:6379".to_string(),
            ttl_seconds: 7200,
            password: Some("secret".to_string()),
            db: Some(1),
            blacklist_patterns: vec!["token".to_string()],
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CacheConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enabled, deserialized.enabled);
        assert_eq!(config.redis_url, deserialized.redis_url);
        assert_eq!(config.ttl_seconds, deserialized.ttl_seconds);
        assert_eq!(config.password, deserialized.password);
        assert_eq!(config.db, deserialized.db);
        assert_eq!(config.blacklist_patterns, deserialized.blacklist_patterns);
    }

    #[test]
    fn test_deserialize_from_json_camel_case() {
        // 测试从 camelCase JSON 反序列化
        let json = r#"{
            "enabled": true,
            "redisUrl": "redis://example.com:6379",
            "ttlSeconds": 1800,
            "password": "test123",
            "db": 2,
            "blacklistPatterns": ["secret", "token"]
        }"#;

        let config: CacheConfig = serde_json::from_str(json).unwrap();

        assert!(config.enabled);
        assert_eq!(config.redis_url, "redis://example.com:6379");
        assert_eq!(config.ttl_seconds, 1800);
        assert_eq!(config.password, Some("test123".to_string()));
        assert_eq!(config.db, Some(2));
        assert_eq!(config.blacklist_patterns, vec!["secret", "token"]);
    }

    #[test]
    fn test_deserialize_with_defaults() {
        // 测试部分字段使用默认值
        let json = r#"{
            "enabled": true
        }"#;

        let config: CacheConfig = serde_json::from_str(json).unwrap();

        assert!(config.enabled);
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.ttl_seconds, 3600);
        assert_eq!(config.password, None);
        assert_eq!(config.db, None);
        assert_eq!(config.blacklist_patterns.len(), 3);
    }

    #[test]
    fn test_ttl_boundary_values() {
        // 测试 TTL 边界值
        let config_zero = CacheConfig {
            ttl_seconds: 0,
            ..Default::default()
        };
        assert_eq!(config_zero.ttl_seconds, 0);

        let config_max = CacheConfig {
            ttl_seconds: u64::MAX,
            ..Default::default()
        };
        assert_eq!(config_max.ttl_seconds, u64::MAX);

        // 测试常见值
        let config_day = CacheConfig {
            ttl_seconds: 86400, // 1 天
            ..Default::default()
        };
        assert_eq!(config_day.ttl_seconds, 86400);
    }

    #[test]
    fn test_blacklist_patterns_validation() {
        // 测试黑名单模式
        let config = CacheConfig {
            enabled: true,
            blacklist_patterns: vec![
                "password".to_string(),
                "api[_-]?key".to_string(),
                "secret".to_string(),
                "token".to_string(),
            ],
            ..Default::default()
        };

        assert_eq!(config.blacklist_patterns.len(), 4);
        assert!(config.blacklist_patterns.contains(&"password".to_string()));
        assert!(config.blacklist_patterns.contains(&"api[_-]?key".to_string()));
    }

    #[test]
    fn test_empty_blacklist() {
        // 测试空黑名单
        let config = CacheConfig {
            enabled: true,
            blacklist_patterns: vec![],
            ..Default::default()
        };

        assert!(config.blacklist_patterns.is_empty());
    }

    #[test]
    fn test_serialize_omits_none_fields() {
        // 测试序列化时省略 None 字段
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            ttl_seconds: 3600,
            password: None,
            db: None,
            blacklist_patterns: vec![],
        };

        let json = serde_json::to_string(&config).unwrap();

        // password 和 db 为 None 时不应出现在 JSON 中
        assert!(!json.contains("password"));
        assert!(!json.contains("\"db\""));
    }
}
