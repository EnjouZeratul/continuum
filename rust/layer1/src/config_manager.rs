//! 配置管理模块
//!
//! 多环境配置、热更新、验证、多提供商管理。

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API 密钥
    pub api_key: String,
    /// API 基础 URL
    pub base_url: String,
    /// 默认模型
    pub model: String,
    /// API 格式: "anthropic" | "openai" | "google"
    #[serde(default = "default_api_format")]
    pub api_format: String,
    /// 默认最大 token 数
    #[serde(default = "default_max_tokens")]
    pub default_max_tokens: u32,
    /// 默认温度
    #[serde(default = "default_temperature")]
    pub default_temperature: f32,
}

fn default_api_format() -> String {
    "openai".to_string()
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_temperature() -> f32 {
    0.7
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            model: "claude-sonnet-4-6".to_string(),
            api_format: default_api_format(),
            default_max_tokens: 4096,
            default_temperature: 0.7,
        }
    }
}

/// 全局设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// 会话自动保存
    #[serde(default = "default_true")]
    pub session_auto_save: bool,
    /// 会话最大历史数
    #[serde(default = "default_session_max_history")]
    pub session_max_history: usize,
    /// 检查点启用
    #[serde(default = "default_true")]
    pub checkpoint_enabled: bool,
    /// 检查点间隔（秒）
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval_sec: u32,
    /// 审计日志启用
    #[serde(default = "default_true")]
    pub audit_enabled: bool,
    /// MCP 启用
    #[serde(default)]
    pub mcp_enabled: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            session_auto_save: true,
            session_max_history: 100,
            checkpoint_enabled: true,
            checkpoint_interval_sec: 60,
            audit_enabled: true,
            mcp_enabled: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_session_max_history() -> usize {
    100
}

fn default_checkpoint_interval() -> u32 {
    60
}

/// 配置管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManager {
    /// 当前激活的提供商
    #[serde(default = "default_provider")]
    pub active_provider: String,
    /// 提供商注册表
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// 全局设置
    #[serde(default)]
    pub settings: GlobalSettings,
    /// 其他配置项
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

fn default_provider() -> String {
    "anthropic".to_string()
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self {
            active_provider: "anthropic".to_string(),
            providers: HashMap::new(),
            settings: GlobalSettings::default(),
            extra: HashMap::new(),
        }
    }
}

impl ConfigManager {
    /// 创建新的配置管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // 读取环境变量
        if let Ok(provider) = std::env::var("CONTINUUM_PROVIDER") {
            config.active_provider = provider;
        }

        // 读取 API 密钥并添加到当前提供商
        if let Ok(api_key) = std::env::var("CONTINUUM_API_KEY") {
            let provider_name = config.active_provider.clone();
            let provider_config = config.providers.entry(provider_name).or_default();
            provider_config.api_key = api_key;
        }

        // 读取基础 URL
        if let Ok(base_url) = std::env::var("CONTINUUM_BASE_URL") {
            let provider_name = config.active_provider.clone();
            let provider_config = config.providers.entry(provider_name).or_default();
            provider_config.base_url = base_url;
        }

        // 读取模型
        if let Ok(model) = std::env::var("CONTINUUM_MODEL") {
            let provider_name = config.active_provider.clone();
            let provider_config = config.providers.entry(provider_name).or_default();
            provider_config.model = model;
        }

        // 读取检查点配置
        if let Ok(val) = std::env::var("CONTINUUM_CHECKPOINT_ENABLED") {
            if let Ok(enabled) = val.parse::<bool>() {
                config.settings.checkpoint_enabled = enabled;
            }
        }

        if let Ok(val) = std::env::var("CONTINUUM_AUDIT_ENABLED") {
            if let Ok(enabled) = val.parse::<bool>() {
                config.settings.audit_enabled = enabled;
            }
        }

