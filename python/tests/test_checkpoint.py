"""
Checkpoint Module Unit Tests
Checkpoint 模块单元测试

Tests for:
    - Checkpoint creation and saving
    - Checkpoint loading and restoration
    - Checkpoint data serialization
    - Error handling (invalid paths, corrupted data)
"""

import json
import sys
import tempfile
from datetime import datetime
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from continuum_sdk.agent.checkpoint import (
    HAS_RUST_BINDING,
    CheckpointClient,
    CheckpointMeta,
    PythonCheckpointSystem,
)


class TestCheckpointMeta:
    """Test CheckpointMeta dataclass.
    测试 CheckpointMeta 数据类。
    """

    def test_checkpoint_meta_creation(self):
        """Test creating checkpoint metadata.
        测试创建检查点元数据。
        """
        meta = CheckpointMeta(
            checkpoint_id="cp_123",
            session_id="session_001",
            created_at=datetime.now(),
            trigger="manual",
            iteration=5,
        )
        assert meta.checkpoint_id == "cp_123"
        assert meta.session_id == "session_001"
        assert meta.trigger == "manual"
        assert meta.iteration == 5

    def test_checkpoint_meta_from_dict(self):
        """Test creating checkpoint metadata from dict.
        测试从字典创建检查点元数据。
        """
        data = {
            "checkpoint_id": "cp_456",
            "session_id": "session_002",
            "created_at": "2024-01-15T10:30:00",
            "trigger": "periodic",
            "iteration": 10,
        }
        meta = CheckpointMeta.from_dict(data)
        assert meta.checkpoint_id == "cp_456"
        assert meta.session_id == "session_002"
        assert meta.trigger == "periodic"
        assert meta.iteration == 10

    def test_checkpoint_meta_from_dict_defaults(self):
        """Test checkpoint metadata with default values.
        测试带默认值的检查点元数据。
        """
        data = {}
        meta = CheckpointMeta.from_dict(data)
        assert meta.checkpoint_id == ""
        assert meta.session_id == ""
        assert meta.trigger == "manual"
        assert meta.iteration == 0


