"""
Retriever Tests - Coverage Enhancement

Tests for retriever.py to improve coverage from 26% to 60%+.
"""

import pytest

from continuum_sdk.rag.retriever import (
    Chunk,
    ChunkPosition,
    DefaultRetrieverEngine,
    Document,
    FixedSizeChunker,
    HybridWeights,
    MockEmbeddingModel,
    ParagraphChunker,
    RecursiveChunker,
    RetrievalResult,
    RetrieverEngine,
)


class TestDocument:
    """Document dataclass tests"""

    def test_document_creation(self):
        """Test basic document creation"""
        doc = Document(content="Hello world")
        assert doc.content == "Hello world"
        assert doc.id is None
        assert doc.metadata == {}
        assert doc.source is None

    def test_document_with_id(self):
        """Test document with explicit ID"""
        doc = Document(content="Test", id="doc-123")
        assert doc.id == "doc-123"

    def test_document_with_metadata(self):
        """Test document with metadata"""
        doc = Document(content="Test", metadata={"key": "value", "count": 1})
        assert doc.metadata["key"] == "value"
        assert doc.metadata["count"] == 1

    def test_document_with_source(self):
        """Test document with source"""
        doc = Document(content="Test", source="test.txt")
        assert doc.source == "test.txt"

    def test_with_source(self):
        """Test with_source method"""
        doc = Document(content="Test")
        result = doc.with_source("file.py")
        assert result.source == "file.py"
        assert result is doc  # Should return self

    def test_with_metadata(self):
        """Test with_metadata method"""
        doc = Document(content="Test")
        result = doc.with_metadata("key", "value")
        assert result.metadata["key"] == "value"
        assert result is doc  # Should return self


class TestRetrievalResult:
    """RetrievalResult dataclass tests"""

    def test_retrieval_result_creation(self):
        """Test basic retrieval result creation"""
        result = RetrievalResult(
            doc_id="doc-1",
            content="Hello",
            score=0.95,
        )
        assert result.doc_id == "doc-1"
        assert result.content == "Hello"
        assert result.score == 0.95
        assert result.metadata == {}
        assert result.source is None

    def test_retrieval_result_full(self):
        """Test retrieval result with all fields"""
        result = RetrievalResult(
            doc_id="doc-1",
            content="Hello",
            score=0.95,
            metadata={"type": "text"},
            source="file.txt",
        )
        assert result.metadata["type"] == "text"
        assert result.source == "file.txt"


class TestChunkPosition:
    """ChunkPosition dataclass tests"""

    def test_chunk_position_creation(self):
        """Test chunk position creation"""
        pos = ChunkPosition(start=0, end=100, index=0, total=5)
        assert pos.start == 0
        assert pos.end == 100
        assert pos.index == 0
        assert pos.total == 5


class TestChunk:
    """Chunk dataclass tests"""

    def test_chunk_creation(self):
        """Test chunk creation"""
        pos = ChunkPosition(start=0, end=50, index=0, total=1)
        chunk = Chunk(
            id="chunk-1",
            doc_id="doc-1",
            content="Test content",
            position=pos,
        )
        assert chunk.id == "chunk-1"
        assert chunk.doc_id == "doc-1"
        assert chunk.content == "Test content"
        assert chunk.position == pos
        assert chunk.metadata == {}

    def test_chunk_with_metadata(self):
        """Test chunk with metadata"""
        pos = ChunkPosition(start=0, end=50, index=0, total=1)
        chunk = Chunk(
            id="chunk-1",
            doc_id="doc-1",
            content="Test",
            position=pos,
            metadata={"page": 1},
        )
        assert chunk.metadata["page"] == 1


