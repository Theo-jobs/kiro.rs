//! Admin API 路由配置

use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use super::{
    handlers::{
        add_credential, claim_auth, delete_credential, get_all_credentials, get_auth_status,
        get_cache_stats, get_credential_balance, get_global_proxy, get_load_balancing_mode,
        get_redis_cache_config, reset_failure_count, set_credential_disabled, set_credential_priority,
        set_load_balancing_mode, start_auth, update_credential_proxy, update_global_proxy,
        update_redis_cache_config,
    },
    middleware::{admin_auth_middleware, AdminState},
};

/// 创建 Admin API 路由
///
/// # 端点
/// - `GET /credentials` - 获取所有凭据状态
/// - `POST /credentials` - 添加新凭据
/// - `DELETE /credentials/:id` - 删除凭据
/// - `POST /credentials/:id/disabled` - 设置凭据禁用状态
/// - `POST /credentials/:id/priority` - 设置凭据优先级
/// - `PUT /credentials/:id/proxy` - 更新凭据代理配置
/// - `POST /credentials/:id/reset` - 重置失败计数
/// - `GET /credentials/:id/balance` - 获取凭据余额
/// - `GET /config/load-balancing` - 获取负载均衡模式
/// - `PUT /config/load-balancing` - 设置负载均衡模式
/// - `GET /config/proxy` - 获取全局代理配置
/// - `PUT /config/proxy` - 更新全局代理配置
///
/// # 认证
/// 需要 Admin API Key 认证，支持：
/// - `x-api-key` header
/// - `Authorization: Bearer <token>` header
pub fn create_admin_router(state: AdminState) -> Router {
    Router::new()
        .route(
            "/credentials",
            get(get_all_credentials).post(add_credential),
        )
        .route("/credentials/{id}", delete(delete_credential))
        .route("/credentials/{id}/disabled", post(set_credential_disabled))
        .route("/credentials/{id}/priority", post(set_credential_priority))
        .route("/credentials/{id}/proxy", put(update_credential_proxy))
        .route("/credentials/{id}/reset", post(reset_failure_count))
        .route("/credentials/{id}/balance", get(get_credential_balance))
        .route("/auth/start", post(start_auth))
        .route("/auth/status/{id}", get(get_auth_status))
        .route("/auth/claim/{id}", post(claim_auth))
        .route(
            "/config/load-balancing",
            get(get_load_balancing_mode).put(set_load_balancing_mode),
        )
        .route(
            "/config/proxy",
            get(get_global_proxy).put(update_global_proxy),
        )
        .route(
            "/config/redis-cache",
            get(get_redis_cache_config).put(update_redis_cache_config),
        )
        .route("/cache/stats", get(get_cache_stats))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .with_state(state)
}
