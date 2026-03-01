// Cache key generation utilities - 缓存 Key 生成工具

use crate::anthropic::types::MessagesRequest;
use sha2::{Digest, Sha256};

/// 生成缓存 Key（基于请求参数的 SHA256 哈希）
///
/// 包含所有影响响应的参数：
/// - model
/// - messages
/// - system (如果存在)
/// - tools (如果存在)
/// - max_tokens
/// - temperature (如果存在)
/// - top_p (如果存在)
/// - top_k (如果存在)
/// - thinking (如果存在)
///
/// # 返回
/// 格式：`kiro:cache:v1:{hash}`
pub fn generate_cache_key(request: &MessagesRequest) -> String {
    let mut hasher = Sha256::new();

    // 核心参数
    hasher.update(request.model.as_bytes());
    hasher.update(b"|messages:");
    if let Ok(messages_json) = serde_json::to_string(&request.messages) {
        hasher.update(messages_json.as_bytes());
    }

    // 可选参数（影响响应）
    if let Some(system) = &request.system {
        hasher.update(b"|system:");
        if let Ok(system_json) = serde_json::to_string(system) {
            hasher.update(system_json.as_bytes());
        }
    }

    if let Some(tools) = &request.tools {
        hasher.update(b"|tools:");
        if let Ok(tools_json) = serde_json::to_string(tools) {
            hasher.update(tools_json.as_bytes());
        }
    }

    hasher.update(b"|max_tokens:");
    hasher.update(request.max_tokens.to_string().as_bytes());

    // temperature, top_p, top_k 等参数（如果 MessagesRequest 有这些字段）
    // 注意：当前 MessagesRequest 结构体中没有这些字段，如果后续添加需要更新

    if let Some(thinking) = &request.thinking {
        hasher.update(b"|thinking:");
        if let Ok(thinking_json) = serde_json::to_string(thinking) {
            hasher.update(thinking_json.as_bytes());
        }
    }

    let hash = hex::encode(hasher.finalize());
    format!("kiro:cache:v1:{}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anthropic::types::{Message, SystemMessage, Thinking, Tool};
    use std::collections::HashMap;

    #[test]
    fn test_generate_cache_key_consistency() {
        let request1 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request2 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request1);
        let key2 = generate_cache_key(&request2);

        assert_eq!(key1, key2, "相同请求应生成相同 Key");
        assert!(key1.starts_with("kiro:cache:v1:"), "Key 应有正确前缀");
    }

    #[test]
    fn test_generate_cache_key_different_params() {
        let request1 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request2 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 2048, // 不同的 max_tokens
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request1);
        let key2 = generate_cache_key(&request2);

        assert_ne!(key1, key2, "不同参数应生成不同 Key");
    }

    #[test]
    fn test_generate_cache_key_with_system() {
        // 测试包含 system 的请求
        let request_without_system = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request_with_system = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: Some(vec![SystemMessage {
                text: "You are a helpful assistant".to_string(),
            }]),
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request_without_system);
        let key2 = generate_cache_key(&request_with_system);

        assert_ne!(key1, key2, "包含 system 的请求应生成不同 Key");
    }

    #[test]
    fn test_generate_cache_key_with_tools() {
        // 测试包含 tools 的请求
        let request_without_tools = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request_with_tools = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: Some(vec![Tool {
                tool_type: None,
                name: "get_weather".to_string(),
                description: "Get weather info".to_string(),
                input_schema: HashMap::new(),
                max_uses: None,
            }]),
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request_without_tools);
        let key2 = generate_cache_key(&request_with_tools);

        assert_ne!(key1, key2, "包含 tools 的请求应生成不同 Key");
    }

    #[test]
    fn test_generate_cache_key_with_thinking() {
        // 测试包含 thinking 的请求
        let request_without_thinking = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request_with_thinking = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: Some(Thinking {
                thinking_type: "enabled".to_string(),
                budget_tokens: 10000,
            }),
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request_without_thinking);
        let key2 = generate_cache_key(&request_with_thinking);

        assert_ne!(key1, key2, "包含 thinking 的请求应生成不同 Key");
    }

    #[test]
    fn test_cache_key_format() {
        // 测试 Key 格式验证
        let request = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Test"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key = generate_cache_key(&request);

        // 验证前缀
        assert!(key.starts_with("kiro:cache:v1:"), "Key 应有正确前缀");

        // 验证格式：kiro:cache:v1:{64位十六进制哈希}
        let parts: Vec<&str> = key.split(':').collect();
        assert_eq!(parts.len(), 4, "Key 应有 4 个部分");
        assert_eq!(parts[0], "kiro");
        assert_eq!(parts[1], "cache");
        assert_eq!(parts[2], "v1");

        // 验证哈希长度（SHA256 = 64 个十六进制字符）
        let hash = parts[3];
        assert_eq!(hash.len(), 64, "SHA256 哈希应为 64 个字符");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "哈希应只包含十六进制字符");
    }

    #[test]
    fn test_cache_key_with_special_characters() {
        // 测试特殊字符处理
        let request = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello 世界! @#$%^&*()"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key = generate_cache_key(&request);

        // 特殊字符应被正确哈希，不影响 Key 格式
        assert!(key.starts_with("kiro:cache:v1:"));
        assert_eq!(key.len(), "kiro:cache:v1:".len() + 64);
    }

    #[test]
    fn test_cache_key_different_models() {
        // 测试不同模型生成不同 Key
        let request1 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request2 = MessagesRequest {
            model: "claude-opus-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request1);
        let key2 = generate_cache_key(&request2);

        assert_ne!(key1, key2, "不同模型应生成不同 Key");
    }

    #[test]
    fn test_cache_key_different_messages() {
        // 测试不同消息内容生成不同 Key
        let request1 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let request2 = MessagesRequest {
            model: "claude-sonnet-4".to_string(),
            max_tokens: 1024,
            messages: vec![Message {
                role: "user".to_string(),
                content: serde_json::json!("World"),
            }],
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            output_config: None,
            metadata: None,
        };

        let key1 = generate_cache_key(&request1);
        let key2 = generate_cache_key(&request2);

        assert_ne!(key1, key2, "不同消息内容应生成不同 Key");
    }
}
