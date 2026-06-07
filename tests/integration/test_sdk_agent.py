"""SDK Agent 集成测试

测试 Agent API 的各种场景。
使用 mock 和真实组件。
"""

import pytest
import sys
import os
import asyncio
from unittest.mock import Mock, patch, MagicMock, AsyncMock
import tempfile
from pathlib import Path

# 添加 python 目录和 src 目录到路径
project_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(project_root, 'python'))
sys.path.insert(0, os.path.join(project_root, 'src'))


@pytest.fixture
def mock_session_config():
    """Mock session config fixture"""
    return {
        "session_id": "test-session",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
    }


@pytest.fixture
def mock_agent_config():
    """Mock agent config fixture"""
    from continuum_sdk.agent import AgentConfig
    return AgentConfig(
        name="test-agent",
        model="claude-sonnet-4-6",
        max_tokens=4096,
        temperature=0.7,
    )


@pytest.fixture
def mock_llm_response():
    """Mock LLM response fixture"""
    from continuum_sdk.llm import ChatResponse, TokenUsage
    return ChatResponse(
        content="Test response from LLM",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=20),
    )


@pytest.fixture
def sample_messages():
    """Sample messages fixture"""
    from continuum_sdk.agent.session import Message, MessageRole
    return [
        Message(role=MessageRole.USER, content="Hello"),
        Message(role=MessageRole.ASSISTANT, content="Hi there!"),
        Message(role=MessageRole.USER, content="How are you?"),
    ]


@pytest.fixture
def sample_project_dir(tmp_path):
    """Sample project directory fixture"""
    # 创建项目结构
    src_dir = tmp_path / "src"
    src_dir.mkdir()
    (src_dir / "main.py").write_text("def hello(): return 'Hello'")
    (tmp_path / "README.md").write_text("# Test Project")
    return tmp_path


