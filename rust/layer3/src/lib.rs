//! # Continuum Layer 3: Capabilities
//!
//! 特定领域的能力扩展。

// Clippy policy (Task 4.5):
// `cast_possible_truncation` is allowed for value-domain-restricted casts where
// the source value is guaranteed far below the target type's max:
//   - LSP line/column numbers (usize -> u32 for Position): real files < 40M lines
//   - elapsed millis (u128 -> u64): would need ~584 million years of runtime
//   - file sizes/counts (u64 -> usize): 64-bit targets == u64; 32-bit targets
//     can't address >4GiB anyway
// Genuine risk casts (e.g. network packet sizes -> u16) MUST use try_from
// explicitly and are NOT covered by this allow.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

pub mod builtin_tools;
pub mod document_loaders;
pub mod example_selectors;
pub mod guard_rails;
pub mod lsp;
pub mod lsp_client;
pub mod memory_system;
pub mod output_parsers;
pub mod process_manager;
pub mod query_engine;
pub mod retriever;
pub mod retriever_engine;
pub mod sandbox_runtime;
pub mod skills;
pub mod text_splitters;
pub mod tool_executor;
pub mod types;
pub mod vector_store;

// Re-export Layer 2 types for upper layers (链式暴露)
pub use sh_layer2;

// Re-export Layer 2 ID utilities for upper layers
pub use sh_layer2::{generate_prefixed_id, generate_short_id};

// Re-export core types
pub use types::{
    CodeLocation, CodeRange, Layer3Error, Layer3Result, MemoryEntry, MemoryQuery, MemoryTier,
    ProcessInfo, ProcessState, QueryResult, QueryType, ToolCategory, ToolId, ToolMeta, ToolRequest,
    ToolResponse,
};

// Re-export LSP module
pub use lsp::{
    client::{ConnectionState, LspClient, SyncLspClient},
    server::{
        clangd_config, gopls_config, pylance_config, pyright_config, rust_analyzer_config,
        typescript_config, LanguageServer, LanguageServerConfig, LanguageServerManager,
    },
    types::*,
    LspError, LspResult,
};
pub use memory_system::{
    DecayPolicy, ImportanceScorer, MemoryStore, MemorySystem as MemorySystemTrait, SessionMemory,
    UnifiedMemorySystem, WorkingMemory,
};
pub use process_manager::{ProcessLimits, ProcessManager as ProcessManagerTrait, ProcessSignal};
pub use query_engine::{CodeAnalyzer, QueryEngine, SymbolInfo, SymbolKind};
pub use retriever::{
    BM25Index, DefaultHybridRetriever, HybridRetriever, HybridRetrieverConfig, ReciprocalRankFusion,
};
pub use retriever_engine::{
    Chunk, ChunkPosition, ChunkingStrategy, DefaultRetrieverEngine, Document, FixedSizeChunker,
    HybridSearchConfig, HybridWeights, Layer1EmbeddingAdapter, ParagraphChunker, RecursiveChunker,
    RetrievalResult, RetrieverEngine,
};
pub use sandbox_runtime::{
    ExecutionResult, SandboxConfig, SandboxId, SandboxRuntime as SandboxRuntimeTrait,
};
pub use tool_executor::{
    ContextualExecutor, DefaultToolExecutor, ExecutionContext, ToolExecutor, ToolValidator,
};
pub use vector_store::{
    DistanceMetric, FileVectorStore, FileVectorStoreFactory, InMemoryVectorStore,
    InMemoryVectorStoreFactory, IndexType, MetadataFilter, VectorItem,
    VectorStore as VectorStoreTrait, VectorStoreConfig, VectorStoreFactory,
};

// Re-export builtin tools for Layer 2 integration
pub use builtin_tools::file_ops::{EditFileTool, ListDirectoryTool, ReadFileTool, WriteFileTool};
pub use builtin_tools::search::{GlobTool, GrepTool};
pub use builtin_tools::shell::BashTool;
pub use builtin_tools::{register_builtin_tools, BuiltinTool, BuiltinToolRegistry, ToolAdapter};
