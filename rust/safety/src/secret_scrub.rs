//! Secret scrubbing for env vars, command args, HTTP responses.
//!
//! Detects and redacts common secret patterns (AWS keys, OpenAI keys, JWTs,
//! private keys, connection strings) to prevent leakage into LLM context.

use regex::Regex;

/// Redactor for known secret patterns.
pub struct SecretScrubber {
    patterns: Vec<(&'static str, Regex)>,
}

impl Default for SecretScrubber {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretScrubber {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                ("AWS Access Key", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
                ("OpenAI Key", Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap()),
                (
                    "Anthropic Key",
                    Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
                ),
                (
                    "GitHub Token",
                    Regex::new(r"gh[pousr]_[A-Za-z0-9]{36,}").unwrap(),
                ),
                (
                    "Bearer Token",
                    Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.=]+").unwrap(),
                ),
                (
                    "JWT",
                    Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap(),
                ),
                (
                    "Private Key",
                    Regex::new(r"-----BEGIN [A-Z ]+PRIVATE KEY-----").unwrap(),
                ),
                (
                    "Connection String",
                    Regex::new(r#"(?i)(postgres|mongodb|redis|amqp)://[^:\s]+:[^@\s]+@"#).unwrap(),
                ),
            ],
        }
    }

    /// Replace matched secrets with `<REDACTED:kind>`.
    pub fn scrub(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (kind, re) in &self.patterns {
            result = re
                .replace_all(&result, format!("<REDACTED:{}>", kind))
                .to_string();
        }
        result
    }

    /// Check if input contains any known secret pattern.
    pub fn contains_secret(&self, input: &str) -> Option<&'static str> {
        for (kind, re) in &self.patterns {
            if re.is_match(input) {
                return Some(kind);
            }
        }
        None
    }
}

/// Environment variable names that always warrant redaction regardless of value.
pub const SENSITIVE_ENV_NAMES: &[&str] = &[
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AZURE_CLIENT_SECRET",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "DEEPSEEK_API_KEY",
    "GEMINI_API_KEY",
    "GLM_API_KEY",
    "GITHUB_TOKEN",
    "GITLAB_TOKEN",
    "GH_TOKEN",
    "DATABASE_URL",
    "DB_PASSWORD",
    "POSTGRES_PASSWORD",
    "REDIS_URL",
    "NPM_TOKEN",
    "PYPI_TOKEN",
    "API_KEY",
    "API_TOKEN",
    "AUTH_TOKEN",
    "BEARER_TOKEN",
    "ACCESS_TOKEN",
    "REFRESH_TOKEN",
    "PRIVATE_KEY",
    "SECRET_KEY",
    "MASTER_KEY",
];

/// Env variable names that are dangerous to set (privilege escalation / injection).
pub const DANGEROUS_ENV_NAMES: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "PYTHONPATH",
    "NODE_PATH",
    "RUBYLIB",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_INDEX_FILE",
    "IFS",
    "BASH_ENV",
    "ENV",
    "PERL5OPT",
    "PERL5LIB",
    "JAVA_TOOL_OPTIONS",
];

/// Validate env var name: alphanumeric + underscore, max 256 chars.
pub fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrub_aws_access_key() {
        let s = SecretScrubber::new();
        let input = "config: AKIAIOSFODNN7EXAMPLE";
        let result = s.scrub(input);
        assert!(result.contains("<REDACTED:"));
        assert!(!result.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_scrub_openai_key() {
        let s = SecretScrubber::new();
        let input = "key=sk-abc123def456ghi789jkl012mno345pqr678";
        let result = s.scrub(input);
        assert!(result.contains("<REDACTED:"));
    }

    #[test]
    fn test_scrub_github_token() {
        let s = SecretScrubber::new();
        let input = "ghp_abcdef0123456789ABCDEF0123456789ABCDEF";
        let result = s.scrub(input);
        assert!(result.contains("<REDACTED:"));
    }

    #[test]
    fn test_scrub_jwt() {
        let s = SecretScrubber::new();
        let input = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.signature";
        let result = s.scrub(input);
        assert!(result.contains("<REDACTED:"));
    }

    #[test]
    fn test_scrub_private_key_pem() {
        let s = SecretScrubber::new();
        let input = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...";
        let result = s.scrub(input);
        assert!(result.contains("<REDACTED:"));
    }

    #[test]
    fn test_scrub_no_false_positive_normal_text() {
        let s = SecretScrubber::new();
        let input = "Hello world! This is a normal sentence with no secrets.";
        let result = s.scrub(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_scrub_preserves_surrounding_text() {
        let s = SecretScrubber::new();
        let input = "before AKIAIOSFODNN7EXAMPLE after";
        let result = s.scrub(input);
        assert!(result.starts_with("before "));
        assert!(result.ends_with(" after"));
    }

    #[test]
    fn test_contains_secret_returns_kind() {
        let s = SecretScrubber::new();
        assert_eq!(
            s.contains_secret("token=AKIAIOSFODNN7EXAMPLE"),
            Some("AWS Access Key")
        );
        assert_eq!(s.contains_secret("normal text"), None);
    }
}
