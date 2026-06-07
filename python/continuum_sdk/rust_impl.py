"""
Rust Implementation Adapters

Wraps sh_python bindings for unified API compatibility.
This module is only used when Rust binding is available.

All 28 classes from sh_python are wrapped:
- Layer 0: SecurityGateway, PermissionManager, Permission, Role
- Layer 1: LlmClient, LlmRequestConfig, LlmResponse, CostTracker, UsageSnapshot, CostEstimate
- Layer 2: AgentRuntime, SessionManager, CheckpointSystem, Agent, Session
- Layer 3: ToolExecutor, QueryEngine, MemorySystem, VectorStore, VectorItem, SearchResult,
           RetrieverEngine, DocumentLoader, TextSplitter, Embeddings
- Layer 4: McpBridge, AuditLogger
"""

from __future__ import annotations

import json
import logging
from collections.abc import Callable
from typing import Any

from .config.providers import get_default_model

try:
    # Layer 0
    from sh_python import (
        Agent as RustAgentBinding,
    )

    # Layer 2
    from sh_python import (
        AgentRuntime as RustAgentRuntimeBinding,
    )
    from sh_python import (
        AuditLogger as RustAuditLoggerBinding,
    )
    from sh_python import (
        CheckpointSystem as RustCheckpointSystemBinding,
    )
    from sh_python import (
        CostEstimate as RustCostEstimateBinding,
    )
    from sh_python import (
        CostTracker as RustCostTrackerBinding,
    )
    from sh_python import (
        DocumentLoader as RustDocumentLoaderBinding,
    )
    from sh_python import (
        Embeddings as RustEmbeddingsBinding,
    )

    # Layer 1
    from sh_python import (
        LlmClient as RustLlmClientBinding,
    )
    from sh_python import (
        LlmRequestConfig as RustLlmRequestConfigBinding,
    )
    from sh_python import (
        LlmResponse as RustLlmResponseBinding,
    )

    # Layer 4
    from sh_python import (
        McpBridge as RustMcpBridgeBinding,
    )
    from sh_python import (
        MemorySystem as RustMemorySystemBinding,
    )
    from sh_python import (
        Permission as RustPermissionBinding,
    )
    from sh_python import (
        PermissionManager as RustPermissionManagerBinding,
    )
    from sh_python import (
        QueryEngine as RustQueryEngineBinding,
    )
    from sh_python import (
        RetrieverEngine as RustRetrieverEngineBinding,
    )
    from sh_python import (
        Role as RustRoleBinding,
    )
    from sh_python import (
        SearchResult as RustSearchResultBinding,
    )
    from sh_python import (
        SecurityGateway as RustSecurityGatewayBinding,
    )
    from sh_python import (
        Session as RustSessionBinding,
    )
    from sh_python import (
        SessionManager as RustSessionManagerBinding,
    )
    from sh_python import (
        TextSplitter as RustTextSplitterBinding,
    )

    # Layer 3
    from sh_python import (
        ToolExecutor as RustToolExecutorBinding,
    )
    from sh_python import (
        UsageSnapshot as RustUsageSnapshotBinding,
    )
    from sh_python import (
        VectorItem as RustVectorItemBinding,
    )
    from sh_python import (
        VectorStore as RustVectorStoreBinding,
    )

    HAS_BINDING = True
except ImportError:
    HAS_BINDING = False
    # Layer 0
    RustSecurityGatewayBinding = None
    RustPermissionManagerBinding = None
    RustPermissionBinding = None
    RustRoleBinding = None
    # Layer 1
    RustLlmClientBinding = None
    RustLlmRequestConfigBinding = None
    RustLlmResponseBinding = None
    RustCostTrackerBinding = None
    RustUsageSnapshotBinding = None
    RustCostEstimateBinding = None
    # Layer 2
    RustAgentRuntimeBinding = None
    RustSessionManagerBinding = None
    RustCheckpointSystemBinding = None
    RustAgentBinding = None
    RustSessionBinding = None
    # Layer 3
    RustToolExecutorBinding = None
    RustQueryEngineBinding = None
    RustMemorySystemBinding = None
    RustVectorStoreBinding = None
    RustVectorItemBinding = None
    RustSearchResultBinding = None
    RustRetrieverEngineBinding = None
    RustDocumentLoaderBinding = None
    RustTextSplitterBinding = None
    RustEmbeddingsBinding = None
    # Layer 4
    RustMcpBridgeBinding = None
    RustAuditLoggerBinding = None

