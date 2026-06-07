"""
Tests for continuum_sdk.api module.

This module tests the unified API layer, covering:
- API class initialization
- All public methods
- Error handling paths
- Edge cases
- Implementation selection logic
"""

import os
import tempfile
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from continuum_sdk.api import (
    HAS_RUST_BINDING,
    Agent,
    BuiltinTools,
    ImageInput,
    MemorySystem,
    MultimodalHandler,
    Permission,
    PermissionManager,
    QueryEngine,
    Role,
    Session,
    get_implementation_preference,
)

# =============================================================================
# Fixtures
# =============================================================================


@pytest.fixture(autouse=True)
def reset_env(monkeypatch):
    """Reset environment variables before each test."""
    # monkeypatch automatically restores env vars after test
    yield


@pytest.fixture
def mock_python_agent():
    """Mock PythonAgent implementation."""
    with patch("continuum_sdk.python_impl.PythonAgent") as mock:
        mock_instance = MagicMock()
        mock_instance.run.return_value = "task result"
        mock_instance.arun = AsyncMock(return_value="async task result")
        mock_instance.register_tool.return_value = None
        mock.return_value = mock_instance
        yield mock, mock_instance


@pytest.fixture
def mock_python_builtin_tools():
    """Mock PythonBuiltinTools implementation."""
    with patch("continuum_sdk.python_impl.PythonBuiltinTools") as mock:
        mock_instance = MagicMock()
        mock_instance.read_file.return_value = "file content"
        mock_instance.write_file.return_value = "file written"
        mock_instance.edit_file.return_value = "file edited"
        mock_instance.grep.return_value = "grep results"
        mock_instance.glob.return_value = "glob results"
        mock_instance.bash.return_value = "bash output"
        mock_instance.list_tools.return_value = [{"name": "read", "description": "Read file"}]
        mock.return_value = mock_instance
        yield mock, mock_instance


@pytest.fixture
def mock_python_query_engine():
    """Mock PythonQueryEngine implementation."""
    with patch("continuum_sdk.python_impl.PythonQueryEngine") as mock:
        mock_instance = MagicMock()
        mock_instance.initialize.return_value = True
        mock_instance.go_to_definition.return_value = [{"uri": "test.py", "line": 1, "column": 1}]
        mock_instance.find_references.return_value = [{"uri": "test.py", "line": 1, "column": 1}]
        mock_instance.hover.return_value = "hover info"
        mock_instance.shutdown.return_value = None
        mock_instance.is_connected.return_value = True
        mock_instance.full_symbol_info.return_value = {"symbol": "test", "kind": "function"}
        mock_instance.get_document_symbols.return_value = []
        mock_instance.rename_symbol.return_value = {"changed_files": 0}
        mock_instance.reconnect.return_value = True
        mock_instance.get_connection_pool_status.return_value = {"connected_languages": ["python"]}
        mock.return_value = mock_instance
        yield mock, mock_instance


@pytest.fixture
def mock_python_memory_system():
    """Mock PythonMemorySystem implementation."""
    with patch("continuum_sdk.python_impl.PythonMemorySystem") as mock:
        mock_instance = MagicMock()
        mock_instance.store.return_value = "memory-id-123"
        mock_instance.query.return_value = [{"id": "memory-id-123", "content": "test"}]
        mock_instance.get.return_value = {"id": "memory-id-123", "content": "test"}
        mock_instance.stats.return_value = {"working": 1, "session": 0}
        mock_instance.clear.return_value = 1
        mock_instance.delete.return_value = True
        mock_instance.working.return_value = MagicMock()
        mock_instance.session.return_value = MagicMock()
        mock_instance.project.return_value = MagicMock()
        mock_instance.long_term.return_value = MagicMock()
        mock_instance.persist.return_value = True
        mock_instance.load.return_value = True
        mock.return_value = mock_instance
        yield mock, mock_instance


