//! 配置加载器

use anyhow::Result;
use sh_core::layer1::ConfigManager;
use std::path::Path;

/// CLI 配置加载器
pub struct ConfigLoader;

impl ConfigLoader {
    /// 从文件加载配置
    pub fn from_file(path: &Path) -> Result<ConfigManager> {
        let mut config = ConfigManager::new();
        config.load_from_file_sync(path)?;
        Ok(config)
    }

    /// 从环境加载配置
    pub fn from_env() -> ConfigManager {
        ConfigManager::from_env()
    }

    /// 加载默认配置
    pub fn load_default() -> ConfigManager {
        ConfigManager::new()
    }

    /// 加载完整配置（默认 + 用户级 + 项目级 + 环境变量）
    pub async fn load_full() -> Result<ConfigManager> {
        ConfigManager::load_full().await
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigLoader;

    #[test]
    fn test_load_default_returns_real_config_manager() {
        let config = ConfigLoader::load_default();
        assert_eq!(config.active_provider, "anthropic");
    }

    #[test]
    fn test_from_env_reads_continuum_provider() {
        std::env::set_var("CONTINUUM_PROVIDER", "openai");
        let config = ConfigLoader::from_env();
        assert_eq!(config.active_provider, "openai");
        std::env::remove_var("CONTINUUM_PROVIDER");
    }

    #[test]
    fn test_from_file_loads_existing_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
active_provider = "openai"

[providers.openai]
api_key = "test-key"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
"#,
        )
        .unwrap();

        let config = ConfigLoader::from_file(&path).unwrap();
        assert_eq!(config.active_provider, "openai");
        assert!(config.providers.contains_key("openai"));
    }
}
