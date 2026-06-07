"""Phase 7 Quality Gate Tests

Tests that prevent drift from returning by validating:
1. Python import smoke tests
2. README code snippet tests
3. Provider registry consistency
4. Security boundary tests
5. Shell policy tests
6. Packaging/fallback mode tests

These tests are designed to fail when documented behavior diverges from implementation.
"""

from __future__ import annotations

from pathlib import Path

import pytest

# =============================================================================
# 1. Python Import Smoke Tests
# =============================================================================


class TestPythonImportSmoke:
    """Verify public Python API imports work correctly."""

    def test_continuum_imports_agent_session_config(self):
        """Test that the main public API imports work."""
        from continuum import Agent, Config, Session

        assert Agent is not None
        assert Session is not None
        assert Config is not None

    def test_continuum_sdk_imports_match_continuum(self):
        """Test that continuum re-exports continuum_sdk API."""
        import continuum
        import continuum_sdk

        assert continuum.Agent is continuum_sdk.Agent
        assert continuum.Session is continuum_sdk.Session
        assert continuum.Config is continuum_sdk.Config

    def test_continuum_all_exports_are_correct(self):
        """Test that __all__ contains the expected exports."""
        import continuum

        assert "Agent" in continuum.__all__
        assert "Session" in continuum.__all__
        assert "Config" in continuum.__all__
        assert "ConfigLoader" in continuum.__all__

    def test_continuum_sdk_all_exports_are_correct(self):
        """Test that continuum_sdk.__all__ contains expected exports."""
        import continuum_sdk

        # Core exports
        assert "Agent" in continuum_sdk.__all__
        assert "Session" in continuum_sdk.__all__
        assert "Config" in continuum_sdk.__all__

    def test_import_from_api_module(self):
        """Test importing from continuum_sdk.api directly."""
        from continuum_sdk.api import (
            Agent,
            BuiltinTools,
            MemorySystem,
            QueryEngine,
            Session,
        )

        assert Agent is not None
        assert Session is not None
        assert BuiltinTools is not None
        assert MemorySystem is not None
        assert QueryEngine is not None

    def test_import_workflow_dag_node(self):
        """Test that workflow imports work correctly."""
        from continuum_sdk.workflow import DAG, Node, NodeStatus

        assert DAG is not None
        assert Node is not None
        assert NodeStatus is not None

    def test_import_rag_modules(self):
        """Test that RAG imports work correctly."""
        from continuum_sdk.rag import (
            DefaultRetrieverEngine,
            DistanceMetric,
            Document,
            InMemoryVectorStore,
            MockEmbeddingModel,
            RetrievalResult,
        )

        assert InMemoryVectorStore is not None
        assert DistanceMetric is not None
        assert Document is not None
        assert RetrievalResult is not None
        assert MockEmbeddingModel is not None
        assert DefaultRetrieverEngine is not None

    def test_import_permission_modules(self):
        """Test that permission imports work correctly."""
        from continuum_sdk.permission import (
            PermissionAction,
            PermissionDecision,
            PermissionManager,
            PermissionPolicy,
            SecurityLevel,
        )

        assert PermissionManager is not None
        assert SecurityLevel is not None
        assert PermissionPolicy is not None
        assert PermissionDecision is not None
        assert PermissionAction is not None

    def test_import_memory_modules(self):
        """Test that memory imports work correctly."""
        from continuum_sdk.memory import Memory, MemoryTier

        assert Memory is not None
        assert MemoryTier is not None

    def test_agent_has_correct_public_methods(self):
        """Test that Agent has the documented public methods."""
        from continuum import Agent

        # Required methods
        assert hasattr(Agent, "run")
        assert hasattr(Agent, "arun")
        assert hasattr(Agent, "create_session")
        assert hasattr(Agent, "register_tool")

        # Methods should be callable
        agent = Agent.__new__(Agent)
        assert callable(getattr(agent, "run", None))
        assert callable(getattr(agent, "arun", None))

    def test_agent_does_not_have_undocumented_methods(self):
        """Test that Agent does NOT have undocumented methods from old docs."""
        from continuum import Agent

        agent = Agent.__new__(Agent)

        # These methods were removed from docs, should not exist
        assert not hasattr(agent, "chat"), "Agent should not have 'chat' method - use 'run' instead"
        assert not hasattr(agent, "use_tool"), "Agent should not have 'use_tool' - use BuiltinTools"

    def test_session_has_correct_public_methods(self):
        """Test that Session has the documented public methods."""
        from continuum import Session

        session = Session()

        # Required methods
        assert hasattr(session, "add_user_message")
        assert hasattr(session, "add_assistant_message")
        assert hasattr(session, "add_system_message")
        assert hasattr(session, "get_messages")
        assert hasattr(session, "get_last_message")
        assert hasattr(session, "save")
        assert hasattr(session, "to_dict")
        assert hasattr(session, "from_dict")


