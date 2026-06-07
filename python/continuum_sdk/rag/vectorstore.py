"""Vector Store Implementation

Vector storage: persistent vector indexing.

Features:
    - In-memory vector store (suitable for testing and development)
    - Multiple distance metric support (Cosine, Euclidean, DotProduct, Manhattan)
    - Batch operation optimization
    - Metadata filtering support
"""

from __future__ import annotations

import math
import threading
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class DistanceMetric(Enum):
    """Distance metric type"""

    COSINE = "cosine"
    EUCLIDEAN = "euclidean"
    DOT_PRODUCT = "dot_product"
    MANHATTAN = "manhattan"


@dataclass
class VectorItem:
    """Vector item"""

    id: str
    vector: list[float]
    metadata: dict[str, Any] = field(default_factory=dict)
    content: str | None = None


@dataclass
class SearchResult:
    """Search result"""

    id: str
    score: float
    content: str
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass
class MetadataFilter:
    """Metadata filter conditions"""

    must: dict[str, Any] = field(default_factory=dict)
    should: dict[str, Any] = field(default_factory=dict)
    must_not: dict[str, Any] = field(default_factory=dict)

    def matches(self, metadata: dict[str, Any]) -> bool:
        """Check if metadata matches filter conditions"""
        # Check must conditions (all must match)
        for key, value in self.must.items():
            if key not in metadata or metadata[key] != value:
                return False

        # Check must_not conditions (none must match)
        for key, value in self.must_not.items():
            if key in metadata and metadata[key] == value:
                return False

        # Check should conditions (at least one must match, pass if empty)
        if self.should:
            matched = False
            for key, value in self.should.items():
                if key in metadata and metadata[key] == value:
                    matched = True
                    break
            if not matched:
                return False

        return True


class VectorStore(ABC):
    """Vector store abstract class"""

    @abstractmethod
    def upsert(self, id: str, vector: list[float], metadata: dict[str, Any] | None = None) -> bool:
        """Insert or update vector

        Args:
            id: Vector unique identifier
            vector: Vector data
            metadata: Metadata

        Returns:
            Whether successful
        """
        pass

    @abstractmethod
    def search(
        self,
        vector: list[float],
        top_k: int = 10,
        filter: MetadataFilter | None = None,
    ) -> list[SearchResult]:
        """Search for similar vectors

        Args:
            vector: Query vector
            top_k: Number of results to return
            filter: Metadata filter conditions

        Returns:
            List of search results
        """
        pass

    @abstractmethod
    def delete(self, id: str) -> bool:
        """Delete vector

        Args:
            id: Vector unique identifier

        Returns:
            Whether successful
        """
        pass

    @abstractmethod
    def get(self, id: str) -> VectorItem | None:
        """Get vector

        Args:
            id: Vector unique identifier

        Returns:
            Vector item or None
        """
        pass

    @abstractmethod
    def count(self) -> int:
        """Get vector count"""
        pass

    @abstractmethod
    def clear(self) -> bool:
        """Clear store"""
        pass


def cosine_similarity(a: list[float], b: list[float]) -> float:
    """Calculate cosine similarity"""
    if len(a) != len(b):
        return 0.0

    dot = sum(x * y for x, y in zip(a, b))
    norm_a = math.sqrt(sum(x * x for x in a))
    norm_b = math.sqrt(sum(x * x for x in b))

    if norm_a == 0.0 or norm_b == 0.0:
        return 0.0

    return dot / (norm_a * norm_b)


def euclidean_similarity(a: list[float], b: list[float]) -> float:
    """Calculate Euclidean similarity (distance converted to similarity)"""
    if len(a) != len(b):
        return 0.0

    sum_sq = sum((x - y) ** 2 for x, y in zip(a, b))
    return 1.0 / (1.0 + math.sqrt(sum_sq))


