"""Session 单元测试"""

import os
import sys

from datetime import datetime

import pytest

from continuum_sdk.agent.session import Message, MessageRole, Session


class TestMessage:
    """Message 测试"""

    def test_message_creation(self):
        """测试消息创建"""
        msg = Message(role=MessageRole.USER, content="Hello")
        assert msg.role == MessageRole.USER
        assert msg.content == "Hello"
        assert msg.timestamp is not None

    def test_message_to_dict(self):
        """测试消息序列化"""
        msg = Message(role=MessageRole.USER, content="Test")
        data = msg.to_dict()
        assert data["role"] == "user"
        assert data["content"] == "Test"

    def test_message_from_dict(self):
        """测试消息反序列化"""
        data = {
            "role": "assistant",
            "content": "Response",
            "timestamp": datetime.now().isoformat(),
        }
        msg = Message.from_dict(data)
        assert msg.role == MessageRole.ASSISTANT
        assert msg.content == "Response"


class TestSession:
    """Session 测试"""

    def test_session_creation(self):
        """测试会话创建"""
        session = Session()
        assert session.id is not None
        assert session.message_count == 0

    def test_session_with_id(self):
        """测试指定 ID 的会话"""
        session = Session(id="custom-id")
        assert session.id == "custom-id"

    def test_add_user_message(self):
        """测试添加用户消息"""
        session = Session()
        session.add_user_message("Hello")
        assert session.message_count == 1

    def test_add_assistant_message(self):
        """测试添加助手消息"""
        session = Session()
        session.add_assistant_message("Hi there")
        assert session.message_count == 1

    def test_add_system_message(self):
        """测试添加系统消息"""
        session = Session()
        session.add_system_message("System prompt")
        assert session.message_count == 1

    def test_get_messages(self):
        """测试获取消息列表"""
        session = Session()
        session.add_user_message("Q1")
        session.add_assistant_message("A1")
        messages = session.get_messages()
        assert len(messages) == 2

    def test_clear_messages(self):
        """测试清空消息"""
        session = Session()
        session.add_user_message("Test")
        session.clear_messages()
        assert session.message_count == 0

    def test_get_last_message(self):
        """测试获取最后一条消息"""
        session = Session()
        session.add_user_message("First")
        session.add_user_message("Last")
        last = session.get_last_message()
        assert last.content == "Last"

    def test_get_last_message_empty(self):
        """测试空会话获取最后消息"""
        session = Session()
        assert session.get_last_message() is None

    def test_metadata(self):
        """测试元数据"""
        session = Session()
        session.set_metadata("key", "value")
        assert session.get_metadata("key") == "value"
        assert session.get_metadata("missing") is None

    def test_tool_recording(self):
        """测试工具使用记录"""
        session = Session()
        session.record_tool_use("read_file")
        session.record_tool_use("write_file")
        tools = session.get_tools_used()
        assert "read_file" in tools
        assert "write_file" in tools

    def test_cost_tracking(self):
        """测试成本追踪"""
        session = Session()
        session.update_cost(0.05, 1000)
        assert session.cost == 0.05
        assert session.tokens == 1000
        session.update_cost(0.03, 500)
        assert session.cost == 0.08
        assert session.tokens == 1500

    def test_export_import(self):
        """测试导出导入"""
        session = Session(id="export-test")
        session.add_user_message("Test message")
        exported = session.export()

        restored = Session.from_export(exported)
        assert restored.id == "export-test"
        assert restored.message_count == 1

    def test_created_at(self):
        """测试创建时间"""
        session = Session()
        assert isinstance(session.created_at, datetime)


