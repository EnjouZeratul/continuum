//! # Permission System
//!
//! Interactive permission system for secure agent execution.
//!
//! Provides capability-based security with:
//! - Interactive confirmation prompts
//! - Permission caching and "remember choice" functionality
//! - Security policy configuration
//! - Audit logging integration

pub mod manager;
pub mod policy;
pub mod types;

pub use manager::{PermissionError, PermissionManager, PermissionResult};
pub use policy::{PermissionPolicy, PermissionRule, SecurityLevel};
pub use types::{
    AuditEntry, CachedPermission, PermissionAction, PermissionContext, PermissionDecision,
    PermissionRequest, PermissionResponse,
};