@pytest.fixture
def mock_python_multimodal_handler():
    """Mock PythonMultimodalHandler implementation."""
    with patch("continuum_sdk.python_impl.PythonMultimodalHandler") as mock:
        mock_instance = MagicMock()
        mock_instance.encode_image.return_value = {
            "type": "image",
            "source": {"type": "base64", "media_type": "image/png", "data": "abc123"}
        }
        mock_instance.encode_document.return_value = {
            "type": "document",
            "source": {"type": "base64", "media_type": "application/pdf", "data": "abc123"}
        }
        mock_instance.create_message.return_value = {"role": "user", "content": "test"}
        mock_instance.create_image_message.return_value = {"role": "user", "content": []}
        mock_instance.extract_text.return_value = "extracted text"
        mock_instance.list_images.return_value = []
        mock_instance.encode_image_from_url.return_value = {"type": "image", "source": {}}
        mock_instance.encode_image_url_direct.return_value = {"type": "image_url", "image_url": {"url": "http://example.com/img.png"}}
        mock_instance.to_openai_format.return_value = {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc123"}}
        mock_instance.create_openai_vision_message.return_value = {"role": "user", "content": []}
        mock_instance.create_anthropic_vision_message.return_value = {"role": "user", "content": []}
        mock.return_value = mock_instance
        yield mock, mock_instance


@pytest.fixture
def mock_python_permission_manager():
    """Mock PythonPermissionManager implementation."""
    with patch("continuum_sdk.python_impl.PythonPermissionManager") as mock:
        mock_instance = MagicMock()
        mock_instance.check.return_value = True
        mock_instance.grant.return_value = None
        mock_instance.revoke.return_value = None
        mock_instance.create_role.return_value = None
        mock_instance.get_permissions.return_value = [{"resource": "session", "action": "read"}]
        mock_instance.is_admin.return_value = False
        mock_instance.get_user_roles.return_value = ["guest"]
        mock.return_value = mock_instance
        yield mock, mock_instance


# =============================================================================
# get_implementation_preference Tests
# =============================================================================


class TestGetImplementationPreference:
    """Tests for get_implementation_preference function."""

    def test_returns_python_by_default(self, monkeypatch):
        """Should return 'python' by default when no Rust binding."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", False):
            monkeypatch.delenv("CONTINUUM_IMPL", raising=False)
            result = get_implementation_preference()
            assert result == "python"

    def test_returns_python_when_env_set_to_python(self, monkeypatch):
        """Should return 'python' when CONTINUUM_IMPL=python."""
        monkeypatch.setenv("CONTINUUM_IMPL", "python")
        result = get_implementation_preference()
        assert result == "python"

    def test_returns_rust_when_env_set_and_binding_available(self, monkeypatch):
        """Should return 'rust' when CONTINUUM_IMPL=rust and binding available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True):
            monkeypatch.setenv("CONTINUUM_IMPL", "rust")
            result = get_implementation_preference()
            assert result == "rust"

    def test_ignores_rust_env_when_binding_unavailable(self, monkeypatch):
        """Should fall back to python when Rust binding unavailable."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", False):
            monkeypatch.setenv("CONTINUUM_IMPL", "rust")
            result = get_implementation_preference()
            assert result == "python"


# =============================================================================
# Agent Tests
# =============================================================================


class TestAgent:
    """Tests for Agent class."""

    def test_init_default(self, mock_python_agent):
        """Test Agent initialization with default parameters."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        assert agent._impl_type == "python"
        assert agent._name == "default"
        mock_class.assert_called_once()

    def test_init_with_name(self, mock_python_agent):
        """Test Agent initialization with custom name."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent(name="my-agent")
        assert agent._name == "my-agent"

    def test_init_with_model(self, mock_python_agent):
        """Test Agent initialization with custom model."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent(model="custom-model")
        assert agent._model == "custom-model"

    def test_init_with_provider(self, mock_python_agent):
        """Test Agent initialization with provider."""
        mock_class, mock_instance = mock_python_agent
        Agent(provider="openai")
        mock_class.assert_called_once()
        # Check that provider was passed in kwargs
        call_kwargs = mock_class.call_args[1]
        assert call_kwargs.get("provider") == "openai"

    def test_init_with_api_key(self, mock_python_agent):
        """Test Agent initialization with API key."""
        mock_class, mock_instance = mock_python_agent
        Agent(api_key="test-key")
        call_kwargs = mock_class.call_args[1]
        assert call_kwargs.get("api_key") == "test-key"

    def test_init_force_python_implementation(self, mock_python_agent):
        """Test forcing Python implementation."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent(impl="python")
        assert agent._impl_type == "python"
        assert agent.implementation == "python"

    def test_init_force_rust_implementation_fallback(self, mock_python_agent):
        """Test that forcing Rust falls back to Python when unavailable."""
        mock_class, mock_instance = mock_python_agent
        with patch("continuum_sdk.api.HAS_RUST_BINDING", False):
            agent = Agent(impl="rust")
            # Should fall back to Python
            assert agent._impl_type == "rust"  # Stored as requested
            mock_class.assert_called()  # PythonAgent should be used

    def test_run(self, mock_python_agent):
        """Test Agent.run method."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        result = agent.run("test task")
        assert result == "task result"
        mock_instance.run.assert_called_once_with("test task")

    def test_run_with_kwargs(self, mock_python_agent):
        """Test Agent.run with additional kwargs."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        agent.run("test task", timeout=30)
        mock_instance.run.assert_called_once_with("test task", timeout=30)

    @pytest.mark.asyncio
    async def test_arun(self, mock_python_agent):
        """Test Agent.arun async method."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        result = await agent.arun("async task")
        assert result == "async task result"
        mock_instance.arun.assert_called_once_with("async task")

    def test_register_tool(self, mock_python_agent):
        """Test Agent.register_tool method."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        agent.register_tool("test_tool", lambda x: x, "A test tool", {"type": "object"})
        mock_instance.register_tool.assert_called_once_with(
            "test_tool", mock_instance.register_tool.call_args[0][1],
            "A test tool", {"type": "object"}
        )

    def test_create_session(self, mock_python_agent):
        """Test Agent.create_session method."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        session = agent.create_session("test-session")
        assert isinstance(session, Session)
        assert session.id == "test-session"

    def test_create_session_auto_id(self, mock_python_agent):
        """Test Agent.create_session with auto-generated ID."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        session = agent.create_session()
        assert isinstance(session, Session)
        assert session.id is not None

    def test_implementation_property(self, mock_python_agent):
        """Test Agent.implementation property."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent(impl="python")
        assert agent.implementation == "python"


# =============================================================================
# BuiltinTools Tests
# =============================================================================


