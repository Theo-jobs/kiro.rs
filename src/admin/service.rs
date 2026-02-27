//! Admin API 业务逻辑服务

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::oidc::{OidcClient, PollResult};
use crate::kiro::token_manager::MultiTokenManager;

use super::auth_session::{AuthSession, AuthSessionStatus, AuthSessionStore};
use super::error::AdminServiceError;
use super::types::{
    AddCredentialRequest, AddCredentialResponse, BalanceResponse, CredentialStatusItem,
    CredentialsStatusResponse, LoadBalancingModeResponse, SetLoadBalancingModeRequest,
};

/// 余额缓存过期时间（秒），10 分钟
const BALANCE_CACHE_TTL_SECS: i64 = 600;

/// 缓存的余额条目（含时间戳）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedBalance {
    /// 缓存时间（Unix 秒）
    cached_at: f64,
    /// 缓存的余额数据
    data: BalanceResponse,
}

/// Admin 服务
///
/// 封装所有 Admin API 的业务逻辑
pub struct AdminService {
    token_manager: Arc<MultiTokenManager>,
    balance_cache: Mutex<HashMap<u64, CachedBalance>>,
    cache_path: Option<PathBuf>,
    auth_sessions: Arc<AuthSessionStore>,
    oidc_client: Arc<OidcClient>,
}

impl AdminService {
    pub fn new(token_manager: Arc<MultiTokenManager>, oidc_client: OidcClient) -> Self {
        let cache_path = token_manager
            .cache_dir()
            .map(|d| d.join("kiro_balance_cache.json"));

        let balance_cache = Self::load_balance_cache_from(&cache_path);

        Self {
            token_manager,
            balance_cache: Mutex::new(balance_cache),
            cache_path,
            auth_sessions: Arc::new(AuthSessionStore::new()),
            oidc_client: Arc::new(oidc_client),
        }
    }

    /// 获取所有凭据状态
    pub fn get_all_credentials(&self) -> CredentialsStatusResponse {
        let snapshot = self.token_manager.snapshot();
        let cache = self.balance_cache.lock();

        let mut credentials: Vec<CredentialStatusItem> = snapshot
            .entries
            .into_iter()
            .map(|entry| {
                // 从缓存中查找对应的余额数据
                let cached_balance = cache.get(&entry.id).map(|cached| {
                    let mut balance = cached.data.clone();
                    balance.cached_at = Some(cached.cached_at);
                    balance
                });

                CredentialStatusItem {
                    id: entry.id,
                    priority: entry.priority,
                    disabled: entry.disabled,
                    failure_count: entry.failure_count,
                    is_current: entry.id == snapshot.current_id,
                    expires_at: entry.expires_at,
                    auth_method: entry.auth_method,
                    has_profile_arn: entry.has_profile_arn,
                    refresh_token_hash: entry.refresh_token_hash,
                    email: entry.email,
                    success_count: entry.success_count,
                    last_used_at: entry.last_used_at.clone(),
                    has_proxy: entry.has_proxy,
                    proxy_url: entry.proxy_url,
                    cached_balance,
                }
            })
            .collect();

        // 按优先级排序（数字越小优先级越高）
        credentials.sort_by_key(|c| c.priority);

        CredentialsStatusResponse {
            total: snapshot.total,
            available: snapshot.available,
            current_id: snapshot.current_id,
            credentials,
        }
    }

    /// 设置凭据禁用状态
    pub fn set_disabled(&self, id: u64, disabled: bool) -> Result<(), AdminServiceError> {
        // 先获取当前凭据 ID，用于判断是否需要切换
        let snapshot = self.token_manager.snapshot();
        let current_id = snapshot.current_id;

        self.token_manager
            .set_disabled(id, disabled)
            .map_err(|e| self.classify_error(e, id))?;

        // 只有禁用的是当前凭据时才尝试切换到下一个
        if disabled && id == current_id {
            let _ = self.token_manager.switch_to_next();
        }
        Ok(())
    }