        config
    }

    /// 从文件加载配置
    pub async fn load_from_file(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(path).await?;
        let loaded: ConfigManager = toml::from_str(&content)?;

        // 合并配置
        self.merge(loaded);
        Ok(())
    }

    /// 从文件同步加载（用于非 async 环境）
    pub fn load_from_file_sync(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let loaded: ConfigManager = toml::from_str(&content)?;
        self.merge(loaded);
        Ok(())
    }

    /// 合并配置 (优先级: env > file > default，所以 file 合入 self)
    pub fn merge(&mut self, other: ConfigManager) {
        // 合并提供商配置
        for (name, provider) in other.providers {
            // 只合并有 API 密钥的提供商
            if !provider.api_key.is_empty() {
                self.providers.insert(name, provider);
            }
        }

        // 合并设置（保留已从环境变量读取的值）
        if other.settings.session_max_history > 0 {
            self.settings.session_max_history = other.settings.session_max_history;
        }
        if other.settings.checkpoint_interval_sec > 0 {
            self.settings.checkpoint_interval_sec = other.settings.checkpoint_interval_sec;
        }

        // 合并其他配置
        self.extra.extend(other.extra);

        // 设置活跃提供商（如果指定的提供商存在）
        if !other.active_provider.is_empty() && self.providers.contains_key(&other.active_provider)
        {
            self.active_provider = other.active_provider;
        }
    }

    /// 获取默认配置路径
    pub fn default_config_path() -> PathBuf {
        // 用户级配置: ~/.continuum/config.toml
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".continuum").join("config.toml")
    }

    /// 获取项目级配置路径
    pub fn project_config_path() -> PathBuf {
        PathBuf::from(".continuum").join("config.toml")
    }

    /// 加载完整配置（环境变量 + 项目级 + 用户级）
    pub async fn load_full() -> Result<Self> {
        // 1. 从默认配置开始
        let mut config = Self::new();

        // 2. 加载用户级配置
        let user_path = Self::default_config_path();
        config.load_from_file(&user_path).await?;

        // 3. 加载项目级配置（覆盖用户级）
        let project_path = Self::project_config_path();
        config.load_from_file(&project_path).await?;

        // 4. 从环境变量加载（最高优先级，覆盖文件配置）
        let env_config = Self::from_env();
        config.merge_env(env_config);

        Ok(config)
    }

    /// 合并环境变量配置（最高优先级）
    fn merge_env(&mut self, env: ConfigManager) {
        // 环境变量配置优先级最高，直接覆盖
        if !env.active_provider.is_empty() {
            self.active_provider = env.active_provider;
        }

        // 合并提供商（环境变量的提供商直接覆盖）
        for (name, provider) in env.providers {
            self.providers.insert(name, provider);
        }

        // 合并设置
        self.settings.audit_enabled = env.settings.audit_enabled;
        self.settings.checkpoint_enabled = env.settings.checkpoint_enabled;
    }

    /// 切换提供商
    pub fn use_provider(&mut self, name: &str) -> Result<()> {
        if !self.providers.contains_key(name) {
            return Err(anyhow!(
                "Provider '{}' not found. Use 'config add-provider' first.",
                name
            ));
        }
        self.active_provider = name.to_string();
        Ok(())
    }

    /// 获取当前提供商配置
    pub fn current(&self) -> Result<&ProviderConfig> {
        self.providers
            .get(&self.active_provider)
            .ok_or_else(|| anyhow!("No provider '{}' configured", self.active_provider))
    }

    /// 添加提供商
    pub fn add_provider(&mut self, name: &str, config: ProviderConfig) {
        self.providers.insert(name.to_string(), config);
    }

    /// 列出所有提供商
    pub fn list_providers(&self) -> Vec<&String> {
        self.providers.keys().collect()
    }

    /// 获取配置值
    pub fn get(&self, key: &str) -> Option<&String> {
        self.extra.get(key)
    }

    /// 设置配置值
    pub fn set(&mut self, key: String, value: String) {
        self.extra.insert(key, value);
    }

    /// 保存配置到文件
    pub async fn save(&self, path: &Path) -> Result<()> {
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = toml::to_string_pretty(&self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// 同步保存配置到文件
    pub fn save_sync(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 解析环境变量引用 ${VAR_NAME}
    pub fn resolve_env_refs(&mut self) {
        // 解析提供商配置中的环境变量引用
        for provider in self.providers.values_mut() {
            provider.api_key = Self::resolve_env_string(&provider.api_key);
            provider.base_url = Self::resolve_env_string(&provider.base_url);
            provider.model = Self::resolve_env_string(&provider.model);
        }

        // 解析其他配置中的环境变量引用
        for value in self.extra.values_mut() {
            *value = Self::resolve_env_string(value);
        }
    }

    /// 解析单个字符串中的环境变量引用
    fn resolve_env_string(s: &str) -> String {
        let mut result = s.to_string();
        // 查找 ${VAR_NAME} 并替换
        while let Some(start) = result.find("${") {
            if let Some(end) = result[start..].find('}') {
                let var_name = &result[start + 2..start + end];
                if let Ok(val) = std::env::var(var_name) {
                    result.replace_range(start..start + end + 1, &val);
                } else {
                    // 环境变量不存在，移除引用标记
                    result.replace_range(start..start + end + 1, "");
                }
            } else {
                break;
            }
        }
        result
    }

    /// 初始化默认配置文件
    pub fn init_default_config(&self) -> Result<PathBuf> {
        let path = Self::default_config_path();

        if path.exists() {
            return Err(anyhow!("Config file already exists at {:?}", path));
        }

        // 创建默认配置
        let default_config = Self {
            active_provider: "anthropic".to_string(),
            providers: {
                let mut map = HashMap::new();
                map.insert(
                    "anthropic".to_string(),
                    ProviderConfig {
                        api_key: "${ANTHROPIC_API_KEY}".to_string(),
                        base_url: "https://api.anthropic.com/v1".to_string(),
                        model: "claude-sonnet-4-6".to_string(),
                        api_format: "anthropic".to_string(),
                        default_max_tokens: 4096,
                        default_temperature: 0.7,
                    },
                );
                map.insert(
                    "openai".to_string(),
                    ProviderConfig {
                        api_key: "${OPENAI_API_KEY}".to_string(),
                        base_url: "https://api.openai.com/v1".to_string(),
                        model: "gpt-4".to_string(),
                        api_format: "openai".to_string(),
                        default_max_tokens: 4096,
                        default_temperature: 0.7,
                    },
                );
                map.insert(
                    "gemini".to_string(),
                    ProviderConfig {
                        api_key: "${GEMINI_API_KEY}".to_string(),
                        base_url: "https://generativelanguage.googleapis.com/v1".to_string(),
                        model: "gemini-pro".to_string(),
                        api_format: "google".to_string(),
                        default_max_tokens: 4096,
                        default_temperature: 0.7,
                    },
                );
                map
            },
            settings: GlobalSettings::default(),
            extra: HashMap::new(),
        };

        default_config.save_sync(&path)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_manager_creation() {
        let config = ConfigManager::new();
        assert_eq!(config.active_provider, "anthropic");
    }

    #[test]
    fn test_provider_config_default() {
        let provider = ProviderConfig::default();
        assert_eq!(provider.default_max_tokens, 4096);
        assert_eq!(provider.default_temperature, 0.7);
    }

    #[test]
    fn test_global_settings_default() {
        let settings = GlobalSettings::default();
        assert!(settings.session_auto_save);
        assert!(settings.checkpoint_enabled);
    }

    #[test]
    fn test_add_provider() {
        let mut config = ConfigManager::new();
        let provider = ProviderConfig {
            api_key: "test_key".to_string(),
            base_url: "https://test.api.com".to_string(),
            model: "test-model".to_string(),
            api_format: "openai".to_string(),
            default_max_tokens: 8192,
            default_temperature: 0.5,
        };
        config.add_provider("test", provider);
        assert!(config.providers.contains_key("test"));
    }

    #[test]
    fn test_use_provider() {
        let mut config = ConfigManager::new();
        let provider = ProviderConfig {
            api_key: "test_key".to_string(),
            base_url: "https://test.api.com".to_string(),
            model: "test-model".to_string(),
            api_format: "openai".to_string(),
            default_max_tokens: 4096,
            default_temperature: 0.7,
        };
        config.add_provider("test", provider);

        config.use_provider("test").unwrap();
        assert_eq!(config.active_provider, "test");
    }

    #[test]
    fn test_use_provider_not_found() {
        let mut config = ConfigManager::new();
        let result = config.use_provider("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_env_string() {
        std::env::set_var("TEST_VAR", "test_value");
        let resolved = ConfigManager::resolve_env_string("${TEST_VAR}");
        assert_eq!(resolved, "test_value");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_set_get_config() {
        let mut config = ConfigManager::new();
        config.set("test_key".to_string(), "test_value".to_string());
        assert_eq!(config.get("test_key"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_list_providers() {
        let mut config = ConfigManager::new();
        let provider = ProviderConfig {
            api_key: "key1".to_string(),
            base_url: "url1".to_string(),
            model: "model1".to_string(),
            api_format: "openai".to_string(),
            default_max_tokens: 4096,
            default_temperature: 0.7,
        };
        config.add_provider("provider1", provider);

        let list = config.list_providers();
        assert!(list.contains(&&"provider1".to_string()));
    }

    #[test]
    fn test_config_serialization() {
        let config = ConfigManager::new();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("active_provider"));
    }
}