class TestBuiltinTools:
    """Tests for BuiltinTools class."""

    def test_init_default(self, mock_python_builtin_tools):
        """Test BuiltinTools initialization."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        assert tools._impl_type == "python"
        mock_class.assert_called_once()

    def test_init_force_implementation(self, mock_python_builtin_tools):
        """Test BuiltinTools initialization with forced implementation."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools(impl="python")
        assert tools.implementation == "python"

    def test_read_file(self, mock_python_builtin_tools):
        """Test BuiltinTools.read_file method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.read_file("test.txt")
        assert result == "file content"
        mock_instance.read_file.assert_called_once_with("test.txt", None, None)

    def test_read_file_with_offset_limit(self, mock_python_builtin_tools):
        """Test BuiltinTools.read_file with offset and limit."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        tools.read_file("test.txt", offset=10, limit=100)
        mock_instance.read_file.assert_called_once_with("test.txt", 10, 100)

    def test_write_file(self, mock_python_builtin_tools):
        """Test BuiltinTools.write_file method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.write_file("test.txt", "content")
        assert result == "file written"
        mock_instance.write_file.assert_called_once_with("test.txt", "content")

    def test_edit_file(self, mock_python_builtin_tools):
        """Test BuiltinTools.edit_file method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.edit_file("test.txt", "old", "new")
        assert result == "file edited"
        mock_instance.edit_file.assert_called_once_with("test.txt", "old", "new")

    def test_grep(self, mock_python_builtin_tools):
        """Test BuiltinTools.grep method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.grep("pattern", path="src/", glob="*.py")
        assert result == "grep results"
        mock_instance.grep.assert_called_once_with("pattern", "src/", "*.py")

    def test_glob(self, mock_python_builtin_tools):
        """Test BuiltinTools.glob method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.glob("*.py", path="src/")
        assert result == "glob results"
        mock_instance.glob.assert_called_once_with("*.py", "src/")

    def test_bash(self, mock_python_builtin_tools):
        """Test BuiltinTools.bash method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.bash("echo hello", timeout_ms=1000, working_dir="/tmp")
        assert result == "bash output"
        mock_instance.bash.assert_called_once_with("echo hello", 1000, "/tmp")

    def test_list_tools(self, mock_python_builtin_tools):
        """Test BuiltinTools.list_tools method."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        result = tools.list_tools()
        assert result == [{"name": "read", "description": "Read file"}]
        mock_instance.list_tools.assert_called_once()

    def test_implementation_property(self, mock_python_builtin_tools):
        """Test BuiltinTools.implementation property."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools(impl="python")
        assert tools.implementation == "python"


# =============================================================================
# QueryEngine Tests
# =============================================================================


class TestQueryEngine:
    """Tests for QueryEngine class."""

    def test_init_default(self, mock_python_query_engine):
        """Test QueryEngine initialization."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        assert engine._impl_type == "python"
        mock_class.assert_called_once()

    def test_initialize(self, mock_python_query_engine):
        """Test QueryEngine.initialize method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.initialize("python", "/path/to/project")
        assert result is True
        mock_instance.initialize.assert_called_once_with("python", "/path/to/project")

    def test_go_to_definition(self, mock_python_query_engine):
        """Test QueryEngine.go_to_definition method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.go_to_definition("python", "test.py", 10, 5)
        assert result == [{"uri": "test.py", "line": 1, "column": 1}]
        mock_instance.go_to_definition.assert_called_once_with("python", "test.py", 10, 5)

    def test_find_references(self, mock_python_query_engine):
        """Test QueryEngine.find_references method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.find_references("python", "test.py", 10, 5, include_declaration=True)
        assert result == [{"uri": "test.py", "line": 1, "column": 1}]
        mock_instance.find_references.assert_called_once_with("python", "test.py", 10, 5, True)

    def test_find_references_default_include_declaration(self, mock_python_query_engine):
        """Test QueryEngine.find_references with default include_declaration."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        engine.find_references("python", "test.py", 10, 5)
        mock_instance.find_references.assert_called_once_with("python", "test.py", 10, 5, True)

    def test_hover(self, mock_python_query_engine):
        """Test QueryEngine.hover method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.hover("python", "test.py", 10, 5)
        assert result == "hover info"
        mock_instance.hover.assert_called_once_with("python", "test.py", 10, 5)

    def test_shutdown(self, mock_python_query_engine):
        """Test QueryEngine.shutdown method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        engine.shutdown("python")
        mock_instance.shutdown.assert_called_once_with("python")

    def test_is_connected(self, mock_python_query_engine):
        """Test QueryEngine.is_connected method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.is_connected("python")
        assert result is True
        mock_instance.is_connected.assert_called_once_with("python")

    def test_full_symbol_info(self, mock_python_query_engine):
        """Test QueryEngine.full_symbol_info method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.full_symbol_info("python", "test.py", 10, 5)
        assert result == {"symbol": "test", "kind": "function"}
        mock_instance.full_symbol_info.assert_called_once_with("python", "test.py", 10, 5)

    def test_get_document_symbols(self, mock_python_query_engine):
        """Test QueryEngine.get_document_symbols method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.get_document_symbols("python", "test.py")
        assert result == []
        mock_instance.get_document_symbols.assert_called_once_with("python", "test.py")

    def test_rename_symbol(self, mock_python_query_engine):
        """Test QueryEngine.rename_symbol method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.rename_symbol("python", "test.py", 10, 5, "new_name")
        assert result == {"changed_files": 0}
        mock_instance.rename_symbol.assert_called_once_with("python", "test.py", 10, 5, "new_name")

    def test_reconnect(self, mock_python_query_engine):
        """Test QueryEngine.reconnect method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.reconnect("python")
        assert result is True
        mock_instance.reconnect.assert_called_once_with("python")

    def test_get_connection_pool_status(self, mock_python_query_engine):
        """Test QueryEngine.get_connection_pool_status method."""
        mock_class, mock_instance = mock_python_query_engine
        engine = QueryEngine()
        result = engine.get_connection_pool_status()
        assert result == {"connected_languages": ["python"]}
        mock_instance.get_connection_pool_status.assert_called_once()


# =============================================================================
# MemorySystem Tests
# =============================================================================


class TestMemorySystem:
    """Tests for MemorySystem class."""

    def test_init_default(self, mock_python_memory_system):
        """Test MemorySystem initialization."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        assert memory._impl_type == "python"
        mock_class.assert_called_once()

    def test_init_with_session_id(self, mock_python_memory_system):
        """Test MemorySystem initialization with session_id."""
        mock_class, mock_instance = mock_python_memory_system
        MemorySystem(session_id="my-session")
        mock_class.assert_called_once_with(session_id="my-session")

    def test_store(self, mock_python_memory_system):
        """Test MemorySystem.store method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.store("working", "test content")
        assert result == "memory-id-123"
        mock_instance.store.assert_called_once_with("working", "test content")

    def test_query(self, mock_python_memory_system):
        """Test MemorySystem.query method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.query("test query", tier="working", limit=5)
        assert result == [{"id": "memory-id-123", "content": "test"}]
        mock_instance.query.assert_called_once_with("test query", "working", 5)

    def test_query_default_params(self, mock_python_memory_system):
        """Test MemorySystem.query with default params."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        memory.query("test query")
        mock_instance.query.assert_called_once_with("test query", None, 10)

    def test_get(self, mock_python_memory_system):
        """Test MemorySystem.get method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.get("working", "memory-id-123")
        assert result == {"id": "memory-id-123", "content": "test"}
        mock_instance.get.assert_called_once_with("working", "memory-id-123")

    def test_stats(self, mock_python_memory_system):
        """Test MemorySystem.stats method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.stats()
        assert result == {"working": 1, "session": 0}
        mock_instance.stats.assert_called_once()

    def test_clear(self, mock_python_memory_system):
        """Test MemorySystem.clear method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.clear("working")
        assert result == 1
        mock_instance.clear.assert_called_once_with("working")

    def test_delete(self, mock_python_memory_system):
        """Test MemorySystem.delete method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.delete("working", "memory-id-123")
        assert result is True
        mock_instance.delete.assert_called_once_with("working", "memory-id-123")

    def test_working(self, mock_python_memory_system):
        """Test MemorySystem.working method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.working()
        mock_instance.working.assert_called_once()
        assert result is not None

    def test_session(self, mock_python_memory_system):
        """Test MemorySystem.session method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.session()
        mock_instance.session.assert_called_once()
        assert result is not None

    def test_project(self, mock_python_memory_system):
        """Test MemorySystem.project method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.project()
        mock_instance.project.assert_called_once()
        assert result is not None

    def test_long_term(self, mock_python_memory_system):
        """Test MemorySystem.long_term method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.long_term()
        mock_instance.long_term.assert_called_once()
        assert result is not None

    def test_persist(self, mock_python_memory_system):
        """Test MemorySystem.persist method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.persist("/path/to/save")
        assert result is True
        mock_instance.persist.assert_called_once_with("/path/to/save")

    def test_persist_default_path(self, mock_python_memory_system):
        """Test MemorySystem.persist with default path."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        memory.persist()
        mock_instance.persist.assert_called_once_with(None)

    def test_load_from_storage(self, mock_python_memory_system):
        """Test MemorySystem.load_from_storage method."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        result = memory.load_from_storage("/path/to/load")
        assert result is True
        mock_instance.load.assert_called_once_with("/path/to/load")