    /// 设置凭据优先级
    pub fn set_priority(&self, id: u64, priority: u32) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_priority(id, priority)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 更新凭据代理配置
    pub fn update_proxy(
        &self,
        id: u64,
        proxy_url: Option<String>,
        proxy_username: Option<String>,
        proxy_password: Option<String>,
    ) -> Result<(), AdminServiceError> {
        self.token_manager
            .update_proxy(id, proxy_url, proxy_username, proxy_password)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 重置失败计数并重新启用
    pub fn reset_and_enable(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .reset_and_enable(id)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 获取凭据余额（带缓存）
    pub async fn get_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        // 先查缓存
        {
            let cache = self.balance_cache.lock();
            if let Some(cached) = cache.get(&id) {
                let now = Utc::now().timestamp() as f64;
                if (now - cached.cached_at) < BALANCE_CACHE_TTL_SECS as f64 {
                    tracing::debug!("凭据 #{} 余额命中缓存", id);
                    return Ok(cached.data.clone());
                }
            }
        }

        // 缓存未命中或已过期，从上游获取
        let balance = self.fetch_balance(id).await?;

        // 更新缓存
        {
            let mut cache = self.balance_cache.lock();
            cache.insert(
                id,
                CachedBalance {
                    cached_at: Utc::now().timestamp() as f64,
                    data: balance.clone(),
                },
            );
        }
        self.save_balance_cache();

        Ok(balance)
    }

    /// 从上游获取余额（无缓存）
    async fn fetch_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        let usage = self
            .token_manager
            .get_usage_limits_for(id)
            .await
            .map_err(|e| self.classify_balance_error(e, id))?;

        let current_usage = usage.current_usage();
        let usage_limit = usage.usage_limit();
        let remaining = (usage_limit - current_usage).max(0.0);
        let usage_percentage = if usage_limit > 0.0 {
            (current_usage / usage_limit * 100.0).min(100.0)
        } else {
            0.0
        };

        // 提取有效时间（freeTrialExpiry）
        let token_expiry = usage
            .usage_breakdown_list
            .first()
            .and_then(|b| b.free_trial_info.as_ref())
            .and_then(|info| info.free_trial_expiry);

        Ok(BalanceResponse {
            id,
            subscription_title: usage.subscription_title().map(|s| s.to_string()),
            current_usage,
            usage_limit,
            remaining,
            usage_percentage,
            next_reset_at: usage.next_date_reset,
            token_expiry,
            cached_at: None, // fetch_balance 不填充 cached_at，由 get_balance 填充
        })
    }

    /// 添加新凭据
    pub async fn add_credential(
        &self,
        req: AddCredentialRequest,
    ) -> Result<AddCredentialResponse, AdminServiceError> {
        // 构建凭据对象
        let email = req.email.clone();
        let new_cred = KiroCredentials {
            id: None,
            access_token: None,
            refresh_token: Some(req.refresh_token),
            profile_arn: None,
            expires_at: None,
            auth_method: Some(req.auth_method),
            client_id: req.client_id,
            client_secret: req.client_secret,
            priority: req.priority,
            region: req.region,
            auth_region: req.auth_region,
            api_region: req.api_region,
            machine_id: req.machine_id,
            email: req.email,
            subscription_title: None, // 将在首次获取使用额度时自动更新
            proxy_url: req.proxy_url,
            proxy_username: req.proxy_username,
            proxy_password: req.proxy_password,
            disabled: false, // 新添加的凭据默认启用
        };

        // 调用 token_manager 添加凭据
        let credential_id = self
            .token_manager
            .add_credential(new_cred)
            .await
            .map_err(|e| self.classify_add_error(e))?;

        // 主动获取订阅等级，避免首次请求时 Free 账号绕过 Opus 模型过滤
        if let Err(e) = self.token_manager.get_usage_limits_for(credential_id).await {
            tracing::warn!("添加凭据后获取订阅等级失败（不影响凭据添加）: {}", e);
        }

        Ok(AddCredentialResponse {
            success: true,
            message: format!("凭据添加成功，ID: {}", credential_id),
            credential_id,
            email,
        })
    }

    /// 删除凭据
    pub fn delete_credential(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .delete_credential(id)
            .map_err(|e| self.classify_delete_error(e, id))?;

        // 清理已删除凭据的余额缓存
        {
            let mut cache = self.balance_cache.lock();
            cache.remove(&id);
        }
        self.save_balance_cache();

        Ok(())
    }