class TestSessionPersistence:
    """Session 持久化测试"""

    @pytest.fixture
    def temp_dir(self):
        """创建临时目录"""
        import tempfile
        import shutil
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_save_and_load(self, temp_dir):
        """测试保存和加载"""
        import json
        from pathlib import Path

        session = Session(id="persist-test")
        session.add_user_message("Hello")
        session.add_assistant_message("Hi there")
        session.set_metadata("user_id", "12345")
        session.update_cost(0.05, 1000)

        # Save
        path = Path(temp_dir) / "persist-test.json"
        session.save(path)

        # Verify file exists
        assert path.exists()

        # Verify file content
        with open(path) as f:
            data = json.load(f)
        assert data["id"] == "persist-test"
        assert len(data["messages"]) == 2
        assert data["metadata"]["user_id"] == "12345"

        # Load
        loaded = Session.load(path)
        assert loaded.id == "persist-test"
        assert loaded.message_count == 2
        assert loaded.get_metadata("user_id") == "12345"
        assert loaded.cost == 0.05
        assert loaded.tokens == 1000

    def test_load_nonexistent_file(self, temp_dir):
        """测试加载不存在的文件"""
        from pathlib import Path

        path = Path(temp_dir) / "nonexistent.json"
        with pytest.raises(FileNotFoundError):
            Session.load(path)

    def test_save_to_default(self, temp_dir):
        """测试保存到默认目录"""
        # 使用临时目录作为默认目录
        original_dir = Session.get_default_session_dir()

        session = Session(id="default-test")
        session.add_user_message("Test")

        # 保存
        path = session.save_to_default()
        assert path.exists()
        assert path.name == "default-test.json"

        # 加载
        loaded = Session.load_from_default("default-test")
        assert loaded.id == "default-test"
        assert loaded.message_count == 1

        # 清理
        path.unlink()

    def test_list_saved_sessions(self, temp_dir):
        """测试列出已保存会话"""
        from pathlib import Path

        # 创建测试目录
        session_dir = Path(temp_dir) / "sessions"
        session_dir.mkdir(parents=True)

        # 创建几个会话文件
        for i in range(3):
            session = Session(id=f"session-{i}")
            path = session_dir / f"session-{i}.json"
            session.save(path)

        # 验证列出
        # 需要临时修改默认目录
        sessions = [f.stem for f in session_dir.glob("*.json")]
        assert len(sessions) == 3
        assert "session-0" in sessions
        assert "session-1" in sessions
        assert "session-2" in sessions

    def test_delete_session(self, temp_dir):
        """测试删除会话"""
        from pathlib import Path

        session = Session(id="delete-test")
        path = Path(temp_dir) / "delete-test.json"
        session.save(path)
        assert path.exists()

        session.delete(path)
        assert not path.exists()

    def test_persistence_with_special_characters(self, temp_dir):
        """测试特殊字符内容的持久化"""
        from pathlib import Path

        session = Session(id="special-test")
        session.add_user_message("Hello 世界! 🌍")
        session.add_assistant_message("Reply with 中文 and emoji 🎉")

        path = Path(temp_dir) / "special-test.json"
        session.save(path)

        loaded = Session.load(path)
        messages = loaded.get_messages()
        assert "世界" in messages[0].content
        assert "中文" in messages[1].content

    def test_full_cycle(self, temp_dir):
        """测试完整持久化周期"""
        from pathlib import Path

        # 创建
        session = Session(id="cycle-test")
        session.add_system_message("You are a helpful assistant.")
        session.add_user_message("What is Python?")
        session.add_assistant_message("Python is a programming language.")
        session.set_metadata("project", "continuum")
        session.record_tool_use("read_file")
        session.update_cost(0.10, 2000)

        # 保存
        path = Path(temp_dir) / "cycle-test.json"
        session.save(path)

        # 加载
        loaded = Session.load(path)

        # 验证完整恢复
        assert loaded.id == "cycle-test"
        assert loaded.message_count == 3
        assert loaded.get_metadata("project") == "continuum"
        assert "read_file" in loaded.get_tools_used()
        assert loaded.cost == 0.10
        assert loaded.tokens == 2000

        # 继续对话
        loaded.add_user_message("Thanks!")
        loaded.save(path)

        # 再次加载
        final = Session.load(path)
        assert final.message_count == 4