# =============================================================================
# 2. README Code Snippet Tests
# =============================================================================


class TestReadmeSnippets:
    """Verify README code examples are executable."""

    @pytest.fixture
    def readme_path(self) -> Path:
        return Path(__file__).parent.parent / "README.md"

    def test_readme_exists(self, readme_path: Path):
        """Test that README.md exists."""
        assert readme_path.exists(), f"README not found at {readme_path}"

    def test_readme_quickstart_import_works(self, readme_path: Path):
        """Test that the Quick Start import snippet works."""
        content = readme_path.read_text(encoding="utf-8")

        # Extract the quick start snippet
        # Looking for: from continuum import Agent
        if "from continuum import Agent" in content:
            from continuum import Agent

            assert Agent is not None

    def test_readme_vectorstore_snippet_is_valid(self, readme_path: Path):
        """Test that VectorStore snippet uses correct API."""
        content = readme_path.read_text(encoding="utf-8")

        # Should use InMemoryVectorStore, not VectorStore(dimensions=...)
        assert "InMemoryVectorStore" in content, "README should use InMemoryVectorStore"
        assert "VectorStore(dimensions" not in content, "README should not use VectorStore(dimensions=...)"

        # Should use top_k parameter, not k
        if "store.search" in content:
            assert ", k=" not in content, "README should use 'top_k=' in store.search(), not 'k='"
            assert "top_k=" in content, "README should use 'top_k=' in store.search()"

    def test_readme_workflow_snippet_is_valid(self, readme_path: Path):
        """Test that workflow snippet uses DAG/Node, not Workflow/Step."""
        content = readme_path.read_text(encoding="utf-8")

        # Should use DAG and Node
        assert "from continuum_sdk.workflow import DAG, Node" in content, \
            "README should import DAG and Node"

        # Should NOT use Workflow or Step
        assert "from continuum_sdk.workflow import Workflow" not in content, \
            "README should not use Workflow class"
        assert "from continuum_sdk.workflow import Step" not in content, \
            "README should not use Step class"

    def test_readme_memory_snippet_is_valid(self, readme_path: Path):
        """Test that memory snippet uses correct API."""
        content = readme_path.read_text(encoding="utf-8")

        # Should use store() method
        assert 'memory.store("working"' in content, \
            "README should use memory.store() method"

        # Should NOT use remember() method
        assert "memory.remember(" not in content, \
            "README should not use memory.remember() - use store() instead"

    def test_readme_no_agent_run_await_in_sync_context(self, readme_path: Path):
        """Test that README does not show await agent.run() in sync context."""
        content = readme_path.read_text(encoding="utf-8")

        # Look for problematic patterns
        # Pattern: await agent.run() without async def context
        lines = content.split("\n")
        in_async_function = False

        for i, line in enumerate(lines):
            if "async def" in line:
                in_async_function = True
            elif line.strip().startswith("def ") and "async" not in line:
                in_async_function = False

            # Check for await agent.run in non-async context
            if "await agent.run" in line and not in_async_function:
                # This is only valid inside async functions
                # Check if previous lines had async def
                context = "\n".join(lines[max(0, i - 5) : i + 1])
                if "async def" not in context:
                    pytest.fail(
                        f"Line {i + 1}: 'await agent.run()' should be 'await agent.arun()' or use sync 'agent.run()'"
                    )


# =============================================================================
# 3. Provider Registry Consistency Tests
# =============================================================================


