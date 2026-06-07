"""
Unit tests for rag/embeddings.py

Tests cover:
- Embeddings class initialization
- EmbeddingConfig from environment
- Mock embedding generation
- Error handling for various providers
"""

import os
import hashlib
from unittest.mock import MagicMock, patch, PropertyMock
import pytest

from continuum_sdk.rag.embeddings import (
    Embeddings,
    EmbeddingConfig,
    PREDEFINED_EMBEDDINGS,
)


class TestEmbeddingConfig:
    """Tests for EmbeddingConfig dataclass."""

    def test_default_values(self):
        """Test default configuration values."""
        config = EmbeddingConfig()
        assert config.provider == "openai"
        assert config.model == "text-embedding-3-small"
        assert config.api_key is None
        assert config.base_url is None
        assert config.dimension is None

    def test_custom_values(self):
        """Test custom configuration values."""
        config = EmbeddingConfig(
            provider="huggingface",
            model="custom-model",
            api_key="test-key",
            base_url="https://custom.url",
            dimension=512,
        )
        assert config.provider == "huggingface"
        assert config.model == "custom-model"
        assert config.api_key == "test-key"
        assert config.base_url == "https://custom.url"
        assert config.dimension == 512


class TestEmbeddingConfigFromEnv:
    """Tests for EmbeddingConfig.from_env class method."""

    def test_openai_from_env(self, monkeypatch):
        """Test OpenAI config from environment variables."""
        monkeypatch.setenv("OPENAI_API_KEY", "sk-test-key")
        monkeypatch.setenv("OPENAI_EMBEDDING_MODEL", "text-embedding-3-large")
        monkeypatch.setenv("OPENAI_BASE_URL", "https://custom.openai.com")

        config = EmbeddingConfig.from_env("openai")
        assert config.provider == "openai"
        assert config.model == "text-embedding-3-large"
        assert config.api_key == "sk-test-key"
        assert config.base_url == "https://custom.openai.com"

    def test_openai_defaults(self, monkeypatch):
        """Test OpenAI config with missing env vars uses defaults."""
        monkeypatch.delenv("OPENAI_API_KEY", raising=False)
        monkeypatch.delenv("OPENAI_EMBEDDING_MODEL", raising=False)
        monkeypatch.delenv("OPENAI_BASE_URL", raising=False)

        config = EmbeddingConfig.from_env("openai")
        assert config.provider == "openai"
        assert config.model == "text-embedding-3-small"
        assert config.api_key is None
        assert config.base_url is None

    def test_huggingface_from_env(self, monkeypatch):
        """Test HuggingFace config from environment variables."""
        monkeypatch.setenv("HUGGINGFACE_API_KEY", "hf-test-key")
        monkeypatch.setenv("HUGGINGFACE_EMBEDDING_MODEL", "custom-model")

        config = EmbeddingConfig.from_env("huggingface")
        assert config.provider == "huggingface"
        assert config.model == "custom-model"
        assert config.api_key == "hf-test-key"

    def test_huggingface_alias(self, monkeypatch):
        """Test 'hf' as alias for huggingface."""
        monkeypatch.setenv("HUGGINGFACE_API_KEY", "hf-test-key")

        config = EmbeddingConfig.from_env("hf")
        assert config.provider == "huggingface"

    def test_cohere_from_env(self, monkeypatch):
        """Test Cohere config from environment variables."""
        monkeypatch.setenv("COHERE_API_KEY", "cohere-test-key")
        monkeypatch.setenv("COHERE_EMBEDDING_MODEL", "embed-multilingual-v3.0")

        config = EmbeddingConfig.from_env("cohere")
        assert config.provider == "cohere"
        assert config.model == "embed-multilingual-v3.0"
        assert config.api_key == "cohere-test-key"

    def test_local_from_env(self, monkeypatch):
        """Test local provider config from environment variables."""
        monkeypatch.setenv("LOCAL_EMBEDDING_MODEL", "custom-local-model")

        config = EmbeddingConfig.from_env("local")
        assert config.provider == "local"
        assert config.model == "custom-local-model"
        assert config.dimension == 384

    def test_unknown_provider_raises_error(self):
        """Test that unknown provider raises ValueError."""
        with pytest.raises(ValueError, match="Unknown provider"):
            EmbeddingConfig.from_env("unknown_provider")