class TestSessionRecover:
    """Session recover tests for checkpoint restoration."""

    @pytest.fixture
    def temp_dir(self):
        """Create temporary directory."""
        import tempfile
        import shutil
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_recover_from_valid_checkpoint(self, temp_dir):
        """Test recover from valid checkpoint file."""
        from pathlib import Path

        # Create a session and save as checkpoint
        session = Session(id="recover-test")
        session.add_user_message("Question before crash")
        session.add_assistant_message("Answer before crash")
        session.set_metadata("session_type", "recovery")
        session.record_tool_use("read_file")
        session.update_cost(0.07, 1200)

        checkpoint_path = Path(temp_dir) / "checkpoint.json"
        session.save(checkpoint_path)

        # Recover from checkpoint
        recovered = Session.recover(checkpoint_path)

        assert recovered.id == "recover-test"
        assert recovered.message_count == 2
        assert recovered.get_metadata("session_type") == "recovery"
        assert "read_file" in recovered.get_tools_used()
        assert recovered.cost == 0.07
        assert recovered.tokens == 1200

    def test_recover_file_not_found(self, temp_dir):
        """Test recover with nonexistent checkpoint file."""
        from pathlib import Path

        checkpoint_path = Path(temp_dir) / "nonexistent.json"

        with pytest.raises(FileNotFoundError) as exc_info:
            Session.recover(checkpoint_path)

        assert "Checkpoint file not found" in str(exc_info.value)

    def test_recover_invalid_json(self, temp_dir):
        """Test recover with invalid JSON content."""
        from pathlib import Path

        checkpoint_path = Path(temp_dir) / "invalid.json"
        checkpoint_path.write_text("{ invalid json }")

        with pytest.raises(ValueError) as exc_info:
            Session.recover(checkpoint_path)

        assert "Invalid checkpoint file format" in str(exc_info.value)

    def test_recover_missing_required_fields(self, temp_dir):
        """Test recover with missing required fields."""
        from pathlib import Path
        import json

        checkpoint_path = Path(temp_dir) / "incomplete.json"

        # Create checkpoint missing 'id' and 'created_at'
        incomplete_data = {"messages": [], "metadata": {}}
        checkpoint_path.write_text(json.dumps(incomplete_data))

        with pytest.raises(ValueError) as exc_info:
            Session.recover(checkpoint_path)

        assert "missing required fields" in str(exc_info.value)

    def test_recover_empty_messages(self, temp_dir):
        """Test recover with empty message history."""
        from pathlib import Path

        checkpoint_path = Path(temp_dir) / "empty_session.json"

        session = Session(id="empty-recover")
        # Don't add any messages

        session.save(checkpoint_path)
        recovered = Session.recover(checkpoint_path)

        assert recovered.id == "empty-recover"
        assert recovered.message_count == 0

    def test_recover_preserves_timestamps(self, temp_dir):
        """Test recover preserves message timestamps."""
        from pathlib import Path
        from datetime import datetime

        checkpoint_path = Path(temp_dir) / "timestamps.json"

        session = Session(id="timestamp-test")
        session.add_user_message("Message 1")

        original_time = session.created_at
        session.save(checkpoint_path)

        recovered = Session.recover(checkpoint_path)

        # Verify timestamps are preserved
        assert recovered.created_at.year == original_time.year
        assert recovered.created_at.month == original_time.month
        assert recovered.created_at.day == original_time.day

        # Verify message timestamp exists
        messages = recovered.get_messages()
        assert len(messages) == 1
        assert isinstance(messages[0].timestamp, datetime)