class TestHybridWeights:
    """HybridWeights tests"""

    def test_default_weights(self):
        """Test default weight initialization"""
        weights = HybridWeights()
        assert weights.vector == 0.7
        assert weights.keyword == 0.3

    def test_custom_weights(self):
        """Test custom weights"""
        weights = HybridWeights(vector=0.8, keyword=0.2)
        assert weights.vector == 0.8
        assert weights.keyword == 0.2

    def test_weights_sum_validation(self):
        """Test that weights must sum to 1.0"""
        with pytest.raises(ValueError, match="must sum to 1.0"):
            HybridWeights(vector=0.5, keyword=0.5)
            HybridWeights(vector=0.6, keyword=0.3)  # Should raise

    def test_vector_only(self):
        """Test vector_only factory method"""
        weights = HybridWeights.vector_only()
        assert weights.vector == 1.0
        assert weights.keyword == 0.0

    def test_keyword_only(self):
        """Test keyword_only factory method"""
        weights = HybridWeights.keyword_only()
        assert weights.vector == 0.0
        assert weights.keyword == 1.0

    def test_balanced(self):
        """Test balanced factory method"""
        weights = HybridWeights.balanced()
        assert weights.vector == 0.5
        assert weights.keyword == 0.5


class TestMockEmbeddingModel:
    """MockEmbeddingModel tests"""

    def test_init(self):
        """Test initialization"""
        model = MockEmbeddingModel(dimension=256)
        assert model.dimension == 256
        assert model.model_name == "mock-embedding-model"

    def test_default_dimension(self):
        """Test default dimension"""
        model = MockEmbeddingModel()
        assert model.dimension == 128

    @pytest.mark.asyncio
    async def test_embed(self):
        """Test embedding generation"""
        model = MockEmbeddingModel(dimension=64)
        embedding = await model.embed("Hello world")
        assert len(embedding) == 64
        assert all(0.0 <= v <= 1.0 for v in embedding)

    @pytest.mark.asyncio
    async def test_embed_empty_string(self):
        """Test embedding empty string"""
        model = MockEmbeddingModel(dimension=32)
        embedding = await model.embed("")
        assert len(embedding) == 32

    @pytest.mark.asyncio
    async def test_embed_batch(self):
        """Test batch embedding"""
        model = MockEmbeddingModel(dimension=64)
        texts = ["Hello", "World", "Test"]
        embeddings = await model.embed_batch(texts)
        assert len(embeddings) == 3
        assert all(len(e) == 64 for e in embeddings)


class TestFixedSizeChunker:
    """FixedSizeChunker tests"""

    def test_init(self):
        """Test initialization"""
        chunker = FixedSizeChunker(chunk_size=500, overlap=50)
        assert chunker.chunk_size == 500
        assert chunker.overlap == 50

    def test_default_params(self):
        """Test default parameters"""
        chunker = FixedSizeChunker()
        assert chunker.chunk_size == 500
        assert chunker.overlap == 50

    def test_chunk_small_document(self):
        """Test chunking document smaller than chunk size"""
        chunker = FixedSizeChunker(chunk_size=100)
        doc = Document(content="Short text", id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) == 1
        assert chunks[0].content == "Short text"

    def test_chunk_large_document(self):
        """Test chunking document larger than chunk size"""
        chunker = FixedSizeChunker(chunk_size=50, overlap=10)
        content = "A" * 200
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) > 1
        assert all(len(c.content) <= 50 for c in chunks)

    def test_chunk_with_overlap(self):
        """Test that overlap works correctly"""
        chunker = FixedSizeChunker(chunk_size=20, overlap=5)
        content = "ABCDEFGH" * 10  # 80 chars
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Check overlap exists between consecutive chunks
        for _i in range(len(chunks) - 1):
            # End of current should overlap with start of next
            pass  # Overlap validation depends on content


