//! AWS OIDC 设备授权客户端
//!
//! 实现 AWS SSO OIDC 设备授权流程，支持：
//! - AWS Builder ID（个人开发者）
//! - 企业 IAM Identity Center（需要 startUrl + region）

use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// OIDC 客户端注册响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterClientResponse {
    pub client_id: String,
    pub client_secret: String,
}

/// 设备授权响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAuthResponse {
    pub device_code: String,
    pub user_code: String,
    /// RFC 8628: verification_uri_complete 是可选的
    pub verification_uri_complete: Option<String>,
    pub verification_uri: Option<String>,
    pub interval: Option<u64>,
    pub expires_in: u64,
}

/// Token 响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// OIDC 错误响应
#[derive(Debug, Deserialize)]
pub struct OidcErrorResponse {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// 轮询结果
#[derive(Debug)]
pub enum PollResult {
    /// 用户尚未完成授权
    Pending,
    /// 服务端要求降速（RFC 8628: 增加轮询间隔 5s）
    SlowDown,
    /// 授权完成
    Completed(TokenResponse),
    /// 授权失败
    Failed(String),
}

/// OIDC 请求的 scope 列表
const OIDC_SCOPES: &[&str] = &[
    "codewhisperer:completions",
    "codewhisperer:analysis",
    "codewhisperer:conversations",
    "codewhisperer:taskassist",
    "codewhisperer:transformations",
];

/// AWS OIDC 客户端
pub struct OidcClient {
    http_client: Client,
    kiro_version: String,
}

impl OidcClient {
    pub fn new(http_client: Client, kiro_version: &str) -> Self {
        Self {
            http_client,
            kiro_version: kiro_version.to_string(),
        }
    }

    /// 构建 OIDC 端点 URL
    fn endpoint(region: &str, path: &str) -> String {
        format!("https://oidc.{}.amazonaws.com{}", region, path)
    }

    /// 构建通用请求头
    fn build_headers(&self, machine_id: &str) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert(
            "user-agent",
            format!("KiroIDE-{}-{}", self.kiro_version, machine_id)
                .parse()
                .unwrap(),
        );
        headers.insert(
            "amz-sdk-request",
            "attempt=1; max=3".parse().unwrap(),
        );
        headers.insert(
            "amz-sdk-invocation-id",
            Uuid::new_v4().to_string().parse().unwrap(),
        );
        headers
    }

    /// 注册 OIDC 客户端
    pub async fn register_client(
        &self,
        region: &str,
        machine_id: &str,
        issuer_url: Option<&str>,
    ) -> anyhow::Result<RegisterClientResponse> {
        let url = Self::endpoint(region, "/client/register");
        let headers = self.build_headers(machine_id);

        let mut body = serde_json::json!({
            "clientName": "Kiro IDE",
            "clientType": "public",
            "scopes": OIDC_SCOPES,
        });

        // 企业模式需要 issuerUrl
        if let Some(issuer) = issuer_url {
            body["issuerUrl"] = serde_json::Value::String(issuer.to_string());
        }

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err: OidcErrorResponse = resp.json().await.unwrap_or(OidcErrorResponse {
                error: status.to_string(),
                error_description: None,
            });
            anyhow::bail!(
                "OIDC register_client 失败 ({}): {} - {}",
                status,
                err.error,
                err.error_description.unwrap_or_default()
            );
        }

        Ok(resp.json().await?)
    }

    /// 设备授权
    pub async fn device_authorize(
        &self,
        region: &str,
        client_id: &str,
        client_secret: &str,
        machine_id: &str,
        start_url: &str,
    ) -> anyhow::Result<DeviceAuthResponse> {
        let url = Self::endpoint(region, "/device_authorization");
        let headers = self.build_headers(machine_id);

        let body = serde_json::json!({
            "clientId": client_id,
            "clientSecret": client_secret,
            "startUrl": start_url,
        });

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err: OidcErrorResponse = resp.json().await.unwrap_or(OidcErrorResponse {
                error: status.to_string(),
                error_description: None,
            });
            anyhow::bail!(
                "OIDC device_authorize 失败 ({}): {} - {}",
                status,
                err.error,
                err.error_description.unwrap_or_default()
            );
        }

        Ok(resp.json().await?)
    }

    /// 单次轮询 token
    pub async fn poll_token_once(
        &self,
        region: &str,
        client_id: &str,
        client_secret: &str,
        device_code: &str,
        machine_id: &str,
    ) -> anyhow::Result<PollResult> {
        let url = Self::endpoint(region, "/token");
        let headers = self.build_headers(machine_id);

        let body = serde_json::json!({
            "clientId": client_id,
            "clientSecret": client_secret,
            "deviceCode": device_code,
            "grantType": "urn:ietf:params:oauth:grant-type:device_code",
        });

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            let token: TokenResponse = resp.json().await?;
            return Ok(PollResult::Completed(token));
        }

        let err: OidcErrorResponse = resp.json().await.unwrap_or(OidcErrorResponse {
            error: "unknown".to_string(),
            error_description: None,
        });

        if err.error == "authorization_pending" {
            Ok(PollResult::Pending)
        } else if err.error == "slow_down" {
            Ok(PollResult::SlowDown)
        } else if err.error == "expired_token" {
            Ok(PollResult::Failed("授权已过期，请重新发起登录".to_string()))
        } else if err.error == "access_denied" {
            Ok(PollResult::Failed("用户拒绝了授权请求".to_string()))
        } else {
            Ok(PollResult::Failed(format!(
                "{}: {}",
                err.error,
                err.error_description.unwrap_or_default()
            )))
        }
    }
}