logger = logging.getLogger(__name__)


class RustAgent:
    """Rust-backed Agent implementation."""

    def __init__(self, name: str = "default", model: str | None = None, **kwargs: Any):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._name = name
        self._model = model or get_default_model("anthropic")
        self._agent = RustAgentBinding(name=name)
        self._tools: dict[str, Callable] = {}

    def run(self, task: str, **kwargs: Any) -> str:
        """Execute task synchronously."""
        # Start agent if not running
        self._agent.start()
        result = self._agent.execute(task)
        return result

    async def arun(self, task: str, **kwargs: Any) -> str:
        """Execute task asynchronously."""
        # Rust binding is synchronous, wrap in async
        import asyncio

        return await asyncio.to_thread(self.run, task, **kwargs)

    def register_tool(
        self,
        name: str,
        func: Callable,
        description: str = "",
        parameters: dict | None = None,
    ) -> None:
        """Register a custom tool."""
        self._tools[name] = func
        # Note: Rust binding tool registration may require additional setup
        logger.warning(
            f"Tool '{name}' registered but Rust binding tool setup is limited"
        )

    def create_session(self) -> RustSession:
        """Create a new session."""
        session_binding = self._agent.create_session()
        return RustSession.from_binding(session_binding)


class RustSession:
    """Rust-backed Session implementation."""

    def __init__(self, session_id: str | None = None):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._session = RustSessionBinding(id=session_id)

    @classmethod
    def from_binding(cls, binding: Any) -> RustSession:
        """Create from existing binding."""
        instance = cls.__new__(cls)
        instance._session = binding
        return instance

    def add_message(self, role: str, content: str) -> None:
        """Add a message."""
        if role == "user":
            self._session.add_user_message(content)
        else:
            self._session.add_assistant_message(content)

    def get_messages(self) -> list[dict[str, str]]:
        """Get all messages."""
        messages = self._session.get_messages()
        return [{"role": r, "content": c} for r, c in messages]

    def save(self) -> str:
        """Save session."""
        return self._session.export()

    @property
    def id(self) -> str:
        """Get session ID."""
        return self._session.id


