mod admin;
mod admin_ui;
mod anthropic;
mod cache;
mod common;
mod http_client;
mod kiro;
mod metrics;
mod model;
pub mod token;

use std::sync::Arc;

use axum::routing::get;
use clap::Parser;
use kiro::model::credentials::{CredentialsConfig, KiroCredentials};
use kiro::provider::KiroProvider;
use kiro::token_manager::MultiTokenManager;
use model::arg::Args;
use model::config::Config;

/// 连接预热：为每个可用凭据发送 HEAD 请求
async fn warm_connections(
    token_manager: Arc<MultiTokenManager>,
    proxy_config: Option<http_client::ProxyConfig>,
    tls_backend: model::config::TlsBackend,
) {
    tracing::info!("开始连接预热...");

    let client = match http_client::build_client(proxy_config.as_ref(), 30, tls_backend) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("预热失败：无法创建 HTTP 客户端: {}", e);
            return;
        }
    };

    let snapshot = token_manager.snapshot();
    let total = snapshot.total;
    let mut success_count = 0;
    let mut failed_count = 0;

    for entry in snapshot.entries.iter() {
        if entry.disabled {
            tracing::debug!("跳过已禁用凭据 #{}", entry.id);
            continue;
        }

        let url = "https://api.anthropic.com/v1/models";
        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_success() || resp.status().as_u16() == 401 {
                    // 401 也算成功，说明连接已建立
                    success_count += 1;
                    tracing::debug!("凭据 #{} 预热成功", entry.id);
                } else {
                    failed_count += 1;
                    tracing::warn!("凭据 #{} 预热失败: HTTP {}", entry.id, resp.status());
                }
            }
            Err(e) => {
                failed_count += 1;
                tracing::warn!("凭据 #{} 预热失败: {}", entry.id, e);
            }
        }
    }

    tracing::info!(
        "连接预热完成: 总计 {} 个凭据，成功 {}，失败 {}",
        total,
        success_count,
        failed_count
    );
}