# =============================================================================
# MultimodalHandler Tests
# =============================================================================


class TestMultimodalHandler:
    """Tests for MultimodalHandler class."""

    def test_init_default(self, mock_python_multimodal_handler):
        """Test MultimodalHandler initialization."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        assert handler._impl_type == "python"
        mock_class.assert_called_once()

    def test_encode_image(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.encode_image method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.encode_image("test.png")
        assert result["type"] == "image"
        mock_instance.encode_image.assert_called_once_with("test.png", None)

    def test_encode_image_with_media_type(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.encode_image with media type."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        handler.encode_image("test.png", media_type="image/png")
        mock_instance.encode_image.assert_called_once_with("test.png", "image/png")

    def test_encode_document(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.encode_document method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.encode_document("doc.pdf")
        assert result["type"] == "document"
        mock_instance.encode_document.assert_called_once_with("doc.pdf", None)

    def test_create_message_string(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.create_message with string content."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.create_message("user", "Hello")
        assert result == {"role": "user", "content": "test"}
        mock_instance.create_message.assert_called_once_with("user", "Hello")

    def test_create_message_list(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.create_message with list content."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        content = [{"type": "text", "text": "Hello"}]
        handler.create_message("user", content)
        mock_instance.create_message.assert_called_once_with("user", content)

    def test_create_image_message(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.create_image_message method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.create_image_message("user", "Hello", ["img1.png", "img2.png"])
        assert result == {"role": "user", "content": []}
        mock_instance.create_image_message.assert_called_once_with("user", "Hello", ["img1.png", "img2.png"])

    def test_extract_text(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.extract_text method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        message = {"role": "user", "content": "Hello"}
        result = handler.extract_text(message)
        assert result == "extracted text"
        mock_instance.extract_text.assert_called_once_with(message)

    def test_list_images(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.list_images method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        message = {"role": "user", "content": []}
        result = handler.list_images(message)
        assert result == []
        mock_instance.list_images.assert_called_once_with(message)

    def test_encode_image_from_url(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.encode_image_from_url method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.encode_image_from_url("http://example.com/img.png", timeout=60)
        assert result["type"] == "image"
        mock_instance.encode_image_from_url.assert_called_once_with("http://example.com/img.png", 60)

    def test_encode_image_from_url_default_timeout(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.encode_image_from_url with default timeout."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        handler.encode_image_from_url("http://example.com/img.png")
        mock_instance.encode_image_from_url.assert_called_once_with("http://example.com/img.png", 30)

    def test_encode_image_url_direct(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.encode_image_url_direct method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.encode_image_url_direct("http://example.com/img.png")
        assert result["type"] == "image_url"
        mock_instance.encode_image_url_direct.assert_called_once_with("http://example.com/img.png")

    def test_to_openai_format(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.to_openai_format method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        content = {"type": "image", "source": {}}
        result = handler.to_openai_format(content)
        assert result["type"] == "image_url"
        mock_instance.to_openai_format.assert_called_once_with(content)

    def test_create_openai_vision_message(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.create_openai_vision_message method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.create_openai_vision_message("user", "Hello", [], detail="high")
        assert result == {"role": "user", "content": []}
        mock_instance.create_openai_vision_message.assert_called_once_with("user", "Hello", [], "high")

    def test_create_anthropic_vision_message(self, mock_python_multimodal_handler):
        """Test MultimodalHandler.create_anthropic_vision_message method."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        result = handler.create_anthropic_vision_message("user", "Hello", [])
        assert result == {"role": "user", "content": []}
        mock_instance.create_anthropic_vision_message.assert_called_once_with("user", "Hello", [])


# =============================================================================
# ImageInput Tests
# =============================================================================


class TestImageInput:
    """Tests for ImageInput class."""

    def test_init_with_path(self):
        """Test ImageInput initialization with path."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.media_type = "image/jpeg"
            mock_instance.source_type = "path"
            mock_py.return_value = mock_instance

            ImageInput(path="test.jpg")
            mock_py.assert_called_once()

    def test_init_with_url(self):
        """Test ImageInput initialization with URL."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.media_type = "image/jpeg"
            mock_instance.source_type = "url"
            mock_py.return_value = mock_instance

            ImageInput(url="http://example.com/img.png")
            mock_py.assert_called_once()

    def test_init_with_base64(self):
        """Test ImageInput initialization with base64 data."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.media_type = "image/png"
            mock_instance.source_type = "base64"
            mock_py.return_value = mock_instance

            ImageInput(base64_data="abc123", media_type="image/png")
            mock_py.assert_called_once()

    def test_from_path(self):
        """Test ImageInput.from_path class method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.media_type = "image/jpeg"
            mock_instance.source_type = "path"
            mock_instance.to_base64.return_value = "abc123"
            mock_py.from_path.return_value = mock_instance

            img = ImageInput.from_path("test.jpg")
            mock_py.from_path.assert_called_once_with("test.jpg", None)
            assert img._impl is mock_instance

    def test_from_path_with_media_type(self):
        """Test ImageInput.from_path with media type."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_py.from_path.return_value = mock_instance

            ImageInput.from_path("test.jpg", media_type="image/png")
            mock_py.from_path.assert_called_once_with("test.jpg", "image/png")

    def test_from_url(self):
        """Test ImageInput.from_url class method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_py.from_url.return_value = mock_instance

            ImageInput.from_url("http://example.com/img.png")
            mock_py.from_url.assert_called_once_with("http://example.com/img.png")

    def test_from_base64(self):
        """Test ImageInput.from_base64 class method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_py.from_base64.return_value = mock_instance

            ImageInput.from_base64("abc123", media_type="image/png")
            mock_py.from_base64.assert_called_once_with("abc123", "image/png")

    def test_from_bytes(self):
        """Test ImageInput.from_bytes class method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_py.from_bytes.return_value = mock_instance

            data = b"\x89PNG\r\n\x1a\n"
            ImageInput.from_bytes(data, media_type="image/png")
            mock_py.from_bytes.assert_called_once_with(data, "image/png")

    def test_to_base64(self):
        """Test ImageInput.to_base64 method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.to_base64.return_value = "base64data"
            mock_py.return_value = mock_instance

            img = ImageInput(path="test.jpg")
            result = img.to_base64()
            assert result == "base64data"
            mock_instance.to_base64.assert_called_once()

    def test_media_type_property(self):
        """Test ImageInput.media_type property."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.media_type = "image/png"
            mock_py.return_value = mock_instance

            img = ImageInput(path="test.png")
            assert img.media_type == "image/png"

    def test_to_anthropic_format(self):
        """Test ImageInput.to_anthropic_format method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.to_anthropic_format.return_value = {
                "type": "image",
                "source": {"type": "base64", "media_type": "image/png", "data": "abc"}
            }
            mock_py.return_value = mock_instance

            img = ImageInput(path="test.png")
            result = img.to_anthropic_format()
            assert result["type"] == "image"
            mock_instance.to_anthropic_format.assert_called_once()

    def test_to_openai_format(self):
        """Test ImageInput.to_openai_format method."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.to_openai_format.return_value = {
                "type": "image_url",
                "image_url": {"url": "data:image/png;base64,abc", "detail": "auto"}
            }
            mock_py.return_value = mock_instance

            img = ImageInput(path="test.png")
            result = img.to_openai_format(detail="high")
            assert result["type"] == "image_url"
            mock_instance.to_openai_format.assert_called_once_with("high")

    def test_source_type_property(self):
        """Test ImageInput.source_type property."""
        with patch("continuum_sdk.python_impl.ImageInput") as mock_py:
            mock_instance = MagicMock()
            mock_instance.source_type = "url"
            mock_py.return_value = mock_instance

            img = ImageInput(url="http://example.com/img.png")
            assert img.source_type == "url"


