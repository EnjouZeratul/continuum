"""
Embeddings API - Python Wrapper

Provides Python wrapper for real embedding APIs with graceful fallback.

Supported providers:
- OpenAI Embeddings API
- HuggingFace Inference API
- Cohere Embed API
- Local models (optional)

Usage:
    from continuum_sdk.rag.embeddings import Embeddings

    # Auto-configure from environment variables
    embeddings = Embeddings.from_env("openai")

    # Or manual configuration
    embeddings = Embeddings(
        provider="openai",
        model="text-embedding-3-small",
        api_key="sk-..."
    )

    # Generate embeddings
    vector = embeddings.embed("Hello world")
    vectors = embeddings.embed_batch(["Hello", "World"])
"""

from __future__ import annotations

import logging
from dataclasses import dataclass

from continuum_sdk.config.loader import _get_env

logger = logging.getLogger(__name__)

# Try to import Rust bindings
try:
    from sh_python import Embeddings as RustEmbeddings

    _RUST_AVAILABLE = True
except ImportError:
    _RUST_AVAILABLE = False
    RustEmbeddings = None

# Try to import OpenAI SDK (as fallback)
try:
    import openai

    _OPENAI_SDK_AVAILABLE = True
except ImportError:
    _OPENAI_SDK_AVAILABLE = False

# Try to import httpx (for other APIs)
try:
    import httpx

    _HTTPX_AVAILABLE = True
except ImportError:
    _HTTPX_AVAILABLE = False


@dataclass
class EmbeddingConfig:
    """Embedding model configuration"""

    provider: str = "openai"
    model: str = "text-embedding-3-small"
    api_key: str | None = None
    base_url: str | None = None
    dimension: int | None = None

    @classmethod
    def from_env(cls, provider: str = "openai") -> EmbeddingConfig:
        """Create configuration from environment variables"""
        if provider == "openai":
            return cls(
                provider="openai",
                model=_get_env("OPENAI_EMBEDDING_MODEL", "text-embedding-3-small")
                or "text-embedding-3-small",
                api_key=_get_env("OPENAI_API_KEY"),
                base_url=_get_env("OPENAI_BASE_URL"),
            )
        elif provider == "huggingface" or provider == "hf":
            return cls(
                provider="huggingface",
                model=_get_env(
                    "HUGGINGFACE_EMBEDDING_MODEL",
                    "sentence-transformers/all-MiniLM-L6-v2",
                )
                or "sentence-transformers/all-MiniLM-L6-v2",
                api_key=_get_env("HUGGINGFACE_API_KEY"),
            )
        elif provider == "cohere":
            return cls(
                provider="cohere",
                model=_get_env("COHERE_EMBEDDING_MODEL", "embed-english-v3.0")
                or "embed-english-v3.0",
                api_key=_get_env("COHERE_API_KEY"),
            )
        elif provider == "local":
            return cls(
                provider="local",
                model=_get_env("LOCAL_EMBEDDING_MODEL", "all-MiniLM-L6-v2")
                or "all-MiniLM-L6-v2",
                dimension=384,
            )
        else:
            raise ValueError(f"Unknown provider: {provider}")