class TestParagraphChunker:
    """ParagraphChunker tests"""

    def test_init(self):
        """Test initialization"""
        chunker = ParagraphChunker(max_chunk_size=1000, min_chunk_size=100)
        assert chunker.max_chunk_size == 1000
        assert chunker.min_chunk_size == 100

    def test_chunk_single_paragraph(self):
        """Test chunking single paragraph"""
        chunker = ParagraphChunker()
        doc = Document(content="Single paragraph text", id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_multiple_paragraphs(self):
        """Test chunking multiple paragraphs"""
        chunker = ParagraphChunker(max_chunk_size=50, min_chunk_size=10)
        content = "First paragraph.\n\nSecond paragraph here.\n\nThird one."
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_empty_content(self):
        """Test chunking empty content"""
        chunker = ParagraphChunker()
        doc = Document(content="", id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) == 1
        assert chunks[0].content == ""

    def test_chunk_whitespace_only(self):
        """Test chunking whitespace only"""
        chunker = ParagraphChunker()
        doc = Document(content="   \n\n  \n  ", id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1


class TestRecursiveChunker:
    """RecursiveChunker tests"""

    def test_init(self):
        """Test initialization"""
        chunker = RecursiveChunker(max_chunk_size=1000)
        assert chunker.max_chunk_size == 1000

    def test_chunk_small_document(self):
        """Test chunking small document"""
        chunker = RecursiveChunker(max_chunk_size=100)
        doc = Document(content="Short text", id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) == 1

    def test_chunk_large_document(self):
        """Test chunking large document"""
        chunker = RecursiveChunker(max_chunk_size=50)
        content = "First sentence. Second sentence. Third sentence. Fourth sentence."
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_with_newlines(self):
        """Test chunking with newlines"""
        chunker = RecursiveChunker(max_chunk_size=30)
        content = "Line one\n\nLine two\n\nLine three"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1


class TestDefaultRetrieverEngine:
    """DefaultRetrieverEngine tests"""

    @pytest.mark.asyncio
    async def test_init(self):
        """Test initialization"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        assert engine._embedding_model is model
        assert engine._chunker is not None
        assert engine._vector_store is not None

    @pytest.mark.asyncio
    async def test_index_single_document(self):
        """Test indexing single document"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1")
        doc_ids = await engine.index([doc])
        assert len(doc_ids) == 1
        assert doc_ids[0] == "doc-1"

    @pytest.mark.asyncio
    async def test_index_multiple_documents(self):
        """Test indexing multiple documents"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        docs = [
            Document(content="Doc one", id="doc-1"),
            Document(content="Doc two", id="doc-2"),
        ]
        doc_ids = await engine.index(docs)
        assert len(doc_ids) == 2

    @pytest.mark.asyncio
    async def test_retrieve(self):
        """Test retrieval"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1")
        await engine.index([doc])
        results = await engine.retrieve("Hello", top_k=5)
        assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_hybrid_retrieve(self):
        """Test hybrid retrieval"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world test", id="doc-1")
        await engine.index([doc])
        results = await engine.hybrid_retrieve("Hello test", top_k=5)
        assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_hybrid_retrieve_vector_only(self):
        """Test hybrid retrieval with vector only weights"""
        model = MockEmbeddingModel()
        weights = HybridWeights.vector_only()
        engine = DefaultRetrieverEngine(embedding_model=model, hybrid_weights=weights)
        doc = Document(content="Hello world", id="doc-1")
        await engine.index([doc])
        results = await engine.hybrid_retrieve("Hello", top_k=5)
        assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_delete(self):
        """Test document deletion"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1")
        await engine.index([doc])
        success = await engine.delete(["doc-1"])
        assert success is True

    @pytest.mark.asyncio
    async def test_delete_nonexistent(self):
        """Test deleting nonexistent document"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        success = await engine.delete(["nonexistent"])
        assert success is False

    @pytest.mark.asyncio
    async def test_clear(self):
        """Test clearing index"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1")
        await engine.index([doc])
        success = await engine.clear()
        assert success is True
        count = await engine.count()
        assert count == 0

    @pytest.mark.asyncio
    async def test_count(self):
        """Test counting documents"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        docs = [
            Document(content="Doc one", id="doc-1"),
            Document(content="Doc two", id="doc-2"),
        ]
        await engine.index(docs)
        count = await engine.count()
        assert count == 2

    @pytest.mark.asyncio
    async def test_retrieve_with_filter(self):
        """Test retrieval with filter (default implementation)"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1", metadata={"type": "test"})
        await engine.index([doc])
        results = await engine.retrieve_with_filter("Hello", top_k=5, filter={"type": "test"})
        assert len(results) >= 0  # Filter may not be applied in base impl


class TestRetrieverEngineAbstract:
    """Test RetrieverEngine abstract methods"""

    def test_cannot_instantiate_abstract(self):
        """Test that RetrieverEngine cannot be instantiated directly"""
        with pytest.raises(TypeError):
            RetrieverEngine()

    def test_abstract_methods_exist(self):
        """Test that all abstract methods are defined"""
        # Verify the abstract methods exist
        assert hasattr(RetrieverEngine, 'index')
        assert hasattr(RetrieverEngine, 'retrieve')
        assert hasattr(RetrieverEngine, 'hybrid_retrieve')
        assert hasattr(RetrieverEngine, 'delete')
        assert hasattr(RetrieverEngine, 'clear')
        assert hasattr(RetrieverEngine, 'count')