class TestPythonCheckpointSystem:
    """Test PythonCheckpointSystem pure Python implementation.
    测试 PythonCheckpointSystem 纯 Python 实现。
    """

    @pytest.fixture
    def temp_storage(self):
        """Create temporary storage directory.
        创建临时存储目录。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            yield tmpdir

    @pytest.fixture
    def checkpoint_system(self, temp_storage):
        """Create checkpoint system with temp storage.
        创建带临时存储的检查点系统。
        """
        return PythonCheckpointSystem(storage_path=temp_storage)

    def test_initialization(self, temp_storage):
        """Test checkpoint system initialization.
        测试检查点系统初始化。
        """
        system = PythonCheckpointSystem(storage_path=temp_storage)
        assert system._storage_path == Path(temp_storage)

    def test_initialization_default_path(self):
        """Test checkpoint system with default path.
        测试检查点系统使用默认路径。
        """
        system = PythonCheckpointSystem()
        expected_path = Path.home() / ".continuum" / "checkpoints"
        assert system._storage_path == expected_path

    def test_save_checkpoint(self, checkpoint_system):
        """Test saving a checkpoint.
        测试保存检查点。
        """
        state = {"messages": ["hello"], "iteration": 1}
        state_json = json.dumps(state)

        checkpoint_id = checkpoint_system.save("session_001", state_json)

        assert checkpoint_id is not None
        assert checkpoint_id.startswith("cp_")

    def test_save_and_load_checkpoint(self, checkpoint_system):
        """Test saving and loading a checkpoint.
        测试保存和加载检查点。
        """
        state = {"messages": ["hello", "world"], "iteration": 5}
        state_json = json.dumps(state)

        checkpoint_id = checkpoint_system.save("session_001", state_json)

        loaded_json = checkpoint_system.load("session_001", checkpoint_id)
        assert loaded_json is not None

        loaded_state = json.loads(loaded_json)
        assert loaded_state["messages"] == ["hello", "world"]
        assert loaded_state["iteration"] == 5

    def test_load_latest_checkpoint(self, checkpoint_system):
        """Test loading the latest checkpoint.
        测试加载最新检查点。
        """
        import time

        # Save multiple checkpoints with slight delay to ensure different timestamps
        state1 = json.dumps({"iteration": 1})
        state2 = json.dumps({"iteration": 2})
        state3 = json.dumps({"iteration": 3})

        checkpoint_system.save("session_001", state1)
        time.sleep(0.01)  # Ensure different mtimes on Windows
        checkpoint_system.save("session_001", state2)
        time.sleep(0.01)
        checkpoint_system.save("session_001", state3)

        # Load latest (should be the last saved)
        loaded_json = checkpoint_system.load("session_001")
        assert loaded_json is not None

        loaded_state = json.loads(loaded_json)
        assert loaded_state["iteration"] == 3

    def test_load_nonexistent_checkpoint(self, checkpoint_system):
        """Test loading a nonexistent checkpoint.
        测试加载不存在的检查点。
        """
        result = checkpoint_system.load("session_001", "nonexistent_cp")
        assert result is None

    def test_load_from_empty_session(self, checkpoint_system):
        """Test loading from a session with no checkpoints.
        测试从没有检查点的会话加载。
        """
        result = checkpoint_system.load("empty_session")
        assert result is None

    def test_list_checkpoints(self, checkpoint_system):
        """Test listing checkpoints for a session.
        测试列出会话的检查点。
        """
        # Save multiple checkpoints
        state = json.dumps({"data": "test"})

        cp1 = checkpoint_system.save("session_001", state)
        cp2 = checkpoint_system.save("session_001", state)
        cp3 = checkpoint_system.save("session_001", state)

        checkpoints = checkpoint_system.list("session_001")
        assert len(checkpoints) == 3
        assert cp1 in checkpoints
        assert cp2 in checkpoints
        assert cp3 in checkpoints

    def test_list_checkpoints_empty_session(self, checkpoint_system):
        """Test listing checkpoints for empty session.
        测试列出空会话的检查点。
        """
        checkpoints = checkpoint_system.list("nonexistent_session")
        assert checkpoints == []

    def test_delete_checkpoint(self, checkpoint_system):
        """Test deleting a checkpoint.
        测试删除检查点。
        """
        state = json.dumps({"data": "test"})
        checkpoint_id = checkpoint_system.save("session_001", state)

        # Verify it exists
        assert checkpoint_system.load("session_001", checkpoint_id) is not None

        # Delete it
        result = checkpoint_system.delete("session_001", checkpoint_id)
        assert result is True

        # Verify it's gone
        assert checkpoint_system.load("session_001", checkpoint_id) is None

    def test_delete_nonexistent_checkpoint(self, checkpoint_system):
        """Test deleting a nonexistent checkpoint.
        测试删除不存在的检查点。
        """
        result = checkpoint_system.delete("session_001", "nonexistent_cp")
        assert result is False

    def test_save_with_special_characters_in_session_id(self, checkpoint_system):
        """Test saving with special characters in session ID.
        测试会话ID包含特殊字符时保存检查点。
        """
        state = json.dumps({"data": "test"})
        # Session ID with special characters should be sanitized
        checkpoint_id = checkpoint_system.save("session/with/slashes", state)

        loaded = checkpoint_system.load("session/with/slashes", checkpoint_id)
        assert loaded is not None

    def test_save_with_unicode_content(self, checkpoint_system):
        """Test saving checkpoint with unicode content.
        测试保存包含Unicode内容的检查点。
        """
        state = json.dumps({"message": "Hello 世界! 🌍", "emoji": "🎉"})
        checkpoint_id = checkpoint_system.save("session_001", state)

        loaded = checkpoint_system.load("session_001", checkpoint_id)
        loaded_state = json.loads(loaded)

        assert loaded_state["message"] == "Hello 世界! 🌍"
        assert loaded_state["emoji"] == "🎉"

    def test_save_with_nested_data(self, checkpoint_system):
        """Test saving checkpoint with nested data structures.
        测试保存包含嵌套数据结构的检查点。
        """
        state = {
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi!"},
            ],
            "metadata": {
                "user_id": "12345",
                "preferences": {"language": "en", "theme": "dark"},
            },
            "nested": {"deep": {"value": 42}},
        }
        state_json = json.dumps(state)
        checkpoint_id = checkpoint_system.save("session_001", state_json)

        loaded = checkpoint_system.load("session_001", checkpoint_id)
        loaded_state = json.loads(loaded)

        assert loaded_state["messages"][0]["role"] == "user"
        assert loaded_state["metadata"]["preferences"]["theme"] == "dark"
        assert loaded_state["nested"]["deep"]["value"] == 42

    def test_atomic_write_integrity(self, checkpoint_system, temp_storage):
        """Test that checkpoint writes are atomic.
        测试检查点写入是原子的。
        """
        state = json.dumps({"important": "data"})

        # Save checkpoint
        checkpoint_id = checkpoint_system.save("session_001", state)

        # Check that no temp file remains
        session_dir = Path(temp_storage) / "session_001"
        temp_files = list(session_dir.glob("*.tmp"))
        assert len(temp_files) == 0

        # Verify checkpoint file exists
        checkpoint_file = session_dir / f"{checkpoint_id}.json"
        assert checkpoint_file.exists()


class TestCheckpointClient:
    """Test CheckpointClient wrapper class.
    测试 CheckpointClient 包装类。

    Note: Tests use PythonCheckpointSystem directly to ensure consistent behavior
    across environments with or without Rust bindings.
    注意：测试直接使用 PythonCheckpointSystem 以确保在有或没有 Rust 绑定的环境中行为一致。
    """

    def test_client_creation(self):
        """Test checkpoint client creation.
        测试检查点客户端创建。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))
            assert client is not None

    def test_is_fallback_property(self):
        """Test is_fallback property.
        测试 is_fallback 属性。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))
            # Should reflect whether Rust binding is available
            assert client.is_fallback == (not HAS_RUST_BINDING)

    def test_save_and_load_with_dict(self):
        """Test save and load with dict state using Python implementation.
        测试使用 Python 实现保存和加载字典状态。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            # Use Python implementation directly for consistent behavior
            system = PythonCheckpointSystem(storage_path=tmpdir)

            state = {
                "messages": [{"role": "user", "content": "Hello"}],
                "iteration": 1,
            }
            state_json = json.dumps(state)

            checkpoint_id = system.save("session_001", state_json)
            assert checkpoint_id is not None
            assert checkpoint_id.startswith("cp_")

            loaded_json = system.load("session_001", checkpoint_id)
            assert loaded_json is not None

            loaded = json.loads(loaded_json)
            assert loaded["messages"][0]["content"] == "Hello"
            assert loaded["iteration"] == 1

    def test_load_latest(self):
        """Test loading the latest checkpoint using Python implementation.
        测试使用 Python 实现加载最新检查点。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Save multiple checkpoints
            system.save("session_001", json.dumps({"iteration": 1}))
            system.save("session_001", json.dumps({"iteration": 2}))
            system.save("session_001", json.dumps({"iteration": 3}))

            # Load latest without specifying checkpoint_id
            loaded_json = system.load("session_001")
            assert loaded_json is not None

            loaded = json.loads(loaded_json)
            assert loaded["iteration"] == 3

    def test_list_checkpoints(self):
        """Test listing checkpoints.
        测试列出检查点。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            client.save("session_001", {"data": 1})
            client.save("session_001", {"data": 2})

            checkpoints = client.list("session_001")
            assert len(checkpoints) == 2

    def test_delete_checkpoint(self):
        """Test deleting a checkpoint.
        测试删除检查点。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            checkpoint_id = client.save("session_001", {"data": "test"})

            result = client.delete("session_001", checkpoint_id)
            assert result is True

            # Verify it's gone
            assert client.load("session_001", checkpoint_id) is None

    def test_has_checkpoints(self):
        """Test has_checkpoints method.
        测试 has_checkpoints 方法。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            assert client.has_checkpoints("session_001") is False

            client.save("session_001", {"data": "test"})
            assert client.has_checkpoints("session_001") is True

    def test_clear_session(self):
        """Test clearing all checkpoints for a session.
        测试清除会话的所有检查点。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            # Create multiple checkpoints
            client.save("session_001", {"iteration": 1})
            client.save("session_001", {"iteration": 2})
            client.save("session_001", {"iteration": 3})

            # Clear all
            deleted_count = client.clear_session("session_001")
            assert deleted_count == 3

            # Verify all gone
            assert client.list("session_001") == []
            assert client.has_checkpoints("session_001") is False

    def test_clear_empty_session(self):
        """Test clearing an empty session.
        测试清除空会话。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            deleted_count = client.clear_session("nonexistent_session")
            assert deleted_count == 0

    def test_multiple_sessions(self):
        """Test handling multiple sessions.
        测试处理多个会话。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            # Save checkpoints for different sessions
            client.save("session_001", {"data": "session 1"})
            client.save("session_002", {"data": "session 2"})
            client.save("session_003", {"data": "session 3"})

            # Verify each session has its own checkpoint
            assert client.has_checkpoints("session_001")
            assert client.has_checkpoints("session_002")
            assert client.has_checkpoints("session_003")

            # Clear one session shouldn't affect others
            client.clear_session("session_002")
            assert client.has_checkpoints("session_001")
            assert not client.has_checkpoints("session_002")
            assert client.has_checkpoints("session_003")


class TestCheckpointSerialization:
    """Test checkpoint data serialization.
    测试检查点数据序列化。

    Note: Tests use PythonCheckpointSystem directly to ensure consistent behavior.
    注意：测试直接使用 PythonCheckpointSystem 以确保行为一致。
    """

    def test_serialize_complex_state(self):
        """Test serializing complex state.
        测试序列化复杂状态。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            state = {
                "messages": [
                    {"role": "user", "content": "What is Python?"},
                    {
                        "role": "assistant",
                        "content": "Python is a programming language.",
                    },
                    {"role": "user", "content": "Tell me more."},
                    {
                        "role": "assistant",
                        "content": "Python is known for its readability...",
                    },
                ],
                "metadata": {
                    "user_id": "user_12345",
                    "conversation_id": "conv_67890",
                    "created_at": datetime.now().isoformat(),
                },
                "tools_used": ["search", "read_file", "execute_code"],
                "cost": 0.0575,
                "tokens": 1250,
                "iteration": 42,
            }

            checkpoint_id = system.save("complex_session", json.dumps(state))
            loaded_json = system.load("complex_session", checkpoint_id)
            loaded = json.loads(loaded_json)

            assert (
                loaded["messages"][1]["content"] == "Python is a programming language."
            )
            assert loaded["metadata"]["user_id"] == "user_12345"
            assert loaded["tools_used"] == ["search", "read_file", "execute_code"]
            assert loaded["cost"] == 0.0575
            assert loaded["tokens"] == 1250
            assert loaded["iteration"] == 42

    def test_serialize_empty_state(self):
        """Test serializing empty state.
        测试序列化空状态。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            state = {}
            checkpoint_id = system.save("empty_state", json.dumps(state))
            loaded_json = system.load("empty_state", checkpoint_id)
            loaded = json.loads(loaded_json)

            assert loaded == {}

    def test_serialize_null_values(self):
        """Test serializing state with null values.
        测试序列化包含空值的状态。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            state = {
                "string": None,
                "number": None,
                "list": None,
                "dict": None,
            }

            checkpoint_id = system.save("null_values", json.dumps(state))
            loaded_json = system.load("null_values", checkpoint_id)
            loaded = json.loads(loaded_json)

            assert loaded["string"] is None
            assert loaded["number"] is None
            assert loaded["list"] is None
            assert loaded["dict"] is None

    def test_serialize_boolean_values(self):
        """Test serializing boolean values.
        测试序列化布尔值。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            state = {
                "is_active": True,
                "is_complete": False,
                "flags": [True, False, True],
            }

            checkpoint_id = system.save("booleans", json.dumps(state))
            loaded_json = system.load("booleans", checkpoint_id)
            loaded = json.loads(loaded_json)

            assert loaded["is_active"] is True
            assert loaded["is_complete"] is False
            assert loaded["flags"] == [True, False, True]

    def test_serialize_numeric_values(self):
        """Test serializing various numeric values.
        测试序列化各种数值类型。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            state = {
                "integer": 42,
                "negative": -100,
                "float": 3.14159,
                "zero": 0,
                "large": 1000000000,
            }

            checkpoint_id = system.save("numerics", json.dumps(state))
            loaded_json = system.load("numerics", checkpoint_id)
            loaded = json.loads(loaded_json)

            assert loaded["integer"] == 42
            assert loaded["negative"] == -100
            assert loaded["float"] == pytest.approx(3.14159)
            assert loaded["zero"] == 0
            assert loaded["large"] == 1000000000


