"""Retriever Engine

Retrieval engine: vector similarity search and RAG support.

Features:
    - Document indexing and retrieval
    - Multiple chunking strategies (fixed size, paragraph, code)
    - Hybrid retrieval (vector + keyword)
    - RAG Pipeline (with reranking)
"""

from __future__ import annotations

import threading
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

from .vectorstore import InMemoryVectorStore

if TYPE_CHECKING:
    pass

__all__ = [
    "Document",
    "RetrievalResult",
    "Chunk",
    "ChunkPosition",
    "RetrieverEngine",
    "DefaultRetrieverEngine",
    "EmbeddingModel",
    "ChunkingStrategy",
    "FixedSizeChunker",
    "HybridWeights",
    "MockEmbeddingModel",
]


@dataclass
class Document:
    """Document structure

    Attributes:
        id: Document ID (optional, auto-generated)
        content: Document content
        metadata: Metadata
        source: Source (file path, URL, etc.)
    """

    content: str
    id: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    source: str | None = None

    def with_source(self, source: str) -> Document:
        """Set source"""
        self.source = source
        return self

    def with_metadata(self, key: str, value: Any) -> Document:
        """Add metadata"""
        self.metadata[key] = value
        return self


@dataclass
class RetrievalResult:
    """Retrieval result

    Attributes:
        doc_id: Document ID
        content: Document content
        score: Similarity score (0.0-1.0)
        metadata: Metadata
        source: Source
    """

    doc_id: str
    content: str
    score: float
    metadata: dict[str, Any] = field(default_factory=dict)
    source: str | None = None


@dataclass
class ChunkPosition:
    """Chunk position

    Attributes:
        start: Start character position
        end: End character position
        index: Chunk index
        total: Total chunks
    """

    start: int
    end: int
    index: int
    total: int


@dataclass
class Chunk:
    """Document chunk

    Attributes:
        id: Chunk ID
        doc_id: Document ID
        content: Chunk content
        position: Position in original text
        metadata: Metadata
    """

    id: str
    doc_id: str
    content: str
    position: ChunkPosition
    metadata: dict[str, Any] = field(default_factory=dict)


class HybridWeights:
    """Hybrid retrieval weight configuration

    Used to configure the weight ratio between vector search and keyword search.
    """

    def __init__(self, vector: float = 0.7, keyword: float = 0.3):
        """Initialize weights

        Args:
            vector: Vector search weight (default 0.7)
            keyword: Keyword search weight (default 0.3)

        Raises:
            ValueError: Weights do not sum to 1.0
        """
        if abs(vector + keyword - 1.0) > 0.001:
            raise ValueError(f"Weights must sum to 1.0, got {vector + keyword}")
        self.vector = vector
        self.keyword = keyword

    @classmethod
    def vector_only(cls) -> HybridWeights:
        """Use only vector search"""
        return cls(vector=1.0, keyword=0.0)

    @classmethod
    def keyword_only(cls) -> HybridWeights:
        """Use only keyword search"""
        return cls(vector=0.0, keyword=1.0)

    @classmethod
    def balanced(cls) -> HybridWeights:
        """Balanced weights"""
        return cls(vector=0.5, keyword=0.5)


class RetrieverEngine(ABC):
    """Retriever engine abstract base class

    Provides vector similarity search capabilities.
    """

    @abstractmethod
    async def index(self, documents: list[Document]) -> list[str]:
        """Index documents

        Args:
            documents: List of documents to index

        Returns:
            List of document IDs
        """
        pass  # pragma: no cover

    @abstractmethod
    async def retrieve(self, query: str, top_k: int = 5) -> list[RetrievalResult]:
        """Retrieve similar documents

        Args:
            query: Query text
            top_k: Number of results to return

        Returns:
            List of retrieval results
        """
        pass  # pragma: no cover

    @abstractmethod
    async def hybrid_retrieve(
        self, query: str, top_k: int = 5, weights: HybridWeights | None = None
    ) -> list[RetrievalResult]:
        """Hybrid retrieval (vector + keyword)

        Args:
            query: Query text
            top_k: Number of results to return
            weights: Weight configuration (default 70% vector + 30% keyword)

        Returns:
            List of retrieval results
        """
        pass  # pragma: no cover

    async def retrieve_with_filter(
        self, query: str, top_k: int = 5, filter: dict[str, Any] | None = None
    ) -> list[RetrievalResult]:
        """Retrieve with filter conditions

        Args:
            query: Query text
            top_k: Number of results to return
            filter: Metadata filter conditions

        Returns:
            List of retrieval results
        """
        _ = filter
        return await self.retrieve(query, top_k)

    @abstractmethod
    async def delete(self, doc_ids: list[str]) -> bool:
        """Delete documents

        Args:
            doc_ids: List of document IDs to delete

        Returns:
            Whether deletion was successful
        """
        pass  # pragma: no cover

    @abstractmethod
    async def clear(self) -> bool:
        """Clear index

        Returns:
            Whether clearing was successful
        """
        pass  # pragma: no cover

    @abstractmethod
    async def count(self) -> int:
        """Get document count

        Returns:
            Document count
        """
        pass  # pragma: no cover