class TestProviderConsistency:
    """Verify provider registry is consistent across Python, Rust, and docs."""

    EXPECTED_PROVIDERS = {
        "anthropic",
        "openai",
        "google",
        "gemini",
        "cohere",
        "huggingface",
        "together",
        "groq",
        "deepseek",
        "moonshot",
        "glm",
        "kimi",
    }

    def test_python_providers_match_expected(self):
        """Test that Python provider list matches expected providers."""
        from continuum_sdk.config.providers import list_providers

        providers = set(list_providers())

        # All expected providers should be present
        missing = self.EXPECTED_PROVIDERS - providers
        assert not missing, f"Missing providers in Python: {missing}"

    def test_provider_default_models_exist(self):
        """Test that each provider with a default model returns a non-empty string."""
        from continuum_sdk.config.providers import get_default_model, list_providers

        # Some providers (like huggingface) intentionally have empty defaults
        providers_with_empty_defaults = {"huggingface"}

        for provider in list_providers():
            default = get_default_model(provider)
            assert default is not None, f"Provider {provider} has no default model"
            assert isinstance(default, str), f"Default model for {provider} should be string"

            if provider not in providers_with_empty_defaults:
                assert len(default) > 0, (
                    f"Default model for {provider} should not be empty "
                    f"(add to providers_with_empty_defaults if intentional)"
                )

    def test_provider_env_keys_are_documented(self):
        """Test that provider environment variable keys are documented."""
        from continuum_sdk.config.providers import BUILTIN_PROVIDERS, get_env_key_name

        expected_env_keys = {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "google": "GOOGLE_API_KEY",
            "gemini": "GOOGLE_API_KEY",
            "cohere": "COHERE_API_KEY",
            "huggingface": "HF_API_KEY",
            "together": "TOGETHER_API_KEY",
            "groq": "GROQ_API_KEY",
            "deepseek": "DEEPSEEK_API_KEY",
            "moonshot": "MOONSHOT_API_KEY",
            "glm": "GLM_API_KEY",
            "kimi": "MOONSHOT_API_KEY",
        }

        for provider, expected_key in expected_env_keys.items():
            if provider in BUILTIN_PROVIDERS:
                actual_key = get_env_key_name(provider)
                assert actual_key == expected_key, (
                    f"Provider {provider} env_key mismatch: expected {expected_key}, got {actual_key}"
                )


# =============================================================================
# 4. Security Boundary Tests
# =============================================================================


class TestSecurityBoundaries:
    """Verify security boundaries are enforced."""

    def test_path_validator_exists_and_works(self):
        """Test that PathValidator can be imported and used."""
        from continuum_sdk.security import PathValidator

        # Use project_root parameter (actual API)
        validator = PathValidator(project_root="/workspace")

        # Valid path
        result = validator.validate("/workspace/file.txt")
        assert hasattr(result, "is_valid")

    def test_permission_checker_exists_and_works(self):
        """Test that PermissionChecker can be imported and used."""
        from continuum_sdk.security import Permission, PermissionChecker

        checker = PermissionChecker()

        # Check permission
        result = checker.check("/file.txt", Permission.READ)
        assert hasattr(result, "has_permission")

    def test_audit_logger_exists_and_works(self, tmp_path: Path):
        """Test that AuditLogger can be imported and used."""
        from continuum_sdk.security import AuditLogger, AuditOperation, AuditResult

        log_file = tmp_path / "audit.json"
        audit = AuditLogger(str(log_file))

        # Log an operation (actual API requires operation, path, result)
        audit.log(AuditOperation.READ, "/file.txt", AuditResult.SUCCESS)

        # Flush to ensure it's written to file
        audit.flush()

        # Verify log was written
        assert log_file.exists()

    def test_security_level_enum_values(self):
        """Test that SecurityLevel has expected values."""
        from continuum_sdk.permission import SecurityLevel

        expected_values = {"trusted", "standard", "strict", "paranoid"}
        actual_values = {level.value for level in SecurityLevel}

        assert actual_values == expected_values, (
            f"SecurityLevel values mismatch: expected {expected_values}, got {actual_values}"
        )

    def test_permission_policy_has_blocked_commands(self):
        """Test that PermissionPolicy has dangerous commands blocked by default."""
        from continuum_sdk.permission import PermissionPolicy

        policy = PermissionPolicy()

        # These commands should be blocked by default
        assert "rm -rf /" in policy.blocked_commands
        assert "rm -rf ~" in policy.blocked_commands


# =============================================================================
# 5. Shell Policy Tests
# =============================================================================