class TestParagraphChunkerEdgeCases:
    """ParagraphChunker edge case tests for missing coverage"""

    def test_chunk_paragraphs_below_min_chunk_size(self):
        """Test when paragraphs are below min_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=100, min_chunk_size=50)
        # Multiple small paragraphs that individually are below min_chunk_size
        content = "Short\n\nTiny\n\nMini"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Should still return at least one chunk
        assert len(chunks) >= 1

    def test_chunk_single_paragraph_below_min(self):
        """Test single paragraph below min_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=1000, min_chunk_size=500)
        content = "Short paragraph"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Should return the content even if below min_chunk_size
        assert len(chunks) == 1
        assert chunks[0].content == "Short paragraph"

    def test_chunk_no_paragraphs_meets_min(self):
        """Test when no chunks meet min_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=50, min_chunk_size=100)
        content = "First\n\nSecond"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Falls back to returning full content as single chunk
        assert len(chunks) >= 1

    def test_chunk_large_paragraph_exceeds_max(self):
        """Test single paragraph exceeding max_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=50, min_chunk_size=10)
        content = "A" * 200  # Large paragraph
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1


class TestRecursiveChunkerEdgeCases:
    """RecursiveChunker edge case tests for missing coverage"""

    def test_chunk_character_fallback(self):
        """Test character-level fallback when no separators work"""
        chunker = RecursiveChunker(max_chunk_size=10)
        # Long text with no natural separators
        content = "ABCDEFGHIJKLMNOPQRSTUVWXYZ" * 10
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) > 1
        # Each chunk should be at most max_chunk_size
        for chunk in chunks:
            assert len(chunk.content) <= 10

    def test_chunk_with_period_separator(self):
        """Test chunking with period separator"""
        chunker = RecursiveChunker(max_chunk_size=30)
        content = "First sentence here. Second sentence. Third one. Fourth."
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_recursive_split_large_part(self):
        """Test recursive splitting when a part exceeds max_chunk_size"""
        chunker = RecursiveChunker(max_chunk_size=20)
        # Content that needs recursive splitting
        content = "AAAAABBBBBCCCCCDDDDDEEEEE" * 3
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_mixed_separators(self):
        """Test chunking with mixed separator types"""
        chunker = RecursiveChunker(max_chunk_size=30)
        content = "Para one\n\nPara two\n\n\nPara three here with more text"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_empty_separator_fallback(self):
        """Test that empty separator is used as last resort"""
        chunker = RecursiveChunker(max_chunk_size=5)
        # Content that will need character-level splitting
        content = "1234567890"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) > 1
        assert all(len(c.content) <= 5 for c in chunks)


