# Kiro.rs 凭据管理指南

## 概述

kiro.rs 支持多凭据管理，通过智能故障转移和负载均衡提升并发能力和可用性。

## 凭据配置

### 配置文件位置

- **Docker 部署**: `/app/config/credentials.json`
- **本地开发**: `./config/credentials.json`

### 配置格式

```json
{
  "credentials": [
    {
      "refresh_token": "your-refresh-token",
      "auth_method": "oidc",
      "client_id": "your-client-id",
      "client_secret": "your-client-secret",
      "priority": 1,
      "region": "us-east-1",
      "auth_region": "us-east-1",
      "api_region": "us-east-1",
      "machine_id": "optional-machine-id",
      "email": "user@example.com",
      "proxy_url": null,
      "proxy_username": null,
      "proxy_password": null,
      "disabled": false
    }
  ]
}
```

## 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `refresh_token` | ✅ | Kiro 刷新令牌 |
| `auth_method` | ✅ | 认证方式（`oidc` 或 `enterprise`） |
| `client_id` | ✅ | OAuth 客户端 ID |
| `client_secret` | ✅ | OAuth 客户端密钥 |
| `priority` | ❌ | 优先级（1-100，默认 50） |
| `region` | ❌ | AWS 区域（默认 `us-east-1`） |
| `auth_region` | ❌ | 认证区域（覆盖 region） |
| `api_region` | ❌ | API 区域（覆盖 region） |
| `machine_id` | ❌ | 机器 ID（Enterprise 模式） |
| `email` | ❌ | 用户邮箱（用于标识） |
| `proxy_url` | ❌ | 代理地址（如 `http://proxy:8080`） |
| `proxy_username` | ❌ | 代理用户名 |
| `proxy_password` | ❌ | 代理密码 |
| `disabled` | ❌ | 是否禁用（默认 false） |

## 添加凭据

### 方法 1: 手动编辑配置文件

1. 编辑 `credentials.json`
2. 在 `credentials` 数组中添加新凭据
3. 重启 kiro.rs 服务

```bash
# Docker 部署
docker restart kiro-rs

# 本地开发
cargo run
```

### 方法 2: 通过 Admin UI

1. 访问 `http://your-host:8990/admin`
2. 输入 Admin API Key
3. 点击「添加凭据」按钮
4. 填写凭据信息
5. 点击「保存」

**优点**: 无需重启服务，立即生效

## 负载均衡模式

在 `config.json` 中配置：

```json
{
  "load_balancing_mode": "priority"
}
```

### 可选模式

| 模式 | 说明 | 适用场景 |
|------|------|----------|
| `priority` | 优先级模式 | 优先使用高优先级凭据 |
| `balanced` | 轮询模式 | 均匀分配请求到所有凭据 |

### 优先级设置建议

- **Power 订阅**: `priority: 1-20`（最高优先级）
- **Pro 订阅**: `priority: 21-50`（中等优先级）
- **Free 订阅**: `priority: 51-100`（最低优先级）

## 故障转移机制

### 自动禁用条件

凭据在以下情况会被自动禁用：

1. **额度用尽**: 返回 `402 MONTHLY_REQUEST_COUNT`
2. **连续失败**: 5 分钟内失败 3 次
3. **认证失败**: 返回 `401 Unauthorized`

### 自动恢复

- 被禁用的凭据不会自动恢复
- 需要通过 Admin UI 手动重置失败计数或重启服务

### 重试策略

- 单凭据最多重试 3 次
- 总重试上限 9 次（跨凭据）
- 指数退避：200ms → 400ms → 800ms → 1600ms

## 代理配置

### 全局代理

在 `config.json` 中配置：

```json
{
  "global_proxy": {
    "url": "http://proxy.example.com:8080",
    "username": "user",
    "password": "pass"
  }
}
```

### 凭据级代理

在 `credentials.json` 中为单个凭据配置：

```json
{
  "proxy_url": "socks5://proxy.example.com:1080",
  "proxy_username": "user",
  "proxy_password": "pass"
}
```

### 优先级

凭据代理 > 全局代理 > 无代理

### 禁用代理

设置 `proxy_url: "direct"` 显式不走代理。

## 最佳实践

### 1. 凭据数量建议

| 并发需求 | 建议凭据数 | 说明 |
|----------|-----------|------|
| 低（< 10 QPS） | 3-5 个 | 基本故障转移 |
| 中（10-50 QPS） | 10-15 个 | 良好并发能力 |
| 高（> 50 QPS） | 20+ 个 | 高并发场景 |

### 2. 优先级设置

- 按订阅类型分层（Power > Pro > Free）
- 同类型凭据使用相同优先级（轮询）
- 避免所有凭据相同优先级（无法区分）

### 3. 区域选择

- 优先选择离用户最近的区域
- 美国用户: `us-east-1` 或 `us-west-2`
- 欧洲用户: `eu-west-1`
- 亚洲用户: `ap-southeast-1`

### 4. 监控和维护

- 定期检查 Admin UI 的凭据状态
- 关注失败计数和余额
- 及时补充额度或添加新凭据

## 常见问题

### Q: 如何查看凭据状态？

A: 访问 Admin UI (`http://your-host:8990/admin`)，查看：
- 凭据列表
- 失败计数
- 订阅等级
- 剩余额度

### Q: 凭据被禁用怎么办？

A:
1. 检查失败原因（额度用尽 / 认证失败）
2. 通过 Admin UI 点击「重置失败计数」
3. 或重启服务自动恢复

### Q: 如何提升并发能力？

A:
1. 增加凭据数量（最直接）
2. 使用 `balanced` 负载均衡模式
3. 优化网络（使用代理或更快的区域）

### Q: 凭据优先级如何工作？

A:
- `priority` 模式：优先使用低数字（高优先级）凭据
- `balanced` 模式：忽略优先级，轮询所有可用凭据

### Q: 代理配置不生效？

A:
1. 检查代理地址格式（`http://` 或 `socks5://`）
2. 验证代理认证信息
3. 查看日志确认代理是否被使用

### Q: 如何安全存储凭据？

A:
1. 使用环境变量（推荐）
2. 加密 credentials.json 文件
3. 限制文件权限（`chmod 600`）
4. 使用 Vault 等密钥管理服务

## 监控指标

通过 Prometheus metrics (`/metrics`) 监控：

- `credential_status{id="X"}` - 凭据状态（1=可用，0=禁用）
- `credential_failures{id="X"}` - 失败计数
- `credential_requests{id="X"}` - 请求计数
- `request_duration_seconds` - 请求延迟

## 故障排查

### 所有凭据都被禁用

**原因**: 可能是网络问题或 Bedrock 服务故障

**解决**:
1. 检查网络连接
2. 查看日志 `docker logs kiro-rs`
3. 重启服务恢复凭据状态

### 请求延迟高

**原因**: 凭据不足或区域选择不当

**解决**:
1. 增加凭据数量
2. 选择更近的区域
3. 启用 HTTP/2（已默认启用）
4. 使用代理加速

### 频繁 429 错误

**原因**: 单凭据请求过快

**解决**:
1. 增加凭据数量分散负载
2. 使用 `balanced` 模式
3. 客户端实现限流

## 相关文档

- [配置文件说明](./CONFIG.md)
- [Admin API 文档](./ADMIN_API.md)
- [性能优化指南](./PERFORMANCE.md)
