# kiro.rs Anthropic Prompt Caching 实施指南

**创建时间**: 2026-03-02 02:23  
**基于**: Anthropic 官方文档 + Rust 实现示例

---

## ✅ 确认：完全可以添加！

**Anthropic Prompt Caching 是官方功能，完全支持！**

---

## 📋 核心信息

### 1. 功能特性
- ✅ **官方支持**: Anthropic 原生功能
- ✅ **零维护**: 无需额外基础设施
- ✅ **自动管理**: 缓存自动过期和刷新
- ✅ **成本节省**: 90% 折扣（缓存命中时）
- ✅ **性能提升**: 延迟降低 85%

### 2. 缓存规则
- **TTL**: 5 分钟（默认）或 1 小时（扩展）
- **最小大小**: 1024 tokens（~2000 字符）
- **位置**: system、messages、tools
- **计费**: 
  - 写入缓存: 25% 额外费用
  - 读取缓存: 90% 折扣

---

## 🔧 实施方案

### 方案 1: 自动缓存（推荐）⭐⭐⭐⭐⭐

**最简单！只需添加一个顶层字段：**

```rust
// 在请求中添加顶层 cache_control
{
    "model": "claude-opus-4-6",
    "max_tokens": 1024,
    "cache_control": {"type": "ephemeral"},  // 👈 添加这一行
    "system": "你是 Kiro...",
    "messages": [...]
}
```

**Anthropic 会自动缓存：**
- ✅ 整个 system prompt
- ✅ 所有 tools 定义
- ✅ 对话历史（最后一条消息之前）

**优点：**
- 零配置
- 自动优化
- 最大化命中率

---

### 方案 2: 手动缓存（精细控制）⭐⭐⭐⭐☆

**在特定位置添加 cache_control：**

```rust
use anthropic_async::types::common::CacheControl;

let req = MessagesCreateRequest {
    model: "claude-opus-4-6".into(),
    max_tokens: 1024,
    
    // System prompt 缓存
    system: Some(vec![
        ContentBlock::Text {
            text: "你是 Kiro，一个 AI 助手...".into(),
            cache_control: Some(CacheControl::ephemeral_1h()),  // 1小时缓存
        }
    ]),
    
    // Tools 缓存
    tools: Some(vec![
        Tool {
            name: "read_file".into(),
            description: "读取文件内容".into(),
            input_schema: {...},
            cache_control: Some(CacheControl::ephemeral_1h()),  // 1小时缓存
        }
    ]),
    
    // Messages 缓存（最后一条消息之前）
    messages: vec![
        Message {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: "历史对话...".into(),
                cache_control: Some(CacheControl::ephemeral_5m()),  // 5分钟缓存
            }],
        },
        Message {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: "当前问题".into(),
                cache_control: None,  // 不缓存当前消息
            }],
        }
    ],
};
```

---

## 🚀 kiro.rs 实施步骤

### Step 1: 添加依赖

```toml
# Cargo.toml
[dependencies]
anthropic-async = "0.x"  # 或使用你当前的 HTTP 客户端
```

### Step 2: 启用 Beta 功能

```rust
// 在初始化时启用
use anthropic_async::config::{AnthropicConfig, BetaFeature};

let config = AnthropicConfig::new()
    .with_beta_features([
        BetaFeature::PromptCaching20240731,      // 基础缓存
        BetaFeature::ExtendedCacheTtl20250411,   // 1小时 TTL
    ]);
```

### Step 3: 修改请求转换器

```rust
// src/anthropic/converter.rs

impl AnthropicRequest {
    pub fn to_kiro_request(&self) -> KiroRequest {
        let mut kiro_req = KiroRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            
            // 添加自动缓存
            cache_control: Some(CacheControl {
                type_: "ephemeral".into(),
            }),
            
            system: self.system.clone(),
            messages: self.messages.clone(),
            tools: self.tools.clone(),
            // ...
        };
        
        kiro_req
    }
}
```

### Step 4: 添加响应头检查

