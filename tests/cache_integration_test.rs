//! 缓存功能集成测试
//!
//! 测试场景：
//! 1. 缓存命中测试
//! 2. 缓存未命中测试
//! 3. 缓存过期测试
//! 4. 黑名单过滤测试
//! 5. Redis 故障降级测试
//! 6. 流式响应模拟测试

use kiro_rs::cache::{CacheConfig, CachedResponse, SimpleCache};

// 注意：Rust 的包名中的 `-` 在代码中会被转换为 `_`
// 所以 `kiro-rs` 包在代码中应该使用 `kiro_rs`
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// 检查 Redis 是否可用
async fn is_redis_available() -> bool {
    let config = CacheConfig {
        enabled: true,
        redis_url: "redis://localhost:6379".to_string(),
        ttl_seconds: 60,
        password: None,
        db: Some(15), // 使用测试专用数据库
        blacklist_patterns: vec![],
    };

    SimpleCache::new(config).await.is_ok()
}

/// 创建测试用缓存实例
async fn create_test_cache(ttl_seconds: u64) -> Option<Arc<SimpleCache>> {
    let config = CacheConfig {
        enabled: true,
        redis_url: "redis://localhost:6379".to_string(),
        ttl_seconds,
        password: None,
        db: Some(15), // 使用测试专用数据库
        blacklist_patterns: vec!["password".to_string(), "secret".to_string()],
    };

    match SimpleCache::new(config).await {
        Ok(cache) => Some(Arc::new(cache)),
        Err(_) => None,
    }
}

/// 测试前清理缓存
async fn cleanup_cache(cache: &SimpleCache) {
    let _ = cache.clear().await;
}

#[tokio::test]
async fn test_cache_hit_flow() {
    // 检查 Redis 是否可用
    if !is_redis_available().await {
        eprintln!("跳过测试：Redis 不可用");
        return;
    }

    let cache = create_test_cache(3600).await.expect("创建缓存失败");
    cleanup_cache(&cache).await;

    // 1. 第一次请求 - 缓存未命中
    let key = "test:cache:hit:key1";
    let response1 = cache.get(key).await.expect("读取缓存失败");
    assert!(response1.is_none(), "首次请求应该缓存未命中");

    // 2. 写入缓存
    let cached_response = CachedResponse {
        events: vec![
            "event: message_start".to_string(),
            "data: {\"type\":\"message_start\"}".to_string(),
            "event: content_block_start".to_string(),
            "data: {\"type\":\"content_block_start\"}".to_string(),
        ],
        cached_at: chrono::Utc::now().timestamp(),
    };

    cache
        .set(key, &cached_response)
        .await
        .expect("写入缓存失败");

    // 3. 第二次请求 - 缓存命中
    let response2 = cache.get(key).await.expect("读取缓存失败");
    assert!(response2.is_some(), "第二次请求应该缓存命中");

    let cached = response2.unwrap();
    assert_eq!(cached.events.len(), 4, "缓存事件数量应该一致");
    assert_eq!(
        cached.events[0], cached_response.events[0],
        "缓存内容应该一致"
    );

    // 清理
    cleanup_cache(&cache).await;
}

#[tokio::test]
async fn test_cache_miss_flow() {
    if !is_redis_available().await {
        eprintln!("跳过测试：Redis 不可用");
        return;
    }

    let cache = create_test_cache(3600).await.expect("创建缓存失败");
    cleanup_cache(&cache).await;

    // 1. 请求不同的 key，每次都应该未命中
    let keys = vec!["test:miss:key1", "test:miss:key2", "test:miss:key3"];

    for key in keys {
        let response = cache.get(key).await.expect("读取缓存失败");
        assert!(response.is_none(), "不同 key 应该缓存未命中: {}", key);
    }

    // 清理
    cleanup_cache(&cache).await;
}

#[tokio::test]
async fn test_cache_expiration() {
    if !is_redis_available().await {
        eprintln!("跳过测试：Redis 不可用");
        return;
    }

    // 创建 TTL 为 2 秒的缓存
    let cache = create_test_cache(2).await.expect("创建缓存失败");
    cleanup_cache(&cache).await;

    let key = "test:expiration:key1";

    // 1. 写入缓存
    let cached_response = CachedResponse {
        events: vec!["event: test".to_string()],
        cached_at: chrono::Utc::now().timestamp(),
    };

    cache
        .set(key, &cached_response)
        .await
        .expect("写入缓存失败");

    // 2. 立即读取 - 应该命中
    let response1 = cache.get(key).await.expect("读取缓存失败");
    assert!(response1.is_some(), "TTL 内应该缓存命中");

    // 3. 等待 TTL 过期（2 秒 + 0.5 秒缓冲）
    sleep(Duration::from_millis(2500)).await;

    // 4. 再次读取 - 应该未命中
    let response2 = cache.get(key).await.expect("读取缓存失败");
    assert!(response2.is_none(), "TTL 过期后应该缓存未命中");

    // 清理
    cleanup_cache(&cache).await;
}