/// Prometheus Metrics 端点处理器
async fn metrics_handler() -> (axum::http::StatusCode, String) {
    match metrics::export_metrics() {
        Ok(metrics) => (axum::http::StatusCode::OK, metrics),
        Err(e) => {
            tracing::error!("导出 Prometheus 指标失败: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error exporting metrics: {}", e),
            )
        }
    }
}

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // 加载配置
    let config_path = args
        .config
        .unwrap_or_else(|| Config::default_config_path().to_string());
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        tracing::error!("加载配置失败: {}", e);
        std::process::exit(1);
    });

    // 加载凭证（支持单对象或数组格式）
    let credentials_path = args
        .credentials
        .unwrap_or_else(|| KiroCredentials::default_credentials_path().to_string());
    let credentials_config = CredentialsConfig::load(&credentials_path).unwrap_or_else(|e| {
        tracing::error!("加载凭证失败: {}", e);
        std::process::exit(1);
    });

    // 判断是否为多凭据格式（用于刷新后回写）
    let is_multiple_format = credentials_config.is_multiple();

    // 转换为按优先级排序的凭据列表
    let credentials_list = credentials_config.into_sorted_credentials();
    tracing::info!("已加载 {} 个凭据配置", credentials_list.len());

    // 获取第一个凭据用于日志显示
    let first_credentials = credentials_list.first().cloned().unwrap_or_default();
    tracing::debug!("主凭证: {:?}", first_credentials);

    // 获取 API Key
    let api_key = config.api_key.clone().unwrap_or_else(|| {
        tracing::error!("配置文件中未设置 apiKey");
        std::process::exit(1);
    });

    // 构建代理配置
    let proxy_config = config.proxy_url.as_ref().map(|url| {
        let mut proxy = http_client::ProxyConfig::new(url);
        if let (Some(username), Some(password)) = (&config.proxy_username, &config.proxy_password) {
            proxy = proxy.with_auth(username, password);
        }
        proxy
    });

    if proxy_config.is_some() {
        tracing::info!("已配置 HTTP 代理: {}", config.proxy_url.as_ref().unwrap());
    }

    // 创建 MultiTokenManager 和 KiroProvider
    let token_manager = MultiTokenManager::new(
        config.clone(),
        credentials_list,
        proxy_config.clone(),
        Some(credentials_path.into()),
        is_multiple_format,
    )
    .unwrap_or_else(|e| {
        tracing::error!("创建 Token 管理器失败: {}", e);
        std::process::exit(1);
    });
    let token_manager = Arc::new(token_manager);
    let kiro_provider = KiroProvider::with_proxy(token_manager.clone(), proxy_config.clone());

    // 初始化 count_tokens 配置
    token::init_config(token::CountTokensConfig {
        api_url: config.count_tokens_api_url.clone(),
        api_key: config.count_tokens_api_key.clone(),
        auth_type: config.count_tokens_auth_type.clone(),
        proxy: proxy_config.clone(),
        tls_backend: config.tls_backend,
    });

    // 初始化缓存（如果配置了）
    let cache = if let Some(cache_config) = &config.cache {
        if cache_config.enabled {
            match cache::SimpleCache::new(cache_config.clone()).await {
                Ok(c) => {
                    tracing::info!("Redis 缓存已启用");
                    Some(Arc::new(c))
                }
                Err(e) => {
                    tracing::warn!("Redis 缓存初始化失败，将禁用缓存: {}", e);
                    None
                }
            }
        } else {
            tracing::info!("Redis 缓存已禁用（配置中 enabled=false）");
            None
        }
    } else {
        tracing::info!("未配置 Redis 缓存");
        None
    };

    // 启动缓存指标更新任务
    if let Some(cache_ref) = &cache {
        let cache_clone = cache_ref.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                let hits = cache_clone.get_hits();
                let misses = cache_clone.get_misses();
                metrics::update_cache_hit_rate(hits, misses);
            }
        });
    }

    // 启动凭据状态指标更新任务
    {
        let token_manager_clone = token_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                let snapshot = token_manager_clone.snapshot();
                for entry in snapshot.entries.iter() {
                    let status = if entry.disabled { 0.0 } else { 1.0 };
                    let auth_method = entry.auth_method.as_deref().unwrap_or("unknown");
                    metrics::CREDENTIAL_STATUS
                        .with_label_values(&[&entry.id.to_string(), auth_method])
                        .set(status);
                }
            }
        });
    }

    // 构建 Anthropic API 路由（从第一个凭据获取 profile_arn）
    let anthropic_app = anthropic::create_router_with_provider(
        &api_key,
        Some(kiro_provider),
        first_credentials.profile_arn.clone(),
        cache.clone(),
    );

    // 构建 Admin API 路由（如果配置了非空的 admin_api_key）
    // 安全检查：空字符串被视为未配置，防止空 key 绕过认证
    let admin_key_valid = config
        .admin_api_key
        .as_ref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false);

    let app = if let Some(admin_key) = &config.admin_api_key {
        if admin_key.trim().is_empty() {
            tracing::warn!("admin_api_key 配置为空，Admin API 未启用");
            anthropic_app
        } else {
            let http_client = http_client::build_client(proxy_config.as_ref(), 30, config.tls_backend)
                .unwrap_or_else(|e| {
                    tracing::error!("创建 OIDC HTTP 客户端失败: {}", e);
                    std::process::exit(1);
                });
            let oidc_client = kiro::oidc::OidcClient::new(http_client, &config.kiro_version);
            let admin_service = admin::AdminService::new(token_manager.clone(), oidc_client);
            let admin_state = admin::AdminState::new(admin_key, admin_service, cache.clone());
            let admin_app = admin::create_admin_router(admin_state);

            // 创建 Admin UI 路由
            let admin_ui_app = admin_ui::create_admin_ui_router();

            tracing::info!("Admin API 已启用");
            tracing::info!("Admin UI 已启用: /admin");
            anthropic_app
                .nest("/api/admin", admin_app)
                .nest("/admin", admin_ui_app)
        }
    } else {
        anthropic_app
    };

    // 添加 /metrics 端点
    let app = app.route("/metrics", get(metrics_handler));

    // 启动服务器
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("启动 Anthropic API 端点: {}", addr);
    tracing::info!("API Key: {}***", &api_key[..(api_key.len() / 2)]);
    tracing::info!("可用 API:");
    tracing::info!("  GET  /v1/models");
    tracing::info!("  POST /v1/messages");
    tracing::info!("  POST /v1/messages/count_tokens");
    tracing::info!("  GET  /metrics");
    if admin_key_valid {
        tracing::info!("Admin API:");
        tracing::info!("  GET  /api/admin/credentials");
        tracing::info!("  POST /api/admin/credentials/:index/disabled");
        tracing::info!("  POST /api/admin/credentials/:index/priority");
        tracing::info!("  POST /api/admin/credentials/:index/reset");
        tracing::info!("  GET  /api/admin/credentials/:index/balance");
        tracing::info!("Admin UI:");
        tracing::info!("  GET  /admin");
    }

    // 连接预热
    warm_connections(token_manager.clone(), proxy_config.clone(), config.tls_backend).await;

    // 绑定监听地址
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("无法绑定地址 {}: {}", addr, e);
            tracing::error!("可能原因：端口已被占用或权限不足");
            std::process::exit(1);
        }
    };

    // 启动服务
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("服务运行失败: {}", e);
        std::process::exit(1);
    }
}
