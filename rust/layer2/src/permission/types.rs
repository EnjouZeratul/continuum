//! # Permission Types
//!
//! Core types for the permission system.

use serde::{Deserialize, Serialize};
use sh_layer1::generate_short_id;
use std::collections::HashMap;

/// Unique identifier for a permission request
pub type PermissionId = String;

/// Actions that require permission
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionAction {
    /// Execute a shell command
    CommandExecute { command: String, args: Vec<String> },
    /// Read a file
    FileRead { path: String },
    /// Write to a file
    FileWrite {
        path: String,
        content_preview: Option<String>,
    },
    /// Delete a file
    FileDelete { path: String },
    /// Make a network request
    NetworkRequest { url: String, method: String },
    /// Access environment variables
    EnvAccess { names: Vec<String> },
    /// Install packages
    PackageInstall { packages: Vec<String> },
    /// Access system resources
    SystemAccess { resource: String },
    /// Custom action with description
    Custom { description: String },
}

impl PermissionAction {
    /// Get a human-readable description of the action
    pub fn description(&self) -> String {
        match self {
            PermissionAction::CommandExecute { command, args } => {
                format!("Execute command: {} {}", command, args.join(" "))
            }
            PermissionAction::FileRead { path } => format!("Read file: {}", path),
            PermissionAction::FileWrite {
                path,
                content_preview,
            } => {
                if let Some(preview) = content_preview {
                    let preview = if preview.len() > 100 {
                        format!("{}...", &preview[..100])
                    } else {
                        preview.clone()
                    };
                    format!("Write to file: {}\nPreview: {}", path, preview)
                } else {
                    format!("Write to file: {}", path)
                }
            }
            PermissionAction::FileDelete { path } => format!("Delete file: {}", path),
            PermissionAction::NetworkRequest { url, method } => {
                format!("{} request to: {}", method, url)
            }
            PermissionAction::EnvAccess { names } => {
                format!("Access environment variables: {}", names.join(", "))
            }
            PermissionAction::PackageInstall { packages } => {
                format!("Install packages: {}", packages.join(", "))
            }
            PermissionAction::SystemAccess { resource } => {
                format!("Access system resource: {}", resource)
            }
            PermissionAction::Custom { description } => description.clone(),
        }
    }

    /// Get the category of this action
    pub fn category(&self) -> &'static str {
        match self {
            PermissionAction::CommandExecute { .. } => "command",
            PermissionAction::FileRead { .. } => "file_read",
            PermissionAction::FileWrite { .. } => "file_write",
            PermissionAction::FileDelete { .. } => "file_delete",
            PermissionAction::NetworkRequest { .. } => "network",
            PermissionAction::EnvAccess { .. } => "environment",
            PermissionAction::PackageInstall { .. } => "package",
            PermissionAction::SystemAccess { .. } => "system",
            PermissionAction::Custom { .. } => "custom",
        }
    }
}

/// Context information for a permission request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionContext {
    /// Agent ID making the request
    pub agent_id: Option<String>,
    /// Session ID where the request originated
    pub session_id: Option<String>,
    /// Task ID if applicable
    pub task_id: Option<String>,
    /// Tool that triggered the request
    pub tool_name: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// A request for permission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Unique request ID
    pub id: PermissionId,
    /// The action being requested
    pub action: PermissionAction,
    /// Context information
    pub context: PermissionContext,
    /// Whether this can be batched with other requests
    pub batchable: bool,
    /// Timestamp of the request
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PermissionRequest {
    /// Create a new permission request
    pub fn new(action: PermissionAction) -> Self {
        Self {
            id: generate_short_id(),
            action,
            context: PermissionContext::default(),
            batchable: false,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add context to the request
    pub fn with_context(mut self, context: PermissionContext) -> Self {
        self.context = context;
        self
    }

    /// Mark as batchable
    pub fn batchable(mut self) -> Self {
        self.batchable = true;
        self
    }
}

/// Decision made for a permission request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    /// Permission granted
    Allow,
    /// Permission denied
    Deny,
    /// Permission granted for this session only
    AllowOnce,
    /// Permission denied for this session only
    DenyOnce,
}

impl PermissionDecision {
    /// Check if this decision allows the action
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            PermissionDecision::Allow | PermissionDecision::AllowOnce
        )
    }

    /// Check if this decision should be remembered
    pub fn should_remember(&self) -> bool {
        matches!(self, PermissionDecision::Allow | PermissionDecision::Deny)
    }
}

/// Response to a permission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponse {
    /// The request being responded to
    pub request_id: PermissionId,
    /// The decision made
    pub decision: PermissionDecision,
    /// Optional reason for the decision
    pub reason: Option<String>,
    /// Timestamp of the response
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PermissionResponse {
    /// Create an allow response
    pub fn allow(request_id: PermissionId) -> Self {
        Self {
            request_id,
            decision: PermissionDecision::Allow,
            reason: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a deny response
    pub fn deny(request_id: PermissionId, reason: Option<String>) -> Self {
        Self {
            request_id,
            decision: PermissionDecision::Deny,
            reason,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create an allow-once response
    pub fn allow_once(request_id: PermissionId) -> Self {
        Self {
            request_id,
            decision: PermissionDecision::AllowOnce,
            reason: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a deny-once response
    pub fn deny_once(request_id: PermissionId, reason: Option<String>) -> Self {
        Self {
            request_id,
            decision: PermissionDecision::DenyOnce,
            reason,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Cached permission entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPermission {
    /// The action pattern that was allowed/denied
    pub action_pattern: String,
    /// The decision made
    pub decision: PermissionDecision,
    /// When this was cached
    pub cached_at: chrono::DateTime<chrono::Utc>,
    /// When this cache entry expires (if applicable)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of times this cached entry has been used
    pub use_count: usize,
}

impl CachedPermission {
    /// Check if this cache entry is still valid
    pub fn is_valid(&self) -> bool {
        if let Some(expires) = self.expires_at {
            expires > chrono::Utc::now()
        } else {
            true
        }
    }

    /// Mark this entry as used
    pub fn use_once(&mut self) {
        self.use_count += 1;
    }
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique audit entry ID
    pub id: String,
    /// The request that was made
    pub request: PermissionRequest,
    /// The response that was given
    pub response: PermissionResponse,
    /// Whether this was from cache
    pub from_cache: bool,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_action_description() {
        let action = PermissionAction::FileRead {
            path: "/test/file.txt".to_string(),
        };
        assert_eq!(action.description(), "Read file: /test/file.txt");
        assert_eq!(action.category(), "file_read");
    }

    #[test]
    fn test_permission_decision_is_allowed() {
        assert!(PermissionDecision::Allow.is_allowed());
        assert!(PermissionDecision::AllowOnce.is_allowed());
        assert!(!PermissionDecision::Deny.is_allowed());
        assert!(!PermissionDecision::DenyOnce.is_allowed());
    }

    #[test]
    fn test_permission_request_builder() {
        let request = PermissionRequest::new(PermissionAction::FileRead {
            path: "test.txt".to_string(),
        })
        .batchable();

        assert!(request.batchable);
        assert!(!request.action.description().is_empty());
    }
}