```rust
// 检查缓存命中情况
if let Some(usage) = response.usage {
    if let Some(cache_read) = usage.cache_read_input_tokens {
        println!("✅ 缓存命中: {} tokens", cache_read);
    }
    if let Some(cache_create) = usage.cache_creation_input_tokens {
        println!("📝 创建缓存: {} tokens", cache_create);
    }
}
```

---

## 📊 OpenClaw 场景优化

### 推荐配置

```rust
// 针对 OpenClaw 的最佳配置
{
    "cache_control": {"type": "ephemeral"},  // 自动缓存
    
    // System prompt (重复率 90%+)
    "system": "你是 Kiro...",  // 自动缓存 1 小时
    
    // Tools (重复率 95%+)
    "tools": [
        {"name": "read", ...},
        {"name": "write", ...},
        // ... 所有工具定义
    ],  // 自动缓存 1 小时
    
    // Messages (对话历史)
    "messages": [
        // 历史消息自动缓存 5 分钟
        {"role": "user", "content": "..."},
        {"role": "assistant", "content": "..."},
        // 当前消息不缓存
        {"role": "user", "content": "当前问题"}
    ]
}
```

---

## 💰 成本分析

### OpenClaw 场景预估

**假设：**
- 每天 100 次调用
- 平均每次请求：
  - System prompt: 2K tokens
  - Tools: 3K tokens
  - 对话历史: 5K tokens
  - 新消息: 1K tokens
  - 总计: 11K tokens

**无缓存成本：**
```
100 次 × 11K tokens × $15/1M = $16.50/天 = $495/月
```

**有缓存成本（80% 命中率）：**
```
首次请求（20次）:
  20 × 11K × $15/1M × 1.25 = $4.13/天  (写入缓存 +25%)

缓存命中（80次）:
  80 × 10K × $1.5/1M = $1.20/天  (90% 折扣)
  80 × 1K × $15/1M = $1.20/天   (新消息全价)

总计: $6.53/天 = $196/月
```

**节省：** $299/月（60% 成本降低）✅

---

## ⚠️ 注意事项

### 1. 最小缓存大小
- 必须 ≥ 1024 tokens
- 小于此值不会缓存
- OpenClaw 的 system + tools 通常 > 5K tokens ✅

### 2. 缓存位置
- ✅ System prompt 末尾
- ✅ Tools 列表末尾
- ✅ Messages 倒数第二条
- ❌ 最后一条 message（当前输入）

### 3. TTL 选择
- **5 分钟**: 对话场景（连续交互）
- **1 小时**: System prompt、Tools（长期稳定）

### 4. 监控指标
```rust
// 响应中的 usage 字段
{
    "input_tokens": 1000,
    "cache_creation_input_tokens": 5000,  // 创建缓存
    "cache_read_input_tokens": 5000,      // 读取缓存
    "output_tokens": 500
}
```

---

## 🎯 实施建议

### 阶段 1: 快速启用（1 小时）⭐⭐⭐⭐⭐
1. 添加顶层 `cache_control: {"type": "ephemeral"}`
2. 启用 Beta 功能
3. 部署测试

**收益：** 立即节省 50-60% 成本

### 阶段 2: 精细优化（可选，1 天）⭐⭐⭐☆☆
1. 分析缓存命中率
2. 调整 TTL 配置
3. 优化缓存位置

**收益：** 额外节省 5-10% 成本

---

## 📚 参考资料

1. **官方文档**: https://platform.claude.com/docs/en/build-with-claude/prompt-caching
2. **Rust 库**: https://lib.rs/crates/anthropic-async
3. **Cookbook**: https://github.com/anthropics/anthropic-cookbook/blob/main/misc/prompt_caching.ipynb

---

## ✅ 结论

**强烈建议立即实施！**

- ✅ 实施简单（1 小时）
- ✅ 零维护成本
- ✅ 立即节省 50-60% 成本
- ✅ 性能提升 85%
- ✅ 完美适配 OpenClaw 场景

**下一步：**
1. 禁用 Redis 缓存
2. 启用 Prompt Caching
3. 监控效果
4. 享受成本节省！🎉

