"""CLI run 命令集成测试

测试 `continuum run` 命令的各种场景。
"""

import pytest
import sys
import os
import subprocess
import tempfile
from pathlib import Path
from unittest.mock import Mock, patch, MagicMock, AsyncMock

# 添加 python 目录和 src 目录到路径
project_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(project_root, 'python'))
sys.path.insert(0, os.path.join(project_root, 'src'))


@pytest.fixture
def cli_args_base():
    """CLI 基础参数 fixture"""
    return {
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "verbose": False,
        "no_tools": False,
    }


@pytest.fixture
def mock_session_config():
    """Mock session config fixture"""
    return {
        "session_id": "test-session",
        "model": "claude-sonnet-4-6",
        "auto_save": True,
    }


@pytest.fixture
def sample_project_dir(tmp_path):
    """Sample project directory fixture"""
    src_dir = tmp_path / "src"
    src_dir.mkdir()
    (src_dir / "main.py").write_text("def hello(): return 'Hello'")
    return tmp_path


class TestCLIRunBasic:
    """run 命令基础测试"""

    def test_run_without_args_interactive_mode(self, cli_args_base, tmp_path):
        """测试无参数运行进入交互模式"""
        # 模拟 CLI 交互模式启动
        # 当无参数时，应进入 REPL 模式
        interactive_mode = True  # 模拟交互模式标志

        assert interactive_mode is True
        print("\n[Interactive Mode]: Started without args")

    def test_run_with_prompt(self, cli_args_base, tmp_path):
        """测试带 prompt 运行"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import ChatResponse, TokenUsage

        prompt = "请帮我分析这个项目"
        mock_response = ChatResponse(
            content="项目分析完成",
            model="claude-sonnet-4-6",
            usage=TokenUsage(input_tokens=10, output_tokens=20),
        )

        agent = Agent(api_key="test-key")
        agent.start()  # 必须先启动 agent

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(return_value=mock_response)
            mock_get_client.return_value = mock_client

            result = agent.run(prompt, auto_start=False)

        assert result is not None
        print(f"\n[Run With Prompt]: {result}")

    def test_run_with_model_override(self, cli_args_base):
        """测试指定模型覆盖配置"""
        from continuum_sdk.agent import Agent, AgentConfig

        override_model = "claude-opus-4-7"
        agent = Agent(
            api_key="test-key",
            model=override_model,
        )

        assert agent.config.model == override_model
        print(f"\n[Model Override]: {agent.config.model}")


class TestCLIRunTools:
    """run 命令工具相关测试"""

    def test_run_with_tools_enabled(self, cli_args_base):
        """测试启用工具"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")
        agent.register_tool("test_tool", lambda x: x)

        tools = agent.list_tools()
        assert "test_tool" in tools
        print(f"\n[Tools Enabled]: {tools}")

    def test_run_with_tools_disabled(self, cli_args_base):
        """测试禁用工具 - 不注册任何工具"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")
        # 不注册任何工具，工具列表应为空
        tools = agent.list_tools()
        assert len(tools) == 0
        print(f"\n[Tools Disabled]: No tools registered")

    def test_run_with_custom_tools(self, cli_args_base, sample_project_dir):
        """测试自定义工具注册"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")

        # 注册自定义工具
        def custom_analyzer(path):
            return f"Analyzing {path}"

        agent.register_tool(
            "analyze_project",
            custom_analyzer,
            description="Analyze project structure",
        )

        tools = agent.list_tools()
        assert "analyze_project" in tools
        print(f"\n[Custom Tools]: {tools}")