class RustBuiltinTools:
    """Rust-backed BuiltinTools implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._executor = RustToolExecutor()

    def read_file(
        self, path: str, offset: int | None = None, limit: int | None = None
    ) -> str:
        """Read file."""
        return self._executor.read_file(path, offset, limit)

    def write_file(self, path: str, content: str) -> str:
        """Write file."""
        return self._executor.write_file(path, content)

    def edit_file(self, path: str, old: str, new: str) -> str:
        """Edit file."""
        args = json.dumps({"path": path, "old_string": old, "new_string": new})
        return self._executor.execute("edit_file", args)

    def grep(
        self, pattern: str, path: str | None = None, glob: str | None = None
    ) -> str:
        """Search content."""
        return self._executor.grep(pattern, path, glob)

    def glob(self, pattern: str, path: str | None = None) -> str:
        """Find files."""
        return self._executor.glob(pattern, path)

    def bash(
        self,
        command: str,
        timeout_ms: int | None = None,
        working_dir: str | None = None,
    ) -> str:
        """Execute command."""
        return self._executor.bash(command, timeout_ms, working_dir)

    def list_tools(self) -> list[dict[str, str]]:
        """List tools."""
        return self._executor.list_tools()

    def is_available(self, name: str) -> bool:
        """Check tool availability."""
        return self._executor.is_available(name)


class RustVectorStore:
    """Rust-backed VectorStore implementation."""

    def __init__(self, metric: str = "cosine"):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._store = RustVectorStoreBinding(metric=metric)

    def upsert(
        self, id: str, vector: list[float], metadata: dict[str, Any] | None = None
    ) -> bool:
        """Insert or update a vector."""
        import json

        return self._store.upsert(id, vector, json.dumps(metadata or {}))

    def search(self, vector: list[float], top_k: int = 10) -> list[dict[str, Any]]:
        """Search for similar vectors."""
        results = self._store.search(vector, top_k)
        return [dict(r) for r in results]

    def get(self, id: str) -> dict[str, Any] | None:
        """Get a vector by ID."""
        result = self._store.get(id)
        return dict(result) if result else None

    def delete(self, id: str) -> bool:
        """Delete a vector by ID."""
        return self._store.delete(id)

    def delete_batch(self, ids: list[str]) -> int:
        """Delete multiple vectors."""
        return self._store.delete_batch(ids)

    def clear(self) -> None:
        """Clear all items."""
        return self._store.clear()

    def count(self) -> int:
        """Get item count."""
        return self._store.count()


class RustRetrieverEngine:
    """Rust-backed RetrieverEngine implementation."""

    def __init__(self, embedding_dimension: int = 128):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._engine = RustRetrieverEngineBinding(
            embedding_dimension=embedding_dimension
        )

    def add_document(
        self, doc_id: str, content: str, metadata: dict[str, Any] | None = None
    ) -> None:
        """Add a document to the knowledge base."""
        import json

        self._engine.add_document(doc_id, content, json.dumps(metadata or {}))

    def retrieve(self, query: str, top_k: int = 5) -> list[dict[str, Any]]:
        """Retrieve similar documents."""
        results = self._engine.retrieve(query, top_k)
        return [dict(r) for r in results]

    def delete_document(self, doc_id: str) -> None:
        """Delete a document."""
        self._engine.delete_document(doc_id)

    def clear(self) -> None:
        """Clear all documents."""
        self._engine.clear()

    def count(self) -> int:
        """Get document count."""
        return self._engine.count()


class RustEmbeddings:
    """Rust-backed Embeddings implementation."""

    def __init__(
        self,
        provider: str = "openai",
        model: str | None = None,
        api_key: str | None = None,
        dimension: int | None = None,
    ):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._embeddings = RustEmbeddingsBinding(
            provider=provider, model=model, api_key=api_key, dimension=dimension
        )

    def embed(self, texts: list[str]) -> list[list[float]]:
        """Generate embeddings for texts."""
        return self._embeddings.embed(texts)

    def embed_query(self, query: str) -> list[float]:
        """Generate embedding for single query."""
        return self._embeddings.embed_query(query)


class RustDocumentLoader:
    """Rust-backed DocumentLoader implementation."""

    def __init__(self, loader_type: str = "text"):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._loader = RustDocumentLoaderBinding(loader_type=loader_type)

    def load(self, path: str) -> list[dict[str, Any]]:
        """Load documents from path."""
        results = self._loader.load(path)
        return [dict(r) for r in results]

    @property
    def supported_extensions(self) -> list[str]:
        """Get supported file extensions."""
        return list(self._loader.supported_extensions())


class RustTextSplitter:
    """Rust-backed TextSplitter implementation."""

    def __init__(self, chunk_size: int = 1000, chunk_overlap: int = 200):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._splitter = RustTextSplitterBinding(
            chunk_size=chunk_size, chunk_overlap=chunk_overlap
        )

    def split(self, text: str) -> list[str]:
        """Split text into chunks."""
        return self._splitter.split(text)

    def set_chunk_size(self, size: int) -> None:
        """Set chunk size."""
        self._splitter.set_chunk_size(size)

    def set_overlap(self, overlap: int) -> None:
        """Set chunk overlap."""
        self._splitter.set_overlap(overlap)

    @property
    def config(self) -> dict[str, int]:
        """Get configuration."""
        return dict(self._splitter.config())


# ============================================================================
# Layer 0: SecurityGateway
# ============================================================================


class RustSecurityGateway:
    """Rust-backed SecurityGateway implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._gateway = RustSecurityGatewayBinding()

    def validate_input(self, input_text: str) -> str:
        """Validate input for security issues."""
        return self._gateway.validate_input(input_text)


# ============================================================================
# Layer 1: LLM, Cost Tracking
# ============================================================================