@runtime_checkable
class EmbeddingModel(Protocol):
    """Embedding model protocol

    Defines text embedding vector generation interface.
    """

    async def embed(self, text: str) -> list[float]:
        """Generate text embedding vector

        Args:
            text: Input text

        Returns:
            Embedding vector
        """
        ...  # pragma: no cover

    async def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embedding vectors in batch

        Args:
            texts: List of input texts

        Returns:
            List of embedding vectors
        """
        ...  # pragma: no cover

    @property
    def dimension(self) -> int:
        """Vector dimension"""
        ...  # pragma: no cover

    @property
    def model_name(self) -> str:
        """Model name"""
        ...  # pragma: no cover


@runtime_checkable
class ChunkingStrategy(Protocol):
    """Chunking strategy protocol

    Defines document chunking interface.
    """

    def chunk(self, document: Document) -> list[Chunk]:
        """Chunk document

        Args:
            document: Input document

        Returns:
            List of chunks
        """
        ...  # pragma: no cover


class FixedSizeChunker:
    """Fixed size chunking strategy

    Chunks by fixed character count with overlap support.
    """

    def __init__(self, chunk_size: int = 500, overlap: int = 50):
        """Initialize chunker

        Args:
            chunk_size: Chunk size (character count)
            overlap: Overlap size
        """
        self.chunk_size = chunk_size
        self.overlap = overlap

    def chunk(self, document: Document) -> list[Chunk]:
        """Chunk document"""
        content = document.content

        # Content smaller than chunk size, return directly
        if len(content) <= self.chunk_size:
            return [
                Chunk(
                    id=f"{document.id or 'doc'}-0",
                    doc_id=document.id or "",
                    content=content,
                    position=ChunkPosition(start=0, end=len(content), index=0, total=1),
                    metadata=document.metadata.copy(),
                )
            ]

        chunks: list[Chunk] = []
        start = 0
        index = 0
        doc_id = document.id or str(uuid.uuid4())

        while start < len(content):
            end = min(start + self.chunk_size, len(content))
            chunks.append(
                Chunk(
                    id=f"{doc_id}-{index}",
                    doc_id=doc_id,
                    content=content[start:end],
                    position=ChunkPosition(start=start, end=end, index=index, total=0),
                    metadata=document.metadata.copy(),
                )
            )

            # Prevent infinite loop: when reaching end, set start = end directly
            start = end - self.overlap if end < len(content) else end
            index += 1

        # Update total chunks
        total = len(chunks)
        for chunk in chunks:
            chunk.position.total = total

        return chunks


class ParagraphChunker:
    """Paragraph chunking strategy

    Chunks by natural paragraph boundaries, preserving semantic integrity.
    Suitable for documents, articles, and other natural language content.
    """

    def __init__(self, max_chunk_size: int = 1000, min_chunk_size: int = 100):
        """Initialize paragraph chunker

        Args:
            max_chunk_size: Maximum chunk size
            min_chunk_size: Minimum chunk size
        """
        self.max_chunk_size = max_chunk_size
        self.min_chunk_size = min_chunk_size

    def chunk(self, document: Document) -> list[Chunk]:
        """Chunk document"""
        content = document.content
        paragraphs = [p for p in content.split("\n") if p.strip()]

        if not paragraphs:
            return [
                Chunk(
                    id=f"{document.id or 'doc'}-0",
                    doc_id=document.id or "",
                    content=content,
                    position=ChunkPosition(start=0, end=len(content), index=0, total=1),
                    metadata=document.metadata.copy(),
                )
            ]

        chunks: list[Chunk] = []
        current_chunk = ""
        start = 0
        index = 0

        for paragraph in paragraphs:
            if len(current_chunk) + len(paragraph) + 1 <= self.max_chunk_size:
                if current_chunk:
                    current_chunk += "\n"
                current_chunk += paragraph
            else:
                if len(current_chunk) >= self.min_chunk_size:
                    end = start + len(current_chunk)
                    chunks.append(
                        Chunk(
                            id=f"{document.id or 'doc'}-{index}",
                            doc_id=document.id or "",
                            content=current_chunk,
                            position=ChunkPosition(
                                start=start, end=end, index=index, total=0
                            ),
                            metadata=document.metadata.copy(),
                        )
                    )
                    start = end
                    index += 1

                current_chunk = paragraph

        # Handle the last chunk
        if len(current_chunk) >= self.min_chunk_size:
            chunks.append(
                Chunk(
                    id=f"{document.id or 'doc'}-{index}",
                    doc_id=document.id or "",
                    content=current_chunk,
                    position=ChunkPosition(
                        start=start, end=len(content), index=index, total=0
                    ),
                    metadata=document.metadata.copy(),
                )
            )

        total = len(chunks) or 1
        for chunk in chunks:
            chunk.position.total = total

        if not chunks:
            return [
                Chunk(
                    id=f"{document.id or 'doc'}-0",
                    doc_id=document.id or "",
                    content=content,
                    position=ChunkPosition(start=0, end=len(content), index=0, total=1),
                    metadata=document.metadata.copy(),
                )
            ]

        return chunks


class RecursiveChunker:
    """Recursive chunking strategy

    Tries multiple separators in order, from largest to smallest.
    Suitable for general text, preserving semantic integrity.
    """

    def __init__(self, max_chunk_size: int = 1000):
        """Initialize recursive chunker

        Args:
            max_chunk_size: Maximum chunk size
        """
        self.max_chunk_size = max_chunk_size
        self._separators = ["\n\n\n", "\n\n", "\n", ". ", " ", ""]

    def chunk(self, document: Document) -> list[Chunk]:
        """Chunk document"""
        return self._recursive_split(document, document.content, 0, 0)

    def _recursive_split(
        self,
        document: Document,
        text: str,
        start_offset: int,
        initial_index: int,
    ) -> list[Chunk]:
        """Recursive chunking"""
        if len(text) <= self.max_chunk_size:
            return [
                Chunk(
                    id=f"{document.id or 'doc'}-{initial_index}",
                    doc_id=document.id or "",
                    content=text,
                    position=ChunkPosition(
                        start=start_offset,
                        end=start_offset + len(text),
                        index=initial_index,
                        total=1,
                    ),
                    metadata=document.metadata.copy(),
                )
            ]

        for separator in self._separators:
            if separator == "":
                # Last resort: split by character
                chunks: list[Chunk] = []
                start = 0
                index = initial_index

                while start < len(text):
                    end = min(start + self.max_chunk_size, len(text))
                    chunks.append(
                        Chunk(
                            id=f"{document.id or 'doc'}-{index}",
                            doc_id=document.id or "",
                            content=text[start:end],
                            position=ChunkPosition(
                                start=start_offset + start,
                                end=start_offset + end,
                                index=index,
                                total=0,
                            ),
                            metadata=document.metadata.copy(),
                        )
                    )
                    start = end
                    index += 1

                total = len(chunks)
                for chunk in chunks:
                    chunk.position.total = total
                return chunks

            if separator in text:
                parts = text.split(separator)
                chunks = []
                current_chunk = ""
                current_start = start_offset
                index = initial_index

                for i, part in enumerate(parts):
                    part_with_sep = f"{part}{separator}" if i < len(parts) - 1 else part

                    if len(current_chunk) + len(part_with_sep) <= self.max_chunk_size:
                        current_chunk += part_with_sep
                    else:
                        if current_chunk:  # pragma: no branch
                            chunks.append(
                                Chunk(
                                    id=f"{document.id or 'doc'}-{index}",
                                    doc_id=document.id or "",
                                    content=current_chunk,
                                    position=ChunkPosition(
                                        start=current_start,
                                        end=current_start + len(current_chunk),
                                        index=index,
                                        total=0,
                                    ),
                                    metadata=document.metadata.copy(),
                                )
                            )
                            current_start += len(current_chunk)
                            index += 1

                        if len(part_with_sep) > self.max_chunk_size:
                            # Recursive split
                            sub_chunks = self._recursive_split(
                                document, part_with_sep, current_start, index
                            )
                            for sub in sub_chunks:
                                current_start = sub.position.end
                                index += 1
                                chunks.append(sub)
                        else:
                            current_chunk = part_with_sep

                if current_chunk:  # pragma: no branch
                    chunks.append(
                        Chunk(
                            id=f"{document.id or 'doc'}-{index}",
                            doc_id=document.id or "",
                            content=current_chunk,
                            position=ChunkPosition(
                                start=current_start,
                                end=start_offset + len(text),
                                index=index,
                                total=0,
                            ),
                            metadata=document.metadata.copy(),
                        )
                    )

                total = len(chunks) or 1
                for chunk in chunks:
                    chunk.position.total = total
                return chunks

        # This return is unreachable because the empty string separator (last in list)
        # is always found in text, triggering the character-level split above.
        return [  # pragma: no cover
            Chunk(
                id=f"{document.id or 'doc'}-{initial_index}",
                doc_id=document.id or "",
                content=text,
                position=ChunkPosition(
                    start=start_offset,
                    end=start_offset + len(text),
                    index=initial_index,
                    total=1,
                ),
                metadata=document.metadata.copy(),
            )
        ]


class MockEmbeddingModel:
    """Mock Embedding model (for testing)

    Generates hash-based pseudo vectors.
    """

    def __init__(self, dimension: int = 128):
        """Initialize Mock model

        Args:
            dimension: Vector dimension
        """
        self._dimension = dimension

    async def embed(self, text: str) -> list[float]:
        """Generate text-based pseudo vector"""
        vector: list[float] = []
        bytes_data = text.encode("utf-8")
        for i in range(self._dimension):
            byte_val = bytes_data[i % len(bytes_data)] if bytes_data else 0
            vector.append(byte_val / 255.0)
        return vector

    async def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embedding vectors in batch"""
        embeddings: list[list[float]] = []
        for text in texts:
            embeddings.append(await self.embed(text))
        return embeddings

    @property
    def dimension(self) -> int:
        """Vector dimension"""
        return self._dimension

    @property
    def model_name(self) -> str:
        """Model name"""
        return "mock-embedding-model"