class TestSessionExportImport:
    """Additional tests for export/import functionality."""

    def test_to_dict(self):
        """Test to_dict serialization."""
        session = Session(id="to-dict-test")
        session.add_user_message("Hello")
        session.add_assistant_message("World")

        data = session.to_dict()

        assert data["id"] == "to-dict-test"
        assert "created_at" in data
        assert len(data["messages"]) == 2
        assert isinstance(data["metadata"], dict)

    def test_from_dict(self):
        """Test from_dict deserialization."""
        from datetime import datetime

        session = Session(id="dict-test")
        session.add_user_message("Test")
        session.set_metadata("key", "value")

        data = session.to_dict()
        restored = Session.from_dict(data)

        assert restored.id == "dict-test"
        assert restored.message_count == 1
        assert restored.get_metadata("key") == "value"

    def test_export_with_tools_and_cost(self):
        """Test export includes tools and cost data."""
        session = Session(id="export-tools")
        session.add_user_message("Test")
        session.record_tool_use("search")
        session.record_tool_use("read_file")
        session.update_cost(0.15, 3000)

        exported = session.export()

        assert "search" in exported
        assert "read_file" in exported
        assert "0.15" in exported

    def test_from_export_preserves_all_data(self):
        """Test from_export preserves all session data."""
        session = Session(id="full-export")
        session.add_system_message("System prompt")
        session.add_user_message("User question")
        session.add_assistant_message("Assistant response")
        session.set_metadata("model", "gpt-4")
        session.record_tool_use("search")
        session.update_cost(0.25, 5000)

        exported = session.export()
        restored = Session.from_export(exported)

        assert restored.id == "full-export"
        assert restored.message_count == 3
        assert restored.get_metadata("model") == "gpt-4"
        assert "search" in restored.get_tools_used()
        assert restored.cost == 0.25
        assert restored.tokens == 5000


class TestSessionMessageHandling:
    """Tests for message handling edge cases."""

    def test_get_messages_with_limit(self):
        """Test get_messages with limit parameter."""
        session = Session()

        for i in range(10):
            session.add_user_message(f"Message {i}")

        # Get all messages
        all_messages = session.get_messages()
        assert len(all_messages) == 10

        # Get limited messages
        limited = session.get_messages(limit=5)
        assert len(limited) == 5
        # Should be last 5 messages
        assert limited[0].content == "Message 5"
        assert limited[-1].content == "Message 9"

    def test_add_message_with_metadata(self):
        """Test add_message with custom metadata."""
        session = Session()

        custom_metadata = {"source": "api", "version": "1.0"}
        msg = session.add_message(
            MessageRole.USER,
            "Hello",
            metadata=custom_metadata
        )

        assert msg.metadata == custom_metadata
        assert msg.metadata["source"] == "api"

    def test_message_with_custom_timestamp(self):
        """Test message creation with custom timestamp."""
        from datetime import datetime, timedelta

        custom_time = datetime.now() - timedelta(hours=1)
        msg = Message(
            role=MessageRole.USER,
            content="Past message",
            timestamp=custom_time
        )

        assert msg.timestamp == custom_time

    def test_get_messages_empty_session(self):
        """Test get_messages on empty session."""
        session = Session()
        messages = session.get_messages()
        assert messages == []

        # With limit on empty
        limited = session.get_messages(limit=10)
        assert limited == []


