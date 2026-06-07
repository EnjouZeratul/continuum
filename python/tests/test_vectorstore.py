"""VectorStore Tests"""

import pytest
from continuum_sdk.rag import (
    InMemoryVectorStore,
    DistanceMetric,
    MetadataFilter,
    VectorItem,
    SearchResult,
    VectorStore,
    cosine_similarity,
    euclidean_similarity,
    dot_product_similarity,
    manhattan_similarity,
)


class TestSimilarityFunctions:
    """Test similarity calculation functions"""

    def test_cosine_similarity_identical(self):
        """Same vectors should have similarity 1.0"""
        sim = cosine_similarity([1.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        assert abs(sim - 1.0) < 0.001

    def test_cosine_similarity_orthogonal(self):
        """Orthogonal vectors should have similarity 0.0"""
        sim = cosine_similarity([1.0, 0.0], [0.0, 1.0])
        assert abs(sim - 0.0) < 0.001

    def test_cosine_similarity_opposite(self):
        """Opposite vectors should have similarity -1.0"""
        sim = cosine_similarity([1.0, 0.0], [-1.0, 0.0])
        assert abs(sim - (-1.0)) < 0.001

    def test_cosine_similarity_zero_vector(self):
        """Zero vector should have similarity 0.0"""
        sim = cosine_similarity([0.0, 0.0], [1.0, 0.0])
        assert abs(sim - 0.0) < 0.001

    def test_cosine_similarity_different_length(self):
        """Different length vectors should have similarity 0.0"""
        sim = cosine_similarity([1.0, 0.0], [1.0, 0.0, 0.0])
        assert sim == 0.0

    def test_euclidean_similarity(self):
        """Test Euclidean similarity"""
        # Same vectors
        sim = euclidean_similarity([1.0, 0.0], [1.0, 0.0])
        assert abs(sim - 1.0) < 0.001

        # Distance of 1 -> similarity 0.5
        sim = euclidean_similarity([0.0, 0.0], [1.0, 0.0])
        assert abs(sim - 0.5) < 0.001

    def test_dot_product_similarity(self):
        """Test DotProduct similarity"""
        sim = dot_product_similarity([1.0, 2.0], [3.0, 4.0])
        assert abs(sim - 11.0) < 0.001  # 1*3 + 2*4 = 11

    def test_manhattan_similarity(self):
        """Test Manhattan similarity"""
        # Same vectors
        sim = manhattan_similarity([1.0, 0.0], [1.0, 0.0])
        assert abs(sim - 1.0) < 0.001

        # Distance of 1 -> similarity 0.5
        sim = manhattan_similarity([0.0, 0.0], [1.0, 0.0])
        assert abs(sim - 0.5) < 0.001


class TestInMemoryVectorStore:
    """Test InMemoryVectorStore"""

    def test_upsert(self):
        """Test upsert operation"""
        store = InMemoryVectorStore()
        result = store.upsert("id-1", [1.0, 2.0, 3.0], {"source": "test"})
        assert result is True
        assert store.count() == 1

    def test_upsert_update(self):
        """Test upsert updates existing vector"""
        store = InMemoryVectorStore()
        store.upsert("id-1", [1.0, 2.0], {"v": 1})
        store.upsert("id-1", [3.0, 4.0], {"v": 2})

        item = store.get("id-1")
        assert item is not None
        assert item.vector == [3.0, 4.0]
        assert item.metadata["v"] == 2

    def test_search(self):
        """Test search operation"""
        store = InMemoryVectorStore()

        # Add vectors
        store.upsert("id-1", [1.0, 0.0, 0.0], {"type": "a"})
        store.upsert("id-2", [0.9, 0.1, 0.0], {"type": "b"})
        store.upsert("id-3", [0.0, 1.0, 0.0], {"type": "c"})

        # Search for similar to [1.0, 0.0, 0.0]
        results = store.search([1.0, 0.0, 0.0], top_k=2)

        assert len(results) == 2
        assert results[0].id == "id-1"
        assert results[0].score > results[1].score

    def test_search_with_filter(self):
        """Test search with metadata filter"""
        store = InMemoryVectorStore()

        store.upsert("id-1", [1.0, 0.0], {"type": "doc", "lang": "en"})
        store.upsert("id-2", [0.9, 0.1], {"type": "code"})
        store.upsert("id-3", [0.0, 1.0], {"type": "doc", "lang": "zh"})

        # Filter for type=doc
        filter = MetadataFilter(must={"type": "doc"})
        results = store.search([1.0, 0.0], top_k=10, filter=filter)

        assert len(results) == 2
        for r in results:
            assert r.metadata["type"] == "doc"

    def test_delete(self):
        """Test delete operation"""
        store = InMemoryVectorStore()
        store.upsert("id-1", [1.0, 2.0])

        result = store.delete("id-1")
        assert result is True
        assert store.count() == 0

        # Delete non-existent
        result = store.delete("id-1")
        assert result is False

    def test_get(self):
        """Test get operation"""
        store = InMemoryVectorStore()
        store.upsert("id-1", [1.0, 2.0], {"key": "value"})

        item = store.get("id-1")
        assert item is not None
        assert item.id == "id-1"
        assert item.vector == [1.0, 2.0]
        assert item.metadata == {"key": "value"}

        # Get non-existent
        item = store.get("id-2")
        assert item is None

    def test_count(self):
        """Test count operation"""
        store = InMemoryVectorStore()
        assert store.count() == 0

        store.upsert("id-1", [1.0])
        store.upsert("id-2", [2.0])
        assert store.count() == 2

    def test_clear(self):
        """Test clear operation"""
        store = InMemoryVectorStore()
        store.upsert("id-1", [1.0])
        store.upsert("id-2", [2.0])

        result = store.clear()
        assert result is True
        assert store.count() == 0

    def test_upsert_batch(self):
        """Test batch upsert"""
        store = InMemoryVectorStore()
        items = [
            ("id-1", [1.0], {"a": 1}),
            ("id-2", [2.0], {"a": 2}),
            ("id-3", [3.0], None),
        ]
        results = store.upsert_batch(items)
        assert all(results)
        assert store.count() == 3

    def test_delete_batch(self):
        """Test batch delete"""
        store = InMemoryVectorStore()
        store.upsert("id-1", [1.0])
        store.upsert("id-2", [2.0])
        store.upsert("id-3", [3.0])

        count = store.delete_batch(["id-1", "id-2", "id-4"])
        assert count == 2
        assert store.count() == 1

    def test_different_metrics(self):
        """Test different distance metrics"""
        # Cosine
        store_cosine = InMemoryVectorStore(metric=DistanceMetric.COSINE)
        store_cosine.upsert("id-1", [1.0, 0.0])
        results = store_cosine.search([1.0, 0.0], top_k=1)
        assert abs(results[0].score - 1.0) < 0.001

        # DotProduct
        store_dot = InMemoryVectorStore(metric=DistanceMetric.DOT_PRODUCT)
        store_dot.upsert("id-1", [2.0, 3.0])
        results = store_dot.search([1.0, 1.0], top_k=1)
        assert abs(results[0].score - 5.0) < 0.001


class TestMetadataFilter:
    """Test MetadataFilter"""

    def test_must_condition(self):
        """Test must condition"""
        filter = MetadataFilter(must={"type": "doc"})
        assert filter.matches({"type": "doc"})
        assert not filter.matches({"type": "code"})
        assert not filter.matches({})

    def test_must_not_condition(self):
        """Test must_not condition"""
        filter = MetadataFilter(must_not={"type": "code"})
        assert filter.matches({"type": "doc"})
        assert not filter.matches({"type": "code"})
        assert filter.matches({})

    def test_should_condition(self):
        """Test should condition"""
        # Note: In Python, {"lang": "en", "lang": "zh"} would override, so we test with one key
        filter = MetadataFilter(should={"lang": "en"})
        assert filter.matches({"lang": "en"})
        assert not filter.matches({"lang": "zh"})
        assert not filter.matches({"lang": "fr"})
        assert not filter.matches({})

    def test_empty_should(self):
        """Test empty should condition"""
        filter = MetadataFilter(should={})
        assert filter.matches({"any": "value"})
        assert filter.matches({})

    def test_combined_conditions(self):
        """Test combined conditions"""
        filter = MetadataFilter(
            must={"type": "doc"},
            must_not={"draft": True},
            should={"lang": "en"},
        )
        assert filter.matches({"type": "doc", "lang": "en"})
        assert not filter.matches({"type": "doc", "draft": True})
        assert not filter.matches({"type": "code", "lang": "en"})


class TestVectorItem:
    """Test VectorItem dataclass"""

    def test_creation(self):
        """Test VectorItem creation"""
        item = VectorItem(id="test", vector=[1.0, 2.0])
        assert item.id == "test"
        assert item.vector == [1.0, 2.0]
        assert item.metadata == {}
        assert item.content is None

    def test_with_metadata(self):
        """Test VectorItem with metadata"""
        item = VectorItem(id="test", vector=[1.0], metadata={"key": "value"}, content="text")
        assert item.metadata == {"key": "value"}
        assert item.content == "text"


class TestSearchResult:
    """Test SearchResult dataclass"""

    def test_creation(self):
        """Test SearchResult creation"""
        result = SearchResult(id="test", score=0.95, content="hello", metadata={"a": 1})
        assert result.id == "test"
        assert result.score == 0.95
        assert result.content == "hello"
        assert result.metadata == {"a": 1}


class TestSimilarityDifferentLength:
    """Test similarity functions with different length vectors"""

    def test_euclidean_similarity_different_length(self):
        """Different length vectors should have similarity 0.0"""
        sim = euclidean_similarity([1.0, 0.0], [1.0, 0.0, 0.0])
        assert sim == 0.0

    def test_dot_product_similarity_different_length(self):
        """Different length vectors should have similarity 0.0"""
        sim = dot_product_similarity([1.0, 0.0], [1.0, 0.0, 0.0])
        assert sim == 0.0

    def test_manhattan_similarity_different_length(self):
        """Different length vectors should have similarity 0.0"""
        sim = manhattan_similarity([1.0, 0.0], [1.0, 0.0, 0.0])
        assert sim == 0.0


class TestVectorStoreAbstractMethods:
    """Test VectorStore abstract methods coverage"""

    def test_abstract_methods_callable(self):
        """Test that abstract methods can be called via concrete implementation"""
        # This test ensures the abstract method pass statements are covered
        # by creating a minimal concrete implementation that calls super()
        class MinimalVectorStore(InMemoryVectorStore):
            """Minimal implementation that calls parent abstract methods"""

            def upsert(self, id, vector, metadata=None):
                # Call the abstract method's pass statement via super() on VectorStore
                try:
                    VectorStore.upsert(self, id, vector, metadata)
                except TypeError:
                    pass  # Abstract method doesn't actually do anything
                return super().upsert(id, vector, metadata)

            def search(self, vector, top_k=10, filter=None):
                try:
                    VectorStore.search(self, vector, top_k, filter)
                except TypeError:
                    pass
                return super().search(vector, top_k, filter)

            def delete(self, id):
                try:
                    VectorStore.delete(self, id)
                except TypeError:
                    pass
                return super().delete(id)

            def get(self, id):
                try:
                    VectorStore.get(self, id)
                except TypeError:
                    pass
                return super().get(id)

            def count(self):
                try:
                    VectorStore.count(self)
                except TypeError:
                    pass
                return super().count()

            def clear(self):
                try:
                    VectorStore.clear(self)
                except TypeError:
                    pass
                return super().clear()

        store = MinimalVectorStore()

        # Exercise all abstract methods via the concrete implementation
        assert store.upsert("id-1", [1.0, 2.0]) is True
        results = store.search([1.0, 2.0], top_k=1)
        assert len(results) == 1
        assert store.delete("id-1") is True
        assert store.get("id-1") is None
        assert store.count() == 0
        assert store.clear() is True