class Embeddings:
    """
    Embedding model wrapper

    Supports multiple providers, automatically selecting the best implementation:
    1. Rust bindings (if available)
    2. OpenAI SDK (if available)
    3. httpx direct API calls

    Example:
        embeddings = Embeddings.from_env("openai")
        vector = embeddings.embed("text")
    """

    def __init__(
        self,
        provider: str = "openai",
        model: str | None = None,
        api_key: str | None = None,
        dimension: int | None = None,
        _use_rust: bool = True,
    ):
        """
        Create embedding model

        Args:
            provider: Provider name ("openai", "huggingface", "cohere", "local")
            model: Model name
            api_key: API key
            dimension: Vector dimension (optional)
            _use_rust: Whether to use Rust bindings (can be disabled for testing)
        """
        self.provider = provider
        self.model = model or self._default_model(provider)
        self.api_key = api_key
        self.dimension = dimension or self._default_dimension(provider, self.model)

        # Try to use Rust bindings (can be disabled)
        self._rust_embeddings = None
        use_rust_bindings = _use_rust and _RUST_AVAILABLE
        if use_rust_bindings and api_key:
            try:
                self._rust_embeddings = RustEmbeddings(
                    provider=provider,
                    model=self.model,
                    api_key=api_key,
                    dimension=self.dimension,
                )
                logger.debug(f"Using Rust bindings for {provider}")
            except (ImportError, AttributeError, TypeError, RuntimeError) as e:
                logger.warning(f"Rust bindings failed: {e}, falling back to Python")

        # If Rust is not available, set up Python fallback
        self._client = None
        if self._rust_embeddings is None and api_key:
            self._init_python_client()

    @staticmethod
    def _default_model(provider: str) -> str:
        """Get default model name"""
        defaults = {
            "openai": "text-embedding-3-small",
            "huggingface": "sentence-transformers/all-MiniLM-L6-v2",
            "cohere": "embed-english-v3.0",
            "local": "all-MiniLM-L6-v2",
        }
        return defaults.get(provider, "text-embedding-3-small")

    @staticmethod
    def _default_dimension(provider: str, model: str) -> int:
        """Get default dimension"""
        if provider == "openai":
            return 3072 if model == "text-embedding-3-large" else 1536
        elif provider == "huggingface":
            return 384 if "MiniLM" in model else 768
        elif provider == "cohere":
            return 1024
        elif provider == "local":
            return 384
        return 1536

    def _init_python_client(self):
        """Initialize Python client"""
        if self.provider == "openai" and _OPENAI_SDK_AVAILABLE:
            self._client = openai.OpenAI(api_key=self.api_key)
            logger.debug("Using OpenAI SDK")
        elif _HTTPX_AVAILABLE:
            self._client = httpx.Client(timeout=30.0)
            logger.debug(f"Using httpx for {self.provider}")
        else:
            logger.warning("No HTTP client available, embeddings will fail")

    @classmethod
    def from_env(cls, provider: str = "openai") -> Embeddings:
        """
        Create embedding model from environment variables

        Args:
            provider: Provider name

        Environment variables:
            - OPENAI_API_KEY / OPENAI_EMBEDDING_MODEL
            - HUGGINGFACE_API_KEY / HUGGINGFACE_EMBEDDING_MODEL
            - COHERE_API_KEY / COHERE_EMBEDDING_MODEL

        Returns:
            Embeddings instance
        """
        config = EmbeddingConfig.from_env(provider)
        return cls(
            provider=config.provider,
            model=config.model,
            api_key=config.api_key,
            dimension=config.dimension,
        )

    def embed(self, text: str) -> list[float]:
        """
        Generate embedding vector for a single text

        Args:
            text: Input text

        Returns:
            Embedding vector (list[float])
        """
        if self._rust_embeddings:
            return self._rust_embeddings.embed(text)

        return self._python_embed(text)

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """
        Generate embedding vectors for multiple texts

        Args:
            texts: List of texts

        Returns:
            List of embedding vectors
        """
        if self._rust_embeddings:
            return self._rust_embeddings.embed_batch(texts)

        return [self._python_embed(t) for t in texts]

    def _python_embed(self, text: str) -> list[float]:
        """Python fallback implementation"""
        if self.provider == "openai":
            return self._openai_embed(text)
        elif self.provider == "huggingface":
            return self._huggingface_embed(text)
        elif self.provider == "cohere":
            return self._cohere_embed(text)
        elif self.provider == "local":
            return self._mock_embed(text)

        raise ValueError(f"Unsupported provider: {self.provider}")

    def _openai_embed(self, text: str) -> list[float]:
        """OpenAI API call"""
        if hasattr(self._client, "embeddings"):
            # Use OpenAI SDK
            response = self._client.embeddings.create(
                model=self.model,
                input=text,
            )
            return response.data[0].embedding
        elif self._client:
            # Use httpx
            url = "https://api.openai.com/v1/embeddings"
            headers = {
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            }
            data = {
                "model": self.model,
                "input": text,
            }
            response = self._client.post(url, headers=headers, json=data)
            response.raise_for_status()
            result = response.json()
            return result["data"][0]["embedding"]

        raise RuntimeError("No client available for OpenAI")

    def _huggingface_embed(self, text: str) -> list[float]:
        """HuggingFace API call"""
        if not self._client:
            raise RuntimeError("No client available for HuggingFace")

        url = f"https://api-inference.huggingface.co/pipeline/feature-extraction/{self.model}"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        data = {"inputs": text}

        response = self._client.post(url, headers=headers, json=data)
        response.raise_for_status()

        embedding = response.json()
        # Ensure returning a flat list
        if isinstance(embedding, list) and len(embedding) > 0:
            if isinstance(embedding[0], list):
                # Some models return [[f32, f32, ...]]
                return embedding[0]
            return embedding

        raise RuntimeError(f"Unexpected HuggingFace response: {embedding}")

    def _cohere_embed(self, text: str) -> list[float]:
        """Cohere API call"""
        if not self._client:
            raise RuntimeError("No client available for Cohere")

        url = "https://api.cohere.ai/v1/embed"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        data = {
            "model": self.model,
            "texts": [text],
            "input_type": "search_document",
        }

        response = self._client.post(url, headers=headers, json=data)
        response.raise_for_status()

        result = response.json()
        return result["embeddings"]["float"][0]

    def _mock_embed(self, text: str) -> list[float]:
        """Mock embedding (for testing)"""
        # Simple hash-based mock, ensures consistency
        import hashlib

        text_hash = hashlib.md5(text.encode()).hexdigest()
        embedding = []
        for i in range(self.dimension):
            # Use hash chunk as pseudo-random value
            chunk = text_hash[i % len(text_hash)]
            val = (int(chunk, 16) / 15.0) - 0.5  # Range: -0.5 to 0.5
            embedding.append(val)

        # Normalize
        norm = sum(v * v for v in embedding) ** 0.5
        if norm > 0:
            embedding = [v / norm for v in embedding]

        return embedding

    def __repr__(self) -> str:
        return f"Embeddings(provider={self.provider}, model={self.model}, dim={self.dimension})"


# Predefined model configurations
PREDEFINED_EMBEDDINGS = {
    "openai_small": EmbeddingConfig(
        provider="openai",
        model="text-embedding-3-small",
        dimension=1536,
    ),
    "openai_large": EmbeddingConfig(
        provider="openai",
        model="text-embedding-3-large",
        dimension=3072,
    ),
    "huggingface_minilm": EmbeddingConfig(
        provider="huggingface",
        model="sentence-transformers/all-MiniLM-L6-v2",
        dimension=384,
    ),
    "cohere_english": EmbeddingConfig(
        provider="cohere",
        model="embed-english-v3.0",
        dimension=1024,
    ),
}


__all__ = [
    "Embeddings",
    "EmbeddingConfig",
    "PREDEFINED_EMBEDDINGS",
    "_RUST_AVAILABLE",
    "_OPENAI_SDK_AVAILABLE",
]
