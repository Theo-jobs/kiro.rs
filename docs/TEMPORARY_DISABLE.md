# 凭据临时禁用与自动恢复机制

## 概述

为了避免临时容量不足导致所有凭据被永久禁用，系统实现了智能的临时禁用与自动恢复机制。

## 禁用类型

### 1. 临时禁用（自动恢复）

**触发条件**：
- 429 Too Many Requests + `INSUFFICIENT_MODEL_CAPACITY`

**特点**：
- ✅ 10 分钟后自动恢复
- ✅ 无需人工干预
- ✅ 避免系统死锁

**日志示例**：
```
WARN  凭据 #21 遇到临时容量不足（INSUFFICIENT_MODEL_CAPACITY），临时禁用 10 分钟
INFO  已切换到凭据 #12（优先级 0）
...（10 分钟后）
INFO  凭据 #21 冷却时间已过（10分钟），自动恢复为可用状态
```

### 2. 永久禁用（需手动恢复）

**触发条件**：
- 401/403 认证失败
- 402 额度用尽（`MONTHLY_REQUEST_COUNT`）
- 连续失败 3 次（其他错误）

**特点**：
- ⚠️ 需要通过 Admin API 手动重置
- ⚠️ 不会自动恢复

**日志示例**：
```
ERROR 凭据 #18 额度已用尽（MONTHLY_REQUEST_COUNT），已被禁用
ERROR 凭据 #19 已连续失败 3 次，已被禁用
```

## 工作流程

### 场景 1：临时容量不足

```
时间线：
14:00 - 凭据 #18 遇到 429 容量不足 → 临时禁用
14:00 - 系统切换到凭据 #19
14:05 - 凭据 #19 遇到 429 容量不足 → 临时禁用
14:05 - 系统切换到凭据 #20
...
14:10 - 凭据 #18 冷却时间到 → 自动恢复 ✅
14:15 - 凭据 #19 冷却时间到 → 自动恢复 ✅
```

### 场景 2：额度用尽

```
时间线：
14:00 - 凭据 #18 额度用尽 → 永久禁用
14:00 - 系统切换到凭据 #19
...
（需要手动重置凭据 #18）
```

## 配置参数

### 冷却时间

```rust
// src/kiro/token_manager.rs
const TEMPORARY_DISABLE_COOLDOWN_MINUTES: i64 = 10;
```

**默认值**：10 分钟

**修改方法**：
1. 编辑 `src/kiro/token_manager.rs`
2. 修改 `TEMPORARY_DISABLE_COOLDOWN_MINUTES` 常量
3. 重新编译部署

## Admin API 操作

### 查看凭据状态

```bash
curl -H "x-api-key: your-admin-key" \
  http://192.168.50.200:8990/api/admin/credentials
```

**响应示例**：
```json
{
  "credentials": [
    {
      "id": 21,
      "disabled": true,
      "disabledReason": "temporary_capacity_issue",
      "disabledAt": "2026-03-03T14:00:00Z"
    }
  ]
}
```

### 手动重置凭据

```bash
curl -X POST \
  -H "x-api-key: your-admin-key" \
  http://192.168.50.200:8990/api/admin/credentials/21/reset
```

## 监控建议

### 1. 查看临时禁用日志

```bash
docker logs kiro-rs 2>&1 | grep "临时禁用"
```

### 2. 查看自动恢复日志

```bash
docker logs kiro-rs 2>&1 | grep "自动恢复"
```

### 3. Prometheus 指标

```bash
curl http://192.168.50.200:8990/metrics | grep credential_status
```

## 常见问题

### Q1: 所有凭据都被临时禁用了怎么办？

**A**: 等待 10 分钟，系统会自动恢复。或者手动重置：

```bash
# 重置所有凭据
for id in {1..11}; do
  curl -X POST -H "x-api-key: your-admin-key" \
    http://192.168.50.200:8990/api/admin/credentials/$id/reset
done
```

### Q2: 如何区分临时禁用和永久禁用？

**A**: 查看 Admin UI 或日志：
- 临时禁用：日志显示 "临时禁用 10 分钟"
- 永久禁用：日志显示 "已被禁用"（无时间限制）

### Q3: 可以修改冷却时间吗？

**A**: 可以，修改 `TEMPORARY_DISABLE_COOLDOWN_MINUTES` 常量并重新编译。

建议值：
- 高峰期频繁：5-10 分钟
- 正常情况：10-15 分钟
- 低峰期：15-20 分钟

### Q4: 临时禁用会影响其他请求吗？

**A**: 不会。系统会立即切换到其他可用凭据，对用户透明。

## 技术细节

### 禁用原因枚举

```rust
enum DisabledReason {
    Manual,                      // 手动禁用
    TooManyFailures,             // 连续失败
    QuotaExceeded,               // 额度用尽
    TemporaryCapacityIssue,      // 临时容量不足（新增）
}
```

### 自动恢复逻辑

```rust
fn auto_recover_temporary_disabled(&self) {
    let now = Utc::now();
    let cooldown = Duration::minutes(10);

    for entry in entries.iter_mut() {
        if entry.disabled_reason == Some(DisabledReason::TemporaryCapacityIssue) {
            if now - entry.disabled_at >= cooldown {
                entry.disabled = false;  // 自动恢复
            }
        }
    }
}
```

### 触发时机

- 每次 `acquire_context()` 调用时自动检查
- 高频调用，确保及时恢复

## 版本历史

- **v2026.2.7**: 初始实现
  - 添加临时禁用机制
  - 10 分钟自动恢复
  - 区分临时/永久禁用