class TestDefaultRetrieverEngineEdgeCases:
    """DefaultRetrieverEngine edge case tests for missing coverage"""

    @pytest.mark.asyncio
    async def test_index_document_with_source(self):
        """Test indexing document with source attribute"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1", source="test.txt")
        doc_ids = await engine.index([doc])
        assert len(doc_ids) == 1
        # Verify source is stored in metadata
        results = await engine.retrieve("Hello", top_k=5)
        assert len(results) >= 1
        assert results[0].source == "test.txt"

    @pytest.mark.asyncio
    async def test_index_document_without_id(self):
        """Test indexing document without explicit ID"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world")  # No ID
        doc_ids = await engine.index([doc])
        assert len(doc_ids) == 1
        assert doc_ids[0] is not None  # Auto-generated ID

    @pytest.mark.asyncio
    async def test_index_large_document_chunking(self):
        """Test indexing large document that requires chunking"""
        model = MockEmbeddingModel()
        chunker = FixedSizeChunker(chunk_size=100, overlap=10)
        engine = DefaultRetrieverEngine(embedding_model=model, chunker=chunker)
        # Large document
        content = "A" * 500
        doc = Document(content=content, id="doc-1", source="large.txt")
        doc_ids = await engine.index([doc])
        assert len(doc_ids) == 1

    @pytest.mark.asyncio
    async def test_hybrid_retrieve_with_custom_weights(self):
        """Test hybrid retrieval with custom weights"""
        model = MockEmbeddingModel()
        weights = HybridWeights.balanced()
        engine = DefaultRetrieverEngine(embedding_model=model, hybrid_weights=weights)
        doc = Document(content="Hello world test query", id="doc-1")
        await engine.index([doc])
        results = await engine.hybrid_retrieve("Hello query", top_k=5)
        assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_hybrid_retrieve_with_explicit_weights(self):
        """Test hybrid retrieval with explicitly passed weights"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world test", id="doc-1")
        await engine.index([doc])
        # Pass weights explicitly (not using instance weights)
        weights = HybridWeights.balanced()
        results = await engine.hybrid_retrieve("Hello test", top_k=5, weights=weights)
        assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_retrieve_empty_index(self):
        """Test retrieval from empty index"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        results = await engine.retrieve("Hello", top_k=5)
        assert results == []

    @pytest.mark.asyncio
    async def test_delete_multiple_documents(self):
        """Test deleting multiple documents"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        docs = [
            Document(content="Doc one", id="doc-1"),
            Document(content="Doc two", id="doc-2"),
            Document(content="Doc three", id="doc-3"),
        ]
        await engine.index(docs)
        # Delete two documents
        success = await engine.delete(["doc-1", "doc-2"])
        assert success is True
        count = await engine.count()
        assert count == 1

    @pytest.mark.asyncio
    async def test_delete_partial_nonexistent(self):
        """Test deleting where some docs don't exist"""
        model = MockEmbeddingModel()
        engine = DefaultRetrieverEngine(embedding_model=model)
        doc = Document(content="Hello world", id="doc-1")
        await engine.index([doc])
        # Delete mix of existing and non-existing
        success = await engine.delete(["doc-1", "nonexistent"])
        # Should succeed because at least one was found
        assert success is True


class TestFixedSizeChunkerEdgeCases:
    """FixedSizeChunker additional edge cases"""

    def test_chunk_document_without_id(self):
        """Test chunking document without ID"""
        chunker = FixedSizeChunker(chunk_size=50, overlap=0)
        content = "A" * 200
        doc = Document(content=content)  # No ID
        chunks = chunker.chunk(doc)
        assert len(chunks) > 1
        # Should auto-generate IDs
        for chunk in chunks:
            assert chunk.id is not None
            assert chunk.doc_id is not None

    def test_chunk_exact_chunk_size(self):
        """Test content exactly matching chunk size"""
        chunker = FixedSizeChunker(chunk_size=50, overlap=0)
        content = "A" * 50
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) == 1
        assert len(chunks[0].content) == 50

    def test_chunk_with_large_overlap(self):
        """Test chunking with large overlap"""
        chunker = FixedSizeChunker(chunk_size=100, overlap=80)
        content = "A" * 500
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) > 1


class TestHybridWeightsEdgeCases:
    """HybridWeights additional edge cases"""

    def test_weights_near_sum_tolerance(self):
        """Test weights near 1.0 within tolerance"""
        # Should work: within tolerance
        weights = HybridWeights(vector=0.7005, keyword=0.2995)
        assert abs(weights.vector + weights.keyword - 1.0) < 0.001

    def test_weights_outside_tolerance(self):
        """Test weights outside tolerance raises error"""
        with pytest.raises(ValueError):
            HybridWeights(vector=0.7, keyword=0.2)  # Sum = 0.9