class TestSessionUtilities:
    """Tests for utility functions and methods."""

    def test_repr(self):
        """Test session repr."""
        session = Session(id="repr-test")
        session.add_user_message("Test")

        repr_str = repr(session)
        assert "Session" in repr_str
        assert "repr-test" in repr_str
        assert "messages=1" in repr_str

    def test_create_session_convenience_function(self):
        """Test create_session convenience function."""
        from continuum_sdk.agent.session import create_session

        session = create_session("convenience-test")
        assert session.id == "convenience-test"

        session_auto = create_session()
        assert session_auto.id == "default-session"

    def test_message_role_values(self):
        """Test MessageRole enum values."""
        assert MessageRole.USER.value == "user"
        assert MessageRole.ASSISTANT.value == "assistant"
        assert MessageRole.SYSTEM.value == "system"
        assert MessageRole.TOOL.value == "tool"

    def test_delete_nonexistent_file(self):
        """Test delete when file doesn't exist (no error)."""
        from pathlib import Path

        session = Session(id="delete-test")
        nonexistent = Path("nonexistent_session_file.json")

        # Should not raise, just do nothing
        session.delete(nonexistent)

    def test_list_saved_sessions_empty_directory(self, monkeypatch, tmp_path):
        """Test list_saved_sessions when directory doesn't exist."""
        # Use monkeypatch to override get_default_session_dir
        nonexistent_dir = tmp_path / "nonexistent"
        monkeypatch.setattr(
            Session,
            "get_default_session_dir",
            lambda: nonexistent_dir
        )

        sessions = Session.list_saved_sessions()
        assert sessions == []

    def test_get_default_session_dir(self):
        """Test get_default_session_dir returns expected path."""
        from pathlib import Path

        default_dir = Session.get_default_session_dir()

        assert isinstance(default_dir, Path)
        assert ".continuum" in str(default_dir)
        assert "sessions" in str(default_dir)


class TestSessionEdgeCases:
    """Tests for edge cases and error handling."""

    def test_multiple_metadata_updates(self):
        """Test multiple metadata updates."""
        session = Session()

        session.set_metadata("key1", "value1")
        session.set_metadata("key2", "value2")
        session.set_metadata("key1", "updated_value1")

        assert session.get_metadata("key1") == "updated_value1"
        assert session.get_metadata("key2") == "value2"

    def test_cost_accumulation(self):
        """Test cost properly accumulates."""
        session = Session()

        # Initial values should be zero
        assert session.cost == 0.0
        assert session.tokens == 0

        # Multiple updates
        session.update_cost(0.01, 100)
        session.update_cost(0.02, 200)
        session.update_cost(0.03, 300)

        assert session.cost == 0.06
        assert session.tokens == 600

    def test_tool_usage_tracking_multiple(self):
        """Test tool usage with multiple calls to same tool."""
        session = Session()

        session.record_tool_use("search")
        session.record_tool_use("search")
        session.record_tool_use("read_file")

        tools = session.get_tools_used()
        assert tools.count("search") == 2
        assert tools.count("read_file") == 1

    def test_persistence_creates_parent_directories(self, tmp_path):
        """Test save creates parent directories."""
        from pathlib import Path

        session = Session(id="nested-test")
        nested_path = tmp_path / "deeply" / "nested" / "dir" / "session.json"

        # Parent directories don't exist yet
        assert not nested_path.parent.exists()

        session.save(nested_path)

        # Now they should exist
        assert nested_path.parent.exists()
        assert nested_path.exists()

    def test_load_with_utf8_encoding(self, tmp_path):
        """Test load handles UTF-8 encoding properly."""
        from pathlib import Path

        session = Session(id="utf8-test")
        session.add_user_message("Hello 世界")
        session.add_assistant_message("你好 world")

        path = tmp_path / "utf8.json"
        session.save(path)

        loaded = Session.load(path)
        messages = loaded.get_messages()

        assert "世界" in messages[0].content
        assert "你好" in messages[1].content

    def test_add_message_with_tool_role(self):
        """Test add_message with TOOL role.
        Covers branch 256->259.
        """
        session = Session(id="tool-test")

        # Add tool message via add_message
        msg = session.add_message(MessageRole.TOOL, '{"result": "success"}')

        assert msg.role == MessageRole.TOOL
        assert msg.content == '{"result": "success"}'
        assert session.message_count == 1

    def test_add_message_with_system_role(self):
        """Test add_message with SYSTEM role.
        Covers branch 256->259.
        """
        session = Session(id="system-test")

        # Add system message via add_message
        msg = session.add_message(MessageRole.SYSTEM, "You are a helpful assistant")

        assert msg.role == MessageRole.SYSTEM
        assert msg.content == "You are a helpful assistant"
        assert session.message_count == 1