class RustLlmClient:
    """Rust-backed LlmClient implementation."""

    def __init__(
        self,
        provider: str = "anthropic",
        api_key: str | None = None,
        base_url: str | None = None,
    ):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._client = RustLlmClientBinding(
            provider=provider, api_key=api_key, base_url=base_url
        )

    def connect(self) -> bool:
        """Connect and verify API."""
        return self._client.connect()

    def is_connected(self) -> bool:
        """Check if connected."""
        return self._client.is_connected()

    def send(
        self,
        messages: list[tuple[str, str]],
        config: RustLlmRequestConfig | None = None,
    ) -> RustLlmResponse:
        """Send messages and get response."""
        if config is None:
            config = RustLlmRequestConfig()
        response_binding = self._client.send(messages, config._config)
        return RustLlmResponse.from_binding(response_binding)

    def send_message(
        self, message: str, config: RustLlmRequestConfig | None = None
    ) -> RustLlmResponse:
        """Send a single message."""
        if config is None:
            config = RustLlmRequestConfig()
        response_binding = self._client.send_message(message, config._config)
        return RustLlmResponse.from_binding(response_binding)

    def provider_name(self) -> str:
        """Get provider name."""
        return self._client.provider_name()

    def supported_models(self) -> list[str]:
        """Get supported models."""
        return self._client.supported_models()


class RustLlmRequestConfig:
    """Rust-backed LlmRequestConfig implementation."""

    def __init__(
        self,
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
    ):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._config = RustLlmRequestConfigBinding(
            model=model or get_default_model("anthropic"),
            max_tokens=max_tokens,
            temperature=temperature,
            system_prompt=system_prompt,
        )

    @property
    def model(self) -> str:
        return self._config.model

    @property
    def max_tokens(self) -> int:
        return self._config.max_tokens

    @property
    def temperature(self) -> float:
        return self._config.temperature

    @property
    def system_prompt(self) -> str | None:
        return self._config.system_prompt


class RustLlmResponse:
    """Rust-backed LlmResponse implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")
        self._response = None

    @classmethod
    def from_binding(cls, binding: Any) -> RustLlmResponse:
        """Create from existing binding."""
        instance = cls.__new__(cls)
        instance._response = binding
        return instance

    @property
    def content(self) -> str:
        return self._response.content

    @property
    def input_tokens(self) -> int:
        return self._response.input_tokens

    @property
    def output_tokens(self) -> int:
        return self._response.output_tokens

    @property
    def model(self) -> str:
        return self._response.model

    @property
    def response_id(self) -> str:
        return self._response.response_id

    def total_tokens(self) -> int:
        """Get total tokens."""
        return self._response.total_tokens()

    def to_json(self) -> str:
        """Convert to JSON."""
        return self._response.to_json()


class RustCostTracker:
    """Rust-backed CostTracker implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._tracker = RustCostTrackerBinding()

    def set_budget_limit(self, limit: float) -> None:
        """Set budget limit."""
        self._tracker.set_budget_limit(limit)

    def record_usage(self, model: str, input_tokens: int, output_tokens: int) -> None:
        """Record usage."""
        self._tracker.record_usage(model, input_tokens, output_tokens)

    def get_current_usage(self) -> RustUsageSnapshot:
        """Get current usage snapshot."""
        snapshot_binding = self._tracker.get_current_usage()
        return RustUsageSnapshot.from_binding(snapshot_binding)

    def estimate_next_step(
        self, model: str, estimated_input: int, estimated_output: int
    ) -> RustCostEstimate:
        """Estimate cost for next step."""
        estimate_binding = self._tracker.estimate_next_step(
            model, estimated_input, estimated_output
        )
        return RustCostEstimate.from_binding(estimate_binding)

    def generate_report(self) -> str:
        """Generate cost report as JSON."""
        return self._tracker.generate_report()

    def reset(self) -> None:
        """Reset tracker."""
        self._tracker.reset()

    def total_cost(self) -> float:
        """Get total cost."""
        return self._tracker.total_cost()