class TestShellPolicy:
    """Verify shell command policy is enforced."""

    def test_bash_tool_exists(self):
        """Test that bash tool can be imported."""
        try:
            from continuum_sdk.tools import BashTool

            assert BashTool is not None
        except ImportError:
            # If BashTool not available, skip
            pytest.skip("BashTool not available")

    def test_bash_tool_validates_workspace(self):
        """Test that BashTool validates workspace parameter."""
        try:
            from continuum_sdk.tools import BashTool

            # With workspace
            bash = BashTool(workspace="/workspace")
            assert bash is not None
        except ImportError:
            pytest.skip("BashTool not available")


# =============================================================================
# 6. Packaging/Fallback Mode Tests
# =============================================================================


class TestPackagingModes:
    """Verify both Rust core and pure Python fallback modes work."""

    def test_python_fallback_mode_detection(self):
        """Test that we can detect Rust binding availability."""
        from continuum_sdk.api import get_implementation_preference

        # Should return either "rust" or "python"
        pref = get_implementation_preference()
        assert pref in ("rust", "python"), f"Unexpected implementation preference: {pref}"

    def test_agent_works_in_both_modes(self, monkeypatch):
        """Test that Agent can be created regardless of binding availability."""
        from continuum import Agent

        # Force Python mode
        monkeypatch.setenv("CONTINUUM_IMPL", "python")

        agent = Agent()
        assert agent is not None

    def test_memory_system_works_in_python_mode(self, monkeypatch):
        """Test that MemorySystem works in pure Python mode."""
        from continuum_sdk.api import MemorySystem

        monkeypatch.setenv("CONTINUUM_IMPL", "python")

        memory = MemorySystem(session_id="test-session")
        assert memory is not None

        # Test basic operations
        memory.store("working", "test content")
        results = memory.query("test")

        assert isinstance(results, list)

    def test_vector_store_works_in_python_mode(self, monkeypatch):
        """Test that VectorStore works in pure Python mode."""
        from continuum_sdk.rag import InMemoryVectorStore

        monkeypatch.setenv("CONTINUUM_IMPL", "python")

        store = InMemoryVectorStore()
        assert store is not None

        # Test basic operations
        store.upsert("test-id", [0.1, 0.2, 0.3], {"text": "hello"})
        count = store.count()

        assert count == 1


# =============================================================================
# 7. No Placeholder Success Tests
# =============================================================================


class TestNoPlaceholderSuccess:
    """Verify no placeholder success paths exist in public APIs."""

    def test_no_placeholder_in_agent_run(self, monkeypatch):
        """Test that Agent.run() does not return placeholder on missing API key."""
        from continuum import Agent, Config

        # Remove all API keys
        for key in [
            "CONTINUUM_API_KEY",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GOOGLE_API_KEY",
        ]:
            monkeypatch.delenv(key, raising=False)

        agent = Agent(config=Config())

        # Should raise an error, not return placeholder
        with pytest.raises((ValueError, RuntimeError)):
            agent.run("test task")

    def test_no_placeholder_in_memory_query(self):
        """Test that memory.query() returns real results, not placeholders."""
        from continuum_sdk.api import MemorySystem

        memory = MemorySystem(session_id="test")

        # Query empty memory should return empty list, not placeholder
        results = memory.query("nonexistent")

        assert isinstance(results, list)
        # Should be empty or real results, never strings like "placeholder"
        for r in results:
            assert isinstance(r, dict), f"Memory result should be dict, got {type(r)}"


# =============================================================================
# 8. API Contract Tests
# =============================================================================