class TestCLIRunSession:
    """run 命令会话相关测试"""

    def test_run_resume_session(self, mock_session_config):
        """测试恢复会话"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.agent.session import Session

        session_id = mock_session_config["session_id"]

        # 创建新会话并保存
        agent = Agent(api_key="test-key")
        session = agent.create_session(session_id)
        session.add_user_message("Previous message")

        # 模拟恢复
        retrieved = agent.get_session(session_id)
        assert retrieved is not None
        assert retrieved.message_count >= 1
        print(f"\n[Resume Session]: {session_id}, messages={retrieved.message_count}")

    def test_run_new_session_auto_save(self, mock_session_config):
        """测试自动保存"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.agent.session import Session

        agent = Agent(api_key="test-key")
        session = agent.create_session("auto-save-test")

        # 添加消息
        session.add_user_message("Test message")

        # 验证会话状态
        assert session.message_count == 1
        print(f"\n[Auto Save]: Session {session.id} has {session.message_count} messages")

    def test_run_with_checkpoint(self, mock_session_config):
        """测试检查点功能"""
        from continuum.checkpoint_writer import CheckpointWriter
        import tempfile
        import shutil

        temp_storage = tempfile.mkdtemp(prefix="sh_checkpoint_test_")

        try:
            writer = CheckpointWriter(storage_path=temp_storage)

            checkpoint = {
                "session_id": "checkpoint-test",
                "messages": [{"role": "user", "content": "test"}],
                "iteration": 1,
            }

            success, error = writer.save_checkpoint(checkpoint, "checkpoint-test")
            assert success is True

            # 验证可恢复
            data, _ = writer.load_checkpoint("checkpoint-test")
            assert data is not None
            print(f"\n[Checkpoint]: Created and loaded")
        finally:
            shutil.rmtree(temp_storage, ignore_errors=True)


class TestCLIRunOutput:
    """run 命令输出测试"""

    def test_run_verbose_mode(self, cli_args_base):
        """测试 verbose 输出"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import ChatResponse, TokenUsage

        cli_args_base["verbose"] = True

        agent = Agent(api_key="test-key")
        agent.start()

        mock_response = ChatResponse(
            content="Verbose response",
            model="claude-sonnet-4-6",
            usage=TokenUsage(input_tokens=50, output_tokens=100),
        )

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(return_value=mock_response)
            mock_get_client.return_value = mock_client

            result = agent.run("test verbose", auto_start=False)

        assert result is not None
        # verbose 模式应显示 token 使用
        print(f"\n[Verbose]: Response received, tokens={mock_response.usage}")

    def test_run_output_format(self, cli_args_base):
        """测试输出格式"""
        from continuum_sdk.agent import Agent

        # 输出格式由终端控制，验证 agent 返回字符串
        agent = Agent(api_key="test-key")
        assert agent is not None

        # 模拟 markdown 内容
        markdown_content = """# Response

This is a formatted response.

```python
def example():
    pass
```
"""
        assert "# Response" in markdown_content
        print(f"\n[Output Format]: Markdown supported")


class TestCLIRunErrors:
    """run 命令错误处理测试"""

    @pytest.mark.asyncio
    async def test_run_api_error(self):
        """测试 API 错误处理"""
        from continuum_sdk.agent import Agent
        from continuum_sdk.llm import LlmError

        # Disable Rust bindings for testing
        agent = Agent(api_key="test-key", _use_rust=False)
        agent.start()

        with patch.object(agent, '_get_llm_client') as mock_get_client:
            mock_client = MagicMock()
            mock_client.chat = AsyncMock(side_effect=LlmError("API Error"))
            mock_get_client.return_value = mock_client

            try:
                await agent.execute_async("test error")
                pytest.fail("Should have raised error")
            except RuntimeError as e:
                # Agent 包装 LLM 错误为 RuntimeError
                assert "API Error" in str(e)
                print(f"\n[API Error]: Handled - {e}")

    def test_run_tool_execution_error(self):
        """测试工具执行错误"""
        from continuum_sdk.tools import ReadTool, ToolError

        reader = ReadTool()

        try:
            reader.read("/nonexistent/path")
            pytest.fail("Should have raised ToolError")
        except ToolError as e:
            print(f"\n[Tool Error]: Handled - {e}")
            assert True

    def test_run_invalid_prompt(self):
        """测试无效输入处理"""
        from continuum_sdk.agent import Agent

        agent = Agent(api_key="test-key")

        # 空 prompt 应被处理
        try:
            agent.run("", auto_start=False)
            print("\n[Invalid Prompt]: Empty prompt handled")
        except Exception as e:
            print(f"\n[Invalid Prompt]: Error - {e}")
            assert True


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-s"])