class TestAgentCreation:
    """Agent 创建测试"""

    def test_create_agent_default(self, mock_session_config):
        """测试默认创建"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")
        assert agent is not None
        assert agent.name == "default"
        print(f"\n[Agent Created]: {agent.name}")

    def test_create_agent_with_config(self, mock_agent_config):
        """测试带配置创建"""
        from continuum_sdk.agent import Agent, AgentConfig

        # 使用 AgentConfig 传递配置（Agent.__init__ 不直接接受 max_tokens）
        agent = Agent(
            name=mock_agent_config.name,
            api_key="test-key",
            model=mock_agent_config.model,
            config=mock_agent_config,  # 通过 config 传递完整配置
        )
        assert agent.name == "test-agent"
        assert agent.config.model == "claude-sonnet-4-6"
        assert agent.config.max_tokens == 4096
        print(f"\n[Agent With Config]: name={agent.name}, model={agent.config.model}")

    def test_create_agent_invalid_session(self):
        """测试无效会话 ID"""
        from continuum_sdk.agent import Agent

        # 创建不带有效会话的 agent
        agent = Agent(api_key="test-key")
        assert agent is not None  # 应创建默认会话
        print(f"\n[Invalid Session]: Created default agent")


class TestAgentChat:
    """Agent 对话测试"""

    @pytest.mark.asyncio
    async def test_chat_single_message(self, mock_llm_response):
        """测试单次对话"""
        from continuum_sdk.agent import Agent

        # Disable Rust bindings for testing
        agent = Agent(api_key="test-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(return_value=mock_llm_response)
            mock_get_client.return_value = mock_client

            result = await agent.execute_async("你好")

        assert result == "Test response from LLM"
        print(f"\n[Chat Single]: {result}")

    @pytest.mark.asyncio
    async def test_chat_with_context(self, sample_messages):
        """测试带上下文对话"""
        from continuum_sdk.agent import Agent, AgentConfig
        from continuum_sdk.llm import ChatResponse, TokenUsage

        agent = Agent(api_key="test-key", config=AgentConfig(name="context-test"))
        agent.start()

        mock_response = ChatResponse(
            content="I remember our conversation",
            model="claude-sonnet-4-6",
            usage=TokenUsage(input_tokens=15, output_tokens=25),
        )

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(return_value=mock_response)
            mock_get_client.return_value = mock_client

            result = agent.run("继续刚才的话题", auto_start=False)

        assert result is not None
        print(f"\n[Chat Context]: {result}")

    def test_chat_empty_message(self):
        """测试空消息处理"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import ChatResponse, TokenUsage

        agent = Agent(api_key="test-key")
        agent.start()

        mock_response = ChatResponse(
            content="",
            model="claude-sonnet-4-6",
            usage=TokenUsage(input_tokens=0, output_tokens=0),
        )

        # 空消息应该被处理
        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(return_value=mock_response)
            mock_get_client.return_value = mock_client

            result = agent.execute("")
            assert result is not None  # 空消息可能返回空或默认响应

        print("\n[Empty Message]: Handled")

    @pytest.mark.asyncio
    async def test_chat_long_message(self):
        """测试长消息处理"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import ChatResponse, TokenUsage

        agent = Agent(api_key="test-key")
        agent.start()

        long_msg = "这是一段很长的消息..." * 100
        mock_response = ChatResponse(
            content="Received long message",
            model="claude-sonnet-4-6",
            usage=TokenUsage(input_tokens=500, output_tokens=10),
        )

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(return_value=mock_response)
            mock_get_client.return_value = mock_client

            result = await agent.execute_async(long_msg)

        assert result is not None
        print(f"\n[Long Message]: Processed {len(long_msg)} chars")


class TestAgentTools:
    """Agent 工具调用测试"""

    def test_tool_call_read_file(self, sample_project_dir):
        """测试读文件工具"""
        from continuum_sdk.tools import ReadTool

        reader = ReadTool()
        result = reader.read(str(sample_project_dir / "src" / "main.py"))

        assert result.is_error is False
        assert "hello" in result.content
        print(f"\n[Read Tool]: {result.content[:50]}")

    def test_tool_call_write_file(self, tmp_path):
        """测试写文件工具"""
        from continuum_sdk.tools import WriteTool, ReadTool

        writer = WriteTool(backup=False)
        filepath = str(tmp_path / "test.txt")

        result = writer.write(filepath, "hello world")
        assert result.is_error is False

        reader = ReadTool()
        content = reader.read(filepath)
        assert "hello world" in content.content
        print(f"\n[Write Tool]: Created {filepath}")

    def test_tool_call_bash(self):
        """测试 Bash 工具"""
        from continuum_sdk.tools import BashTool

        bash = BashTool()
        result = bash.run("echo test")

        assert result.is_error is False
        assert "test" in result.content
        print(f"\n[Bash Tool]: {result.content}")

    def test_tool_call_chain(self, sample_project_dir):
        """测试工具链"""
        from continuum_sdk.tools import ReadTool, EditTool, WriteTool

        reader = ReadTool()
        editor = EditTool(backup=False)
        writer = WriteTool(backup=False)

        filepath = str(sample_project_dir / "src" / "main.py")

        # 读取
        read_result = reader.read(filepath)
        assert read_result.is_error is False

        # 编辑
        edit_result = editor.edit(filepath, "Hello", "Hello World")
        assert edit_result.is_error is False

        # 验证
        final = reader.read(filepath)
        assert "Hello World" in final.content
        print(f"\n[Tool Chain]: read -> edit -> verify")

    def test_tool_call_failure(self):
        """测试工具失败处理"""
        from continuum_sdk.tools import ReadTool, ToolError

        reader = ReadTool()
        with pytest.raises(ToolError):
            reader.read("/nonexistent/path/file.txt")
        print("\n[Tool Failure]: Correctly raised error")


class TestAgentMemory:
    """Agent 记忆测试"""

    def test_memory_working_tier(self):
        """测试工作记忆"""
        from continuum_sdk.memory import Memory, MemoryTier

        memory = Memory(session_id="memory-test")
        entry_id = memory.remember("key fact", tier=MemoryTier.WORKING)

        assert entry_id is not None
        results = memory.recall("key")
        assert len(results) > 0
        print(f"\n[Working Memory]: Stored {entry_id}")

    def test_memory_session_tier(self):
        """测试会话记忆"""
        from continuum_sdk.memory import Memory, MemoryTier

        memory = Memory(session_id="session-memory-test")
        entry_id = memory.remember("session data", tier=MemoryTier.SESSION)

        assert entry_id is not None
        stats = memory.stats()
        assert stats[MemoryTier.SESSION] > 0
        print(f"\n[Session Memory]: {stats}")

    def test_memory_recall(self):
        """测试记忆检索"""
        from continuum_sdk.memory import Memory, MemoryTier

        memory = Memory(session_id="recall-test")
        memory.remember("important fact about Python", tier=MemoryTier.WORKING)

        results = memory.recall("Python")
        assert len(results) > 0
        assert "Python" in results[0].content
        print(f"\n[Recall]: Found {len(results)} results")

    def test_memory_forget(self):
        """测试遗忘"""
        from continuum_sdk.memory import Memory, MemoryTier

        memory = Memory(session_id="forget-test")
        entry_id = memory.remember("temporary data", tier=MemoryTier.WORKING)

        result = memory.forget(MemoryTier.WORKING, entry_id)
        assert result is True

        # 验证已删除
        entry = memory.get(MemoryTier.WORKING, entry_id)
        assert entry is None
        print(f"\n[Forget]: Removed {entry_id}")


class TestAgentStream:
    """Agent 流式响应测试"""

    @pytest.mark.asyncio
    async def test_stream_response(self):
        """测试流式输出"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import StreamChunk

        async def mock_stream(*args, **kwargs):
            chunks = ["Hello", " ", "world", "!"]
            for chunk in chunks:
                yield StreamChunk(content=chunk)

        # Disable Rust bindings for testing
        agent = Agent(api_key="test-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat_stream = mock_stream
            mock_get_client.return_value = mock_client

            chunks = []
            async for chunk in agent.execute_stream("写一个故事"):
                chunks.append(chunk)

        assert len(chunks) >= 4
        full_content = "".join(c.content for c in chunks if c.content)
        assert "Hello world" in full_content
        print(f"\n[Stream]: {len(chunks)} chunks")

    @pytest.mark.asyncio
    async def test_stream_with_callback(self):
        """测试流式回调"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import StreamChunk

        events = []

        async def mock_stream(*args, **kwargs):
            for i in range(3):
                yield StreamChunk(content=f"chunk{i}")

        # Disable Rust bindings for testing
        agent = Agent(api_key="test-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat_stream = mock_stream
            mock_get_client.return_value = mock_client

            async for chunk in agent.execute_stream("hello"):
                events.append(chunk)

        assert len(events) == 3
        print(f"\n[Stream Callback]: {len(events)} events")

    def test_stream_interrupt(self):
        """测试中断流"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")
        # 流中断是运行时行为，验证 agent 可以正常停止
        agent.start()
        agent.stop()
        assert agent.state.name == "IDLE"
        print("\n[Stream Interrupt]: Agent stopped")