class RustUsageSnapshot:
    """Rust-backed UsageSnapshot implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")
        self._snapshot = None

    @classmethod
    def from_binding(cls, binding: Any) -> RustUsageSnapshot:
        """Create from existing binding."""
        instance = cls.__new__(cls)
        instance._snapshot = binding
        return instance

    @property
    def total_input_tokens(self) -> int:
        return self._snapshot.total_input_tokens

    @property
    def total_output_tokens(self) -> int:
        return self._snapshot.total_output_tokens

    @property
    def total_cost_usd(self) -> float:
        return self._snapshot.total_cost_usd

    @property
    def budget_remaining(self) -> float | None:
        return self._snapshot.budget_remaining

    def model_costs(self) -> str:
        """Get model costs as JSON."""
        return self._snapshot.model_costs()


class RustCostEstimate:
    """Rust-backed CostEstimate implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")
        self._estimate = None

    @classmethod
    def from_binding(cls, binding: Any) -> RustCostEstimate:
        """Create from existing binding."""
        instance = cls.__new__(cls)
        instance._estimate = binding
        return instance

    @property
    def min_tokens(self) -> int:
        return self._estimate.min_tokens

    @property
    def max_tokens(self) -> int:
        return self._estimate.max_tokens

    @property
    def estimated_cost_usd(self) -> float:
        return self._estimate.estimated_cost_usd

    @property
    def confidence(self) -> str:
        return self._estimate.confidence


# ============================================================================
# Layer 2: AgentRuntime, SessionManager, CheckpointSystem
# ============================================================================