#[tokio::test]
async fn test_blacklist_filtering() {
    if !is_redis_available().await {
        eprintln!("跳过测试：Redis 不可用");
        return;
    }

    let cache = create_test_cache(3600).await.expect("创建缓存失败");
    cleanup_cache(&cache).await;

    // 1. 包含敏感词的请求应该不被缓存
    let sensitive_requests = vec![
        r#"{"user": "admin", "password": "123456"}"#,
        r#"{"api_key": "sk-xxx"}"#,
        r#"{"secret": "my-secret"}"#,
    ];

    for request in sensitive_requests {
        let should_cache = cache.should_cache(request);
        assert!(
            !should_cache,
            "包含敏感词的请求不应该被缓存: {}",
            request
        );
    }

    // 2. 正常请求应该可以缓存
    let normal_request = r#"{"user": "admin", "action": "list"}"#;
    let should_cache = cache.should_cache(normal_request);
    assert!(should_cache, "正常请求应该可以被缓存");

    // 清理
    cleanup_cache(&cache).await;
}

#[tokio::test]
async fn test_redis_failure_fallback() {
    // 1. 使用错误的 Redis URL
    let config = CacheConfig {
        enabled: true,
        redis_url: "redis://invalid-host:9999".to_string(),
        ttl_seconds: 3600,
        password: None,
        db: None,
        blacklist_patterns: vec![],
    };

    // 2. 创建缓存应该失败
    let result = SimpleCache::new(config).await;
    assert!(result.is_err(), "错误的 Redis URL 应该创建失败");

    // 3. 如果缓存创建失败，服务应该降级到无缓存模式
    // 这个测试验证了错误处理逻辑
}

#[tokio::test]
async fn test_stream_simulation_from_cache() {
    if !is_redis_available().await {
        eprintln!("跳过测试：Redis 不可用");
        return;
    }

    let cache = create_test_cache(3600).await.expect("创建缓存失败");
    cleanup_cache(&cache).await;

    let key = "test:stream:key1";

    // 1. 模拟流式响应的 SSE 事件
    let sse_events = vec![
        "event: message_start".to_string(),
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}".to_string(),
        "".to_string(),
        "event: content_block_start".to_string(),
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}".to_string(),
        "".to_string(),
        "event: content_block_delta".to_string(),
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}".to_string(),
        "".to_string(),
        "event: content_block_stop".to_string(),
        "data: {\"type\":\"content_block_stop\",\"index\":0}".to_string(),
        "".to_string(),
        "event: message_delta".to_string(),
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":5}}".to_string(),
        "".to_string(),
        "event: message_stop".to_string(),
        "data: {\"type\":\"message_stop\"}".to_string(),
        "".to_string(),
    ];

    // 2. 写入缓存
    let cached_response = CachedResponse {
        events: sse_events.clone(),
        cached_at: chrono::Utc::now().timestamp(),
    };

    cache
        .set(key, &cached_response)
        .await
        .expect("写入缓存失败");

    // 3. 从缓存读取
    let response = cache
        .get(key)
        .await
        .expect("读取缓存失败")
        .expect("缓存应该命中");

    // 4. 验证 SSE 格式正确
    assert_eq!(
        response.events.len(),
        sse_events.len(),
        "事件数量应该一致"
    );

    // 5. 验证事件顺序一致
    for (i, event) in response.events.iter().enumerate() {
        assert_eq!(event, &sse_events[i], "事件 {} 应该一致", i);
    }

    // 6. 验证关键事件存在
    assert!(
        response.events.iter().any(|e| e.contains("message_start")),
        "应该包含 message_start 事件"
    );
    assert!(
        response.events.iter().any(|e| e.contains("content_block_delta")),
        "应该包含 content_block_delta 事件"
    );
    assert!(
        response.events.iter().any(|e| e.contains("message_stop")),
        "应该包含 message_stop 事件"
    );

    // 清理
    cleanup_cache(&cache).await;
}

#[tokio::test]
async fn test_cache_disabled() {
    // 测试禁用缓存的情况
    let config = CacheConfig {
        enabled: false, // 禁用缓存
        redis_url: "redis://localhost:6379".to_string(),
        ttl_seconds: 3600,
        password: None,
        db: Some(15),
        blacklist_patterns: vec![],
    };

    if let Ok(cache) = SimpleCache::new(config).await {
        let key = "test:disabled:key1";

        // 1. 写入缓存（应该被忽略）
        let cached_response = CachedResponse {
            events: vec!["test".to_string()],
            cached_at: chrono::Utc::now().timestamp(),
        };

        cache.set(key, &cached_response).await.expect("写入应该成功但被忽略");

        // 2. 读取缓存（应该返回 None）
        let response = cache.get(key).await.expect("读取应该成功");
        assert!(response.is_none(), "禁用缓存时应该总是返回 None");
    }
}