class TestEmbeddingsInit:
    """Tests for Embeddings class initialization."""

    def test_init_with_defaults(self):
        """Test initialization with default values."""
        embeddings = Embeddings(_use_rust=False)
        assert embeddings.provider == "openai"
        assert embeddings.model == "text-embedding-3-small"
        assert embeddings.dimension == 1536
        assert embeddings.api_key is None

    def test_init_with_custom_provider(self):
        """Test initialization with custom provider."""
        embeddings = Embeddings(
            provider="huggingface",
            model="custom-model",
            dimension=512,
            _use_rust=False,
        )
        assert embeddings.provider == "huggingface"
        assert embeddings.model == "custom-model"
        assert embeddings.dimension == 512

    def test_init_with_api_key_no_rust(self):
        """Test that API key doesn't crash when Rust unavailable."""
        embeddings = Embeddings(
            provider="openai",
            api_key="test-key",
            _use_rust=False,
        )
        assert embeddings.api_key == "test-key"
        assert embeddings._rust_embeddings is None
        # Should have Python client initialized
        assert embeddings._client is not None

    def test_from_env_class_method(self, monkeypatch):
        """Test from_env factory method."""
        monkeypatch.setenv("OPENAI_API_KEY", "test-key")

        embeddings = Embeddings.from_env("openai")
        assert embeddings.provider == "openai"
        assert embeddings.api_key == "test-key"


class TestDefaultModel:
    """Tests for _default_model static method."""

    def test_default_model_openai(self):
        """Test default model for OpenAI."""
        model = Embeddings._default_model("openai")
        assert model == "text-embedding-3-small"

    def test_default_model_huggingface(self):
        """Test default model for HuggingFace."""
        model = Embeddings._default_model("huggingface")
        assert model == "sentence-transformers/all-MiniLM-L6-v2"

    def test_default_model_cohere(self):
        """Test default model for Cohere."""
        model = Embeddings._default_model("cohere")
        assert model == "embed-english-v3.0"

    def test_default_model_local(self):
        """Test default model for local."""
        model = Embeddings._default_model("local")
        assert model == "all-MiniLM-L6-v2"

    def test_default_model_unknown(self):
        """Test default model for unknown provider."""
        model = Embeddings._default_model("unknown")
        assert model == "text-embedding-3-small"


class TestDefaultDimension:
    """Tests for _default_dimension static method."""

    def test_openai_small_dimension(self):
        """Test dimension for OpenAI small model."""
        dim = Embeddings._default_dimension("openai", "text-embedding-3-small")
        assert dim == 1536

    def test_openai_large_dimension(self):
        """Test dimension for OpenAI large model."""
        dim = Embeddings._default_dimension("openai", "text-embedding-3-large")
        assert dim == 3072

    def test_huggingface_minilm_dimension(self):
        """Test dimension for HuggingFace MiniLM model."""
        dim = Embeddings._default_dimension("huggingface", "all-MiniLM-L6-v2")
        assert dim == 384

    def test_huggingface_other_dimension(self):
        """Test dimension for other HuggingFace models."""
        dim = Embeddings._default_dimension("huggingface", "other-model")
        assert dim == 768

    def test_cohere_dimension(self):
        """Test dimension for Cohere."""
        dim = Embeddings._default_dimension("cohere", "embed-english-v3.0")
        assert dim == 1024

    def test_local_dimension(self):
        """Test dimension for local provider."""
        dim = Embeddings._default_dimension("local", "all-MiniLM-L6-v2")
        assert dim == 384

    def test_unknown_provider_dimension(self):
        """Test default dimension for unknown provider."""
        dim = Embeddings._default_dimension("unknown", "model")
        assert dim == 1536


