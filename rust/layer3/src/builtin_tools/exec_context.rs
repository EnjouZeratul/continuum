//! Execution context for context-aware built-in tools.
//!
//! `BuiltinTool::execute_with_context` receives this struct to access
//! session-scoped state. The default `execute(args)` method (used by
//! 47 sessionless tools) does not see it.
//!
//! # v1.1.0 minimal scope
//!
//! Context is process-wide (set via `set_current_context` at session start).
//! v1.1.1+ may thread it explicitly through every call site. For single-
//! session agent loops (the common case), process-wide is correct.

use crate::builtin_tools::read_state::ReadStateStore;
use std::sync::{Arc, LazyLock, RwLock};

/// Identity + state passed to context-aware tools.
#[derive(Clone, Default)]
pub struct ExecutionContext {
    /// Session identifier. Empty for ad-hoc / single-session defaults.
    pub session_id: String,
    /// LLM-issued tool_call identifier (for tracing / log correlation).
    pub call_id: String,
    /// Per-session read-state tracker (for stale-read prevention).
    /// If `None`, falls back to the global store.
    pub read_state: Option<Arc<ReadStateStore>>,
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("session_id", &self.session_id)
            .field("call_id", &self.call_id)
            .field(
                "read_state",
                &self.read_state.as_ref().map(|_| "<ReadStateStore>"),
            )
            .finish()
    }
}

impl ExecutionContext {
    /// Resolve the effective ReadStateStore: per-session if set, else global.
    pub fn read_state_store(&self) -> Arc<ReadStateStore> {
        self.read_state
            .clone()
            .unwrap_or_else(crate::builtin_tools::read_state::global_store)
    }

    /// Construct a session-scoped context.
    pub fn for_session(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            call_id: String::new(),
            read_state: None,
        }
    }

    /// Builder: attach a call_id.
    pub fn with_call_id(mut self, call_id: impl Into<String>) -> Self {
        self.call_id = call_id.into();
        self
    }

    /// Builder: attach a per-session ReadStateStore.
    pub fn with_read_state(mut self, store: Arc<ReadStateStore>) -> Self {
        self.read_state = Some(store);
        self
    }
}

/// Process-wide "current" context. Set by the agent runtime at session start.
static CURRENT: LazyLock<RwLock<Option<ExecutionContext>>> = LazyLock::new(|| RwLock::new(None));

/// Set the current execution context (called by agent runtime / session manager).
pub fn set_current_context(ctx: ExecutionContext) {
    *CURRENT.write().unwrap() = Some(ctx);
}

/// Clear the current context (called at session end).
pub fn clear_current_context() {
    *CURRENT.write().unwrap() = None;
}

/// Get a snapshot of the current context, or default if none set.
pub fn current_context() -> ExecutionContext {
    CURRENT.read().unwrap().clone().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_context_uses_global_store() {
        let ctx = ExecutionContext::default();
        let s1 = ctx.read_state_store();
        let s2 = crate::builtin_tools::read_state::global_store();
        assert!(Arc::ptr_eq(&s1, &s2));
    }

    #[test]
    fn test_for_session_builder() {
        let ctx = ExecutionContext::for_session("sess-123").with_call_id("call-456");
        assert_eq!(ctx.session_id, "sess-123");
        assert_eq!(ctx.call_id, "call-456");
    }

    #[test]
    fn test_set_and_clear_current_context() {
        set_current_context(ExecutionContext::for_session("test-sess"));
        let ctx = current_context();
        assert_eq!(ctx.session_id, "test-sess");
        clear_current_context();
        let ctx = current_context();
        assert_eq!(ctx.session_id, "");
    }
}
