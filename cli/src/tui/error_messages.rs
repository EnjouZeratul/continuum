//! 用户友好的错误消息模块
//!
//! 将技术化错误消息转换为友好提示，包含解决建议。

/// 用户友好的错误消息
pub struct FriendlyError {
    /// 错误标题
    pub title: String,
    /// 错误描述
    pub description: String,
    /// 解决建议
    pub suggestions: Vec<String>,
}

impl FriendlyError {
    /// 格式化为显示字符串
    pub fn format(&self) -> String {
        let mut result = format!("{}\n\n{}\n", self.title, self.description);

        if !self.suggestions.is_empty() {
            result.push_str("\n解决方案:\n");
            for (i, suggestion) in self.suggestions.iter().enumerate() {
                result.push_str(&format!("  {}. {}\n", i + 1, suggestion));
            }
        }

        result
    }
}

/// 错误消息友好化转换器
pub struct ErrorFriendlyizer;

impl ErrorFriendlyizer {
    /// 转换配置错误
    pub fn config_error(error_type: &str, context: &str) -> FriendlyError {
        match error_type {
            "no_providers" => FriendlyError {
                title: "需要配置 API 提供商".to_string(),
                description: "Continuum 需要配置至少一个 LLM 提供商才能使用.".to_string(),
                suggestions: vec![
                    "运行 `continuum config add-provider anthropic --key YOUR_API_KEY`".to_string(),
                    "或设置环境变量 `CONTINUUM_API_KEY`".to_string(),
                    "查看帮助: `continuum config --help`".to_string(),
                ],
            },
            "api_key_missing" => FriendlyError {
                title: "API 密钥未设置".to_string(),
                description: format!("提供商 '{}' 需要 API 密钥才能连接.", context),
                suggestions: vec![
                    format!(
                        "运行 `continuum config set provider.{}.api_key YOUR_KEY`",
                        context
                    ),
                    "或设置对应的环境变量".to_string(),
                    "获取密钥: Anthropic → console.anthropic.com, OpenAI → platform.openai.com"
                        .to_string(),
                ],
            },
            "provider_not_found" => FriendlyError {
                title: "找不到指定的提供商".to_string(),
                description: format!("提供商 '{}' 未在配置中找到.", context),
                suggestions: vec![
                    "检查配置文件中的提供商名称".to_string(),
                    "运行 `continuum config list` 查看可用提供商".to_string(),
                    "使用 `continuum config add-provider` 添加新提供商".to_string(),
                ],
            },
            "config_load_failed" => FriendlyError {
                title: "配置加载失败".to_string(),
                description: "无法读取配置文件，可能是文件损坏或格式错误.".to_string(),
                suggestions: vec![
                    "检查配置文件格式是否正确".to_string(),
                    "运行 `continuum config validate` 验证配置".to_string(),
                    "尝试重新创建配置: `continuum config init`".to_string(),
                ],
            },
            _ => FriendlyError {
                title: "配置错误".to_string(),
                description: context.to_string(),
                suggestions: vec![
                    "运行 `continuum config --help` 查看配置帮助".to_string(),
                    "检查配置文件内容".to_string(),
                ],
            },
        }
    }

    /// 转换网络错误
    pub fn network_error(error_type: &str) -> FriendlyError {
        match error_type {
            "connection_timeout" => FriendlyError {
                title: "连接超时".to_string(),
                description: "无法连接到 API 服务器，请检查网络连接.".to_string(),
                suggestions: vec![
                    "检查网络连接是否正常".to_string(),
                    "尝试使用代理或 VPN".to_string(),
                    "稍后重试".to_string(),
                ],
            },
            "connection_refused" => FriendlyError {
                title: "连接被拒绝".to_string(),
                description: "API 服务器拒绝了连接请求.".to_string(),
                suggestions: vec![
                    "检查 API 密钥是否正确".to_string(),
                    "检查 base_url 配置是否正确".to_string(),
                    "确认服务是否可用".to_string(),
                ],
            },
            "ssl_error" => FriendlyError {
                title: "SSL/TLS 错误".to_string(),
                description: "安全连接建立失败.".to_string(),
                suggestions: vec![
                    "检查系统时间是否正确".to_string(),
                    "更新系统 SSL 证书".to_string(),
                    "检查是否需要代理配置".to_string(),
                ],
            },
            _ => FriendlyError {
                title: "网络连接问题".to_string(),
                description: "无法与服务器通信，请稍后重试.".to_string(),
                suggestions: vec![
                    "检查网络连接".to_string(),
                    "检查 API 密钥配置".to_string(),
                    "查看日志获取详细信息".to_string(),
                ],
            },
        }
    }