    /// 启动 OIDC 认证流程
    pub async fn start_auth(
        &self,
        req: super::types::AuthStartRequest,
    ) -> Result<super::types::AuthStartResponse, AdminServiceError> {
        // 清理过期会话
        self.auth_sessions.cleanup_expired();

        // 验证参数
        let (region, start_url, issuer_url) = if req.mode == "enterprise" {
            let start_url = req.start_url.as_deref().unwrap_or("").to_string();
            // 校验 URL：必须是 https:// 开头且有 host 部分
            if !start_url.starts_with("https://") || start_url.len() <= "https://".len() {
                return Err(AdminServiceError::AuthSessionInvalidState(
                    "企业模式需要有效的 HTTPS 格式 startUrl".to_string(),
                ));
            }
            // 确保 host 部分不为空（https:// 后面至少有一个非 / 字符）
            let after_scheme = &start_url["https://".len()..];
            if after_scheme.starts_with('/') || after_scheme.trim().is_empty() {
                return Err(AdminServiceError::AuthSessionInvalidState(
                    "startUrl 缺少有效的主机名".to_string(),
                ));
            }
            let region = req.region.as_deref().unwrap_or("").to_string();
            if !is_valid_aws_region(&region) {
                return Err(AdminServiceError::AuthSessionInvalidState(
                    "region 格式无效，应为 us-east-1 等格式".to_string(),
                ));
            }
            (region.clone(), start_url.clone(), Some(start_url))
        } else {
            // Builder ID 模式
            (
                "us-east-1".to_string(),
                "https://view.awsapps.com/start".to_string(),
                None,
            )
        };

        // 生成 machine_id
        let uuid_str = uuid::Uuid::new_v4().to_string().replace('-', "");
        let machine_id = format!("{}{}", uuid_str, uuid_str);

        // 注册客户端
        let reg = self
            .oidc_client
            .register_client(&region, &machine_id, issuer_url.as_deref())
            .await
            .map_err(|e| AdminServiceError::UpstreamError(e.to_string()))?;

        // 设备授权
        let auth = self
            .oidc_client
            .device_authorize(
                &region,
                &reg.client_id,
                &reg.client_secret,
                &machine_id,
                &start_url,
            )
            .await
            .map_err(|e| AdminServiceError::UpstreamError(e.to_string()))?;

        // 创建会话
        let auth_id = uuid::Uuid::new_v4().to_string();
        let session = AuthSession {
            id: auth_id.clone(),
            status: AuthSessionStatus::Pending,
            created_at: chrono::Utc::now(),
            region: region.clone(),
            start_url,
            machine_id: machine_id.clone(),
        };
        self.auth_sessions.insert(session).map_err(|msg| {
            AdminServiceError::AuthSessionInvalidState(msg.to_string())
        })?;

        // 后台轮询 task
        let sessions = self.auth_sessions.clone();
        let oidc = self.oidc_client.clone();
        let client_id = reg.client_id.clone();
        let client_secret = reg.client_secret.clone();
        let device_code = auth.device_code.clone();
        let poll_region = region.clone();
        let poll_machine_id = machine_id.clone();
        let poll_auth_id = auth_id.clone();
        let interval = auth.interval.unwrap_or(5).max(5);

        tokio::spawn(async move {
            let mut current_interval = interval;
            for _ in 0..120 {
                tokio::time::sleep(tokio::time::Duration::from_secs(current_interval)).await;

                // 检查会话是否还存在（可能被用户取消）
                if sessions.get_status(&poll_auth_id).is_none() {
                    tracing::debug!("OIDC 会话 {} 已被移除，停止轮询", poll_auth_id);
                    return;
                }

                match oidc
                    .poll_token_once(
                        &poll_region,
                        &client_id,
                        &client_secret,
                        &device_code,
                        &poll_machine_id,
                    )
                    .await
                {
                    Ok(PollResult::Pending) => continue,
                    Ok(PollResult::SlowDown) => {
                        // RFC 8628: 增加轮询间隔 5 秒
                        current_interval += 5;
                        tracing::debug!(
                            "OIDC 会话 {} 收到 slow_down，间隔增至 {}s",
                            poll_auth_id,
                            current_interval
                        );
                        continue;
                    }
                    Ok(PollResult::Completed(token)) => {
                        sessions.update_status(
                            &poll_auth_id,
                            AuthSessionStatus::Completed {
                                refresh_token: token.refresh_token,
                                client_id: client_id.clone(),
                                client_secret: client_secret.clone(),
                                region: poll_region.clone(),
                            },
                        );
                        tracing::info!("OIDC 会话 {} 授权完成", poll_auth_id);
                        return;
                    }
                    Ok(PollResult::Failed(msg)) => {
                        sessions.update_status(
                            &poll_auth_id,
                            AuthSessionStatus::Failed(msg.clone()),
                        );
                        tracing::warn!("OIDC 会话 {} 授权失败: {}", poll_auth_id, msg);
                        return;
                    }
                    Err(e) => {
                        tracing::warn!("OIDC 轮询出错: {}", e);
                        // 网络错误继续重试
                        continue;
                    }
                }
            }

            // 超时
            sessions.update_status(
                &poll_auth_id,
                AuthSessionStatus::Failed("授权超时".to_string()),
            );
        });

        Ok(super::types::AuthStartResponse {
            auth_id,
            verification_uri: auth
                .verification_uri_complete
                .or(auth.verification_uri)
                .unwrap_or_default(),
            user_code: auth.user_code,
            expires_in: auth.expires_in,
        })
    }