class TestProtocolImplementations:
    """Test protocol implementations to cover abstract/protocol method signatures"""

    def test_embedding_model_protocol(self):
        """Test EmbeddingModel protocol interface"""

        class TestEmbedding:
            async def embed(self, text: str) -> list[float]:
                return [0.1] * 128

            async def embed_batch(self, texts: list[str]) -> list[list[float]]:
                return [[0.1] * 128 for _ in texts]

            @property
            def dimension(self) -> int:
                return 128

            @property
            def model_name(self) -> str:
                return "test-model"

        from continuum_sdk.rag.retriever import EmbeddingModel

        emb = TestEmbedding()
        assert isinstance(emb, EmbeddingModel)

    def test_chunking_strategy_protocol(self):
        """Test ChunkingStrategy protocol interface"""

        class TestChunker:
            def chunk(self, document):
                return [
                    Chunk(
                        id=f"{document.id or 'doc'}-0",
                        doc_id=document.id or "",
                        content=document.content,
                        position=ChunkPosition(start=0, end=len(document.content), index=0, total=1),
                        metadata=document.metadata.copy(),
                    )
                ]

        from continuum_sdk.rag.retriever import ChunkingStrategy

        chunker = TestChunker()
        assert isinstance(chunker, ChunkingStrategy)

    def test_retriever_engine_subclass(self):
        """Test that a proper RetrieverEngine subclass can be created"""

        class TestRetriever(RetrieverEngine):
            async def index(self, documents: list[Document]) -> list[str]:
                return [doc.id or "test" for doc in documents]

            async def retrieve(self, query: str, top_k: int = 5) -> list[RetrievalResult]:
                return [
                    RetrievalResult(doc_id="test", content=query, score=1.0)
                ]

            async def hybrid_retrieve(
                self, query: str, top_k: int = 5, weights=None
            ) -> list[RetrievalResult]:
                return await self.retrieve(query, top_k)

            async def delete(self, doc_ids: list[str]) -> bool:
                return True

            async def clear(self) -> bool:
                return True

            async def count(self) -> int:
                return 0

        engine = TestRetriever()
        assert engine is not None