def dot_product_similarity(a: list[float], b: list[float]) -> float:
    """Calculate dot product similarity"""
    if len(a) != len(b):
        return 0.0

    return sum(x * y for x, y in zip(a, b))


def manhattan_similarity(a: list[float], b: list[float]) -> float:
    """Calculate Manhattan similarity (distance converted to similarity)"""
    if len(a) != len(b):
        return 0.0

    sum_abs = sum(abs(x - y) for x, y in zip(a, b))
    return 1.0 / (1.0 + sum_abs)


class InMemoryVectorStore(VectorStore):
    """In-memory vector store implementation

    Uses in-memory storage for vectors, supports basic similarity search.
    Suitable for testing and development environments, not for large-scale production use.
    """

    def __init__(self, metric: DistanceMetric = DistanceMetric.COSINE):
        """Initialize in-memory vector store

        Args:
            metric: Distance metric type
        """
        self._data: dict[str, VectorItem] = {}
        self._metric = metric
        self._lock = threading.RLock()

        # Select similarity calculation function
        self._similarity_funcs = {
            DistanceMetric.COSINE: cosine_similarity,
            DistanceMetric.EUCLIDEAN: euclidean_similarity,
            DistanceMetric.DOT_PRODUCT: dot_product_similarity,
            DistanceMetric.MANHATTAN: manhattan_similarity,
        }

    def _compute_similarity(self, a: list[float], b: list[float]) -> float:
        """Calculate vector similarity"""
        return self._similarity_funcs[self._metric](a, b)

    def upsert(self, id: str, vector: list[float], metadata: dict[str, Any] | None = None) -> bool:
        """Insert or update vector"""
        with self._lock:
            self._data[id] = VectorItem(
                id=id,
                vector=vector,
                metadata=metadata or {},
            )
            return True

    def search(
        self,
        vector: list[float],
        top_k: int = 10,
        filter: MetadataFilter | None = None,
    ) -> list[SearchResult]:
        """Search for similar vectors"""
        with self._lock:
            # Filter first
            candidates: list[VectorItem] = []
            for item in self._data.values():
                if filter is None or filter.matches(item.metadata):
                    candidates.append(item)

            # Calculate similarity
            scores: list[tuple[VectorItem, float]] = []
            for item in candidates:
                score = self._compute_similarity(vector, item.vector)
                scores.append((item, score))

            # Sort by similarity descending
            scores.sort(key=lambda x: x[1], reverse=True)

            # Take top_k
            results: list[SearchResult] = []
            for item, score in scores[:top_k]:
                results.append(SearchResult(
                    id=item.id,
                    score=score,
                    content=item.content or "",
                    metadata=item.metadata,
                ))

            return results

    def delete(self, id: str) -> bool:
        """Delete vector"""
        with self._lock:
            if id in self._data:
                del self._data[id]
                return True
            return False

    def get(self, id: str) -> VectorItem | None:
        """Get vector"""
        with self._lock:
            return self._data.get(id)

    def count(self) -> int:
        """Get vector count"""
        with self._lock:
            return len(self._data)

    def clear(self) -> bool:
        """Clear store"""
        with self._lock:
            self._data.clear()
            return True

    # Batch operations
    def upsert_batch(self, items: list[tuple[str, list[float], dict[str, Any] | None]]) -> list[bool]:
        """Batch insert or update vectors

        Args:
            items: List of (id, vector, metadata) tuples

        Returns:
            List of operation results
        """
        results = []
        with self._lock:
            for id, vector, metadata in items:
                self._data[id] = VectorItem(
                    id=id,
                    vector=vector,
                    metadata=metadata or {},
                )
                results.append(True)
        return results

    def delete_batch(self, ids: list[str]) -> int:
        """Batch delete vectors

        Args:
            ids: List of vector IDs

        Returns:
            Number of deletions
        """
        count = 0
        with self._lock:
            for id in ids:
                if id in self._data:
                    del self._data[id]
                    count += 1
        return count
