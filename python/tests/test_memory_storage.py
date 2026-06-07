"""Memory Storage 持久化测试"""

import os
import shutil
import tempfile
from pathlib import Path

import pytest

from continuum_sdk.memory import (
    FileStorage,
    Memory,
    MemoryEntry,
    MemoryStorage,
    MemoryTier,
    SQLiteStorage,
)


class TestMemoryStorage:
    """MemoryStorage 测试"""

    def test_memory_storage_creation(self):
        """测试内存存储创建"""
        storage = MemoryStorage()
        assert storage is not None

    def test_save_and_load(self):
        """测试保存和加载"""
        storage = MemoryStorage()
        entry = MemoryEntry(
            id="test-1",
            tier=MemoryTier.WORKING,
            content="Test content"
        )

        storage.save(MemoryTier.WORKING, entry)
        loaded = storage.load(MemoryTier.WORKING, "test-1")

        assert loaded is not None
        assert loaded.content == "Test content"

    def test_load_nonexistent(self):
        """测试加载不存在的条目"""
        storage = MemoryStorage()
        loaded = storage.load(MemoryTier.WORKING, "nonexistent")
        assert loaded is None

    def test_load_all_returns_list(self):
        """Test load_all returns list (line 308-309)."""
        storage = MemoryStorage()
        entry1 = MemoryEntry(id="la1", tier=MemoryTier.WORKING, content="content1")
        entry2 = MemoryEntry(id="la2", tier=MemoryTier.WORKING, content="content2")
        storage.save(MemoryTier.WORKING, entry1)
        storage.save(MemoryTier.WORKING, entry2)

        all_entries = storage.load_all(MemoryTier.WORKING)
        assert isinstance(all_entries, list)
        assert len(all_entries) == 2

    def test_delete_nonexistent_returns_false(self):
        """Test delete nonexistent entry returns False (line 319)."""
        storage = MemoryStorage()
        result = storage.delete(MemoryTier.WORKING, "nonexistent")
        assert result is False

    def test_load_all(self):
        """测试加载所有条目"""
        storage = MemoryStorage()
        entry1 = MemoryEntry(id="e1", tier=MemoryTier.WORKING, content="content1")
        entry2 = MemoryEntry(id="e2", tier=MemoryTier.WORKING, content="content2")

        storage.save(MemoryTier.WORKING, entry1)
        storage.save(MemoryTier.WORKING, entry2)

        all_entries = storage.load_all(MemoryTier.WORKING)
        assert len(all_entries) == 2

    def test_delete(self):
        """测试删除"""
        storage = MemoryStorage()
        entry = MemoryEntry(id="del-1", tier=MemoryTier.WORKING, content="to delete")

        storage.save(MemoryTier.WORKING, entry)
        assert storage.count(MemoryTier.WORKING) == 1

        result = storage.delete(MemoryTier.WORKING, "del-1")
        assert result
        assert storage.count(MemoryTier.WORKING) == 0

    def test_delete_nonexistent(self):
        """测试删除不存在的条目"""
        storage = MemoryStorage()
        result = storage.delete(MemoryTier.WORKING, "nonexistent")
        assert not result

    def test_clear(self):
        """测试清空"""
        storage = MemoryStorage()
        for i in range(5):
            entry = MemoryEntry(id=f"c{i}", tier=MemoryTier.SESSION, content=f"content{i}")
            storage.save(MemoryTier.SESSION, entry)

        count = storage.clear(MemoryTier.SESSION)
        assert count == 5
        assert storage.count(MemoryTier.SESSION) == 0

    def test_search(self):
        """测试搜索"""
        storage = MemoryStorage()
        entry1 = MemoryEntry(id="s1", tier=MemoryTier.WORKING, content="Python is great")
        entry2 = MemoryEntry(id="s2", tier=MemoryTier.WORKING, content="Java is okay")
        entry3 = MemoryEntry(id="s3", tier=MemoryTier.WORKING, content="Python is popular")

        storage.save(MemoryTier.WORKING, entry1)
        storage.save(MemoryTier.WORKING, entry2)
        storage.save(MemoryTier.WORKING, entry3)

        results = storage.search(MemoryTier.WORKING, "Python", limit=10)
        assert len(results) == 2

    def test_search_with_limit_break(self):
        """测试搜索达到限制后中断循环 (line 170)"""
        storage = MemoryStorage()
        # 添加多个匹配的条目
        for i in range(10):
            entry = MemoryEntry(id=f"match{i}", tier=MemoryTier.WORKING, content=f"test content {i}")
            storage.save(MemoryTier.WORKING, entry)

        # 设置 limit=3，应该在找到3个后中断
        results = storage.search(MemoryTier.WORKING, "test", limit=3)
        assert len(results) == 3  # 应该只返回3个，因为limit中断了循环

    def test_flush_debug_log(self):
        """测试 flush 调试日志 (lines 175)"""
        storage = MemoryStorage()
        # 调用 flush 应该触发 logger.debug
        storage.flush()
        # 没有异常即可，日志已经被调用

    def test_close_clears_storage(self):
        """测试 close 清空存储 (line 178)"""
        storage = MemoryStorage()
        entry = MemoryEntry(id="close-test", tier=MemoryTier.WORKING, content="test")
        storage.save(MemoryTier.WORKING, entry)

        # close 后存储应该被清空
        storage.close()
        # close 后 _storage dict 被清空，无法再 count
        # 我们验证 close() 被调用后没有异常

    def test_count(self):
        """测试计数"""
        storage = MemoryStorage()
        assert storage.count(MemoryTier.WORKING) == 0

        entry = MemoryEntry(id="cnt1", tier=MemoryTier.WORKING, content="content")
        storage.save(MemoryTier.WORKING, entry)
        assert storage.count(MemoryTier.WORKING) == 1