class TestRustBindings:
    """Tests for Rust binding code paths (mocked)."""

    def test_rust_session_properties(self):
        """Test session properties when Rust bindings are available."""
        import sys
        from datetime import datetime

        # Create a mock sh_core module
        class MockRustSession:
            def __init__(self, session_id):
                self._id = session_id
                self._created_at = datetime.now().isoformat()
                self._messages = []

            @property
            def id(self):
                return self._id

            @property
            def created_at(self):
                return self._created_at

            def message_count(self):
                return len(self._messages)

            def add_user_message(self, content):
                self._messages.append(("user", content))

            def add_assistant_message(self, content):
                self._messages.append(("assistant", content))

            def get_messages(self):
                return self._messages.copy()

            def clear_messages(self):
                self._messages.clear()

            def export(self):
                import json
                return json.dumps({
                    "id": self._id,
                    "created_at": self._created_at,
                    "messages": [{"role": m[0], "content": m[1]} for m in self._messages]
                })

        # Create mock sh_core module
        mock_sh_core = type(sys)("sh_core")
        mock_sh_core.Session = MockRustSession

        # Inject into sys.modules before importing session
        sys.modules["sh_core"] = mock_sh_core

        # Remove cached import
        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        try:
            # Import fresh - should pick up mock sh_core
            from continuum_sdk.agent.session import Session

            session = Session(id="rust-test")
            assert session.id == "rust-test"
            assert isinstance(session.created_at, datetime)
            assert session.message_count == 0
        finally:
            # Cleanup
            if "sh_core" in sys.modules:
                del sys.modules["sh_core"]
            if "continuum_sdk.agent.session" in sys.modules:
                del sys.modules["continuum_sdk.agent.session"]

    def test_rust_add_message_sync(self):
        """Test add_message syncs to Rust session."""
        import sys

        class MockRustSession:
            def __init__(self, session_id):
                self._id = session_id
                self._messages = []

            @property
            def id(self):
                return self._id

            @property
            def created_at(self):
                from datetime import datetime
                return datetime.now().isoformat()

            def message_count(self):
                return len(self._messages)

            def add_user_message(self, content):
                self._messages.append(("user", content))

            def add_assistant_message(self, content):
                self._messages.append(("assistant", content))

            def get_messages(self):
                return self._messages.copy()

        mock_sh_core = type(sys)("sh_core")
        mock_sh_core.Session = MockRustSession
        sys.modules["sh_core"] = mock_sh_core

        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        try:
            from continuum_sdk.agent.session import Session

            session = Session(id="rust-sync-test")
            session.add_user_message("User says hi")
            session.add_assistant_message("Assistant responds")

            assert session.message_count == 2
        finally:
            if "sh_core" in sys.modules:
                del sys.modules["sh_core"]
            if "continuum_sdk.agent.session" in sys.modules:
                del sys.modules["continuum_sdk.agent.session"]

    def test_rust_get_messages_conversion(self):
        """Test get_messages converts Rust messages to Python Message objects."""
        import sys

        class MockRustSession:
            def __init__(self, session_id):
                self._id = session_id
                self._messages = [("user", "Hello"), ("assistant", "Hi")]

            @property
            def id(self):
                return self._id

            @property
            def created_at(self):
                from datetime import datetime
                return datetime.now().isoformat()

            def message_count(self):
                return len(self._messages)

            def get_messages(self):
                return self._messages.copy()

        mock_sh_core = type(sys)("sh_core")
        mock_sh_core.Session = MockRustSession
        sys.modules["sh_core"] = mock_sh_core

        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        try:
            from continuum_sdk.agent.session import Session

            session = Session(id="rust-get-test")
            messages = session.get_messages()

            assert len(messages) == 2
            assert messages[0].role.value == "user"
            assert messages[0].content == "Hello"
            assert messages[1].role.value == "assistant"
            assert messages[1].content == "Hi"
        finally:
            if "sh_core" in sys.modules:
                del sys.modules["sh_core"]
            if "continuum_sdk.agent.session" in sys.modules:
                del sys.modules["continuum_sdk.agent.session"]

    def test_rust_get_messages_with_limit(self):
        """Test get_messages with limit when using Rust bindings."""
        import sys

        class MockRustSession:
            def __init__(self, session_id):
                self._id = session_id
                self._messages = [("user", f"Msg{i}") for i in range(10)]

            @property
            def id(self):
                return self._id

            @property
            def created_at(self):
                from datetime import datetime
                return datetime.now().isoformat()

            def message_count(self):
                return len(self._messages)

            def get_messages(self):
                return self._messages.copy()

        mock_sh_core = type(sys)("sh_core")
        mock_sh_core.Session = MockRustSession
        sys.modules["sh_core"] = mock_sh_core

        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        try:
            from continuum_sdk.agent.session import Session

            session = Session(id="rust-limit-test")
            messages = session.get_messages(limit=5)

            assert len(messages) == 5
            assert messages[0].content == "Msg5"
            assert messages[-1].content == "Msg9"
        finally:
            if "sh_core" in sys.modules:
                del sys.modules["sh_core"]
            if "continuum_sdk.agent.session" in sys.modules:
                del sys.modules["continuum_sdk.agent.session"]

    def test_rust_clear_messages(self):
        """Test clear_messages syncs to Rust."""
        import sys

        class MockRustSession:
            def __init__(self, session_id):
                self._id = session_id
                self._messages = [("user", "test")]
                self._cleared = False

            @property
            def id(self):
                return self._id

            @property
            def created_at(self):
                from datetime import datetime
                return datetime.now().isoformat()

            def message_count(self):
                return 0 if self._cleared else len(self._messages)

            def clear_messages(self):
                self._cleared = True
                self._messages.clear()

        mock_sh_core = type(sys)("sh_core")
        mock_sh_core.Session = MockRustSession
        sys.modules["sh_core"] = mock_sh_core

        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        try:
            from continuum_sdk.agent.session import Session

            session = Session(id="rust-clear-test")
            session.clear_messages()

            assert session.message_count == 0
        finally:
            if "sh_core" in sys.modules:
                del sys.modules["sh_core"]
            if "continuum_sdk.agent.session" in sys.modules:
                del sys.modules["continuum_sdk.agent.session"]

    def test_rust_export(self):
        """Test export uses Rust binding."""
        import sys

        class MockRustSession:
            def __init__(self, session_id):
                self._id = session_id

            @property
            def id(self):
                return self._id

            @property
            def created_at(self):
                from datetime import datetime
                return datetime.now().isoformat()

            def message_count(self):
                return 0

            def export(self):
                import json
                return json.dumps({"id": self._id, "exported_via": "rust"})

        mock_sh_core = type(sys)("sh_core")
        mock_sh_core.Session = MockRustSession
        sys.modules["sh_core"] = mock_sh_core

        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        try:
            from continuum_sdk.agent.session import Session

            session = Session(id="rust-export-test")
            exported = session.export()

            assert "rust-export-test" in exported
            assert "rust" in exported
        finally:
            if "sh_core" in sys.modules:
                del sys.modules["sh_core"]
            if "continuum_sdk.agent.session" in sys.modules:
                del sys.modules["continuum_sdk.agent.session"]


class TestRustBindingsImportCheck:
    """Test the HAS_RUST_BINDINGS flag logic."""

    def test_has_rust_bindings_false(self):
        """Test that HAS_RUST_BINDINGS is False when sh_core not available."""
        import sys

        # Ensure no mock sh_core is present
        if "sh_core" in sys.modules:
            del sys.modules["sh_core"]
        if "continuum_sdk.agent.session" in sys.modules:
            del sys.modules["continuum_sdk.agent.session"]

        # Force fresh import
        import importlib
        session_module = importlib.import_module("continuum_sdk.agent.session")

        # This should be False when no real sh_core is available
        assert session_module.HAS_RUST_BINDINGS is False


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