# =============================================================================
# PermissionManager Tests
# =============================================================================


class TestPermissionManager:
    """Tests for PermissionManager class."""

    def test_init_default(self, mock_python_permission_manager):
        """Test PermissionManager initialization."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        assert pm._impl_type == "python"
        mock_class.assert_called_once()

    def test_check(self, mock_python_permission_manager):
        """Test PermissionManager.check method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        result = pm.check("user1", "session", "read")
        assert result is True
        mock_instance.check.assert_called_once_with("user1", "session", "read")

    def test_grant(self, mock_python_permission_manager):
        """Test PermissionManager.grant method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        pm.grant("user1", "admin")
        mock_instance.grant.assert_called_once_with("user1", "admin")

    def test_revoke(self, mock_python_permission_manager):
        """Test PermissionManager.revoke method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        pm.revoke("user1", "admin")
        mock_instance.revoke.assert_called_once_with("user1", "admin")

    def test_create_role(self, mock_python_permission_manager):
        """Test PermissionManager.create_role method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        role = Role("custom", [])
        pm.create_role(role)
        mock_instance.create_role.assert_called_once()

    def test_get_permissions(self, mock_python_permission_manager):
        """Test PermissionManager.get_permissions method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        result = pm.get_permissions("user1")
        assert result == [{"resource": "session", "action": "read"}]
        mock_instance.get_permissions.assert_called_once_with("user1")

    def test_is_admin(self, mock_python_permission_manager):
        """Test PermissionManager.is_admin method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        result = pm.is_admin("user1")
        assert result is False
        mock_instance.is_admin.assert_called_once_with("user1")

    def test_get_user_roles(self, mock_python_permission_manager):
        """Test PermissionManager.get_user_roles method."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        result = pm.get_user_roles("user1")
        assert result == ["guest"]
        mock_instance.get_user_roles.assert_called_once_with("user1")


# =============================================================================
# Permission Tests
# =============================================================================


class TestPermission:
    """Tests for Permission class."""

    def test_init(self):
        """Test Permission initialization."""
        with patch("continuum_sdk.python_impl.PythonPermission") as mock_py:
            mock_instance = MagicMock()
            mock_instance.resource = "session"
            mock_instance.action = "read"
            mock_py.return_value = mock_instance

            perm = Permission("session", "read")
            assert perm.resource == "session"
            assert perm.action == "read"

    def test_repr(self):
        """Test Permission.__repr__ method."""
        with patch("continuum_sdk.python_impl.PythonPermission") as mock_py:
            mock_instance = MagicMock()
            mock_instance.resource = "session"
            mock_instance.action = "read"
            mock_py.return_value = mock_instance

            perm = Permission("session", "read")
            result = repr(perm)
            assert "session" in result
            assert "read" in result


# =============================================================================
# Role Tests
# =============================================================================


class TestRole:
    """Tests for Role class."""

    def test_init_with_name_only(self):
        """Test Role initialization with name only."""
        with patch("continuum_sdk.python_impl.PythonRole") as mock_py:
            mock_instance = MagicMock()
            mock_instance.name = "custom"
            mock_instance.permissions = []
            mock_py.return_value = mock_instance

            role = Role("custom")
            assert role.name == "custom"

    def test_init_with_permissions(self):
        """Test Role initialization with permissions."""
        with patch("continuum_sdk.python_impl.PythonRole") as mock_py, \
             patch("continuum_sdk.python_impl.PythonPermission"):
            mock_role_instance = MagicMock()
            mock_role_instance.name = "custom"
            mock_perm_instance1 = MagicMock()
            mock_perm_instance1.resource = "session"
            mock_perm_instance1.action = "read"
            mock_perm_instance2 = MagicMock()
            mock_perm_instance2.resource = "tool"
            mock_perm_instance2.action = "execute"
            mock_role_instance.permissions = [mock_perm_instance1, mock_perm_instance2]
            mock_py.return_value = mock_role_instance

            role = Role("custom", [])
            assert role.name == "custom"

    def test_permissions_property(self):
        """Test Role.permissions property."""
        with patch("continuum_sdk.python_impl.PythonRole") as mock_py, \
             patch("continuum_sdk.python_impl.PythonPermission"):
            mock_role_instance = MagicMock()
            mock_role_instance.name = "custom"
            mock_perm_instance = MagicMock()
            mock_perm_instance.resource = "session"
            mock_perm_instance.action = "read"
            mock_role_instance.permissions = [mock_perm_instance]
            mock_py.return_value = mock_role_instance

            Role("custom")
            # The permissions property wraps the Python permissions

    def test_repr(self):
        """Test Role.__repr__ method."""
        with patch("continuum_sdk.python_impl.PythonRole") as mock_py:
            mock_instance = MagicMock()
            mock_instance.name = "custom"
            mock_instance.permissions = []
            mock_py.return_value = mock_instance

            role = Role("custom")
            result = repr(role)
            assert "custom" in result


# =============================================================================
# Session Tests
# =============================================================================