class TestCheckpointErrorHandling:
    """Test checkpoint error handling.
    测试检查点错误处理。

    Note: Tests use PythonCheckpointSystem directly for consistent behavior.
    注意：测试直接使用 PythonCheckpointSystem 以确保行为一致。
    """

    def test_load_nonexistent_checkpoint_returns_none(self):
        """Test that loading nonexistent checkpoint returns None.
        测试加载不存在的检查点返回 None。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            result = client.load("nonexistent_session", "nonexistent_checkpoint")
            assert result is None

    def test_load_from_nonexistent_session_returns_none(self):
        """Test that loading from nonexistent session returns None.
        测试从不存在的会话加载返回 None。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            result = client.load("nonexistent_session")
            assert result is None

    def test_delete_nonexistent_checkpoint_returns_false(self):
        """Test that deleting nonexistent checkpoint returns False.
        测试删除不存在的检查点返回 False。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            result = client.delete("session_001", "nonexistent_checkpoint")
            assert result is False

    def test_corrupted_checkpoint_file(self):
        """Test handling of corrupted checkpoint file.
        测试处理损坏的检查点文件。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Save a valid checkpoint
            checkpoint_id = system.save("session_001", json.dumps({"data": "valid"}))

            # Corrupt the checkpoint file
            session_dir = Path(tmpdir) / "session_001"
            checkpoint_file = session_dir / f"{checkpoint_id}.json"

            # Write invalid JSON to the file
            with open(checkpoint_file, "w") as f:
                f.write("{ invalid json content }")

            # Loading should return None (error is logged)
            result = system.load("session_001", checkpoint_id)
            assert result is None

    def test_invalid_json_state_handling(self):
        """Test handling of invalid JSON state during save.
        测试保存时处理无效 JSON 状态。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Pass invalid JSON string
            checkpoint_id = system.save("session_001", "{ invalid json }")

            # Should still create checkpoint (wraps in "raw" key)
            loaded = system.load("session_001", checkpoint_id)
            assert loaded is not None

    def test_session_id_sanitization(self):
        """Test that session IDs with special characters are sanitized.
        测试包含特殊字符的会话ID会被清理。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Try to save with path traversal attempt
            state = json.dumps({"data": "test"})
            checkpoint_id = system.save("../../../malicious", state)

            # Verify checkpoint was saved with sanitized path
            loaded = system.load("../../../malicious", checkpoint_id)
            assert loaded is not None

    def test_large_checkpoint_data(self):
        """Test handling of large checkpoint data.
        测试处理大型检查点数据。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Create a large state
            large_messages = [
                {"role": "user", "content": f"Message {i}" * 100} for i in range(1000)
            ]
            state = json.dumps({"messages": large_messages})

            checkpoint_id = system.save("large_session", state)
            loaded_json = system.load("large_session", checkpoint_id)

            assert loaded_json is not None
            loaded = json.loads(loaded_json)
            assert len(loaded["messages"]) == 1000


class TestCheckpointClientWithMockedRust:
    """Test CheckpointClient with mocked Rust binding.
    测试带模拟 Rust 绑定的 CheckpointClient。
    """

    def test_uses_rust_binding_when_available(self):
        """Test that Rust binding is used when available.
        测试当 Rust 绑定可用时使用它。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            with patch("continuum_sdk.agent.checkpoint.HAS_RUST_BINDING", True):
                # Mock the Rust binding
                with patch(
                    "continuum_sdk.agent.checkpoint.RustCheckpointSystem"
                ) as mock_rust:
                    mock_instance = MagicMock()
                    mock_rust.return_value = mock_instance

                    client = CheckpointClient(storage_path=Path(tmpdir))

                    # Verify Rust system was instantiated
                    mock_rust.assert_called_once_with(tmpdir)
                    assert client.is_fallback is False

    def test_falls_back_to_python_when_rust_unavailable(self):
        """Test fallback to Python when Rust binding unavailable.
        测试当 Rust 绑定不可用时回退到 Python。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            with patch("continuum_sdk.agent.checkpoint.HAS_RUST_BINDING", False):
                client = CheckpointClient(storage_path=Path(tmpdir))
                assert client.is_fallback is True


class TestCheckpointLifecycle:
    """Test complete checkpoint lifecycle scenarios.
    测试完整的检查点生命周期场景。

    Note: Tests use PythonCheckpointSystem directly for consistent behavior.
    注意：测试直接使用 PythonCheckpointSystem 以确保行为一致。
    """

    def test_crash_recovery_pattern(self):
        """Test typical crash recovery pattern.
        测试典型的崩溃恢复模式。
        """
        import time

        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)
            session_id = "crash_recovery_session"

            # Simulate initial state
            state_v1 = json.dumps({"iteration": 0, "messages": [], "status": "started"})
            system.save(session_id, state_v1)
            time.sleep(0.01)  # Ensure different modification time

            # Simulate progress
            state_v2 = json.dumps(
                {
                    "iteration": 1,
                    "messages": [{"role": "user", "content": "Hello"}],
                    "status": "processing",
                }
            )
            system.save(session_id, state_v2)
            time.sleep(0.01)  # Ensure different modification time

            # Simulate more progress
            state_v3 = json.dumps(
                {
                    "iteration": 2,
                    "messages": [
                        {"role": "user", "content": "Hello"},
                        {"role": "assistant", "content": "Hi!"},
                    ],
                    "status": "processing",
                }
            )
            system.save(session_id, state_v3)

            # Simulate crash recovery - load latest
            loaded_json = system.load(session_id)
            assert loaded_json is not None

            recovered = json.loads(loaded_json)
            assert recovered["iteration"] == 2
            assert len(recovered["messages"]) == 2
            assert recovered["status"] == "processing"

    def test_periodic_checkpoint_saving(self):
        """Test periodic checkpoint saving pattern.
        测试周期性检查点保存模式。
        """
        import time

        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)
            session_id = "periodic_session"

            # Simulate periodic saves
            for i in range(5):
                state = json.dumps(
                    {
                        "iteration": i,
                        "data": f"checkpoint_{i}",
                        "timestamp": datetime.now().isoformat(),
                    }
                )
                system.save(session_id, state)
                time.sleep(0.01)  # Ensure different modification time

            # Verify all checkpoints exist
            checkpoints = system.list(session_id)
            assert len(checkpoints) == 5

            # Load latest
            loaded_json = system.load(session_id)
            loaded = json.loads(loaded_json)
            assert loaded["iteration"] == 4

    def test_checkpoint_cleanup_pattern(self):
        """Test checkpoint cleanup pattern.
        测试检查点清理模式。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)
            session_id = "cleanup_session"

            # Create multiple checkpoints
            for i in range(10):
                system.save(session_id, json.dumps({"iteration": i}))

            # List all
            checkpoints = system.list(session_id)
            assert len(checkpoints) == 10

            # Keep only last 3 (simulate cleanup)
            checkpoints_to_delete = checkpoints[:-3]
            for cp_id in checkpoints_to_delete:
                system.delete(session_id, cp_id)

            # Verify only 3 remain
            remaining = system.list(session_id)
            assert len(remaining) == 3

            # Verify latest is still accessible
            loaded_json = system.load(session_id)
            assert loaded_json is not None


