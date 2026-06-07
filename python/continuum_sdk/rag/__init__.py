"""Continuum SDK RAG Module

Retrieval-Augmented Generation components.

Features:
    - VectorStore: In-memory vector storage
    - Multiple distance metrics (Cosine, Euclidean, DotProduct, Manhattan)
    - Metadata filtering support
    - Batch operations
    - RetrieverEngine: Document indexing and retrieval
    - Hybrid search (vector + keyword)
    - Configurable weights

Quick Start:
    >>> from continuum_sdk.rag import InMemoryVectorStore, DistanceMetric
    >>>
    >>> # Create store with cosine similarity
    >>> store = InMemoryVectorStore(metric=DistanceMetric.COSINE)
    >>>
    >>> # Add vectors
    >>> store.upsert("doc-1", [0.1, 0.2, 0.3], {"source": "test.txt"})
    True
    >>>
    >>> # Search similar vectors
    >>> results = store.search([0.1, 0.2, 0.3], top_k=5)
    >>> print(results[0].id)
    doc-1

RetrieverEngine:
    >>> from continuum_sdk.rag import DefaultRetrieverEngine, MockEmbeddingModel, FixedSizeChunker, Document
    >>>
    >>> # Create engine
    >>> engine = DefaultRetrieverEngine(
    ...     embedding_model=MockEmbeddingModel(128),
    ...     chunker=FixedSizeChunker()
    ... )
    >>>
    >>> # Index documents
    >>> doc = Document(content="Hello world", source="test.txt")
    >>> doc_ids = await engine.index([doc])
    >>>
    >>> # Retrieve
    >>> results = await engine.retrieve("Hello", top_k=5)
"""

from .retriever import (
    Chunk,
    ChunkingStrategy,
    ChunkPosition,
    DefaultRetrieverEngine,
    Document,
    EmbeddingModel,
    FixedSizeChunker,
    HybridWeights,
    MockEmbeddingModel,
    ParagraphChunker,
    RecursiveChunker,
    RetrievalResult,
    RetrieverEngine,
)
from .vectorstore import (
    DistanceMetric,
    InMemoryVectorStore,
    MetadataFilter,
    SearchResult,
    VectorItem,
    VectorStore,
    cosine_similarity,
    dot_product_similarity,
    euclidean_similarity,
    manhattan_similarity,
)

__all__ = [
    # VectorStore
    "VectorStore",
    "InMemoryVectorStore",
    "VectorItem",
    "SearchResult",
    "MetadataFilter",
    "DistanceMetric",
    "cosine_similarity",
    "euclidean_similarity",
    "dot_product_similarity",
    "manhattan_similarity",
    # RetrieverEngine
    "Document",
    "RetrievalResult",
    "Chunk",
    "ChunkPosition",
    "RetrieverEngine",
    "DefaultRetrieverEngine",
    "EmbeddingModel",
    "ChunkingStrategy",
    "FixedSizeChunker",
    "ParagraphChunker",
    "RecursiveChunker",
    "HybridWeights",
    "MockEmbeddingModel",
]