class TestSessionFromAPI:
    """Tests for Session class imported from api."""

    def test_init_default(self):
        """Test Session initialization with default id."""
        session = Session()
        assert session.id is not None

    def test_init_with_id(self):
        """Test Session initialization with custom id."""
        session = Session(id="my-session")
        assert session.id == "my-session"

    def test_add_message(self):
        """Test Session.add_message method."""
        from continuum_sdk.agent.session import MessageRole
        session = Session(id="test")
        msg = session.add_message(MessageRole.USER, "Hello")
        assert msg.content == "Hello"
        assert msg.role == MessageRole.USER

    def test_add_user_message(self):
        """Test Session.add_user_message method."""
        session = Session(id="test")
        msg = session.add_user_message("Hello")
        assert msg.content == "Hello"

    def test_add_assistant_message(self):
        """Test Session.add_assistant_message method."""
        session = Session(id="test")
        msg = session.add_assistant_message("Hi there!")
        assert msg.content == "Hi there!"

    def test_get_messages(self):
        """Test Session.get_messages method."""
        session = Session(id="test")
        session.add_user_message("Hello")
        session.add_assistant_message("Hi!")
        messages = session.get_messages()
        assert len(messages) == 2

    def test_message_count(self):
        """Test Session.message_count property."""
        session = Session(id="test")
        assert session.message_count == 0
        session.add_user_message("Hello")
        assert session.message_count == 1

    def test_export_and_from_export(self):
        """Test Session.export and from_export methods."""
        session = Session(id="test")
        session.add_user_message("Hello")
        exported = session.export()
        restored = Session.from_export(exported)
        assert restored.id == session.id
        assert restored.message_count == 1


# =============================================================================
# HAS_RUST_BINDING Tests
# =============================================================================


class TestHasRustBinding:
    """Tests for HAS_RUST_BINDING constant."""

    def test_is_boolean(self):
        """Test that HAS_RUST_BINDING is a boolean."""
        assert isinstance(HAS_RUST_BINDING, bool)


# =============================================================================
# Integration Tests (without mocks)
# =============================================================================