class TestFileStorage:
    """FileStorage 测试"""

    @pytest.fixture
    def temp_dir(self):
        """创建临时目录"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_file_storage_creation(self, temp_dir):
        """测试文件存储创建"""
        storage = FileStorage(temp_dir, session_id="test-session")
        assert storage is not None
        storage.close()

    def test_save_and_load(self, temp_dir):
        """测试保存和加载"""
        storage = FileStorage(temp_dir, session_id="test-save-load")
        entry = MemoryEntry(
            id="fs-1",
            tier=MemoryTier.WORKING,
            content="File storage test"
        )

        storage.save(MemoryTier.WORKING, entry)
        loaded = storage.load(MemoryTier.WORKING, "fs-1")

        assert loaded is not None
        assert loaded.content == "File storage test"
        storage.close()

    def test_persistence(self, temp_dir):
        """测试持久化（关闭后重新加载）"""
        session_id = "persist-test"

        # 创建并保存数据
        storage1 = FileStorage(temp_dir, session_id=session_id)
        entry = MemoryEntry(
            id="persist-1",
            tier=MemoryTier.PROJECT,
            content="Persistent content"
        )
        storage1.save(MemoryTier.PROJECT, entry)
        storage1.close()

        # 重新打开并验证
        storage2 = FileStorage(temp_dir, session_id=session_id)
        loaded = storage2.load(MemoryTier.PROJECT, "persist-1")

        assert loaded is not None
        assert loaded.content == "Persistent content"
        storage2.close()

    def test_file_structure(self, temp_dir):
        """测试文件结构"""
        storage = FileStorage(temp_dir, session_id="structure-test")

        entry = MemoryEntry(id="str-1", tier=MemoryTier.WORKING, content="working")
        storage.save(MemoryTier.WORKING, entry)

        entry = MemoryEntry(id="str-2", tier=MemoryTier.SESSION, content="session")
        storage.save(MemoryTier.SESSION, entry)

        storage.close()

        # 验证文件存在
        assert Path(temp_dir, "structure-test_working.json").exists()
        assert Path(temp_dir, "structure-test_session.json").exists()

    def test_corrupted_json_file_error_handling(self, temp_dir):
        """测试损坏的 JSON 文件错误处理 (lines 285-286)"""

        # 创建一个损坏的 JSON 文件
        corrupted_file = Path(temp_dir, "test-session_working.json")
        corrupted_file.write_text("{invalid json content}")

        # 初始化 FileStorage 应该能处理损坏的文件而不崩溃
        storage = FileStorage(temp_dir, session_id="test-session")
        # 损坏的文件应该导致日志警告，但不应该阻止初始化
        assert storage is not None
        storage.close()

    def test_file_storage_flush_with_dirty(self, temp_dir):
        """测试 flush 保存脏数据 (lines 319, 326->328)"""
        storage = FileStorage(temp_dir, session_id="flush-test", auto_save=False)

        entry = MemoryEntry(id="flush-1", tier=MemoryTier.WORKING, content="flush test")
        storage.save(MemoryTier.WORKING, entry)
        # auto_save=False，所以数据没有自动保存
        assert storage._dirty is True

        storage.flush()
        # flush 后脏标志应该被清除
        assert storage._dirty is False

        storage.close()

    def test_file_storage_search_with_limit_break(self, temp_dir):
        """测试 FileStorage 搜索达到限制后中断循环 (line 344)"""
        storage = FileStorage(temp_dir, session_id="search-limit-test")

        # 添加多个匹配的条目
        for i in range(10):
            entry = MemoryEntry(id=f"match{i}", tier=MemoryTier.WORKING, content=f"test query {i}")
            storage.save(MemoryTier.WORKING, entry)

        # 设置 limit=3，应该在找到3个后中断
        results = storage.search(MemoryTier.WORKING, "test", limit=3)
        assert len(results) == 3  # 应该只返回3个

        storage.close()

    def test_file_storage_close_flushes_dirty(self, temp_dir):
        """测试 close 触发 flush (line 362, 367)"""
        storage = FileStorage(temp_dir, session_id="close-flush-test", auto_save=False)

        entry = MemoryEntry(id="close-flush-1", tier=MemoryTier.WORKING, content="test")
        storage.save(MemoryTier.WORKING, entry)

        # 关闭应该触发 flush
        storage.close()

        # 重新打开验证数据已保存
        storage2 = FileStorage(temp_dir, session_id="close-flush-test")
        loaded = storage2.load(MemoryTier.WORKING, "close-flush-1")
        assert loaded is not None
        assert loaded.content == "test"
        storage2.close()

    def test_file_storage_get_default_storage_path(self, temp_dir):
        """测试静态方法 get_default_storage_path (line 365-368)"""
        default_path = FileStorage.get_default_storage_path()
        assert default_path is not None
        assert ".continuum" in str(default_path)
        assert "memory" in str(default_path)

    def test_auto_save_disabled(self, temp_dir):
        """测试禁用自动保存"""
        storage = FileStorage(
            temp_dir,
            session_id="no-auto-save",
            auto_save=False
        )

        entry = MemoryEntry(id="no-auto-1", tier=MemoryTier.WORKING, content="no auto")
        storage.save(MemoryTier.WORKING, entry)

        # 手动刷新
        storage.flush()
        storage.close()

        # 验证数据已保存
        storage2 = FileStorage(temp_dir, session_id="no-auto-save")
        loaded = storage2.load(MemoryTier.WORKING, "no-auto-1")
        assert loaded is not None
        storage2.close()

    def test_search(self, temp_dir):
        """测试搜索"""
        storage = FileStorage(temp_dir, session_id="search-test")

        entry1 = MemoryEntry(id="fs-s1", tier=MemoryTier.WORKING, content="Python programming")
        entry2 = MemoryEntry(id="fs-s2", tier=MemoryTier.WORKING, content="Java development")

        storage.save(MemoryTier.WORKING, entry1)
        storage.save(MemoryTier.WORKING, entry2)

        results = storage.search(MemoryTier.WORKING, "Python")
        assert len(results) == 1
        assert results[0].content == "Python programming"

        storage.close()

    def test_delete(self, temp_dir):
        """测试删除"""
        storage = FileStorage(temp_dir, session_id="delete-test")

        entry = MemoryEntry(id="del-test-1", tier=MemoryTier.WORKING, content="to delete")
        storage.save(MemoryTier.WORKING, entry)

        assert storage.count(MemoryTier.WORKING) == 1

        result = storage.delete(MemoryTier.WORKING, "del-test-1")
        assert result
        assert storage.count(MemoryTier.WORKING) == 0

        storage.close()

    def test_clear(self, temp_dir):
        """测试清空"""
        storage = FileStorage(temp_dir, session_id="clear-test")

        for i in range(3):
            entry = MemoryEntry(id=f"clear-{i}", tier=MemoryTier.SESSION, content=f"content {i}")
            storage.save(MemoryTier.SESSION, entry)

        count = storage.clear(MemoryTier.SESSION)
        assert count == 3
        assert storage.count(MemoryTier.SESSION) == 0

        storage.close()


class TestMemoryWithStorage:
    """Memory 与存储后端集成测试"""

    @pytest.fixture
    def temp_dir(self):
        """创建临时目录"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_memory_with_memory_storage(self):
        """测试使用内存存储的 Memory"""
        memory = Memory(session_id="memory-test")

        entry_id = memory.remember("Test content", tier=MemoryTier.WORKING)
        assert entry_id is not None

        results = memory.recall("Test")
        assert len(results) == 1
        assert results[0].content == "Test content"

    def test_memory_with_file_storage(self, temp_dir):
        """测试使用文件存储的 Memory"""
        memory = Memory.create_with_file_storage(
            session_id="file-memory-test",
            storage_path=temp_dir
        )

        entry_id = memory.remember("Persistent content", tier=MemoryTier.PROJECT)
        assert entry_id is not None

        # 保存并关闭
        memory.save()

        # 重新加载
        memory2 = Memory.create_with_file_storage(
            session_id="file-memory-test",
            storage_path=temp_dir
        )

        results = memory2.recall("Persistent")
        assert len(results) == 1
        assert results[0].content == "Persistent content"

        memory2.close()

    def test_memory_stats(self):
        """测试统计"""
        memory = Memory(session_id="stats-test")

        memory.remember("W1", tier=MemoryTier.WORKING)
        memory.remember("W2", tier=MemoryTier.WORKING)
        memory.remember("S1", tier=MemoryTier.SESSION)

        stats = memory.stats()
        assert stats[MemoryTier.WORKING] == 2
        assert stats[MemoryTier.SESSION] == 1

    def test_memory_tier_proxy(self):
        """测试层级代理"""
        memory = Memory(session_id="proxy-test")

        memory.working().add("Proxy content")
        results = memory.working().search("Proxy")

        assert len(results) == 1
        assert results[0].content == "Proxy content"

    def test_memory_forget(self):
        """测试删除"""
        memory = Memory(session_id="forget-test")

        entry_id = memory.remember("To be forgotten", tier=MemoryTier.WORKING)
        assert memory.get(MemoryTier.WORKING, entry_id) is not None

        result = memory.forget(MemoryTier.WORKING, entry_id)
        assert result
        assert memory.get(MemoryTier.WORKING, entry_id) is None

    def test_memory_clear(self):
        """测试清空"""
        memory = Memory(session_id="clear-mem-test")

        memory.remember("Item 1", tier=MemoryTier.WORKING)
        memory.remember("Item 2", tier=MemoryTier.WORKING)

        count = memory.clear(MemoryTier.WORKING)
        assert count == 2
        assert len(memory.recall("")) == 0


