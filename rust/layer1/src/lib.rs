//! # Continuum Layer 1: Foundation
//!
//! 基础设施层，为上层提供核心能力。

pub mod cache_manager;
pub mod config_manager;
pub mod cost_tracker;
pub mod embeddings;
pub mod error_handler;
pub mod event_bus;
pub mod llm_client;
pub mod observability;
pub mod storage_engine;
pub mod streaming;
pub mod utils;

pub use cache_manager::CacheManager;
pub use config_manager::{ConfigManager, GlobalSettings, ProviderConfig};
pub use cost_tracker::CostTracker;
pub use embeddings::{
    CacheStats, CohereEmbeddings, EmbeddingCache, EmbeddingModel, EmbeddingProvider, Embeddings,
    EmbeddingsConfig, EmbeddingsFactory, HuggingFaceEmbeddings, LocalEmbeddings, OpenAIEmbeddings,
    DEFAULT_EMBEDDING_DIMENSION, DEFAULT_EMBEDDING_MODEL,
};

// MockEmbeddingModel 仅在启用 mock feature 或测试配置下导出
#[cfg(any(feature = "mock", test))]
pub use embeddings::MockEmbeddingModel;
pub use error_handler::{ErrorHandler, ShError, ShResult};
pub use event_bus::{Event, EventBus, HandlerId};
pub use llm_client::{
    LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, LlmResponse, Message, MessageRole,
    TokenUsage,
};
pub use observability::{
    Counter, Gauge, Histogram, LogFormat, LogLevel, MetricValue, Observability,
    ObservabilityConfig, SpanGuard,
};
pub use storage_engine::StorageEngine;
pub use streaming::{
    AbortableStream,
    CallbackStream,
    ContentBlockType,
    ContentDelta,
    // HTTP exports
    HttpAdapter,
    HttpConfig,
    HttpRequest,
    HttpResponseStream,
    MessageStream,
    OnChunkCallback,
    SseEvent,
    SseParser,
    SseStream,
    StreamEvent,
    StreamHandler,
    StreamProvider,
    StreamState,
    StreamUsage,
    // WebSocket exports
    WebSocketAdapter,
    WebSocketConfig,
    WebSocketMessage,
    WebSocketMessageStream,
};
pub use utils::{generate_prefixed_id, generate_short_id};