class RustAgentRuntime:
    """Rust-backed AgentRuntime implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._runtime = RustAgentRuntimeBinding()

    def run(self, task: str, config: dict[str, Any] | None = None) -> dict[str, Any]:
        """Run agent with task."""
        # Convert config dict to PyAgentConfig
        if config is None:
            config = {}
        result = self._runtime.run(task, self._make_config(config))
        return {
            "session_id": result.session_id,
            "final_state": result.final_state,
            "iterations": result.iterations,
            "tokens_used": result.tokens_used,
            "messages": result.get_messages(),
        }

    def start(self, task: str, config: dict[str, Any] | None = None) -> str:
        """Start agent session."""
        if config is None:
            config = {}
        return self._runtime.start(task, self._make_config(config))

    def pause(self, session_id: str) -> None:
        """Pause agent."""
        self._runtime.pause(session_id)

    def resume(self, session_id: str) -> None:
        """Resume agent."""
        self._runtime.resume(session_id)

    def stop(self, session_id: str) -> None:
        """Stop agent."""
        self._runtime.stop(session_id)

    def status(self, session_id: str) -> str:
        """Get agent status."""
        return self._runtime.status(session_id)

    def send_message(self, session_id: str, message: str) -> None:
        """Send message to agent."""
        self._runtime.send_message(session_id, message)

    def register_tool(
        self,
        name: str,
        description: str,
        callable_func: Callable,
        parameters: dict | None = None,
    ) -> None:
        """Register a Python tool."""
        self._runtime.register_tool(name, description, callable_func, parameters)

    def list_tools(self) -> list[str]:
        """List available tools."""
        return self._runtime.list_tools()

    def _make_config(self, config: dict[str, Any]) -> Any:
        """Make PyAgentConfig from dict."""
        from sh_python import AgentConfig

        return AgentConfig(
            agent_id=config.get("agent_id"),
            model=config.get("model")
            or get_default_model("openai"),  # Default to openai config
            temperature=config.get("temperature", 0.7),
            max_iterations=config.get("max_iterations", 100),
            system_prompt=config.get("system_prompt"),
        )


class RustSessionManager:
    """Rust-backed SessionManager implementation."""

    def __init__(self, max_sessions: int = 100):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._manager = RustSessionManagerBinding(max_sessions=max_sessions)

    def create(
        self, model: str | None = None, max_iterations: int | None = None
    ) -> str:
        """Create a new session."""
        return self._manager.create(model, max_iterations)

    def get(self, session_id: str) -> dict[str, Any] | None:
        """Get session info."""
        result = self._manager.get(session_id)
        if result is None:
            return None
        return json.loads(result)

    def delete(self, session_id: str) -> bool:
        """Delete session."""
        return self._manager.delete(session_id)

    def list(self) -> list[tuple[str, str, str]]:
        """List all sessions as (session_id, agent_id, state)."""
        return self._manager.list()

    def stats(self) -> tuple[int, int, int]:
        """Get session stats: (total, max, active)."""
        return self._manager.stats()

    def set_state(self, session_id: str, state: str) -> bool:
        """Set session state."""
        return self._manager.set_state(session_id, state)

    def add_message(self, session_id: str, role: str, content: str) -> bool:
        """Add message to session."""
        return self._manager.add_message(session_id, role, content)

    def get_messages(self, session_id: str) -> list[tuple[str, str]]:
        """Get session messages."""
        return self._manager.get_messages(session_id)


class RustCheckpointSystem:
    """Rust-backed CheckpointSystem implementation."""

    def __init__(self, storage_path: str | None = None):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._checkpoint = RustCheckpointSystemBinding(storage_path=storage_path)

    def save(self, session_id: str, data: str) -> str:
        """Save checkpoint, returns checkpoint ID."""
        return self._checkpoint.save(session_id, data)

    def load(self, session_id: str, checkpoint_id: str | None = None) -> str | None:
        """Load checkpoint."""
        return self._checkpoint.load(session_id, checkpoint_id)

    def list(self, session_id: str) -> list[str]:
        """List checkpoints for session."""
        return self._checkpoint.list(session_id)

    def delete(self, session_id: str, checkpoint_id: str) -> bool:
        """Delete checkpoint."""
        return self._checkpoint.delete(session_id, checkpoint_id)


# ============================================================================
# Layer 3: QueryEngine, MemorySystem, ToolExecutor
# ============================================================================


class RustQueryEngine:
    """Rust-backed QueryEngine implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._engine = RustQueryEngineBinding()

    def initialize(self, language: str, root_path: str) -> bool:
        """Initialize for a language."""
        return self._engine.initialize(language, root_path)

    def go_to_definition(
        self, language: str, file_path: str, line: int, column: int
    ) -> list[dict[str, Any]]:
        """Go to definition."""
        results = self._engine.go_to_definition(language, file_path, line, column)
        return [dict(r) for r in results]

    def find_references(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
        include_declaration: bool = True,
    ) -> list[dict[str, Any]]:
        """Find references."""
        results = self._engine.find_references(
            language, file_path, line, column, include_declaration
        )
        return [dict(r) for r in results]

    def hover(
        self, language: str, file_path: str, line: int, column: int
    ) -> str | None:
        """Get hover info."""
        return self._engine.hover(language, file_path, line, column)

    def get_document_symbols(
        self, language: str, file_path: str
    ) -> list[dict[str, Any]]:
        """Get document symbols (outline of classes, functions, etc.)."""
        results = self._engine.get_document_symbols(language, file_path)
        return [dict(r) for r in results]

    def full_symbol_info(
        self, language: str, file_path: str, line: int, column: int
    ) -> dict[str, Any]:
        """Get full symbol information including definition, references, and hover."""
        return dict(self._engine.full_symbol_info(language, file_path, line, column))

    def rename_symbol(
        self, language: str, file_path: str, line: int, column: int, new_name: str
    ) -> dict[str, Any] | None:
        """Rename a symbol across the workspace."""
        result = self._engine.rename_symbol(language, file_path, line, column, new_name)
        return dict(result) if result else None

    def open_document(self, language: str, file_path: str) -> None:
        """Open a document for editing."""
        self._engine.open_document(language, file_path)

    def close_document(self, language: str, file_path: str) -> None:
        """Close a document."""
        self._engine.close_document(language, file_path)

    def shutdown(self, language: str) -> None:
        """Shutdown language server."""
        self._engine.shutdown(language)

    def shutdown_all(self) -> None:
        """Shutdown all language servers."""
        self._engine.shutdown_all()

    def is_connected(self, language: str) -> bool:
        """Check if connected."""
        return self._engine.is_connected(language)


