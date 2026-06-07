//! continuum config 子命令

use anyhow::Result;
use std::path::PathBuf;

use crate::cli::ConfigCmd;
use sh_core::layer1::{ConfigManager, ProviderConfig};

/// 获取配置文件路径
fn get_config_path() -> PathBuf {
    ConfigManager::default_config_path()
}

pub fn execute(cmd: ConfigCmd) -> Result<()> {
    match cmd {
        ConfigCmd::Show { key } => {
            show_config(key)?;
        }
        ConfigCmd::Set { key, value } => {
            set_config(key, value)?;
        }
        ConfigCmd::Get { key } => {
            get_config(key)?;
        }
        ConfigCmd::Init { force } => {
            init_config(force)?;
        }
        ConfigCmd::Keys => {
            show_keys();
        }
        ConfigCmd::List => {
            list_providers()?;
        }
        ConfigCmd::AddProvider {
            name,
            key,
            url,
            model,
        } => {
            add_provider(name, key, url, model)?;
        }
        ConfigCmd::Use { provider } => {
            use_provider(provider)?;
        }
    }

    Ok(())
}

/// 显示配置
fn show_config(key: Option<String>) -> Result<()> {
    let path = get_config_path();
    let mut config = ConfigManager::new();

    if path.exists() {
        config.load_from_file_sync(&path)?;
    }

    // 解析环境变量引用
    config.resolve_env_refs();

    if let Some(k) = key {
        // 显示特定配置项
        if k.starts_with("provider.") {
            // 显示提供商配置
            let parts: Vec<&str> = k.split('.').collect();
            if parts.len() >= 2 {
                let provider_name = parts[1];
                if let Some(provider) = config.providers.get(provider_name) {
                    if parts.len() == 2 {
                        println!("Provider '{}':", provider_name);
                        println!(
                            "  api_key: {}",
                            if provider.api_key.is_empty() {
                                "(not set)"
                            } else {
                                "(set)"
                            }
                        );
                        println!("  base_url: {}", provider.base_url);
                        println!("  model: {}", provider.model);
                        println!("  max_tokens: {}", provider.default_max_tokens);
                        println!("  temperature: {}", provider.default_temperature);
                    } else if parts.len() >= 3 {
                        let field = parts[2];
                        match field {
                            "api_key" => println!(
                                "{}",
                                if provider.api_key.is_empty() {
                                    "(not set)"
                                } else {
                                    "(set)"
                                }
                            ),
                            "base_url" => println!("{}", provider.base_url),
                            "model" => println!("{}", provider.model),
                            "max_tokens" => println!("{}", provider.default_max_tokens),
                            "temperature" => println!("{}", provider.default_temperature),
                            _ => println!("Unknown field: {}", field),
                        }
                    }
                } else {
                    println!("Provider '{}' not found", provider_name);
                }
            }
        } else if k.starts_with("settings.") {
            // 显示设置
            let parts: Vec<&str> = k.split('.').collect();
            if parts.len() >= 2 {
                let field = parts[1];
                match field {
                    "session_auto_save" => println!("{}", config.settings.session_auto_save),
                    "session_max_history" => println!("{}", config.settings.session_max_history),
                    "checkpoint_enabled" => println!("{}", config.settings.checkpoint_enabled),
                    "checkpoint_interval" => {
                        println!("{}s", config.settings.checkpoint_interval_sec)
                    }
                    "audit_enabled" => println!("{}", config.settings.audit_enabled),
                    "mcp_enabled" => println!("{}", config.settings.mcp_enabled),
                    _ => println!("Unknown setting: {}", field),
                }
            }
        } else {
            // 显示额外配置项
            match config.get(&k) {
                Some(v) => println!("{}", v),
                None => println!("Key '{}' not found", k),
            }
        }
    } else {
        // 显示所有配置
        println!("Active provider: {}", config.active_provider);
        println!();

        println!("Providers:");
        for (name, provider) in &config.providers {
            println!("  [{}]", name);
            println!("    base_url: {}", provider.base_url);
            println!("    model: {}", provider.model);
            println!("    max_tokens: {}", provider.default_max_tokens);
            println!("    temperature: {}", provider.default_temperature);
        }
        println!();

        println!("Settings:");
        println!("  session_auto_save: {}", config.settings.session_auto_save);
        println!(
            "  session_max_history: {}",
            config.settings.session_max_history
        );
        println!(
            "  checkpoint_enabled: {}",
            config.settings.checkpoint_enabled
        );
        println!(
            "  checkpoint_interval: {}s",
            config.settings.checkpoint_interval_sec
        );
        println!("  audit_enabled: {}", config.settings.audit_enabled);
        println!("  mcp_enabled: {}", config.settings.mcp_enabled);

        if !config.extra.is_empty() {
            println!();
            println!("Extra config:");
            for (k, v) in &config.extra {
                println!("  {}: {}", k, v);
            }
        }
    }

    Ok(())
}