# Re-export from vectorstore for convenience
VectorStoreAdapter = InMemoryVectorStore


class DefaultRetrieverEngine(RetrieverEngine):
    """Default retriever engine implementation

    Combines Embedding model, chunking strategy, and vector store to provide complete RAG functionality.

    Example:
        >>> from continuum_sdk.rag import DefaultRetrieverEngine, MockEmbeddingModel, FixedSizeChunker
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

    def __init__(
        self,
        embedding_model: EmbeddingModel,
        chunker: ChunkingStrategy | None = None,
        vector_store: InMemoryVectorStore | None = None,
        hybrid_weights: HybridWeights | None = None,
    ):
        """Initialize retriever engine

        Args:
            embedding_model: Embedding model
            chunker: Chunking strategy (default FixedSizeChunker)
            vector_store: Vector store (default InMemoryVectorStore)
            hybrid_weights: Hybrid retrieval weights (default 70% vector + 30% keyword)
        """
        self._embedding_model = embedding_model
        self._chunker = chunker or FixedSizeChunker()
        self._vector_store = vector_store or InMemoryVectorStore()
        self._hybrid_weights = hybrid_weights or HybridWeights()

        # Document index (document ID -> chunk ID list)
        self._doc_index: dict[str, list[str]] = {}
        # Chunk content cache (chunk ID -> content)
        self._chunk_cache: dict[str, str] = {}
        self._lock = threading.RLock()

    async def index(self, documents: list[Document]) -> list[str]:
        """Index documents"""
        doc_ids: list[str] = []

        for doc in documents:
            # Generate document ID
            doc_id = doc.id or str(uuid.uuid4())

            # Create document copy with ID
            doc_with_id = Document(
                id=doc_id,
                content=doc.content,
                metadata=doc.metadata.copy(),
                source=doc.source,
            )

            # Chunk
            chunks = self._chunker.chunk(doc_with_id)
            chunk_ids = [c.id for c in chunks]
            chunk_contents = [c.content for c in chunks]

            # Generate embeddings in batch
            embeddings = await self._embedding_model.embed_batch(chunk_contents)

            # Build vector items and store
            with self._lock:
                for chunk, embedding in zip(chunks, embeddings, strict=True):
                    metadata = chunk.metadata.copy()
                    metadata["doc_id"] = chunk.doc_id
                    metadata["chunk_index"] = chunk.position.index
                    if doc.source:
                        metadata["source"] = doc.source

                    # Use VectorStore's sync interface
                    self._vector_store.upsert(
                        chunk.id,
                        embedding,
                        metadata,
                    )

                    # Cache chunk content
                    self._chunk_cache[chunk.id] = chunk.content

                # Record document index
                self._doc_index[doc_id] = chunk_ids

            doc_ids.append(doc_id)

        return doc_ids

    async def retrieve(self, query: str, top_k: int = 5) -> list[RetrievalResult]:
        """Retrieve similar documents"""
        # Generate query vector
        query_embedding = await self._embedding_model.embed(query)

        # Search similar vectors (using VectorStore's sync interface)
        results = self._vector_store.search(query_embedding, top_k)

        # Convert to RetrievalResult
        retrieval_results: list[RetrievalResult] = []
        with self._lock:
            for r in results:
                content = r.content or self._chunk_cache.get(r.id, "")
                retrieval_results.append(
                    RetrievalResult(
                        doc_id=r.id,
                        content=content,
                        score=r.score,
                        metadata=r.metadata.copy(),
                        source=r.metadata.get("source"),
                    )
                )

        return retrieval_results

    async def hybrid_retrieve(
        self, query: str, top_k: int = 5, weights: HybridWeights | None = None
    ) -> list[RetrievalResult]:
        """Hybrid retrieval (vector + keyword)"""
        w = weights or self._hybrid_weights

        # Vector search
        vector_results = await self.retrieve(query, top_k * 2)

        if w.keyword == 0.0:
            return vector_results[:top_k]

        # Keyword match enhancement
        query_lower = query.lower()
        query_keywords = query_lower.split()

        # Recalculate scores (vector score + keyword match bonus)
        scored_results: list[RetrievalResult] = []
        for r in vector_results:
            content_lower = r.content.lower()
            keyword_matches = sum(1 for kw in query_keywords if kw in content_lower)

            # Hybrid score
            keyword_score = (
                (keyword_matches / max(len(query_keywords), 1)) * w.keyword
                if query_keywords
                else 0.0
            )
            final_score = r.score * w.vector + keyword_score

            scored_results.append(
                RetrievalResult(
                    doc_id=r.doc_id,
                    content=r.content,
                    score=final_score,
                    metadata=r.metadata.copy(),
                    source=r.source,
                )
            )

        # Sort by score and truncate
        scored_results.sort(key=lambda x: x.score, reverse=True)
        return scored_results[:top_k]

    async def delete(self, doc_ids: list[str]) -> bool:
        """Delete documents"""
        all_chunk_ids: list[str] = []

        with self._lock:
            for doc_id in doc_ids:
                if doc_id in self._doc_index:
                    chunk_ids = self._doc_index.pop(doc_id)
                    for chunk_id in chunk_ids:
                        self._chunk_cache.pop(chunk_id, None)
                    all_chunk_ids.extend(chunk_ids)

        if not all_chunk_ids:
            return False

        # Use VectorStore's sync batch delete interface
        count = self._vector_store.delete_batch(all_chunk_ids)
        return count > 0

    async def clear(self) -> bool:
        """Clear index"""
        # Use VectorStore's sync interface
        self._vector_store.clear()
        with self._lock:
            self._doc_index.clear()
            self._chunk_cache.clear()
        return True

    async def count(self) -> int:
        """Get document count"""
        with self._lock:
            return len(self._doc_index)