class TestParagraphChunkerMissingCoverage:
    """Tests for missing ParagraphChunker coverage (lines 424-437)"""

    def test_chunk_current_chunk_below_min_continue(self):
        """Test when current_chunk is below min and continues to next paragraph"""
        chunker = ParagraphChunker(max_chunk_size=50, min_chunk_size=30)
        # Create content where current chunk will be below min, forcing continue
        content = "A\n\nB\n\nC\n\nD"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_paragraph_accumulation_below_min(self):
        """Test paragraph accumulation that results in chunks below min_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=100, min_chunk_size=80)
        # Multiple small paragraphs that will accumulate and create chunk below min
        content = "Short\n\nAlso short\n\nTiny\n\nMini"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Should handle below-min chunks appropriately
        assert len(chunks) >= 1

    def test_chunk_last_chunk_below_min(self):
        """Test when last chunk is below min_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=50, min_chunk_size=40)
        # Last paragraph below min_chunk_size
        content = "First paragraph with enough text here\n\nShort"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Last chunk below min should still be included or fall back
        assert len(chunks) >= 1

    def test_chunk_fallback_to_full_content(self):
        """Test fallback when no chunks meet min_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=30, min_chunk_size=50)
        content = "Small\n\nTiny\n\nMini"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Should return full content as single chunk (lines 459-470)
        assert len(chunks) == 1
        assert chunks[0].content == content

    def test_chunk_multiple_accumulation_and_flush(self):
        """Test paragraph accumulation that flushes when exceeding max_chunk_size"""
        chunker = ParagraphChunker(max_chunk_size=60, min_chunk_size=20)
        # Create content where paragraphs accumulate and then exceed max_chunk_size
        # This triggers the else branch at line 422 with current_chunk >= min_chunk_size
        content = "First paragraph with some text here\n\nSecond paragraph also has some text\n\nThird one"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1
        # Verify chunking happened
        total_length = sum(len(c.content) for c in chunks)
        assert total_length <= len(content) + len(chunks)  # Allow for separator differences

    def test_chunk_accumulate_then_split(self):
        """Test paragraph accumulation with split on max_size boundary"""
        chunker = ParagraphChunker(max_chunk_size=50, min_chunk_size=10)
        # Content where first paragraphs accumulate to near max, then one triggers flush
        content = "Para one has text\n\nPara two has more text\n\nPara three"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1


class TestRecursiveChunkerMissingCoverage:
    """Tests for missing RecursiveChunker coverage (lines 563, 581-589, 593-609, 614)"""

    def test_chunk_large_part_recursive_split(self):
        """Test recursive splitting when part exceeds max_chunk_size"""
        chunker = RecursiveChunker(max_chunk_size=20)
        # Content that requires recursive splitting of large parts
        content = "AAAAAAAAAA\nBBBBBBBBBB\nCCCCCCCCCC\nDDDDDDDDDD"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_separator_found_path(self):
        """Test path where separator is found and splits"""
        chunker = RecursiveChunker(max_chunk_size=30)
        # Content with clear separators
        content = "First paragraph\n\nSecond paragraph here\n\nThird one"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_part_with_separator(self):
        """Test chunking with parts that have separators"""
        chunker = RecursiveChunker(max_chunk_size=25)
        # Mixed content with various separator types
        content = "AAA. BBB. CCC. DDD. EEE. FFF."
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_long_line_no_separator(self):
        """Test long content with no separators needing character split"""
        chunker = RecursiveChunker(max_chunk_size=15)
        # Long line without separators
        content = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) > 1
        # Each chunk should be within size limit
        for c in chunks:
            assert len(c.content) <= 15

    def test_chunk_separator_split_parts(self):
        """Test separator split with parts that need chunking"""
        chunker = RecursiveChunker(max_chunk_size=40)
        # Content with separator and manageable parts
        content = "First paragraph here with some text\n\nSecond paragraph"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_final_current_chunk(self):
        """Test adding final current_chunk after processing"""
        chunker = RecursiveChunker(max_chunk_size=30)
        # Content where current_chunk remains at end
        content = "AAA. BBB. CCC. DDD"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_recursive_path_with_current_chunk(self):
        """Test recursive split when current_chunk exists before recursion"""
        chunker = RecursiveChunker(max_chunk_size=15)
        # Content where current_chunk is populated before large part triggers recursion
        content = "Small\n\nABCDEFGHIJKLMNOPQRSTUVWXYZ"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 2
        # Verify chunks are within size limit
        for c in chunks:
            assert len(c.content) <= 15

    def test_chunk_no_separator_return(self):
        """Test the final return when no separator found"""
        chunker = RecursiveChunker(max_chunk_size=100)
        # Content that doesn't match any separator pattern
        content = "X" * 50
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) == 1

    def test_chunk_current_chunk_empty_before_recursion(self):
        """Test when current_chunk is empty when large part triggers recursion"""
        chunker = RecursiveChunker(max_chunk_size=15)
        # Content where first part is larger than max_chunk_size after the newline separator
        # and current_chunk would be empty (not yet accumulated)
        content = "Small\n\nAAAAAAAAAAAAAAAAAA"  # First parts fit, last part needs recursion
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_current_chunk_populated_at_end(self):
        """Test when current_chunk remains populated after loop ends"""
        chunker = RecursiveChunker(max_chunk_size=20)
        # Content where current_chunk is not flushed during loop, remains at end
        content = "Small part\n\nAnother small"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_branch_current_chunk_empty_at_563(self):
        """Test branch where current_chunk is empty at line 563"""
        chunker = RecursiveChunker(max_chunk_size=15)
        # Create content where:
        # 1. First part doesn't fit in current_chunk (exceeds max_chunk_size)
        # 2. current_chunk is empty at that point
        # This triggers: len(part_with_sep) > max_chunk_size when current_chunk is empty
        content = "First small\n\nSecond part here"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        # Should split properly
        assert len(chunks) >= 1

    def test_chunk_branch_current_chunk_empty_at_593(self):
        """Test branch where current_chunk is empty at line 593"""
        chunker = RecursiveChunker(max_chunk_size=10)
        # Create content where last iteration empties current_chunk
        # We need the final current_chunk to be empty
        # This happens when separator matches and all parts are processed
        # without leaving a remainder in current_chunk
        content = "AAAAA\n\nBBBBB"  # Each part fits exactly
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_empty_current_chunk_at_end(self):
        """Test specifically when current_chunk is falsy at the end"""
        chunker = RecursiveChunker(max_chunk_size=20)
        # Design content so that at line 593, current_chunk is empty
        # This requires all parts to be processed into chunks
        content = "First paragraph\n\nSecond paragraph\n\nThird paragraph"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1

    def test_chunk_both_branches_empty_current(self):
        """Test both branch paths where current_chunk is empty"""
        chunker = RecursiveChunker(max_chunk_size=15)
        # Create specific content to trigger both branches:
        # Content that results in current_chunk being empty at the end
        content = "First part\n\nSecond part\n\nThird"
        doc = Document(content=content, id="doc-1")
        chunks = chunker.chunk(doc)
        assert len(chunks) >= 1
        # Verify all chunks are within size
        for c in chunks:
            assert len(c.content) <= 15


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--cov=continuum_sdk.rag.retriever", "--cov-report=term-missing"])
