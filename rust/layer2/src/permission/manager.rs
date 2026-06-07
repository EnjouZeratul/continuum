//! # Permission Manager
//!
//! Central manager for the permission system.

use crate::permission::policy::{PermissionPolicy, SecurityLevel};
use crate::permission::types::{
    AuditEntry, CachedPermission, PermissionAction, PermissionDecision, PermissionRequest,
    PermissionResponse,
};
use parking_lot::RwLock;
use sh_layer1::generate_short_id;
use std::collections::HashMap;
use std::sync::Arc;

/// Callback type for interactive permission prompts
pub type PermissionPromptCallback =
    Box<dyn Fn(&PermissionRequest) -> PermissionResponse + Send + Sync>;

/// Result type for permission operations
pub type PermissionResult<T> = Result<T, PermissionError>;

/// Errors that can occur in the permission system
#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    /// Action was denied
    #[error("Permission denied: {0}")]
    Denied(String),

    /// Action is blocked by policy
    #[error("Action blocked by security policy: {0}")]
    BlockedByPolicy(String),

    /// No prompt callback configured
    #[error("No permission prompt callback configured")]
    NoCallback,

    /// Cache error
    #[error("Permission cache error: {0}")]
    CacheError(String),

    /// Invalid request
    #[error("Invalid permission request: {0}")]
    InvalidRequest(String),
}

/// The permission manager
pub struct PermissionManager {
    /// Security policy
    policy: RwLock<PermissionPolicy>,
    /// Permission cache
    cache: RwLock<HashMap<String, CachedPermission>>,
    /// Audit log
    audit_log: RwLock<Vec<AuditEntry>>,
    /// Optional prompt callback for interactive mode
    prompt_callback: RwLock<Option<Arc<PermissionPromptCallback>>>,
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new(PermissionPolicy::default())
    }
}

impl PermissionManager {
    /// Create a new permission manager with the given policy
    pub fn new(policy: PermissionPolicy) -> Self {
        Self {
            policy: RwLock::new(policy),
            cache: RwLock::new(HashMap::new()),
            audit_log: RwLock::new(Vec::new()),
            prompt_callback: RwLock::new(None),
        }
    }

    /// Set the prompt callback for interactive mode
    pub fn set_prompt_callback(&self, callback: PermissionPromptCallback) {
        *self.prompt_callback.write() = Some(Arc::new(callback));
    }

    /// Clear the prompt callback
    pub fn clear_prompt_callback(&self) {
        *self.prompt_callback.write() = None;
    }

    /// Update the security policy
    pub fn set_policy(&self, policy: PermissionPolicy) {
        *self.policy.write() = policy;
    }

    /// Get the current security level
    pub fn security_level(&self) -> SecurityLevel {
        self.policy.read().level
    }

    /// Check if an action is allowed, prompting if necessary
    pub fn check_permission(
        &self,
        request: PermissionRequest,
    ) -> PermissionResult<PermissionResponse> {
        let category = request.action.category();

        // Check if category is blocked by policy
        if self.policy.read().is_category_blocked(category) {
            return Err(PermissionError::BlockedByPolicy(format!(
                "Category '{}' is blocked by security policy",
                category
            )));
        }

        // Check for hard-coded blocks
        if let Some(block_reason) = self.check_blocked(&request.action) {
            return Err(PermissionError::BlockedByPolicy(block_reason));
        }

        // Check for auto-approval
        if self.policy.read().should_auto_approve(category) {
            let response = PermissionResponse::allow(request.id.clone());
            self.log_audit(&request, &response, false);
            return Ok(response);
        }

        // Check cache
        if self.policy.read().enable_cache {
            if let Some(cached) = self.check_cache(&request) {
                let response = PermissionResponse {
                    request_id: request.id.clone(),
                    decision: cached.decision,
                    reason: Some("From cache".to_string()),
                    timestamp: chrono::Utc::now(),
                };
                self.log_audit(&request, &response, true);
                return Ok(response);
            }
        }

        // Prompt user if callback is set
        if let Some(callback) = self.prompt_callback.read().as_ref() {
            let response = callback(&request);
            let description = request.action.description();

            // Cache if allowed to remember
            if response.decision.should_remember() && self.policy.read().enable_cache {
                self.cache_permission(&request.action, response.decision);
            }

            // Log audit
            self.log_audit(&request, &response, false);

            // Return error if denied
            if !response.decision.is_allowed() {
                return Err(PermissionError::Denied(format!(
                    "User denied: {}",
                    description
                )));
            }

            Ok(response)
        } else {
            // No callback - deny by default in non-trusted mode
            if self.policy.read().level == SecurityLevel::Trusted {
                let response = PermissionResponse::allow(request.id.clone());
                self.log_audit(&request, &response, false);
                Ok(response)
            } else {
                Err(PermissionError::NoCallback)
            }
        }
    }

    /// Check if an action is blocked by policy
    fn check_blocked(&self, action: &PermissionAction) -> Option<String> {
        let policy = self.policy.read();

        match action {
            PermissionAction::CommandExecute { command, .. } => {
                for blocked in &policy.blocked_commands {
                    if command.starts_with(blocked) || command.contains(blocked) {
                        return Some(format!(
                            "Command '{}' is blocked by security policy",
                            command
                        ));
                    }
                }
                None
            }
            PermissionAction::FileRead { path }
            | PermissionAction::FileWrite { path, .. }
            | PermissionAction::FileDelete { path } => {
                if policy.is_path_blocked(path) {
                    return Some(format!("Path '{}' is blocked by security policy", path));
                }
                None
            }
            PermissionAction::NetworkRequest { url, .. } => {
                if policy.blocked_urls.iter().any(|u| url.contains(u)) {
                    return Some(format!("URL '{}' is blocked by security policy", url));
                }
                None
            }
            _ => None,
        }
    }