    /// 获取认证会话状态
    pub fn get_auth_status(
        &self,
        auth_id: &str,
    ) -> Result<super::types::AuthStatusResponse, AdminServiceError> {
        match self.auth_sessions.get_status(auth_id) {
            Some(AuthSessionStatus::Pending) => Ok(super::types::AuthStatusResponse {
                status: "pending".to_string(),
                error: None,
            }),
            Some(AuthSessionStatus::Completed { .. }) => Ok(super::types::AuthStatusResponse {
                status: "completed".to_string(),
                error: None,
            }),
            Some(AuthSessionStatus::Failed(msg)) => Ok(super::types::AuthStatusResponse {
                status: "failed".to_string(),
                error: Some(msg),
            }),
            None => Err(AdminServiceError::AuthSessionNotFound(
                auth_id.to_string(),
            )),
        }
    }

    /// 领取认证结果，创建凭据（原子操作：先 remove 再处理，防止重复 claim）
    pub async fn claim_auth(
        &self,
        auth_id: &str,
        req: super::types::AuthClaimRequest,
    ) -> Result<AddCredentialResponse, AdminServiceError> {
        // 原子移除：防止并发 claim 竞态
        let session = self
            .auth_sessions
            .remove(auth_id)
            .ok_or_else(|| AdminServiceError::AuthSessionNotFound(auth_id.to_string()))?;

        let (refresh_token, client_id, client_secret, region) = match &session.status {
            AuthSessionStatus::Completed {
                refresh_token,
                client_id,
                client_secret,
                region,
            } => (
                refresh_token.clone(),
                client_id.clone(),
                client_secret.clone(),
                region.clone(),
            ),
            AuthSessionStatus::Pending => {
                // 放回会话，因为还没完成
                let _ = self.auth_sessions.insert(session);
                return Err(AdminServiceError::AuthSessionInvalidState(
                    "认证尚未完成".to_string(),
                ));
            }
            AuthSessionStatus::Failed(msg) => {
                return Err(AdminServiceError::AuthSessionInvalidState(
                    format!("认证已失败: {}", msg),
                ));
            }
        };

        // 构建凭据
        let new_cred = KiroCredentials {
            id: None,
            access_token: None,
            refresh_token: Some(refresh_token),
            profile_arn: None,
            expires_at: None,
            auth_method: Some("idc".to_string()),
            client_id: Some(client_id),
            client_secret: Some(client_secret),
            priority: req.priority,
            region: Some(region.clone()),
            auth_region: Some(region.clone()),
            api_region: Some(region),
            machine_id: Some(session.machine_id.clone()),
            email: None,
            subscription_title: None,
            proxy_url: req.proxy_url,
            proxy_username: req.proxy_username,
            proxy_password: req.proxy_password,
            disabled: false,
        };

        // 添加凭据
        let credential_id = self
            .token_manager
            .add_credential(new_cred)
            .await
            .map_err(|e| self.classify_add_error(e))?;

        // 主动获取订阅等级
        if let Err(e) = self.token_manager.get_usage_limits_for(credential_id).await {
            tracing::warn!("OIDC 添加凭据后获取订阅等级失败: {}", e);
        }

        Ok(AddCredentialResponse {
            success: true,
            message: format!("OIDC 凭据添加成功，ID: {}", credential_id),
            credential_id,
            email: None,
        })
    }

    /// 获取负载均衡模式
    pub fn get_load_balancing_mode(&self) -> LoadBalancingModeResponse {
        LoadBalancingModeResponse {
            mode: self.token_manager.get_load_balancing_mode(),
        }
    }

    /// 设置负载均衡模式
    pub fn set_load_balancing_mode(
        &self,
        req: SetLoadBalancingModeRequest,
    ) -> Result<LoadBalancingModeResponse, AdminServiceError> {
        // 验证模式值
        if req.mode != "priority" && req.mode != "balanced" {
            return Err(AdminServiceError::InvalidCredential(
                "mode 必须是 'priority' 或 'balanced'".to_string(),
            ));
        }

        self.token_manager
            .set_load_balancing_mode(req.mode.clone())
            .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;

        Ok(LoadBalancingModeResponse { mode: req.mode })
    }