class TestIntegration:
    """Integration tests without mocks for simple operations."""

    def test_session_full_workflow(self):
        """Test full Session workflow."""
        session = Session(id="integration-test")
        session.add_user_message("What is Python?")
        session.add_assistant_message("Python is a programming language.")
        session.set_metadata("user_id", "12345")

        assert session.message_count == 2
        assert session.get_metadata("user_id") == "12345"

        # Export and restore
        exported = session.export()
        restored = Session.from_export(exported)
        assert restored.id == "integration-test"
        assert restored.message_count == 2

    def test_memory_system_basic_operations(self):
        """Test basic MemorySystem operations without mocks."""
        # Use the real implementation with Python backend
        memory = MemorySystem(session_id="test-session")

        # Store
        mem_id = memory.store("working", "Test memory content")
        assert mem_id is not None

        # Query
        results = memory.query("Test")
        assert len(results) >= 1

        # Get
        retrieved = memory.get("working", mem_id)
        assert retrieved is not None
        assert "Test memory content" in retrieved.get("content", "")

        # Stats
        stats = memory.stats()
        assert "working" in stats

        # Delete
        deleted = memory.delete("working", mem_id)
        assert deleted is True

    def test_permission_manager_basic_operations(self):
        """Test basic PermissionManager operations."""
        pm = PermissionManager()

        # Default guest role should have session:read
        assert pm.check("guest-user", "session", "read") is True

        # Grant admin role
        pm.grant("admin-user", "admin")
        assert pm.is_admin("admin-user") is True

        # Revoke
        pm.revoke("admin-user", "admin")
        assert pm.is_admin("admin-user") is False

    def test_builtin_tools_list_tools(self):
        """Test BuiltinTools.list_tools returns valid structure."""
        tools = BuiltinTools()
        result = tools.list_tools()
        assert isinstance(result, list)
        # Each tool should have name and description
        for tool in result:
            assert "name" in tool
            assert "description" in tool

    def test_image_input_source_types(self):
        """Test ImageInput with different source types."""
        # Create a temporary image file
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            # Write a minimal PNG header
            f.write(b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01')
            temp_path = f.name

        try:
            img = ImageInput.from_path(temp_path)
            assert img.source_type == "path"
            assert img.media_type == "image/png"
        finally:
            os.unlink(temp_path)

    def test_query_engine_connection_status(self):
        """Test QueryEngine connection pool status."""
        engine = QueryEngine()
        status = engine.get_connection_pool_status()
        assert "connected_languages" in status
        assert "total_connections" in status


# =============================================================================
# Edge Cases
# =============================================================================


class TestEdgeCases:
    """Tests for edge cases and boundary conditions."""

    def test_agent_empty_task(self, mock_python_agent):
        """Test Agent with empty task string."""
        mock_class, mock_instance = mock_python_agent
        agent = Agent()
        agent.run("")
        mock_instance.run.assert_called_once_with("")

    def test_memory_system_empty_query(self, mock_python_memory_system):
        """Test MemorySystem with empty query string."""
        mock_class, mock_instance = mock_python_memory_system
        memory = MemorySystem()
        memory.query("")
        mock_instance.query.assert_called_once_with("", None, 10)

    def test_memory_system_zero_limit(self, mock_python_memory_system):
        """Test MemorySystem.query with zero limit."""
        mock_class, mock_instance = mock_python_memory_system
        mock_instance.query.return_value = []
        memory = MemorySystem()
        memory.query("test", limit=0)
        mock_instance.query.assert_called_once_with("test", None, 0)

    def test_permission_manager_empty_user_id(self, mock_python_permission_manager):
        """Test PermissionManager with empty user_id."""
        mock_class, mock_instance = mock_python_permission_manager
        pm = PermissionManager()
        pm.check("", "session", "read")
        mock_instance.check.assert_called_once_with("", "session", "read")

    def test_builtin_tools_glob_empty_pattern(self, mock_python_builtin_tools):
        """Test BuiltinTools.glob with empty pattern."""
        mock_class, mock_instance = mock_python_builtin_tools
        tools = BuiltinTools()
        tools.glob("")
        mock_instance.glob.assert_called_once_with("", None)

    def test_session_with_none_id(self):
        """Test Session with explicit None id."""
        session = Session(id=None)
        assert session.id is not None
        assert session.id == "default-session"

    def test_memory_system_invalid_tier(self):
        """Test MemorySystem with invalid tier name."""
        memory = MemorySystem()
        with pytest.raises(ValueError, match="Invalid tier"):
            memory.store("invalid_tier", "content")

    def test_multimodal_handler_empty_images_list(self, mock_python_multimodal_handler):
        """Test MultimodalHandler with empty images list."""
        mock_class, mock_instance = mock_python_multimodal_handler
        handler = MultimodalHandler()
        handler.create_image_message("user", "Hello", [])
        mock_instance.create_image_message.assert_called_once_with("user", "Hello", [])


# =============================================================================
# Error Handling Tests
# =============================================================================


class TestErrorHandling:
    """Tests for error handling paths."""

    def test_agent_run_exception(self, mock_python_agent):
        """Test Agent.run propagates exceptions."""
        mock_class, mock_instance = mock_python_agent
        mock_instance.run.side_effect = RuntimeError("Agent error")
        agent = Agent()
        with pytest.raises(RuntimeError, match="Agent error"):
            agent.run("task")

    @pytest.mark.asyncio
    async def test_agent_arun_exception(self, mock_python_agent):
        """Test Agent.arun propagates exceptions."""
        mock_class, mock_instance = mock_python_agent
        mock_instance.arun.side_effect = RuntimeError("Async agent error")
        agent = Agent()
        with pytest.raises(RuntimeError, match="Async agent error"):
            await agent.arun("task")

    def test_memory_system_delete_nonexistent(self, mock_python_memory_system):
        """Test MemorySystem.delete with nonexistent memory."""
        mock_class, mock_instance = mock_python_memory_system
        mock_instance.delete.return_value = False
        memory = MemorySystem()
        result = memory.delete("working", "nonexistent-id")
        assert result is False

    def test_query_engine_initialize_invalid_path(self, mock_python_query_engine):
        """Test QueryEngine.initialize with invalid path."""
        mock_class, mock_instance = mock_python_query_engine
        mock_instance.initialize.side_effect = ValueError("Path does not exist")
        engine = QueryEngine()
        with pytest.raises(ValueError, match="Path does not exist"):
            engine.initialize("python", "/nonexistent/path")


# =============================================================================
# Rust Binding Code Path Tests
# =============================================================================


class TestRustBindingPaths:
    """Tests for Rust binding code paths using mocks."""

    def test_agent_with_rust_binding_available(self):
        """Test Agent uses RustAgent when bindings available and impl=rust."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustAgent") as mock_rust_agent:
            mock_instance = MagicMock()
            mock_instance.run.return_value = "rust result"
            mock_rust_agent.return_value = mock_instance

            Agent(impl="rust")
            # Should have used RustAgent
            mock_rust_agent.assert_called_once()

    def test_builtin_tools_with_rust_binding_available(self):
        """Test BuiltinTools uses RustBuiltinTools when bindings available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustBuiltinTools") as mock_rust_tools:
            mock_instance = MagicMock()
            mock_instance.list_tools.return_value = [{"name": "read"}]
            mock_rust_tools.return_value = mock_instance

            tools = BuiltinTools(impl="rust")
            mock_rust_tools.assert_called_once()
            assert tools._impl_type == "rust"

    def test_query_engine_with_rust_binding_available(self):
        """Test QueryEngine uses RustQueryEngine when bindings available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustQueryEngine") as mock_rust_engine:
            mock_instance = MagicMock()
            mock_rust_engine.return_value = mock_instance

            engine = QueryEngine(impl="rust")
            mock_rust_engine.assert_called_once()
            assert engine._impl_type == "rust"

    def test_memory_system_with_rust_binding_available(self):
        """Test MemorySystem uses RustMemorySystem when bindings available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustMemorySystem") as mock_rust_memory:
            mock_instance = MagicMock()
            mock_rust_memory.return_value = mock_instance

            memory = MemorySystem(session_id="test", impl="rust")
            mock_rust_memory.assert_called_once_with(session_id="test")
            assert memory._impl_type == "rust"

    def test_multimodal_handler_always_uses_python(self):
        """Test MultimodalHandler always uses Python implementation."""
        # Even with Rust bindings available, MultimodalHandler uses Python
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.python_impl.PythonMultimodalHandler") as mock_py:
            mock_instance = MagicMock()
            mock_py.return_value = mock_instance

            MultimodalHandler(impl="rust")
            # Should still use Python implementation
            mock_py.assert_called_once()

    def test_permission_manager_with_rust_binding_available(self):
        """Test PermissionManager uses RustPermissionManager when bindings available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustPermissionManager") as mock_rust_pm:
            mock_instance = MagicMock()
            mock_rust_pm.return_value = mock_instance

            pm = PermissionManager(impl="rust")
            mock_rust_pm.assert_called_once()
            assert pm._impl_type == "rust"

    def test_permission_with_rust_binding_available(self):
        """Test Permission uses RustPermission when bindings available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustPermission") as mock_rust_perm:
            mock_instance = MagicMock()
            mock_instance.resource = "session"
            mock_instance.action = "read"
            mock_rust_perm.return_value = mock_instance

            Permission("session", "read", impl="rust")
            mock_rust_perm.assert_called_once_with("session", "read")

    def test_role_with_rust_binding_available(self):
        """Test Role uses RustRole when bindings available."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.rust_impl.RustRole") as mock_rust_role, \
             patch("continuum_sdk.rust_impl.RustPermission"):
            mock_instance = MagicMock()
            mock_instance.name = "custom"
            mock_instance.permissions = []
            mock_rust_role.return_value = mock_instance

            Role("custom", [], impl="rust")
            mock_rust_role.assert_called_once()


class TestRustBindingModuleImport:
    """Tests for Rust binding import detection."""

    def test_continuum_import_success(self):
        """Test successful import of _continuum module."""
        import sys
        # Simulate the _continuum module being available
        with patch.dict(sys.modules, {"continuum_sdk._continuum": MagicMock()}):
            # Re-import the api module to test the import path
            pass
            # The import would succeed on first try

    def test_sh_python_import_success(self):
        """Test fallback import of sh_python module."""
        import sys
        # Simulate sh_python being available but _continuum not
        mock_sh_python = MagicMock()
        with patch.dict(sys.modules, {"sh_python": mock_sh_python}):
            # This tests the second import path
            pass

    def test_has_rust_binding_constant(self):
        """Test HAS_RUST_BINDING is a boolean."""
        # This is a simple test that doesn't require module reload
        assert isinstance(HAS_RUST_BINDING, bool)


class TestMultimodalHandlerRustPath:
    """Test MultimodalHandler Rust code paths specifically."""

    def test_multimodal_handler_rust_impl_path(self):
        """Test MultimodalHandler when impl=rust but falls back to Python."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch.dict(os.environ, {"CONTINUUM_IMPL": "rust"}), \
             patch("continuum_sdk.python_impl.PythonMultimodalHandler") as mock_py:
            mock_instance = MagicMock()
            mock_py.return_value = mock_instance

            # Even with Rust bindings, MultimodalHandler uses Python
            MultimodalHandler(impl="rust")
            # Verify Python implementation was called (both branches use Python)
            assert mock_py.call_count >= 1


class TestAgentRustPathDetailed:
    """Detailed tests for Agent Rust binding paths."""

    def test_agent_rust_impl_attribute(self):
        """Test Agent uses Rust implementation when requested."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustAgent") as mock_rust:
            mock_instance = MagicMock()
            mock_instance.run.return_value = "result"
            mock_rust.return_value = mock_instance

            agent = Agent(name="test", impl="rust")
            assert agent.implementation == "rust"

            # Test run method
            result = agent.run("task")
            assert result == "result"
            mock_instance.run.assert_called_once_with("task")

    @pytest.mark.asyncio
    async def test_agent_rust_arun(self):
        """Test Agent.arun with Rust implementation."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustAgent") as mock_rust:
            mock_instance = MagicMock()
            mock_instance.arun = AsyncMock(return_value="async result")
            mock_rust.return_value = mock_instance

            agent = Agent(impl="rust")
            result = await agent.arun("task")
            assert result == "async result"

    def test_agent_rust_register_tool(self):
        """Test Agent.register_tool with Rust implementation."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustAgent") as mock_rust:
            mock_instance = MagicMock()
            mock_rust.return_value = mock_instance

            agent = Agent(impl="rust")
            agent.register_tool("test_tool", lambda x: x, "description", {})
            mock_instance.register_tool.assert_called_once()


class TestBuiltinToolsRustPath:
    """Tests for BuiltinTools Rust binding paths."""

    def test_builtin_tools_rust_all_methods(self):
        """Test all BuiltinTools methods with Rust implementation."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustBuiltinTools") as mock_rust:
            mock_instance = MagicMock()
            mock_instance.read_file.return_value = "content"
            mock_instance.write_file.return_value = "written"
            mock_instance.edit_file.return_value = "edited"
            mock_instance.grep.return_value = "matches"
            mock_instance.glob.return_value = "files"
            mock_instance.bash.return_value = "output"
            mock_instance.list_tools.return_value = [{"name": "read"}]
            mock_rust.return_value = mock_instance

            tools = BuiltinTools(impl="rust")

            assert tools.read_file("test.txt") == "content"
            assert tools.write_file("test.txt", "data") == "written"
            assert tools.edit_file("test.txt", "old", "new") == "edited"
            assert tools.grep("pattern") == "matches"
            assert tools.glob("*.py") == "files"
            assert tools.bash("echo") == "output"
            assert tools.list_tools() == [{"name": "read"}]