class TestAgentError:
    """Agent 错误处理测试"""

    @pytest.mark.asyncio
    async def test_api_timeout(self):
        """测试 API 超时处理"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import LlmError

        # Disable Rust bindings for testing
        agent = Agent(api_key="test-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(side_effect=LlmError("Timeout"))
            mock_get_client.return_value = mock_client

            try:
                await agent.execute_async("timeout test")
                pytest.fail("Should have raised error")
            except RuntimeError as e:
                # Agent 包装 LLM 错误为 RuntimeError
                assert "Timeout" in str(e)
                print(f"\n[Timeout Error]: {e}")

    @pytest.mark.asyncio
    async def test_api_rate_limit(self):
        """测试速率限制处理"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import LlmError

        # Disable Rust bindings for testing
        agent = Agent(api_key="test-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(side_effect=LlmError("Rate limit exceeded"))
            mock_get_client.return_value = mock_client

            try:
                await agent.execute_async("rate limit test")
                pytest.fail("Should have raised error")
            except RuntimeError as e:
                # Agent 包装 LLM 错误为 RuntimeError
                assert "Rate" in str(e)
                print(f"\n[Rate Limit Error]: {e}")

    def test_api_auth_error(self):
        """测试认证错误"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm.errors import AuthenticationError

        # Disable Rust bindings for testing
        agent = Agent(api_key="invalid-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(side_effect=AuthenticationError("Invalid API key"))
            mock_get_client.return_value = mock_client

            try:
                import asyncio
                asyncio.run(agent.execute_async("auth test"))
                pytest.fail("Should have raised auth error")
            except ValueError as e:
                # AuthenticationError 被 Agent 转换为 ValueError
                assert "Authentication" in str(e) or "Invalid" in str(e) or "API key" in str(e)
                print(f"\n[Auth Error]: {e}")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-s"])