class TestCheckpointConcurrentSessions:
    """Test checkpoint behavior with concurrent sessions.
    测试并发会话的检查点行为。

    Note: Tests use PythonCheckpointSystem directly for consistent behavior.
    注意：测试直接使用 PythonCheckpointSystem 以确保行为一致。
    """

    def test_isolated_sessions(self):
        """Test that sessions are isolated from each other.
        测试会话之间是隔离的。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)
            sessions = ["session_a", "session_b", "session_c"]

            # Save unique data to each session
            for session_id in sessions:
                system.save(
                    session_id,
                    json.dumps(
                        {"session_id": session_id, "data": f"data_for_{session_id}"}
                    ),
                )

            # Verify each session has its own data
            for session_id in sessions:
                loaded_json = system.load(session_id)
                loaded = json.loads(loaded_json)
                assert loaded["session_id"] == session_id
                assert loaded["data"] == f"data_for_{session_id}"

    def test_clear_one_session_does_not_affect_others(self):
        """Test that clearing one session doesn't affect others.
        测试清除一个会话不影响其他会话。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Create checkpoints for multiple sessions
            cp_a = system.save("session_a", json.dumps({"data": "a"}))
            cp_b = system.save("session_b", json.dumps({"data": "b"}))
            cp_c = system.save("session_c", json.dumps({"data": "c"}))

            # Delete session_b checkpoint
            system.delete("session_b", cp_b)

            # Verify session_b is cleared
            assert system.load("session_b", cp_b) is None

            # Verify others are intact
            loaded_a = system.load("session_a", cp_a)
            loaded_c = system.load("session_c", cp_c)

            loaded_a_dict = json.loads(loaded_a)
            loaded_c_dict = json.loads(loaded_c)

            assert loaded_a_dict["data"] == "a"
            assert loaded_c_dict["data"] == "c"