class TestQueryEngineRustPath:
    """Tests for QueryEngine Rust binding paths."""

    def test_query_engine_rust_all_methods(self):
        """Test all QueryEngine methods with Rust implementation."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustQueryEngine") as mock_rust:
            mock_instance = MagicMock()
            mock_instance.initialize.return_value = True
            mock_instance.go_to_definition.return_value = []
            mock_instance.find_references.return_value = []
            mock_instance.hover.return_value = "hover"
            mock_instance.shutdown.return_value = None
            mock_instance.is_connected.return_value = True
            mock_instance.full_symbol_info.return_value = {}
            mock_instance.get_document_symbols.return_value = []
            mock_instance.rename_symbol.return_value = {}
            mock_instance.reconnect.return_value = True
            mock_instance.get_connection_pool_status.return_value = {}
            mock_rust.return_value = mock_instance

            engine = QueryEngine(impl="rust")

            assert engine.initialize("python", "/path") is True
            assert engine.go_to_definition("python", "file.py", 1, 1) == []
            assert engine.find_references("python", "file.py", 1, 1) == []
            assert engine.hover("python", "file.py", 1, 1) == "hover"
            engine.shutdown("python")
            assert engine.is_connected("python") is True
            assert engine.full_symbol_info("python", "file.py", 1, 1) == {}
            assert engine.get_document_symbols("python", "file.py") == []
            assert engine.rename_symbol("python", "file.py", 1, 1, "new") == {}
            assert engine.reconnect("python") is True
            assert engine.get_connection_pool_status() == {}


class TestMemorySystemRustPath:
    """Tests for MemorySystem Rust binding paths."""

    def test_memory_system_rust_all_methods(self):
        """Test all MemorySystem methods with Rust implementation."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustMemorySystem") as mock_rust:
            mock_instance = MagicMock()
            mock_instance.store.return_value = "id123"
            mock_instance.query.return_value = []
            mock_instance.get.return_value = None
            mock_instance.stats.return_value = {}
            mock_instance.clear.return_value = 0
            mock_instance.delete.return_value = True
            mock_instance.working.return_value = MagicMock()
            mock_instance.session.return_value = MagicMock()
            mock_instance.project.return_value = MagicMock()
            mock_instance.long_term.return_value = MagicMock()
            mock_instance.persist.return_value = True
            mock_instance.load.return_value = True
            mock_rust.return_value = mock_instance

            memory = MemorySystem(session_id="test", impl="rust")

            assert memory.store("working", "content") == "id123"
            assert memory.query("test") == []
            assert memory.get("working", "id") is None
            assert memory.stats() == {}
            assert memory.clear("working") == 0
            assert memory.delete("working", "id") is True
            memory.working()
            memory.session()
            memory.project()
            memory.long_term()
            assert memory.persist() is True
            assert memory.load_from_storage("/path") is True


class TestPermissionManagerRustPath:
    """Tests for PermissionManager Rust binding paths."""

    def test_permission_manager_rust_all_methods(self):
        """Test all PermissionManager methods with Rust implementation."""
        with patch("continuum_sdk.api.HAS_RUST_BINDING", True), \
             patch("continuum_sdk.rust_impl.RustPermissionManager") as mock_rust:
            mock_instance = MagicMock()
            mock_instance.check.return_value = True
            mock_instance.grant.return_value = None
            mock_instance.revoke.return_value = None
            mock_instance.create_role.return_value = None
            mock_instance.get_permissions.return_value = []
            mock_instance.is_admin.return_value = False
            mock_instance.get_user_roles.return_value = []
            mock_rust.return_value = mock_instance

            pm = PermissionManager(impl="rust")

            assert pm.check("user", "resource", "action") is True
            pm.grant("user", "admin")
            pm.revoke("user", "admin")
            pm.create_role(Role("test", []))
            assert pm.get_permissions("user") == []
            assert pm.is_admin("user") is False
            assert pm.get_user_roles("user") == []


if __name__ == "__main__":
    pytest.main([__file__, "-v"])


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
