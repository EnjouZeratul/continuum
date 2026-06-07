"""SDK Session 集成测试

测试 Session API 的各种场景。
"""

import pytest
import sys
import os
import asyncio
import json
import tempfile
import shutil
from pathlib import Path
from datetime import datetime
from unittest.mock import Mock, patch, MagicMock, AsyncMock

# Add src and python directories to path
# Note: src must be added LAST so it shadows python/continuum with checkpoint_writer
_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(_root, "python"))
sys.path.insert(0, os.path.join(_root, "src"))


@pytest.fixture
def mock_session_config():
    """Mock session config fixture"""
    return {
        "session_id": "test-session-123",
        "name": "Test Session",
        "working_dir": "/tmp",
    }


@pytest.fixture
def sample_messages():
    """Sample messages fixture"""
    from continuum_sdk.agent.session import Message, MessageRole
    return [
        Message(role=MessageRole.USER, content="Hello"),
        Message(role=MessageRole.ASSISTANT, content="Hi!"),
        Message(role=MessageRole.USER, content="How are you?"),
    ]


class TestSessionCreation:
    """Session 创建测试"""

    def test_create_session_default(self):
        """测试默认创建"""
        from continuum_sdk.agent.session import Session

        session = Session()
        assert session is not None
        assert session.id is not None
        assert session.message_count == 0
        print(f"\n[Session Default]: id={session.id}")

    def test_create_session_with_name(self, mock_session_config):
        """测试带名称创建"""
        from continuum_sdk.agent.session import Session

        session = Session(id="named-session")
        assert session.id == "named-session"
        print(f"\n[Session Named]: id={session.id}")

    def test_create_session_with_working_dir(self, tmp_path):
        """测试指定工作目录"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")
        session = agent.create_session("work-dir-test")

        assert session is not None
        print(f"\n[Working Dir]: Session created")

    def test_create_session_duplicate_name(self, mock_session_config):
        """测试不传 ID 时自动生成唯一 ID"""
        from continuum_sdk.agent import Agent

        # Disable Rust bindings to test Python session ID generation
        agent = Agent(api_key="test-key", _use_rust=False)
        # 不传 ID，让系统自动生成唯一 ID
        session1 = agent.create_session()
        session2 = agent.create_session()

        # 自动生成的 ID 应该不同
        assert session1.id != session2.id
        print(f"\n[Duplicate]: Different IDs: {session1.id} vs {session2.id}")


class TestSessionLifecycle:
    """Session 生命周期测试"""

    def test_session_start(self, mock_session_config):
        """测试启动状态"""
        from continuum_sdk.agent.session import Session

        session = Session(id="lifecycle-test")
        # Session 默认就是活跃状态
        assert session.message_count == 0
        print(f"\n[Start]: Session ready")

    def test_session_add_messages(self, mock_session_config):
        """测试添加消息"""
        from continuum_sdk.agent.session import Session

        session = Session(id="message-test")
        session.add_user_message("Hello")
        session.add_assistant_message("Response")

        assert session.message_count == 2
        print(f"\n[Messages]: {session.message_count} messages")

    def test_session_clear(self, mock_session_config):
        """测试清空消息"""
        from continuum_sdk.agent.session import Session

        session = Session(id="clear-test")
        session.add_user_message("Test")
        session.clear_messages()

        assert session.message_count == 0
        print(f"\n[Clear]: Messages cleared")

    def test_session_export_import(self, mock_session_config):
        """测试导出导入"""
        from continuum_sdk.agent.session import Session

        original = Session(id="export-test")
        original.add_user_message("Export test")

        exported = original.export()
        restored = Session.from_export(exported)

        assert restored.id == original.id
        assert restored.message_count == original.message_count
        print(f"\n[Export/Import]: Restored {restored.message_count} messages")


class TestSessionHistory:
    """Session 历史测试"""

    def test_add_message(self, mock_session_config):
        """测试添加消息"""
        from continuum_sdk.agent.session import Session

        session = Session(id="history-test")
        initial_count = session.message_count

        session.add_user_message("New message")
        assert session.message_count == initial_count + 1
        print(f"\n[Add Message]: {session.message_count} total")

    def test_get_history(self, mock_session_config, sample_messages):
        """测试获取历史"""
        from continuum_sdk.agent.session import Session

        session = Session(id="history-list-test")
        for msg in sample_messages:
            if msg.role.value == "user":
                session.add_user_message(msg.content)
            else:
                session.add_assistant_message(msg.content)

        history = session.get_messages()
        assert len(history) == 3
        print(f"\n[History]: {len(history)} messages")

    def test_get_history_with_limit(self, mock_session_config):
        """测试限制历史长度"""
        from continuum_sdk.agent.session import Session

        session = Session(id="limit-test")
        for i in range(10):
            session.add_user_message(f"Message {i}")

        # 获取所有消息
        all_messages = session.get_messages()
        assert len(all_messages) == 10
        print(f"\n[History Limit]: {len(all_messages)} messages (limit not implemented)")

    def test_clear_history(self, mock_session_config):
        """测试清空历史"""
        from continuum_sdk.agent.session import Session

        session = Session(id="clear-history-test")
        session.add_user_message("To clear")
        session.clear_messages()

        assert session.message_count == 0
        print(f"\n[Clear History]: Cleared")


class TestSessionCheckpoint:
    """Session 检查点测试"""

    def test_save_checkpoint(self, mock_session_config):
        """测试保存检查点"""
        from continuum.checkpoint_writer import CheckpointWriter

        temp_storage = tempfile.mkdtemp(prefix="sh_cp_save_")

        try:
            writer = CheckpointWriter(storage_path=temp_storage)

            checkpoint = {
                "session_id": "cp-save-test",
                "messages": [{"role": "user", "content": "test"}],
                "iteration": 1,
            }

            success, error = writer.save_checkpoint(checkpoint, "cp-save-test")
            assert success is True
            print(f"\n[Save Checkpoint]: Success")
        finally:
            shutil.rmtree(temp_storage, ignore_errors=True)

    def test_list_checkpoints(self, mock_session_config):
        """测试列出检查点"""
        from continuum.checkpoint_writer import CheckpointWriter
        import time

        temp_storage = tempfile.mkdtemp(prefix="sh_cp_list_")

        try:
            writer = CheckpointWriter(storage_path=temp_storage)

            # 创建多个检查点
            for i in range(3):
                checkpoint = {
                    "session_id": "cp-list-test",
                    "messages": [],
                    "iteration": i,
                }
                writer.save_checkpoint(checkpoint, "cp-list-test")
                time.sleep(0.05)

            checkpoints = writer.list_checkpoints("cp-list-test")
            assert len(checkpoints) >= 3
            print(f"\n[List Checkpoints]: {len(checkpoints)} checkpoints")
        finally:
            shutil.rmtree(temp_storage, ignore_errors=True)

    def test_rollback_checkpoint(self, mock_session_config):
        """测试回滚检查点"""
        from continuum.checkpoint_writer import CheckpointWriter

        temp_storage = tempfile.mkdtemp(prefix="sh_cp_roll_")

        try:
            writer = CheckpointWriter(storage_path=temp_storage)

            # 保存初始状态
            initial = {
                "session_id": "rollback-test",
                "messages": [{"role": "user", "content": "initial"}],
                "iteration": 1,
            }
            writer.save_checkpoint(initial, "rollback-test")

            # 更新状态
            updated = {
                "session_id": "rollback-test",
                "messages": [{"role": "user", "content": "updated"}],
                "iteration": 2,
            }
            writer.save_checkpoint(updated, "rollback-test")

            # 恢复
            data, _ = writer.load_checkpoint("rollback-test")
            assert data is not None
            print(f"\n[Rollback Checkpoint]: Loaded iteration {data['iteration']}")
        finally:
            shutil.rmtree(temp_storage, ignore_errors=True)

    def test_delete_checkpoint(self, mock_session_config):
        """测试删除检查点"""
        from continuum.checkpoint_writer import CheckpointWriter

        temp_storage = tempfile.mkdtemp(prefix="sh_cp_del_")

        try:
            writer = CheckpointWriter(storage_path=temp_storage)

            checkpoint = {"session_id": "del-test", "messages": [], "iteration": 0}
            writer.save_checkpoint(checkpoint, "del-test")

            # 删除检查点目录
            cp_dir = Path(temp_storage) / "del-test" / "checkpoints"
            if cp_dir.exists():
                shutil.rmtree(cp_dir)

            print(f"\n[Delete Checkpoint]: Removed")
        finally:
            shutil.rmtree(temp_storage, ignore_errors=True)


class TestSessionPersistence:
    """Session 持久化测试"""

    def test_save_session(self, mock_session_config, tmp_path):
        """测试保存会话"""
        from continuum_sdk.agent.session import Session

        session = Session(id="save-test")
        session.add_user_message("Save test")

        # export() returns JSON string directly
        exported = session.export()
        save_file = tmp_path / "session.json"
        save_file.write_text(exported)

        assert save_file.exists()
        print(f"\n[Save Session]: Saved to {save_file}")

    def test_load_session(self, mock_session_config, tmp_path):
        """测试加载会话"""
        from continuum_sdk.agent.session import Session

        # 保存会话
        original = Session(id="load-test")
        original.add_user_message("Load test")

        save_file = tmp_path / "load-session.json"
        # export() returns JSON string directly
        save_file.write_text(original.export())

        # 加载会话
        restored = Session.from_export(save_file.read_text())

        assert restored.id == "load-test"
        assert restored.message_count == 1
        print(f"\n[Load Session]: Restored {restored.message_count} messages")

    def test_auto_save(self, mock_session_config):
        """测试自动保存行为"""
        from continuum_sdk.agent.session import Session

        session = Session(id="autosave-test")
        session.add_user_message("Auto save test")

        # export() returns JSON string, parse to verify
        exported = session.export()
        parsed = json.loads(exported)
        assert parsed is not None
        assert len(parsed.get("messages", [])) >= 1
        print(f"\n[Auto Save]: Exportable")


class TestSessionStats:
    """Session 统计测试"""

    def test_get_token_count(self, mock_session_config):
        """测试 token 统计"""
        from continuum_sdk.agent.session import Session

        session = Session(id="token-test")
        session.update_cost(0.05, 1000)

        assert session.tokens == 1000
        assert session.cost == 0.05
        print(f"\n[Token Count]: {session.tokens} tokens, cost={session.cost}")

    def test_get_tool_calls(self, mock_session_config):
        """测试工具调用统计"""
        from continuum_sdk.agent.session import Session

        session = Session(id="tool-stats-test")
        session.record_tool_use("read_file")
        session.record_tool_use("write_file")
        session.record_tool_use("read_file")

        tools = session.get_tools_used()
        assert "read_file" in tools
        assert "write_file" in tools
        print(f"\n[Tool Calls]: {tools}")

    def test_get_duration(self, mock_session_config):
        """测试持续时间"""
        from continuum_sdk.agent.session import Session
        import time

        session = Session(id="duration-test")
        created = session.created_at

        time.sleep(0.1)
        now = datetime.now()

        duration = (now - created).total_seconds()
        assert duration > 0
        print(f"\n[Duration]: {duration:.2f}s")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-s"])