    /// Check cache for a cached decision
    fn check_cache(&self, request: &PermissionRequest) -> Option<CachedPermission> {
        let cache = self.cache.read();
        let key = self.cache_key(&request.action);

        if let Some(cached) = cache.get(&key) {
            if cached.is_valid() {
                let mut cached = cached.clone();
                cached.use_once();
                self.cache.write().insert(key, cached.clone());
                return Some(cached);
            }
        }

        None
    }

    /// Cache a permission decision
    fn cache_permission(&self, action: &PermissionAction, decision: PermissionDecision) {
        let key = self.cache_key(action);
        let expire_seconds = self.policy.read().cache_expire_seconds;

        let cached = CachedPermission {
            action_pattern: key.clone(),
            decision,
            cached_at: chrono::Utc::now(),
            expires_at: if expire_seconds > 0 {
                Some(chrono::Utc::now() + chrono::Duration::seconds(expire_seconds as i64))
            } else {
                None
            },
            use_count: 0,
        };

        self.cache.write().insert(key, cached);
    }

    /// Generate cache key for an action
    fn cache_key(&self, action: &PermissionAction) -> String {
        match action {
            PermissionAction::CommandExecute { command, args } => {
                format!("cmd:{}:{}", command, args.join(" "))
            }
            PermissionAction::FileRead { path } => format!("read:{}", path),
            PermissionAction::FileWrite { path, .. } => format!("write:{}", path),
            PermissionAction::FileDelete { path } => format!("delete:{}", path),
            PermissionAction::NetworkRequest { url, method } => format!("net:{}:{}", method, url),
            PermissionAction::EnvAccess { names } => format!("env:{}", names.join(",")),
            PermissionAction::PackageInstall { packages } => format!("pkg:{}", packages.join(",")),
            PermissionAction::SystemAccess { resource } => format!("sys:{}", resource),
            PermissionAction::Custom { description } => format!("custom:{}", description),
        }
    }

    /// Log an audit entry
    fn log_audit(
        &self,
        request: &PermissionRequest,
        response: &PermissionResponse,
        from_cache: bool,
    ) {
        if !self.policy.read().audit_enabled {
            return;
        }

        let entry = AuditEntry {
            id: generate_short_id(),
            request: request.clone(),
            response: response.clone(),
            from_cache,
            timestamp: chrono::Utc::now(),
        };

        let max = self.policy.read().max_audit_entries;
        let mut log = self.audit_log.write();
        log.push(entry);

        // Trim if exceeds max
        if log.len() > max {
            let excess = log.len() - max;
            log.drain(0..excess);
        }
    }

    /// Get audit log entries
    pub fn get_audit_log(&self) -> Vec<AuditEntry> {
        self.audit_log.read().clone()
    }

    /// Clear audit log
    pub fn clear_audit_log(&self) {
        self.audit_log.write().clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read();
        let total = cache.len();
        let valid = cache.values().filter(|c| c.is_valid()).count();
        (total, valid)
    }

    /// Clear permission cache
    pub fn clear_cache(&self) {
        self.cache.write().clear();
    }

    /// Create a permission request for command execution
    pub fn request_command(&self, command: &str, args: Vec<String>) -> PermissionRequest {
        PermissionRequest::new(PermissionAction::CommandExecute {
            command: command.to_string(),
            args,
        })
    }

    /// Create a permission request for file read
    pub fn request_file_read(&self, path: &str) -> PermissionRequest {
        PermissionRequest::new(PermissionAction::FileRead {
            path: path.to_string(),
        })
    }

    /// Create a permission request for file write
    pub fn request_file_write(
        &self,
        path: &str,
        content_preview: Option<&str>,
    ) -> PermissionRequest {
        PermissionRequest::new(PermissionAction::FileWrite {
            path: path.to_string(),
            content_preview: content_preview.map(|s| s.to_string()),
        })
    }

    /// Create a permission request for file delete
    pub fn request_file_delete(&self, path: &str) -> PermissionRequest {
        PermissionRequest::new(PermissionAction::FileDelete {
            path: path.to_string(),
        })
    }

    /// Create a permission request for network request
    pub fn request_network(&self, url: &str, method: &str) -> PermissionRequest {
        PermissionRequest::new(PermissionAction::NetworkRequest {
            url: url.to_string(),
            method: method.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_manager_creation() {
        let manager = PermissionManager::default();
        assert_eq!(manager.security_level(), SecurityLevel::Standard);
    }

    #[test]
    fn test_trusted_policy_auto_approves() {
        let manager = PermissionManager::new(PermissionPolicy::trusted());
        let request = PermissionRequest::new(PermissionAction::FileRead {
            path: "/test/file.txt".to_string(),
        });

        let result = manager.check_permission(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_blocked_path_denied() {
        let manager = PermissionManager::default();
        let request = PermissionRequest::new(PermissionAction::FileRead {
            path: ".env".to_string(),
        });

        let result = manager.check_permission(request);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PermissionError::BlockedByPolicy(_)
        ));
    }

    #[test]
    fn test_cache_stats() {
        let manager = PermissionManager::default();
        let (total, valid) = manager.cache_stats();
        assert_eq!(total, 0);
        assert_eq!(valid, 0);
    }
}