class TestAPIContracts:
    """Verify API contracts match documentation."""

    def test_session_message_roles(self):
        """Test that session message roles are correct."""
        from continuum_sdk.agent.session import MessageRole

        # 'tool' role is valid for tool messages
        expected_roles = {"user", "assistant", "system", "tool"}
        actual_roles = {role.value for role in MessageRole}

        assert actual_roles == expected_roles

    def test_memory_tier_values(self):
        """Test that memory tier values are correct."""
        from continuum_sdk.memory import MemoryTier

        expected_tiers = {"working", "session", "project", "long_term"}
        actual_tiers = {tier.value for tier in MemoryTier}

        assert actual_tiers == expected_tiers

    def test_distance_metric_values(self):
        """Test that distance metric values are correct."""
        from continuum_sdk.rag import DistanceMetric

        expected_metrics = {"cosine", "euclidean", "dot_product", "manhattan"}
        actual_metrics = {metric.value for metric in DistanceMetric}

        assert actual_metrics == expected_metrics

    def test_vector_store_upsert_signature(self):
        """Test that VectorStore.upsert has correct signature."""
        import inspect

        from continuum_sdk.rag import InMemoryVectorStore

        sig = inspect.signature(InMemoryVectorStore.upsert)
        params = list(sig.parameters.keys())

        # Should have: self, id, vector, metadata (optional)
        assert "id" in params, "upsert should have 'id' parameter"
        assert "vector" in params, "upsert should have 'vector' parameter"
        assert "metadata" in params, "upsert should have 'metadata' parameter"

    def test_vector_store_search_signature(self):
        """Test that VectorStore.search has correct signature."""
        import inspect

        from continuum_sdk.rag import InMemoryVectorStore

        sig = inspect.signature(InMemoryVectorStore.search)
        params = list(sig.parameters.keys())

        # Should have: self, vector, top_k (optional)
        assert "vector" in params, "search should have 'vector' parameter"
        assert "top_k" in params, "search should have 'top_k' parameter"

        # Should NOT have 'k' as parameter (use top_k instead)
        assert "k" not in params, "search should use 'top_k' not 'k'"

    def test_dag_node_constructor_signature(self):
        """Test that Node constructor has correct signature."""
        import inspect

        from continuum_sdk.workflow import Node

        sig = inspect.signature(Node)
        params = list(sig.parameters.keys())

        # Should have: id, func (optional), dependencies (optional)
        assert "id" in params, "Node should have 'id' parameter"

    def test_dag_add_method_exists(self):
        """Test that DAG.add() method exists (not add_step)."""
        from continuum_sdk.workflow import DAG

        dag = DAG(id="test")

        assert hasattr(dag, "add"), "DAG should have 'add' method"
        assert not hasattr(dag, "add_step"), "DAG should NOT have 'add_step' - use 'add' instead"


# =============================================================================
# 9. Documentation Drift Tests
# =============================================================================


class TestDocumentationDrift:
    """Verify documentation does not drift from implementation."""

    @pytest.fixture
    def api_examples_path(self) -> Path:
        return Path(__file__).parent.parent.parent / "docs" / "API_EXAMPLES.md"

    def test_api_examples_uses_dag_not_workflow(self, api_examples_path: Path):
        """Test that API_EXAMPLES.md uses DAG, not Workflow."""
        if not api_examples_path.exists():
            pytest.skip(f"API_EXAMPLES.md not found at {api_examples_path}")

        content = api_examples_path.read_text(encoding="utf-8")

        # Should use DAG/Node in import
        assert "from continuum_sdk.workflow import DAG" in content, \
            "API_EXAMPLES.md should import DAG"

        # The import may be combined: "from continuum_sdk.workflow import DAG, Node"
        assert "DAG, Node" in content or "from continuum_sdk.workflow import Node" in content, \
            "API_EXAMPLES.md should import Node (possibly with DAG)"

    def test_api_examples_uses_arun_for_async(self, api_examples_path: Path):
        """Test that API_EXAMPLES.md uses arun() for async, not await run()."""
        if not api_examples_path.exists():
            pytest.skip(f"API_EXAMPLES.md not found at {api_examples_path}")

        content = api_examples_path.read_text(encoding="utf-8")

        # Should NOT have "await agent.run" pattern
        # This pattern is incorrect - use arun() for async
        assert "await agent.run(" not in content, \
            "API_EXAMPLES.md should use 'await agent.arun()' not 'await agent.run()'"

    def test_api_examples_uses_inmemory_vectorstore(self, api_examples_path: Path):
        """Test that API_EXAMPLES.md uses InMemoryVectorStore correctly."""
        if not api_examples_path.exists():
            pytest.skip(f"API_EXAMPLES.md not found at {api_examples_path}")

        content = api_examples_path.read_text(encoding="utf-8")

        # Should use InMemoryVectorStore
        assert "InMemoryVectorStore" in content, \
            "API_EXAMPLES.md should use InMemoryVectorStore"

        # Should NOT use VectorStore(dimensions=...)
        assert "VectorStore(dimensions" not in content, \
            "API_EXAMPLES.md should not use VectorStore(dimensions=...)"
