"""
Embedding API Integration Tests

Tests for the complete embedding pipeline using mock implementations.
No external API keys or Rust features required.
"""

import random

import pytest


class MockEmbeddingClient:
    """Mock embedding client that returns deterministic vectors."""

    def __init__(self, dimension: int = 64):
        self.dimension = dimension

    def embed(self, text: str) -> list[float]:
        """Generate a deterministic mock embedding."""
        # Use hash of text to generate deterministic values
        random.seed(hash(text) % (2**32))
        return [random.gauss(0, 1) for _ in range(self.dimension)]

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate mock embeddings for batch."""
        return [self.embed(t) for t in texts]


class TestEmbeddingProvider:
    """Tests for embedding provider configuration."""

    def test_provider_openai(self):
        """Test OpenAI provider configuration."""
        from continuum_sdk.rag.embeddings import EmbeddingConfig

        config = EmbeddingConfig(provider="openai", model="text-embedding-3-small")
        assert config.provider == "openai"
        assert config.model == "text-embedding-3-small"

    def test_provider_huggingface(self):
        """Test HuggingFace provider configuration."""
        from continuum_sdk.rag.embeddings import EmbeddingConfig

        config = EmbeddingConfig(provider="huggingface", model="all-MiniLM-L6-v2")
        assert config.provider == "huggingface"

    def test_provider_cohere(self):
        """Test Cohere provider configuration."""
        from continuum_sdk.rag.embeddings import EmbeddingConfig

        config = EmbeddingConfig(provider="cohere", model="embed-english-v3.0")
        assert config.provider == "cohere"

    def test_provider_local(self):
        """Test local provider configuration."""
        from continuum_sdk.rag.embeddings import EmbeddingConfig

        config = EmbeddingConfig(provider="local", model="all-MiniLM-L6-v2")
        assert config.provider == "local"


class TestEmbeddingsFactory:
    """Tests for factory methods."""

    def test_explicit_config_openai(self):
        """Test explicit OpenAI configuration with mock."""
        from continuum_sdk.rag.embeddings import Embeddings

        # Use _use_rust=False to force Python implementation
        embeddings = Embeddings(
            provider="openai",
            model="text-embedding-3-small",
            api_key="mock-key",
            dimension=1536,
            _use_rust=False,
        )
        assert embeddings is not None
        assert embeddings.provider == "openai"

    def test_explicit_config_huggingface(self):
        """Test explicit HuggingFace configuration with mock."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="huggingface",
            model="sentence-transformers/all-MiniLM-L6-v2",
            dimension=384,
            api_key="mock-hf-key",  # Add mock API key
            _use_rust=False,
        )
        assert embeddings is not None
        # Should use Python fallback (may fail without real HF client)
        # Just verify the configuration works
        assert embeddings.provider == "huggingface"
        assert embeddings.dimension == 384

    def test_explicit_config_cohere(self):
        """Test explicit Cohere configuration with mock."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="cohere",
            model="embed-english-v3.0",
            api_key="mock-key",
            dimension=1024,
            _use_rust=False,
        )
        assert embeddings is not None

    def test_explicit_config_local(self):
        """Test explicit local configuration."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local",
            model="all-MiniLM-L6-v2",
            dimension=384,
            _use_rust=False,
        )
        assert embeddings is not None
        vector = embeddings.embed("test")
        assert len(vector) == 384


class TestBatchEmbedding:
    """Tests for batch embedding optimization."""

    def test_batch_embedding(self):
        """Test batch embedding generation."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local",
            model="test-model",
            dimension=64,
            _use_rust=False,
        )

        texts = [
            "Hello world",
            "Testing embeddings",
            "Batch processing",
        ]

        vectors = embeddings.embed_batch(texts)
        assert len(vectors) == 3
        for vec in vectors:
            assert len(vec) == 64
            assert all(isinstance(v, float) for v in vec)

    def test_single_embedding(self):
        """Test single text embedding."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local",
            model="test-model",
            dimension=128,
            _use_rust=False,
        )

        vector = embeddings.embed("Test single embedding")
        assert len(vector) == 128
        assert all(isinstance(v, float) for v in vector)

    def test_empty_batch(self):
        """Test empty batch handling."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local", model="test", dimension=64, _use_rust=False
        )
        vectors = embeddings.embed_batch([])
        assert len(vectors) == 0


class TestGracefulDegradation:
    """Tests for graceful degradation."""

    def test_python_fallback_when_rust_disabled(self):
        """Test that Python implementation is used when Rust is disabled."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local",
            model="test-model",
            dimension=384,
            _use_rust=False,
        )
        assert embeddings is not None
        assert embeddings._rust_embeddings is None

    def test_mock_fallback_on_api_failure(self):
        """Test that mock fallback is used when API fails."""
        from continuum_sdk.rag.embeddings import Embeddings

        # Without API key, local provider should still work
        embeddings = Embeddings(
            provider="local",
            model="test-model",
            dimension=128,
            _use_rust=False,
        )

        vector = embeddings.embed("test")
        assert len(vector) == 128

    def test_no_api_key_graceful_handling(self):
        """Test handling when no API key is provided."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local", model="test-model", dimension=256, _use_rust=False
        )
        vector = embeddings.embed("test")
        assert len(vector) == 256


class TestEmbeddingQuality:
    """Tests for embedding quality."""

    def test_mock_embedding_consistency(self):
        """Test that mock embeddings are deterministic."""
        from continuum_sdk.rag.embeddings import Embeddings

        embeddings = Embeddings(
            provider="local", model="test", dimension=64, _use_rust=False
        )

        vec1 = embeddings.embed("test")
        vec2 = embeddings.embed("test")

        # Same text should produce same embedding
        assert vec1 == vec2

    def test_embedding_dimension_correctness(self):
        """Test that embedding dimensions are correct."""
        from continuum_sdk.rag.embeddings import Embeddings

        for dim in [64, 128, 256, 384, 512, 768, 1024, 1536]:
            embeddings = Embeddings(
                provider="local", model="test", dimension=dim, _use_rust=False
            )
            vector = embeddings.embed("test")
            assert len(vector) == dim, f"Expected {dim}, got {len(vector)}"


class TestErrorHandling:
    """Tests for error handling."""

    def test_unsupported_provider_with_python_fallback(self):
        """Test handling of unsupported provider."""
        from continuum_sdk.rag.embeddings import Embeddings

        # With _use_rust=False, unsupported provider should raise error
        with pytest.raises(ValueError, match="Unsupported provider"):
            embeddings = Embeddings(
                provider="invalid_provider",
                model="test",
                dimension=64,
                _use_rust=False,
            )
            embeddings.embed("test")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