    // ============ 余额缓存持久化 ============

    fn load_balance_cache_from(cache_path: &Option<PathBuf>) -> HashMap<u64, CachedBalance> {
        let path = match cache_path {
            Some(p) => p,
            None => return HashMap::new(),
        };

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return HashMap::new(),
        };

        // 文件中使用字符串 key 以兼容 JSON 格式
        let map: HashMap<String, CachedBalance> = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("解析余额缓存失败，将忽略: {}", e);
                return HashMap::new();
            }
        };

        let now = Utc::now().timestamp() as f64;
        map.into_iter()
            .filter_map(|(k, v)| {
                let id = k.parse::<u64>().ok()?;
                // 丢弃超过 TTL 的条目
                if (now - v.cached_at) < BALANCE_CACHE_TTL_SECS as f64 {
                    Some((id, v))
                } else {
                    None
                }
            })
            .collect()
    }

    fn save_balance_cache(&self) {
        let path = match &self.cache_path {
            Some(p) => p,
            None => return,
        };

        // 持有锁期间完成序列化和写入，防止并发损坏
        let cache = self.balance_cache.lock();
        let map: HashMap<String, &CachedBalance> =
            cache.iter().map(|(k, v)| (k.to_string(), v)).collect();

        match serde_json::to_string_pretty(&map) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("保存余额缓存失败: {}", e);
                }
            }
            Err(e) => tracing::warn!("序列化余额缓存失败: {}", e),
        }
    }

    // ============ 错误分类 ============

    /// 分类简单操作错误（set_disabled, set_priority, reset_and_enable）
    fn classify_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类余额查询错误（可能涉及上游 API 调用）
    fn classify_balance_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();

        // 1. 凭据不存在
        if msg.contains("不存在") {
            return AdminServiceError::NotFound { id };
        }

        // 2. 上游服务错误特征：HTTP 响应错误或网络错误
        let is_upstream_error =
            // HTTP 响应错误（来自 refresh_*_token 的错误消息）
            msg.contains("凭证已过期或无效") ||
            msg.contains("权限不足") ||
            msg.contains("已被限流") ||
            msg.contains("服务器错误") ||
            msg.contains("Token 刷新失败") ||
            msg.contains("暂时不可用") ||
            // 网络错误（reqwest 错误）
            msg.contains("error trying to connect") ||
            msg.contains("connection") ||
            msg.contains("timeout") ||
            msg.contains("timed out");

        if is_upstream_error {
            AdminServiceError::UpstreamError(msg)
        } else {
            // 3. 默认归类为内部错误（本地验证失败、配置错误等）
            // 包括：缺少 refreshToken、refreshToken 已被截断、无法生成 machineId 等
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类添加凭据错误
    fn classify_add_error(&self, e: anyhow::Error) -> AdminServiceError {
        let msg = e.to_string();

        // 凭据验证失败（refreshToken 无效、格式错误等）
        let is_invalid_credential = msg.contains("缺少 refreshToken")
            || msg.contains("refreshToken 为空")
            || msg.contains("refreshToken 已被截断")
            || msg.contains("凭据已存在")
            || msg.contains("refreshToken 重复")
            || msg.contains("凭证已过期或无效")
            || msg.contains("权限不足")
            || msg.contains("已被限流");

        if is_invalid_credential {
            AdminServiceError::InvalidCredential(msg)
        } else if msg.contains("error trying to connect")
            || msg.contains("connection")
            || msg.contains("timeout")
        {
            AdminServiceError::UpstreamError(msg)
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类删除凭据错误
    fn classify_delete_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else if msg.contains("只能删除已禁用的凭据") || msg.contains("请先禁用凭据") {
            AdminServiceError::InvalidCredential(msg)
        } else {
            AdminServiceError::InternalError(msg)
        }
    }
}

/// AWS region 格式验证（如 us-east-1, ap-southeast-1, us-gov-west-1）
fn is_valid_aws_region(region: &str) -> bool {
    let parts: Vec<&str> = region.split('-').collect();
    if parts.len() < 3 {
        return false;
    }
    // 最后一部分必须是数字
    let last = parts.last().unwrap();
    if last.is_empty() || !last.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // 其余部分必须是小写字母
    parts[..parts.len() - 1]
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()))
}