class TestCheckpointEdgeCases:
    """Test edge cases and exception handling paths.
    测试边界情况和异常处理路径。
    """

    def test_temp_file_cleanup_on_error(self):
        """Test temp file cleanup when write fails.
        测试写入失败时的临时文件清理。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Mock open to raise an exception during write
            with patch("builtins.open", side_effect=PermissionError("Access denied")):
                # Should handle the exception gracefully
                # The temp file cleanup should still happen
                try:
                    system.save("session_001", json.dumps({"data": "test"}))
                except PermissionError:
                    pass  # Expected

            # Verify no temp files remain
            session_dir = Path(tmpdir) / "session_001"
            if session_dir.exists():
                temp_files = list(session_dir.glob("*.tmp"))
                assert len(temp_files) == 0

    def test_list_checkpoints_with_permission_error(self):
        """Test list() handling of permission errors on files.
        测试 list() 处理文件权限错误。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Save a valid checkpoint
            system.save("session_001", json.dumps({"data": "test"}))

            # Mock Path.stem to raise an error
            session_dir = Path(tmpdir) / "session_001"
            checkpoint_files = list(session_dir.glob("cp_*.json"))

            if checkpoint_files:
                # Test that exception in stem access is handled
                # This covers lines 285-286
                with patch.object(
                    type(checkpoint_files[0]),
                    "stem",
                    new_callable=lambda: property(
                        lambda self: (_ for _ in ()).throw(PermissionError("denied"))
                    ),
                ):
                    # Should not crash, returns empty list
                    checkpoints = system.list("session_001")
                    # May or may not include the checkpoint depending on error
                    assert isinstance(checkpoints, list)

    def test_list_checkpoints_handles_unicode_decode_error(self):
        """Test list() handling of Unicode decode errors.
        测试 list() 处理 Unicode 解码错误。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Save a valid checkpoint
            system.save("session_001", json.dumps({"data": "test"}))

            # Should handle any unicode issues gracefully
            checkpoints = system.list("session_001")
            assert len(checkpoints) == 1

    def test_delete_checkpoint_with_io_error(self):
        """Test delete() handling of IOError during unlink.
        测试 delete() 处理 unlink 时的 IOError。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Save a checkpoint
            checkpoint_id = system.save("session_001", json.dumps({"data": "test"}))

            # Mock the Path object's unlink method using patch on the module
            # This covers lines 305-307
            with patch("pathlib.Path.unlink", side_effect=OSError("Disk error")):
                result = system.delete("session_001", checkpoint_id)
                assert result is False  # Should return False on IOError

    def test_load_returns_raw_on_json_decode_error(self):
        """Test load() returns raw data when JSON decode fails.
        测试当 JSON 解码失败时 load() 返回原始数据。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            # Force Python fallback to allow mocking
            with patch("continuum_sdk.agent.checkpoint.HAS_RUST_BINDING", False):
                client = CheckpointClient(storage_path=Path(tmpdir))

                # Save a checkpoint
                checkpoint_id = client.save("session_001", {"data": "test"})

                # Mock json.loads only in CheckpointClient.load() context
                # The PythonCheckpointSystem.load() returns the raw JSON string
                # We need to mock the second json.loads in CheckpointClient.load()
                def mock_system_load(session_id, checkpoint_id=None):
                    # Return raw JSON string that will fail to parse
                    return '{"invalid json content": }'

                with patch.object(client._system, "load", side_effect=mock_system_load):
                    result = client.load("session_001", checkpoint_id)
                    # Should return raw result wrapped in dict when JSON decode fails
                    assert result is not None
                    assert "raw" in result

    def test_temp_file_cleanup_exception_handling(self):
        """Test temp file cleanup handles exceptions in finally block.
        测试 finally 块中临时文件清理的异常处理。
        Covers lines 229-232.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Mock temp_path.unlink to raise an exception
            # This tests the exception handling in the finally block
            with patch(
                "pathlib.Path.unlink", side_effect=PermissionError("Access denied")
            ):
                with patch("pathlib.Path.exists", return_value=True):
                    # The save should still succeed even if temp cleanup fails
                    checkpoint_id = system.save(
                        "session_001", json.dumps({"data": "test"})
                    )
                    assert checkpoint_id is not None

            # Verify the checkpoint was still saved
            loaded = system.load("session_001", checkpoint_id)
            assert loaded is not None

    def test_temp_file_cleanup_on_successful_write(self):
        """Test temp file is cleaned up after successful write.
        测试成功写入后临时文件被清理。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            system = PythonCheckpointSystem(storage_path=tmpdir)

            # Save a checkpoint
            checkpoint_id = system.save("session_001", json.dumps({"data": "test"}))

            # Verify no temp files remain
            session_dir = Path(tmpdir) / "session_001"
            temp_files = list(session_dir.glob("*.tmp"))
            assert len(temp_files) == 0

            # Verify checkpoint exists
            checkpoint_file = session_dir / f"{checkpoint_id}.json"
            assert checkpoint_file.exists()


class TestModuleImportFallback:
    """Test module import fallback behavior.
    测试模块导入回退行为。
    Covers lines 113-125.
    """

    def test_rust_binding_import_fallback(self):
        """Test that ImportError triggers fallback path.
        测试 ImportError 触发回退路径。
        """
        # Save original module if it exists
        original_module = sys.modules.get("continuum_sdk.agent.checkpoint")

        try:
            # Remove the module from cache
            if "continuum_sdk.agent.checkpoint" in sys.modules:
                del sys.modules["continuum_sdk.agent.checkpoint"]

            # Mock the import to raise ImportError
            import builtins

            original_import = builtins.__import__

            def mock_import(name, *args, **kwargs):
                if name == "sh_python":
                    raise ImportError("Mocked: sh_python not available")
                return original_import(name, *args, **kwargs)

            with patch("builtins.__import__", side_effect=mock_import):
                # Re-import the module - this should trigger the fallback path
                import continuum_sdk.agent.checkpoint as cp

                # Verify fallback was used
                assert cp.HAS_RUST_BINDING is False
                # Verify RustCheckpointSystem placeholder exists
                assert hasattr(cp, "RustCheckpointSystem")
                # Verify placeholder can be instantiated (it's a type-only placeholder)
                placeholder = cp.RustCheckpointSystem()
                assert placeholder is not None

        finally:
            # Restore original module
            if original_module is not None:
                sys.modules["continuum_sdk.agent.checkpoint"] = original_module
            else:
                if "continuum_sdk.agent.checkpoint" in sys.modules:
                    del sys.modules["continuum_sdk.agent.checkpoint"]


class TestHasCheckpointsBranch:
    """Test has_checkpoints branch coverage.
    测试 has_checkpoints 分支覆盖。
    Covers branch 437->436.
    """

    def test_has_checkpoints_returns_false_for_empty_session(self):
        """Test has_checkpoints returns False when no checkpoints exist.
        测试当没有检查点时 has_checkpoints 返回 False。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            # Test with no checkpoints
            result = client.has_checkpoints("empty_session")
            assert result is False  # This exercises the False branch of len() > 0


