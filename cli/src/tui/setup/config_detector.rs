//! Configuration detection for first-run setup

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

/// Provider detection source
#[derive(Debug, Clone)]
pub enum DetectionSource {
    /// Detected from environment variable
    EnvVar(String),
    /// Detected from config file
    ConfigFile(PathBuf),
}

/// A detected provider configuration
#[derive(Debug, Clone)]
pub struct DetectedProvider {
    /// Provider name (anthropic, openai, gemini)
    pub name: String,
    /// Where the configuration was detected
    pub source: DetectionSource,
    /// Whether API key is set
    pub api_key_set: bool,
}

/// Result of configuration detection
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// All detected providers
    pub providers: Vec<DetectedProvider>,
    /// Whether any valid configuration exists
    pub has_valid_config: bool,
    /// Config file path if exists
    pub config_file_path: Option<PathBuf>,
}

/// Configuration detector
pub struct ConfigDetector {
    /// Environment variable mappings for providers
    env_mappings: HashMap<&'static str, &'static str>,
}

impl ConfigDetector {
    /// Create new detector
    pub fn new() -> Self {
        let mut env_mappings = HashMap::new();
        env_mappings.insert("anthropic", "ANTHROPIC_API_KEY");
        env_mappings.insert("openai", "OPENAI_API_KEY");
        env_mappings.insert("google", "GOOGLE_API_KEY");
        env_mappings.insert("gemini", "GEMINI_API_KEY");
        env_mappings.insert("deepseek", "DEEPSEEK_API_KEY");
        env_mappings.insert("glm", "GLM_API_KEY");
        env_mappings.insert("qwen", "QWEN_API_KEY");
        env_mappings.insert("kimi", "KIMI_API_KEY");
        env_mappings.insert("moonshot", "MOONSHOT_API_KEY");
        env_mappings.insert("grok", "GROK_API_KEY");

        Self { env_mappings }
    }

    /// Detect configuration from environment and config file
    pub fn detect(&self) -> Result<DetectionResult> {
        let mut providers = Vec::new();
        let mut has_valid_config = false;

        // Check environment variables
        for (provider_name, env_var) in &self.env_mappings {
            if let Ok(value) = std::env::var(env_var) {
                if !value.is_empty() {
                    providers.push(DetectedProvider {
                        name: provider_name.to_string(),
                        source: DetectionSource::EnvVar(env_var.to_string()),
                        api_key_set: true,
                    });
                    has_valid_config = true;
                }
            }
        }

        // Check for CONTINUUM_API_KEY (generic fallback)
        if let Ok(value) = std::env::var("CONTINUUM_API_KEY") {
            if !value.is_empty() && providers.is_empty() {
                providers.push(DetectedProvider {
                    name: "anthropic".to_string(), // Default to anthropic
                    source: DetectionSource::EnvVar("CONTINUUM_API_KEY".to_string()),
                    api_key_set: true,
                });
                has_valid_config = true;
            }
        }

        // Check config file
        let config_path = self.get_config_path();
        let config_file_path = if config_path.exists() {
            // Try to load and validate config
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if content.contains("api_key") && !content.contains("api_key: \"\"") {
                    // Check for configured providers in file
                    for provider_name in &[
                        "anthropic",
                        "openai",
                        "google",
                        "gemini",
                        "deepseek",
                        "glm",
                        "qwen",
                        "kimi",
                        "moonshot",
                        "grok",
                    ] {
                        if content.contains(&format!("[{}]", provider_name))
                            || content.contains(&format!("{}:", provider_name))
                        {
                            // Check if this provider has api_key set
                            let has_key = self.provider_has_key_in_config(&content, provider_name);
                            if has_key {
                                providers.push(DetectedProvider {
                                    name: provider_name.to_string(),
                                    source: DetectionSource::ConfigFile(config_path.clone()),
                                    api_key_set: true,
                                });
                                has_valid_config = true;
                            }
                        }
                    }
                }
            }
            Some(config_path)
        } else {
            None
        };

        Ok(DetectionResult {
            providers,
            has_valid_config,
            config_file_path,
        })
    }

    /// Get default config path
    fn get_config_path(&self) -> PathBuf {
        sh_core::layer1::ConfigManager::default_config_path()
    }

    /// Check if provider has API key in config content
    fn provider_has_key_in_config(&self, content: &str, provider: &str) -> bool {
        // Simple check - look for non-empty api_key in provider section
        content.contains(&format!("{}_api_key", provider))
            || content.contains(&format!("{}.api_key", provider))
    }

    /// Get environment variable name for provider
    pub fn get_env_var(&self, provider: &str) -> Option<&'static str> {
        self.env_mappings.get(provider).copied()
    }
}

impl Default for ConfigDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = ConfigDetector::new();
        assert!(detector.get_env_var("anthropic").is_some());
        assert!(detector.get_env_var("openai").is_some());
    }

    #[test]
    fn test_detect_without_env() {
        // This test verifies detection works even without env vars set
        let detector = ConfigDetector::new();
        let result = detector.detect();
        assert!(result.is_ok());
    }

    #[test]
    fn test_detection_result_defaults() {
        let result = DetectionResult {
            providers: vec![],
            has_valid_config: false,
            config_file_path: None,
        };
        assert!(!result.has_valid_config);
        assert!(result.providers.is_empty());
    }
}