class RustMemorySystem:
    """Rust-backed MemorySystem implementation."""

    def __init__(self, session_id: str | None = None):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._memory = RustMemorySystemBinding(session_id=session_id)

    def store(self, tier: str, content: str) -> str:
        """Store memory, returns memory ID."""
        return self._memory.store(tier, content)

    def query(
        self, query: str, tier: str | None = None, limit: int | None = None
    ) -> list[dict[str, Any]]:
        """Query memory."""
        results = self._memory.query(query, tier, limit)
        return [dict(r) for r in results]

    def get(self, tier: str, memory_id: str) -> dict[str, Any] | None:
        """Get memory by ID."""
        result = self._memory.get(tier, memory_id)
        return dict(result) if result else None

    def delete(self, tier: str, memory_id: str) -> bool:
        """Delete a memory entry."""
        return self._memory.delete(tier, memory_id)

    def stats(self) -> dict[str, int]:
        """Get memory stats."""
        return dict(self._memory.stats())

    def clear(self, tier: str) -> int:
        """Clear tier, returns count cleared."""
        return self._memory.clear(tier)

    # Convenience methods for tier-specific operations
    def store_working(self, content: str) -> str:
        """Store in working memory."""
        return self._memory.store_working(content)

    def store_session(self, content: str) -> str:
        """Store in session memory."""
        return self._memory.store_session(content)

    def store_project(self, content: str) -> str:
        """Store in project memory."""
        return self._memory.store_project(content)

    def store_longterm(self, content: str) -> str:
        """Store in long-term memory."""
        return self._memory.store_longterm(content)

    def query_working(
        self, query: str, limit: int | None = None
    ) -> list[dict[str, Any]]:
        """Query working memory."""
        return [dict(r) for r in self._memory.query_working(query, limit)]

    def query_session(
        self, query: str, limit: int | None = None
    ) -> list[dict[str, Any]]:
        """Query session memory."""
        return [dict(r) for r in self._memory.query_session(query, limit)]

    def query_project(
        self, query: str, limit: int | None = None
    ) -> list[dict[str, Any]]:
        """Query project memory."""
        return [dict(r) for r in self._memory.query_project(query, limit)]

    def query_longterm(
        self, query: str, limit: int | None = None
    ) -> list[dict[str, Any]]:
        """Query long-term memory."""
        return [dict(r) for r in self._memory.query_longterm(query, limit)]


class RustToolExecutor:
    """Rust-backed ToolExecutor implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._executor = RustToolExecutorBinding()

    def execute(self, name: str, args: dict[str, Any]) -> str:
        """Execute a tool."""
        return self._executor.execute(name, json.dumps(args))

    def read_file(
        self, path: str, offset: int | None = None, limit: int | None = None
    ) -> str:
        """Read file."""
        return self._executor.read_file(path, offset, limit)

    def write_file(self, path: str, content: str) -> str:
        """Write file."""
        return self._executor.write_file(path, content)

    def bash(
        self,
        command: str,
        timeout_ms: int | None = None,
        working_dir: str | None = None,
    ) -> str:
        """Execute bash command."""
        return self._executor.bash(command, timeout_ms, working_dir)

    def grep(
        self, pattern: str, path: str | None = None, glob: str | None = None
    ) -> str:
        """Grep search."""
        return self._executor.grep(pattern, path, glob)

    def glob(self, pattern: str, path: str | None = None) -> str:
        """Glob find."""
        return self._executor.glob(pattern, path)

    def list_tools(self) -> list[tuple[str, str]]:
        """List available tools."""
        return self._executor.list_tools()

    def is_available(self, name: str) -> bool:
        """Check if tool is available."""
        return self._executor.is_available(name)


class RustVectorItem:
    """Rust-backed VectorItem implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")
        self._item = None

    @classmethod
    def from_binding(cls, binding: Any) -> RustVectorItem:
        """Create from existing binding."""
        instance = cls.__new__(cls)
        instance._item = binding
        return instance

    @property
    def id(self) -> str:
        return self._item.id

    @property
    def vector(self) -> list[float]:
        return list(self._item.vector)

    @property
    def content(self) -> str:
        return self._item.content

    def get_metadata(self) -> str:
        """Get metadata as JSON."""
        return self._item.get_metadata()


class RustSearchResult:
    """Rust-backed SearchResult implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")
        self._result = None

    @classmethod
    def from_binding(cls, binding: Any) -> RustSearchResult:
        """Create from existing binding."""
        instance = cls.__new__(cls)
        instance._result = binding
        return instance

    @property
    def id(self) -> str:
        return self._result.id

    @property
    def score(self) -> float:
        return self._result.score

    @property
    def content(self) -> str:
        return self._result.content

    def get_metadata(self) -> str:
        """Get metadata as JSON."""
        return self._result.get_metadata()


# ============================================================================
# Layer 4: McpBridge, AuditLogger
# ============================================================================


class RustMcpBridge:
    """Rust-backed McpBridge implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._bridge = RustMcpBridgeBinding()