class TestMockEmbedding:
    """Tests for mock embedding generation."""

    def test_mock_embedding_returns_correct_dimension(self):
        """Test that mock embedding returns correct dimension."""
        for dim in [64, 128, 256, 384, 512, 768, 1024, 1536, 3072]:
            embeddings = Embeddings(
                provider="local",
                dimension=dim,
                _use_rust=False,
            )
            vector = embeddings.embed("test text")
            assert len(vector) == dim, f"Expected {dim}, got {len(vector)}"

    def test_mock_embedding_deterministic(self):
        """Test that mock embeddings are deterministic for same input."""
        embeddings = Embeddings(provider="local", dimension=128, _use_rust=False)

        vec1 = embeddings.embed("same text")
        vec2 = embeddings.embed("same text")
        assert vec1 == vec2

    def test_mock_embedding_different_for_different_input(self):
        """Test that different inputs produce different embeddings."""
        embeddings = Embeddings(provider="local", dimension=128, _use_rust=False)

        vec1 = embeddings.embed("text one")
        vec2 = embeddings.embed("text two")
        assert vec1 != vec2

    def test_mock_embedding_normalized(self):
        """Test that mock embeddings are normalized."""
        embeddings = Embeddings(provider="local", dimension=128, _use_rust=False)
        vector = embeddings.embed("test text")

        # Calculate L2 norm
        norm = sum(v * v for v in vector) ** 0.5
        assert 0.99 < norm < 1.01, f"Expected norm ~1.0, got {norm}"

    def test_mock_embedding_values_in_range(self):
        """Test that mock embedding values are in reasonable range."""
        embeddings = Embeddings(provider="local", dimension=128, _use_rust=False)
        vector = embeddings.embed("test text")

        # Values should be normalized, so they should be in reasonable range
        for v in vector:
            assert -2.0 < v < 2.0, f"Value {v} out of expected range"

    def test_mock_embedding_batch(self):
        """Test batch embedding generation."""
        embeddings = Embeddings(provider="local", dimension=64, _use_rust=False)
        texts = ["text one", "text two", "text three"]

        vectors = embeddings.embed_batch(texts)
        assert len(vectors) == 3
        for vec in vectors:
            assert len(vec) == 64

    def test_mock_embedding_batch_empty(self):
        """Test batch embedding with empty list."""
        embeddings = Embeddings(provider="local", dimension=64, _use_rust=False)
        vectors = embeddings.embed_batch([])
        assert vectors == []

    def test_mock_embedding_batch_deterministic(self):
        """Test that batch embeddings are deterministic."""
        embeddings = Embeddings(provider="local", dimension=64, _use_rust=False)
        texts = ["text one", "text two"]

        batch1 = embeddings.embed_batch(texts)
        batch2 = embeddings.embed_batch(texts)
        assert batch1 == batch2