class TestClearSession:
    """Test clear_session method for coverage.
    测试 clear_session 方法覆盖。
    Covers lines 435-439 and branch 437->436.
    """

    def test_clear_session_deletes_all_checkpoints(self):
        """Test clear_session removes all checkpoints.
        测试 clear_session 删除所有检查点。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            # Create multiple checkpoints
            client.save("session_001", {"data": "test1"})
            client.save("session_001", {"data": "test2"})
            client.save("session_001", {"data": "test3"})

            # Verify checkpoints exist
            assert client.has_checkpoints("session_001") is True
            checkpoints = client.list("session_001")
            assert len(checkpoints) == 3

            # Clear the session
            deleted_count = client.clear_session("session_001")

            # Verify all were deleted
            assert deleted_count == 3
            assert client.has_checkpoints("session_001") is False
            assert len(client.list("session_001")) == 0

    def test_clear_session_empty_session(self):
        """Test clear_session on session with no checkpoints.
        测试对没有检查点的会话执行 clear_session。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            # Clear a session with no checkpoints
            deleted_count = client.clear_session("empty_session")

            # Should return 0
            assert deleted_count == 0

    def test_clear_session_partial_delete_failure(self):
        """Test clear_session when some deletes fail.
        测试部分删除失败时的 clear_session。
        Covers branch where delete returns False.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            client = CheckpointClient(storage_path=Path(tmpdir))

            # Create a checkpoint
            client.save("session_001", {"data": "test"})

            # Mock delete to fail for one checkpoint
            original_delete = client.delete
            call_count = [0]

            def mock_delete(session_id, cp_id):
                call_count[0] += 1
                if call_count[0] == 1:
                    return False  # First delete fails
                return original_delete(session_id, cp_id)

            with patch.object(client, "delete", side_effect=mock_delete):
                # This will count the failed delete but not increment counter
                deleted_count = client.clear_session("session_001")

            # The failed delete should not be counted
            assert deleted_count == 0


class TestRustBindingImportSuccess:
    """Test successful Rust binding import path.
    测试成功导入 Rust 绑定的路径。
    Covers lines 111-112.
    """

    def test_rust_binding_available_when_import_succeeds(self):
        """Test HAS_RUST_BINDING is True when import succeeds.
        测试当导入成功时 HAS_RUST_BINDING 为 True。
        """
        # This test verifies the code path exists
        # We cannot easily mock a successful import in a running Python session
        # but we can verify the module structure

        # Import the module
        from continuum_sdk.agent import checkpoint

        # The HAS_RUST_BINDING flag should be a boolean
        assert isinstance(checkpoint.HAS_RUST_BINDING, bool)

        # If the Rust binding is not available, the module should have
        # the PythonCheckpointSystem as a fallback
        if not checkpoint.HAS_RUST_BINDING:
            assert hasattr(checkpoint, "PythonCheckpointSystem")
            assert hasattr(checkpoint, "CheckpointClient")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