    /// 转换 API 错误
    pub fn api_error(error_type: &str, context: &str) -> FriendlyError {
        match error_type {
            "rate_limit" => FriendlyError {
                title: "请求频率超限".to_string(),
                description: "API 请求次数超出限制，请稍后重试.".to_string(),
                suggestions: vec![
                    "等待几分钟后重试".to_string(),
                    "考虑升级 API 计划".to_string(),
                    "减少并发请求数量".to_string(),
                ],
            },
            "invalid_request" => FriendlyError {
                title: "请求格式错误".to_string(),
                description: "发送的请求格式不符合 API 要求.".to_string(),
                suggestions: vec![
                    "检查输入内容是否过长".to_string(),
                    "检查特殊字符处理".to_string(),
                    "查看 API 文档了解限制".to_string(),
                ],
            },
            "model_not_found" => FriendlyError {
                title: "模型不可用".to_string(),
                description: format!("模型 '{}' 可能不存在或您无权访问.", context),
                suggestions: vec![
                    "运行 `/model` 查看可用模型".to_string(),
                    "检查 API 计划是否包含该模型".to_string(),
                    "切换到其他模型".to_string(),
                ],
            },
            "context_length" => FriendlyError {
                title: "内容超出长度限制".to_string(),
                description: "对话内容超出了模型的最大处理长度.".to_string(),
                suggestions: vec![
                    "使用 `/clear` 清空历史记录".to_string(),
                    "开始新会话: `/new`".to_string(),
                    "精简输入内容".to_string(),
                ],
            },
            "authentication" => FriendlyError {
                title: "API 认证失败".to_string(),
                description: "API 密钥无效或已过期.".to_string(),
                suggestions: vec![
                    "检查 API 密钥是否正确".to_string(),
                    "确认密钥未过期".to_string(),
                    "重新获取 API 密钥".to_string(),
                ],
            },
            "insufficient_quota" => FriendlyError {
                title: "配额不足".to_string(),
                description: "API 使用配额已用尽.".to_string(),
                suggestions: vec![
                    "检查账户余额".to_string(),
                    "升级 API 计划".to_string(),
                    "等待配额重置（通常按月）".to_string(),
                ],
            },
            _ => FriendlyError {
                title: "API 调用失败".to_string(),
                description: context.to_string(),
                suggestions: vec![
                    "稍后重试".to_string(),
                    "检查配置".to_string(),
                    "运行 `/tokens` 查看 token 使用情况".to_string(),
                ],
            },
        }
    }

    /// 从原始错误字符串提取友好消息
    pub fn from_raw_error(raw: &str) -> FriendlyError {
        // 检测错误类型
        let lower = raw.to_lowercase();

        if lower.contains("no providers") || lower.contains("provider not found") {
            Self::config_error("provider_not_found", raw)
        } else if lower.contains("api key") || lower.contains("authentication") {
            Self::api_error("authentication", raw)
        } else if lower.contains("rate limit") || lower.contains("too many requests") {
            Self::api_error("rate_limit", raw)
        } else if lower.contains("timeout") || lower.contains("connection") {
            Self::network_error("connection_timeout")
        } else if lower.contains("model") || lower.contains("not found") {
            Self::api_error("model_not_found", raw)
        } else if lower.contains("context") || lower.contains("length") || lower.contains("token") {
            Self::api_error("context_length", raw)
        } else if lower.contains("quota")
            || lower.contains("insufficient")
            || lower.contains("balance")
        {
            Self::api_error("insufficient_quota", raw)
        } else {
            FriendlyError {
                title: "操作遇到问题".to_string(),
                description: raw.to_string(),
                suggestions: vec![
                    "查看帮助: `/help`".to_string(),
                    "运行 `/debug` 查看详细信息".to_string(),
                    "稍后重试".to_string(),
                ],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_no_providers() {
        let error = ErrorFriendlyizer::config_error("no_providers", "");
        assert!(error.title.contains("配置"));
        assert!(!error.suggestions.is_empty());
    }

    #[test]
    fn test_api_error_rate_limit() {
        let error = ErrorFriendlyizer::api_error("rate_limit", "");
        assert!(error.title.contains("频率"));
        assert!(error.suggestions.len() >= 2);
    }

    #[test]
    fn test_network_error() {
        let error = ErrorFriendlyizer::network_error("connection_timeout");
        assert!(error.title.contains("连接"));
    }

    #[test]
    fn test_format_output() {
        let error = FriendlyError {
            title: "测试错误".to_string(),
            description: "这是一个测试".to_string(),
            suggestions: vec!["建议1".to_string(), "建议2".to_string()],
        };
        let formatted = error.format();
        assert!(formatted.contains("测试错误"));
        assert!(formatted.contains("解决方案"));
        assert!(formatted.contains("建议1"));
    }

    #[test]
    fn test_from_raw_error() {
        let error = ErrorFriendlyizer::from_raw_error("rate limit exceeded");
        assert!(error.title.contains("频率"));

        let error = ErrorFriendlyizer::from_raw_error("API key invalid");
        assert!(error.title.contains("认证"));
    }
}