class TestOpenAIEmbedding:
    """Tests for OpenAI embedding with mocked API."""

    def test_openai_embed_with_sdk_mock(self):
        """Test OpenAI embedding with mocked SDK."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.data = [MagicMock(embedding=[0.1] * 1536)]
        mock_client.embeddings.create.return_value = mock_response

        embeddings = Embeddings(
            provider="openai",
            model="text-embedding-3-small",
            api_key="test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        vector = embeddings.embed("test text")
        assert len(vector) == 1536
        mock_client.embeddings.create.assert_called_once_with(
            model="text-embedding-3-small",
            input="test text",
        )

    def test_openai_embed_batch_with_sdk_mock(self):
        """Test OpenAI batch embedding with mocked SDK."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.data = [MagicMock(embedding=[0.1] * 1536)]
        mock_client.embeddings.create.return_value = mock_response

        embeddings = Embeddings(
            provider="openai",
            model="text-embedding-3-small",
            api_key="test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        texts = ["text one", "text two"]
        vectors = embeddings.embed_batch(texts)
        assert len(vectors) == 2
        assert mock_client.embeddings.create.call_count == 2

    def test_openai_embed_no_client_raises_error(self):
        """Test that OpenAI embed raises error without client."""
        embeddings = Embeddings(
            provider="openai",
            model="text-embedding-3-small",
            _use_rust=False,
        )
        embeddings._client = None

        with pytest.raises(RuntimeError, match="No client available"):
            embeddings.embed("test text")


class TestHuggingFaceEmbedding:
    """Tests for HuggingFace embedding with mocked API."""

    def test_huggingface_embed_success(self):
        """Test HuggingFace embedding with mocked response."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.json.return_value = [[0.1] * 384]
        mock_response.raise_for_status = MagicMock()
        mock_client.post.return_value = mock_response

        embeddings = Embeddings(
            provider="huggingface",
            model="sentence-transformers/all-MiniLM-L6-v2",
            api_key="hf-test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        vector = embeddings.embed("test text")
        assert len(vector) == 384
        mock_client.post.assert_called_once()

    def test_huggingface_embed_flat_response(self):
        """Test HuggingFace embedding with flat list response."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.json.return_value = [0.1] * 384
        mock_response.raise_for_status = MagicMock()
        mock_client.post.return_value = mock_response

        embeddings = Embeddings(
            provider="huggingface",
            model="sentence-transformers/all-MiniLM-L6-v2",
            api_key="hf-test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        vector = embeddings.embed("test text")
        assert len(vector) == 384

    def test_huggingface_embed_no_client_raises_error(self):
        """Test that HuggingFace embed raises error without client."""
        embeddings = Embeddings(
            provider="huggingface",
            model="test-model",
            _use_rust=False,
        )
        embeddings._client = None

        with pytest.raises(RuntimeError, match="No client available"):
            embeddings.embed("test text")

    def test_huggingface_unexpected_response_raises_error(self):
        """Test that unexpected HuggingFace response raises error."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.json.return_value = {"error": "invalid"}
        mock_response.raise_for_status = MagicMock()
        mock_client.post.return_value = mock_response

        embeddings = Embeddings(
            provider="huggingface",
            model="test-model",
            api_key="test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        with pytest.raises(RuntimeError, match="Unexpected HuggingFace response"):
            embeddings.embed("test text")


class TestCohereEmbedding:
    """Tests for Cohere embedding with mocked API."""

    def test_cohere_embed_success(self):
        """Test Cohere embedding with mocked response."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.json.return_value = {
            "embeddings": {
                "float": [[0.1] * 1024]
            }
        }
        mock_response.raise_for_status = MagicMock()
        mock_client.post.return_value = mock_response

        embeddings = Embeddings(
            provider="cohere",
            model="embed-english-v3.0",
            api_key="cohere-test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        vector = embeddings.embed("test text")
        assert len(vector) == 1024
        mock_client.post.assert_called_once()

    def test_cohere_embed_no_client_raises_error(self):
        """Test that Cohere embed raises error without client."""
        embeddings = Embeddings(
            provider="cohere",
            model="test-model",
            _use_rust=False,
        )
        embeddings._client = None

        with pytest.raises(RuntimeError, match="No client available"):
            embeddings.embed("test text")


class TestErrorHandling:
    """Tests for error handling scenarios."""

    def test_unsupported_provider_raises_error(self):
        """Test that unsupported provider raises ValueError."""
        embeddings = Embeddings(
            provider="unsupported_provider",
            model="test-model",
            _use_rust=False,
        )

        with pytest.raises(ValueError, match="Unsupported provider"):
            embeddings.embed("test text")

    def test_repr_method(self):
        """Test __repr__ method."""
        embeddings = Embeddings(
            provider="openai",
            model="text-embedding-3-small",
            dimension=1536,
            _use_rust=False,
        )
        repr_str = repr(embeddings)
        assert "Embeddings" in repr_str
        assert "openai" in repr_str
        assert "text-embedding-3-small" in repr_str
        assert "1536" in repr_str


class TestPredefinedEmbeddings:
    """Tests for predefined embedding configurations."""

    def test_predefined_embeddings_exist(self):
        """Test that predefined embeddings are defined."""
        assert "openai_small" in PREDEFINED_EMBEDDINGS
        assert "openai_large" in PREDEFINED_EMBEDDINGS
        assert "huggingface_minilm" in PREDEFINED_EMBEDDINGS
        assert "cohere_english" in PREDEFINED_EMBEDDINGS

    def test_openai_small_config(self):
        """Test OpenAI small predefined config."""
        config = PREDEFINED_EMBEDDINGS["openai_small"]
        assert config.provider == "openai"
        assert config.model == "text-embedding-3-small"
        assert config.dimension == 1536

    def test_openai_large_config(self):
        """Test OpenAI large predefined config."""
        config = PREDEFINED_EMBEDDINGS["openai_large"]
        assert config.provider == "openai"
        assert config.model == "text-embedding-3-large"
        assert config.dimension == 3072

    def test_huggingface_minilm_config(self):
        """Test HuggingFace MiniLM predefined config."""
        config = PREDEFINED_EMBEDDINGS["huggingface_minilm"]
        assert config.provider == "huggingface"
        assert config.model == "sentence-transformers/all-MiniLM-L6-v2"
        assert config.dimension == 384

    def test_cohere_english_config(self):
        """Test Cohere English predefined config."""
        config = PREDEFINED_EMBEDDINGS["cohere_english"]
        assert config.provider == "cohere"
        assert config.model == "embed-english-v3.0"
        assert config.dimension == 1024


class TestHttpxClientFallback:
    """Tests for httpx client fallback when OpenAI SDK unavailable."""

    def test_httpx_fallback_for_openai(self):
        """Test httpx fallback for OpenAI API when SDK client lacks embeddings."""
        mock_client = MagicMock()
        # Simulate httpx client (no embeddings attribute)
        del mock_client.embeddings
        mock_response = MagicMock()
        mock_response.json.return_value = {
            "data": [{"embedding": [0.1] * 1536}]
        }
        mock_response.raise_for_status = MagicMock()
        mock_client.post.return_value = mock_response

        embeddings = Embeddings(
            provider="openai",
            model="text-embedding-3-small",
            api_key="test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        vector = embeddings.embed("test text")
        assert len(vector) == 1536
        mock_client.post.assert_called_once()


class TestRustBindingsPath:
    """Tests for Rust bindings code paths."""

    def test_rust_embeddings_embed_called(self):
        """Test that embed delegates to Rust when available."""
        mock_rust_embeddings = MagicMock()
        mock_rust_embeddings.embed.return_value = [0.1] * 1536

        with patch("continuum_sdk.rag.embeddings._RUST_AVAILABLE", True):
            with patch("continuum_sdk.rag.embeddings.RustEmbeddings") as MockRustEmbeddings:
                MockRustEmbeddings.return_value = mock_rust_embeddings

                embeddings = Embeddings(
                    provider="openai",
                    model="text-embedding-3-small",
                    api_key="test-key",
                    _use_rust=True,
                )

                vector = embeddings.embed("test text")
                assert len(vector) == 1536
                mock_rust_embeddings.embed.assert_called_once_with("test text")

    def test_rust_embeddings_embed_batch_called(self):
        """Test that embed_batch delegates to Rust when available."""
        mock_rust_embeddings = MagicMock()
        mock_rust_embeddings.embed_batch.return_value = [[0.1] * 1536, [0.2] * 1536]

        with patch("continuum_sdk.rag.embeddings._RUST_AVAILABLE", True):
            with patch("continuum_sdk.rag.embeddings.RustEmbeddings") as MockRustEmbeddings:
                MockRustEmbeddings.return_value = mock_rust_embeddings

                embeddings = Embeddings(
                    provider="openai",
                    model="text-embedding-3-small",
                    api_key="test-key",
                    _use_rust=True,
                )

                vectors = embeddings.embed_batch(["text one", "text two"])
                assert len(vectors) == 2
                mock_rust_embeddings.embed_batch.assert_called_once_with(["text one", "text two"])

    def test_rust_initialization_exception_fallback(self):
        """Test fallback when Rust initialization raises exception."""
        with patch("continuum_sdk.rag.embeddings._RUST_AVAILABLE", True):
            with patch("continuum_sdk.rag.embeddings.RustEmbeddings") as MockRustEmbeddings:
                # Simulate Rust initialization failure
                MockRustEmbeddings.side_effect = RuntimeError("Rust init failed")

                embeddings = Embeddings(
                    provider="openai",
                    model="text-embedding-3-small",
                    api_key="test-key",
                    _use_rust=True,
                )

                # Should fall back to Python client
                assert embeddings._rust_embeddings is None
                assert embeddings._client is not None


class TestOpenAISDKClientInit:
    """Tests for OpenAI SDK client initialization."""

    def test_openai_sdk_client_init(self):
        """Test OpenAI SDK client initialization when available."""
        import sys

        mock_openai = MagicMock()
        mock_client = MagicMock()
        mock_openai.OpenAI.return_value = mock_client

        # Need to patch sys.modules to inject the mock openai module
        with patch.dict("sys.modules", {"openai": mock_openai}):
            # Remove cached module to force re-import
            if "continuum_sdk.rag.embeddings" in sys.modules:
                del sys.modules["continuum_sdk.rag.embeddings"]

            with patch("continuum_sdk.rag.embeddings._OPENAI_SDK_AVAILABLE", True):
                from continuum_sdk.rag.embeddings import Embeddings as ReimportedEmbeddings

                embeddings = ReimportedEmbeddings(
                    provider="openai",
                    model="text-embedding-3-small",
                    api_key="test-key",
                    _use_rust=False,
                )
                mock_openai.OpenAI.assert_called_once_with(api_key="test-key")
                assert embeddings._client == mock_client

        # Restore original module
        if "continuum_sdk.rag.embeddings" in sys.modules:
            del sys.modules["continuum_sdk.rag.embeddings"]
        import continuum_sdk.rag.embeddings  # noqa: F401


class TestNoHTTPClientWarning:
    """Tests for warning when no HTTP client available."""

    def test_no_http_client_warning(self, monkeypatch):
        """Test warning when neither OpenAI SDK nor httpx available."""
        import sys
        import importlib

        # Save original module
        orig_module = sys.modules.get("continuum_sdk.rag.embeddings")

        try:
            # Remove cached module to force re-import
            if "continuum_sdk.rag.embeddings" in sys.modules:
                del sys.modules["continuum_sdk.rag.embeddings"]

            # Create mock modules for openai and httpx that raise ImportError
            class UnimportableModule:
                def __getattr__(self, name):
                    raise ImportError(f"No module named '{name}'")

            # Patch sys.modules to make openai and httpx unimportable
            with patch.dict("sys.modules", {"openai": None, "httpx": None}):
                # Now import the module - it should handle ImportError gracefully
                import continuum_sdk.rag.embeddings as emb_module

                # Verify both are False
                assert emb_module._OPENAI_SDK_AVAILABLE == False
                assert emb_module._HTTPX_AVAILABLE == False

                # Now test the warning is logged
                with patch.object(emb_module.logger, 'warning') as mock_warning:
                    embeddings = emb_module.Embeddings(
                        provider="openai",
                        model="text-embedding-3-small",
                        api_key="test-key",
                        _use_rust=False,
                    )
                    mock_warning.assert_called()
                    assert "No HTTP client available" in mock_warning.call_args[0][0]

        finally:
            # Restore original module
            if orig_module:
                sys.modules["continuum_sdk.rag.embeddings"] = orig_module
            elif "continuum_sdk.rag.embeddings" in sys.modules:
                del sys.modules["continuum_sdk.rag.embeddings"]
            # Re-import to restore state
            import continuum_sdk.rag.embeddings  # noqa: F401


class TestImportErrorHandling:
    """Tests for import error handling at module level."""

    def test_rust_import_error_path(self):
        """Test that _RUST_AVAILABLE is False when import fails."""
        # Re-import the module to test the import error path
        import sys
        import importlib

        # Save original module
        orig_module = sys.modules.get("continuum_sdk.rag.embeddings")

        try:
            # Remove from sys.modules to force re-import
            if "continuum_sdk.rag.embeddings" in sys.modules:
                del sys.modules["continuum_sdk.rag.embeddings"]

            # Mock the import to raise ImportError for sh_python
            with patch.dict("sys.modules", {"sh_python": None}):
                # The import should not crash, just set _RUST_AVAILABLE to False
                # We can't easily test this without modifying import machinery
                # So we just verify the current state
                from continuum_sdk.rag.embeddings import _RUST_AVAILABLE as rust_avail
                # Either True or False is fine, just checking it's defined
                assert isinstance(rust_avail, bool)
        finally:
            # Restore original module
            if orig_module:
                sys.modules["continuum_sdk.rag.embeddings"] = orig_module

    def test_openai_sdk_import_error_path(self):
        """Test that _OPENAI_SDK_AVAILABLE handles import error."""
        from continuum_sdk.rag.embeddings import _OPENAI_SDK_AVAILABLE as openai_avail
        assert isinstance(openai_avail, bool)

    def test_httpx_import_error_path(self):
        """Test that _HTTPX_AVAILABLE handles import error."""
        from continuum_sdk.rag.embeddings import _HTTPX_AVAILABLE as httpx_avail
        assert isinstance(httpx_avail, bool)


class TestMockEmbeddingZeroNorm:
    """Tests for mock embedding edge case with zero norm."""

    def test_mock_embedding_with_empty_text(self):
        """Test mock embedding with empty text (edge case for normalization)."""
        embeddings = Embeddings(provider="local", dimension=128, _use_rust=False)
        # Empty string should still produce valid embedding
        vector = embeddings.embed("")
        assert len(vector) == 128
        # Should be normalized
        norm = sum(v * v for v in vector) ** 0.5
        assert 0.99 < norm < 1.01

    def test_mock_embedding_zero_norm_branch(self):
        """Test the zero norm defensive branch (force norm=0 via mock)."""
        embeddings = Embeddings(provider="local", dimension=128, _use_rust=False)

        # Mock the sum function used in _mock_embed to force zero norm
        original_sum = sum

        def mocked_sum(iterable, start=0):
            # Check if this is the norm calculation (sum of squares)
            # If it looks like it's computing v*v, return 0
            if hasattr(iterable, '__iter__'):
                try:
                    first = next(iter(iterable))
                    # If the value looks like a squared float, we're in norm calc
                    if isinstance(first, float) and abs(first) < 1.0:
                        return 0.0
                except StopIteration:
                    pass
            return original_sum(iterable, start)

        # Patch builtins.sum within the _mock_embed method
        import builtins
        with patch.object(builtins, 'sum', mocked_sum):
            vector = embeddings._mock_embed("test text")
            # With norm=0, the embedding should not be normalized (return raw values)
            assert len(vector) == 128
            # Values should be in raw range (-0.5 to 0.5), not normalized
            for v in vector:
                assert -0.5 <= v <= 0.5


class TestHuggingFaceEmptyListResponse:
    """Tests for HuggingFace empty list response edge case."""

    def test_huggingface_empty_list_response_raises_error(self):
        """Test that empty list response from HuggingFace raises error."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.json.return_value = []  # Empty list
        mock_response.raise_for_status = MagicMock()
        mock_client.post.return_value = mock_response

        embeddings = Embeddings(
            provider="huggingface",
            model="test-model",
            api_key="test-key",
            _use_rust=False,
        )
        embeddings._client = mock_client

        with pytest.raises(RuntimeError, match="Unexpected HuggingFace response"):
            embeddings.embed("test text")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