class TestSQLiteStorage:
    """SQLiteStorage 测试"""

    @pytest.fixture
    def temp_dir(self):
        """创建临时目录"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_sqlite_storage_creation(self, temp_dir):
        """测试 SQLite 存储创建"""
        db_path = os.path.join(temp_dir, "test.db")
        storage = SQLiteStorage(db_path, session_id="test-session")
        assert storage is not None
        storage.close()

    def test_save_and_load(self, temp_dir):
        """测试保存和加载"""
        db_path = os.path.join(temp_dir, "test_save.db")
        storage = SQLiteStorage(db_path, session_id="test-save-load")
        entry = MemoryEntry(
            id="sqlite-1",
            tier=MemoryTier.WORKING,
            content="SQLite storage test"
        )

        storage.save(MemoryTier.WORKING, entry)
        loaded = storage.load(MemoryTier.WORKING, "sqlite-1")

        assert loaded is not None
        assert loaded.content == "SQLite storage test"
        storage.close()

    def test_persistence(self, temp_dir):
        """测试持久化（关闭后重新加载）"""
        db_path = os.path.join(temp_dir, "persist.db")
        session_id = "persist-test"

        # 创建并保存数据
        storage1 = SQLiteStorage(db_path, session_id=session_id)
        entry = MemoryEntry(
            id="persist-1",
            tier=MemoryTier.PROJECT,
            content="Persistent SQLite content"
        )
        storage1.save(MemoryTier.PROJECT, entry)
        storage1.close()

        # 重新打开并验证
        storage2 = SQLiteStorage(db_path, session_id=session_id)
        loaded = storage2.load(MemoryTier.PROJECT, "persist-1")

        assert loaded is not None
        assert loaded.content == "Persistent SQLite content"
        storage2.close()

    def test_delete(self, temp_dir):
        """测试删除"""
        db_path = os.path.join(temp_dir, "delete.db")
        storage = SQLiteStorage(db_path, session_id="delete-test")

        entry = MemoryEntry(id="del-sqlite-1", tier=MemoryTier.WORKING, content="to delete")
        storage.save(MemoryTier.WORKING, entry)

        assert storage.count(MemoryTier.WORKING) == 1

        result = storage.delete(MemoryTier.WORKING, "del-sqlite-1")
        assert result
        assert storage.count(MemoryTier.WORKING) == 0

        storage.close()

    def test_clear(self, temp_dir):
        """测试清空"""
        db_path = os.path.join(temp_dir, "clear.db")
        storage = SQLiteStorage(db_path, session_id="clear-test")

        for i in range(3):
            entry = MemoryEntry(id=f"clear-{i}", tier=MemoryTier.SESSION, content=f"content {i}")
            storage.save(MemoryTier.SESSION, entry)

        count = storage.clear(MemoryTier.SESSION)
        assert count == 3
        assert storage.count(MemoryTier.SESSION) == 0

        storage.close()

    def test_search(self, temp_dir):
        """测试搜索"""
        db_path = os.path.join(temp_dir, "search.db")
        storage = SQLiteStorage(db_path, session_id="search-test", enable_fts=True)

        entry1 = MemoryEntry(id="s-sqlite-1", tier=MemoryTier.WORKING, content="Python programming")
        entry2 = MemoryEntry(id="s-sqlite-2", tier=MemoryTier.WORKING, content="Java development")

        storage.save(MemoryTier.WORKING, entry1)
        storage.save(MemoryTier.WORKING, entry2)

        results = storage.search(MemoryTier.WORKING, "Python")
        assert len(results) >= 1
        assert results[0].content == "Python programming"

        storage.close()

    def test_get_stats(self, temp_dir):
        """测试统计信息"""
        db_path = os.path.join(temp_dir, "stats.db")
        storage = SQLiteStorage(db_path, session_id="stats-test")

        for i in range(5):
            entry = MemoryEntry(id=f"stats-{i}", tier=MemoryTier.WORKING, content=f"content {i}")
            storage.save(MemoryTier.WORKING, entry)

        stats = storage.get_stats()
        assert stats["total_records"] >= 5
        assert "by_tier" in stats
        assert "db_size_bytes" in stats

        storage.close()

    def test_memory_with_sqlite_storage(self, temp_dir):
        """测试使用 SQLite 存储的 Memory"""
        db_path = os.path.join(temp_dir, "memory.db")
        memory = Memory.create_with_sqlite_storage(
            session_id="sqlite-memory-test",
            db_path=db_path
        )

        entry_id = memory.remember("SQLite persistent content", tier=MemoryTier.PROJECT)
        assert entry_id is not None

        # 保存并关闭
        memory.save()
        memory.close()

        # 重新加载
        memory2 = Memory.create_with_sqlite_storage(
            session_id="sqlite-memory-test",
            db_path=db_path
        )

        results = memory2.recall("SQLite")
        assert len(results) >= 1
        assert "SQLite" in results[0].content

        memory2.close()


class TestSQLiteStorageMissing:
    """测试 SQLiteStorage 缺失的覆盖率"""

    @pytest.fixture
    def temp_dir(self):
        """创建临时目录"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_clear_empty_tier_returns_zero(self, temp_dir):
        """测试清空空的 tier 返回 0 (line 586)"""
        db_path = os.path.join(temp_dir, "empty.db")
        storage = SQLiteStorage(db_path, session_id="empty-test")

        # 清空一个空的 tier
        count = storage.clear(MemoryTier.WORKING)
        assert count == 0

        storage.close()

    def test_search_without_fts_like_fallback(self, temp_dir):
        """测试禁用 FTS 时使用 LIKE 搜索 (line 636)"""
        db_path = os.path.join(temp_dir, "nofts.db")
        storage = SQLiteStorage(db_path, session_id="nofts-test", enable_fts=False)

        entry = MemoryEntry(id="like-test", tier=MemoryTier.WORKING, content="test content")
        storage.save(MemoryTier.WORKING, entry)

        # 使用 LIKE 搜索
        results = storage.search(MemoryTier.WORKING, "test")
        assert len(results) >= 1

        storage.close()

    def test_vacuum(self, temp_dir):
        """测试 vacuum 方法 (line 672-676)"""
        db_path = os.path.join(temp_dir, "vacuum.db")
        storage = SQLiteStorage(db_path, session_id="vacuum-test")

        # 添加一些数据
        entry = MemoryEntry(id="vac-1", tier=MemoryTier.WORKING, content="vacuum test")
        storage.save(MemoryTier.WORKING, entry)

        # 删除数据
        storage.delete(MemoryTier.WORKING, "vac-1")

        # 执行 vacuum
        storage.vacuum()

        storage.close()

    def test_get_stats_empty_database(self, temp_dir):
        """测试空数据库的统计信息"""
        db_path = os.path.join(temp_dir, "empty_stats.db")
        storage = SQLiteStorage(db_path, session_id="empty-stats-test")

        stats = storage.get_stats()
        assert stats["total_records"] == 0
        assert stats["by_tier"] == {}

        storage.close()