class RustAuditLogger:
    """Rust-backed AuditLogger implementation."""

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._logger = RustAuditLoggerBinding()

    def log(self, user_id: str, action: str, resource_type: str) -> None:
        """Log an audit entry."""
        self._logger.log(user_id, action, resource_type)

    def count(self) -> int:
        """Get audit entry count."""
        return self._logger.count()


# ============================================================================
# Layer 0: Permission Management
# ============================================================================


class RustPermission:
    """Rust-backed Permission implementation."""

    def __init__(self, resource: str, action: str):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._permission = RustPermissionBinding(resource, action)

    @property
    def resource(self) -> str:
        return self._permission.resource

    @property
    def action(self) -> str:
        return self._permission.action

    def __repr__(self) -> str:
        return f"Permission(resource='{self.resource}', action='{self.action}')"

    def __eq__(self, other: object) -> bool:
        if isinstance(other, RustPermission):
            return self.resource == other.resource and self.action == other.action
        return False

    def __hash__(self) -> int:
        return hash((self.resource, self.action))


class RustRole:
    """Rust-backed Role implementation."""

    def __init__(self, name: str, permissions: list[RustPermission] | None = None):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        perms = permissions or []
        self._role = RustRoleBinding(name, [p._permission for p in perms])

    @property
    def name(self) -> str:
        return self._role.name

    @property
    def permissions(self) -> list[RustPermission]:
        return [RustPermission(p.resource, p.action) for p in self._role.permissions]

    def __repr__(self) -> str:
        return f"Role(name='{self.name}', permissions={len(self.permissions)})"


class RustPermissionManager:
    """Rust-backed PermissionManager implementation.

    RBAC permission management system.

    Example:
        >>> pm = RustPermissionManager()
        >>> pm.grant("user1", "admin")
        >>> pm.check("user1", "session", "read")
        True
        >>> pm.revoke("user1", "admin")
    """

    def __init__(self):
        if not HAS_BINDING:
            raise RuntimeError("Rust binding not available")

        self._manager = RustPermissionManagerBinding()

    def check(self, user_id: str, resource: str, action: str) -> bool:
        """Check if user has permission."""
        return self._manager.check(user_id, resource, action)

    def grant(self, user_id: str, role_name: str) -> None:
        """Grant role to user."""
        self._manager.grant(user_id, role_name)

    def revoke(self, user_id: str, role_name: str) -> None:
        """Revoke role from user."""
        self._manager.revoke(user_id, role_name)

    def create_role(self, role: RustRole) -> None:
        """Create custom role."""
        self._manager.create_role(role._role)

    def get_permissions(self, user_id: str) -> list[dict[str, str]]:
        """Get all permissions for user."""
        return [
            {"resource": p.resource, "action": p.action}
            for p in self._manager.get_permissions(user_id)
        ]

    def is_admin(self, user_id: str) -> bool:
        """Check if user has admin privileges."""
        return self._manager.is_admin(user_id)

    def get_user_roles(self, user_id: str) -> list[str]:
        """Get user's roles."""
        return list(self._manager.get_user_roles(user_id))


__all__ = [
    # Layer 0
    "RustSecurityGateway",
    "RustPermissionManager",
    "RustPermission",
    "RustRole",
    # Layer 1
    "RustLlmClient",
    "RustLlmRequestConfig",
    "RustLlmResponse",
    "RustCostTracker",
    "RustUsageSnapshot",
    "RustCostEstimate",
    # Layer 2
    "RustAgent",
    "RustSession",
    "RustAgentRuntime",
    "RustSessionManager",
    "RustCheckpointSystem",
    # Layer 3
    "RustBuiltinTools",
    "RustToolExecutor",
    "RustQueryEngine",
    "RustMemorySystem",
    "RustVectorStore",
    "RustVectorItem",
    "RustSearchResult",
    "RustRetrieverEngine",
    "RustDocumentLoader",
    "RustTextSplitter",
    "RustEmbeddings",
    # Layer 4
    "RustMcpBridge",
    "RustAuditLogger",
    # Constants
    "HAS_BINDING",
]