/// 设置配置项
fn set_config(key: String, value: String) -> Result<()> {
    let path = get_config_path();
    let mut config = ConfigManager::new();

    if path.exists() {
        config.load_from_file_sync(&path)?;
    }

    // 保存用于打印的副本
    let key_display = key.clone();
    let value_display = value.clone();

    // 解析设置项
    if key.starts_with("settings.") {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() >= 2 {
            let field = parts[1];
            match field {
                "session_auto_save" => {
                    config.settings.session_auto_save = value.parse::<bool>()?;
                }
                "session_max_history" => {
                    config.settings.session_max_history = value.parse::<usize>()?;
                }
                "checkpoint_enabled" => {
                    config.settings.checkpoint_enabled = value.parse::<bool>()?;
                }
                "checkpoint_interval" => {
                    config.settings.checkpoint_interval_sec = value.parse::<u32>()?;
                }
                "audit_enabled" => {
                    config.settings.audit_enabled = value.parse::<bool>()?;
                }
                "mcp_enabled" => {
                    config.settings.mcp_enabled = value.parse::<bool>()?;
                }
                _ => {
                    println!("Unknown setting: {}", field);
                    return Ok(());
                }
            }
        }
    } else if key.starts_with("provider.") {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() >= 3 {
            let provider_name = parts[1];
            let field = parts[2];
            let provider = config
                .providers
                .entry(provider_name.to_string())
                .or_default();
            match field {
                "api_key" => provider.api_key = value,
                "base_url" => provider.base_url = value,
                "model" => provider.model = value,
                "max_tokens" => provider.default_max_tokens = value.parse::<u32>()?,
                "temperature" => provider.default_temperature = value.parse::<f32>()?,
                _ => {
                    println!("Unknown field: {}", field);
                    return Ok(());
                }
            }
        }
    } else {
        config.set(key, value);
    }

    config.save_sync(&path)?;
    println!("Set {} = {}", key_display, value_display);

    Ok(())
}

/// 获取配置项
fn get_config(key: String) -> Result<()> {
    show_config(Some(key))
}

/// 初始化配置文件
fn init_config(force: bool) -> Result<()> {
    let path = get_config_path();

    if path.exists() && !force {
        println!("Config file already exists at {:?}", path);
        println!("Use --force to overwrite");
        return Ok(());
    }

    let config = ConfigManager::new();
    config.save_sync(&path)?;
    println!("Initialized config at {:?}", path);
    println!();
    println!("Default providers configured:");
    println!("  anthropic - model: claude-sonnet-4-6");
    println!("  openai - model: gpt-4");
    println!("  gemini - model: gemini-pro");
    println!();
    println!("Set your API key:");
    println!("  continuum config set provider.anthropic.api_key YOUR_KEY");

    Ok(())
}

/// 显示所有配置键
fn show_keys() {
    println!("Available configuration keys:");
    println!();

    println!("Provider settings:");
    println!("  provider.<name>.api_key");
    println!("  provider.<name>.base_url");
    println!("  provider.<name>.model");
    println!("  provider.<name>.max_tokens");
    println!("  provider.<name>.temperature");
    println!();

    println!("Global settings:");
    println!("  settings.session_auto_save");
    println!("  settings.session_max_history");
    println!("  settings.checkpoint_enabled");
    println!("  settings.checkpoint_interval");
    println!("  settings.audit_enabled");
    println!("  settings.mcp_enabled");
}

/// 列出所有提供商
fn list_providers() -> Result<()> {
    let path = get_config_path();
    let mut config = ConfigManager::new();

    if path.exists() {
        config.load_from_file_sync(&path)?;
    }

    println!("Available providers:");
    for name in config.list_providers() {
        let is_active = name == &config.active_provider;
        let marker = if is_active { " (active)" } else { "" };
        println!("  {}{}", name, marker);
    }

    if config.providers.is_empty() {
        println!("  (no providers configured)");
        println!();
        println!("Add a provider:");
        println!("  continuum config add-provider anthropic --key YOUR_API_KEY");
    }

    Ok(())
}

/// 添加提供商
fn add_provider(
    name: String,
    key: String,
    url: Option<String>,
    model: Option<String>,
) -> Result<()> {
    let path = get_config_path();
    let mut config = ConfigManager::new();

    if path.exists() {
        config.load_from_file_sync(&path)?;
    }

    // 设置默认值
    let default_url = match name.as_str() {
        "anthropic" => "https://api.anthropic.com/v1",
        "openai" => "https://api.openai.com/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        "glm" => "https://open.bigmodel.cn/api/paas/v4",
        "qwen" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "kimi" | "moonshot" => "https://api.moonshot.cn/v1",
        "grok" => "https://api.x.ai/v1",
        _ => "",
    };

    let default_model = match name.as_str() {
        "anthropic" => "claude-sonnet-4-6",
        "openai" => "gpt-4",
        "gemini" => "gemini-pro",
        "deepseek" => "deepseek-chat",
        "glm" => "glm-4-flash",
        "qwen" => "qwen-plus",
        "kimi" | "moonshot" => "moonshot-v1-8k",
        "grok" => "grok-3",
        _ => "",
    };

    let base_url = url.unwrap_or_else(|| default_url.to_string());
    let provider_model = model.unwrap_or_else(|| default_model.to_string());

    let provider = ProviderConfig {
        api_key: key,
        base_url: base_url.clone(),
        model: provider_model.clone(),
        default_max_tokens: 4096,
        default_temperature: 0.7,
    };

    config.add_provider(&name, provider);
    config.save_sync(&path)?;

    println!("Added provider '{}'", name);
    println!("  base_url: {}", base_url);
    println!("  model: {}", provider_model);
    println!();
    println!("Switch to this provider:");
    println!("  continuum config use {}", name);

    Ok(())
}

/// 切换提供商
fn use_provider(provider: String) -> Result<()> {
    let path = get_config_path();
    let mut config = ConfigManager::new();

    if path.exists() {
        config.load_from_file_sync(&path)?;
    }

    config.use_provider(&provider)?;
    config.save_sync(&path)?;

    println!("Switched to provider '{}'", provider);

    Ok(())
}