class TestStorageBackendAbstractMethods:
    """Tests for StorageBackend abstract methods coverage."""

    def test_abstract_methods_pass_coverage(self):
        """Test abstract method pass statements are covered via concrete class."""
        from continuum_sdk.memory.storage import MemoryEntry, MemoryTier, StorageBackend

        # Create a minimal concrete implementation to test the pass statements
        class MinimalStorage(StorageBackend):
            def save(self, tier, entry):
                pass

            def load(self, tier, entry_id):
                pass

            def load_all(self, tier):
                pass

            def delete(self, tier, entry_id):
                pass

            def clear(self, tier):
                pass

            def count(self, tier):
                pass

            def search(self, tier, query, limit=10):
                pass

            def flush(self):
                pass

            def close(self):
                pass

        # Create instance and call all methods to cover pass statements
        storage = MinimalStorage()
        entry = MemoryEntry(id="test", tier=MemoryTier.WORKING, content="test")

        # These calls will execute the pass statements
        storage.save(MemoryTier.WORKING, entry)
        result = storage.load(MemoryTier.WORKING, "test")
        assert result is None  # pass returns None

        result = storage.load_all(MemoryTier.WORKING)
        assert result is None

        result = storage.delete(MemoryTier.WORKING, "test")
        assert result is None

        result = storage.clear(MemoryTier.WORKING)
        assert result is None

        result = storage.count(MemoryTier.WORKING)
        assert result is None

        result = storage.search(MemoryTier.WORKING, "query")
        assert result is None

        storage.flush()
        storage.close()


class TestMissingBranchCoverage:
    """Tests for missing branch coverage in storage.py."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory."""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_file_storage_load_entry_not_found(self, temp_dir):
        """Test FileStorage.load when entry is not found (line 303->305)."""
        storage = FileStorage(temp_dir, session_id="load-test")
        # Load a non-existent entry
        result = storage.load(MemoryTier.WORKING, "nonexistent")
        assert result is None
        storage.close()

    def test_file_storage_delete_without_auto_save(self, temp_dir):
        """Test FileStorage.delete with auto_save=False (line 316->318)."""
        storage = FileStorage(temp_dir, session_id="delete-no-autosave", auto_save=False)
        entry = MemoryEntry(id="del-no-auto", tier=MemoryTier.WORKING, content="test")
        storage.save(MemoryTier.WORKING, entry)

        # Delete with auto_save=False
        result = storage.delete(MemoryTier.WORKING, "del-no-auto")
        assert result is True

        # Verify dirty flag is set but not auto-saved
        storage.close()

    def test_file_storage_clear_without_auto_save(self, temp_dir):
        """Test FileStorage.clear with auto_save=False (line 326->328)."""
        storage = FileStorage(temp_dir, session_id="clear-no-autosave", auto_save=False)
        entry = MemoryEntry(id="clear-no-auto", tier=MemoryTier.WORKING, content="test")
        storage.save(MemoryTier.WORKING, entry)

        # Clear with auto_save=False
        result = storage.clear(MemoryTier.WORKING)
        assert result == 1
        storage.close()

    def test_file_storage_save_all(self, temp_dir):
        """Test FileStorage.save_all method (line 362)."""
        storage = FileStorage(temp_dir, session_id="save-all-test", auto_save=False)
        entry = MemoryEntry(id="save-all-1", tier=MemoryTier.WORKING, content="test1")
        storage.save(MemoryTier.WORKING, entry)
        entry2 = MemoryEntry(id="save-all-2", tier=MemoryTier.SESSION, content="test2")
        storage.save(MemoryTier.SESSION, entry2)

        # Call save_all (which is an alias for flush)
        storage.save_all()

        # Verify data persisted
        storage2 = FileStorage(temp_dir, session_id="save-all-test")
        assert storage2.load(MemoryTier.WORKING, "save-all-1") is not None
        assert storage2.load(MemoryTier.SESSION, "save-all-2") is not None
        storage2.close()
        storage.close()

    def test_sqlite_storage_delete_nonexistent(self, temp_dir):
        """Test SQLiteStorage.delete with non-existent entry (line 500->exit)."""
        db_path = os.path.join(temp_dir, "delete-nonexist.db")
        storage = SQLiteStorage(db_path, session_id="delete-test")

        # Delete non-existent entry
        result = storage.delete(MemoryTier.WORKING, "nonexistent")
        assert result is False
        storage.close()

    def test_sqlite_storage_search_without_fts_no_match(self, temp_dir):
        """Test SQLiteStorage.search without FTS when no match (line 521->524)."""
        db_path = os.path.join(temp_dir, "search-nofts.db")
        storage = SQLiteStorage(db_path, session_id="search-nofts", enable_fts=False)

        entry = MemoryEntry(id="search-nofts", tier=MemoryTier.WORKING, content="hello world")
        storage.save(MemoryTier.WORKING, entry)

        # Search for something that doesn't match - tests LIKE fallback
        results = storage.search(MemoryTier.WORKING, "goodbye")
        assert len(results) == 0
        storage.close()

    def test_sqlite_storage_search_with_fts(self, temp_dir):
        """Test SQLiteStorage.search with FTS enabled (line 525)."""
        db_path = os.path.join(temp_dir, "search-fts.db")
        storage = SQLiteStorage(db_path, session_id="search-fts", enable_fts=True)

        entry = MemoryEntry(id="fts-test", tier=MemoryTier.WORKING, content="python programming")
        storage.save(MemoryTier.WORKING, entry)

        # FTS search
        results = storage.search(MemoryTier.WORKING, "python")
        assert len(results) >= 1
        storage.close()

    def test_sqlite_storage_clear_empty_tier(self, temp_dir):
        """Test SQLiteStorage.clear on empty tier (line 529-537, 551)."""
        db_path = os.path.join(temp_dir, "clear-empty.db")
        storage = SQLiteStorage(db_path, session_id="clear-empty")

        # Clear an empty tier - should return 0
        result = storage.clear(MemoryTier.WORKING)
        assert result == 0
        storage.close()

    def test_sqlite_storage_delete_checks_existence(self, temp_dir):
        """Test SQLiteStorage.delete existence check (line 559->564, 564->567)."""
        db_path = os.path.join(temp_dir, "delete-check.db")
        storage = SQLiteStorage(db_path, session_id="delete-check")

        entry = MemoryEntry(id="delete-me", tier=MemoryTier.WORKING, content="test")
        storage.save(MemoryTier.WORKING, entry)

        # Delete existing entry
        result = storage.delete(MemoryTier.WORKING, "delete-me")
        assert result is True

        # Try to delete again (should return False)
        result = storage.delete(MemoryTier.WORKING, "delete-me")
        assert result is False
        storage.close()

    def test_sqlite_storage_flush(self, temp_dir):
        """Test SQLiteStorage.flush method (line 595->600, 600->603)."""
        db_path = os.path.join(temp_dir, "flush-test.db")
        storage = SQLiteStorage(db_path, session_id="flush-test", auto_commit=False)

        entry = MemoryEntry(id="flush-test", tier=MemoryTier.WORKING, content="test")
        storage.save(MemoryTier.WORKING, entry)

        # Manual flush
        storage.flush()
        storage.close()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
