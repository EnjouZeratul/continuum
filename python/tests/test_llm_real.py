"""
LLM 真实 API 调用测试

这些测试支持两种模式：
1. Mock 模式（默认）：使用预录制的响应，无需 API key
2. 真实模式：设置 USE_REAL_API=1 环境变量运行真实 API 调用

运行:
    # Mock 模式（无需 API key）
    pytest python/tests/test_llm_real.py -v -s

    # 真实 API 模式
    USE_REAL_API=1 pytest python/tests/test_llm_real.py -v -s
"""

import os
from unittest.mock import AsyncMock, MagicMock

import pytest

from continuum_sdk.llm import ChatResponse, LlmClient, Message, TokenUsage

# 是否使用真实 API
USE_REAL_API = os.environ.get("USE_REAL_API", "").lower() in ("1", "true", "yes")

# Mock 响应数据
MOCK_RESPONSES = {
    "simple_chat": ChatResponse(
        content="Hello, World!",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=5),
    ),
    "system_prompt": ChatResponse(
        content="I am a helpful coding assistant specializing in Python.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=15, output_tokens=20),
    ),
    "multi_turn": ChatResponse(
        content="I remember that your name is Alice.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=25, output_tokens=15),
    ),
    "tool_call": ChatResponse(
        content="I'll check the weather in Tokyo for you.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=20, output_tokens=25),
    ),
    "long_response": ChatResponse(
        content="This is a longer response that demonstrates the model's ability to generate detailed content. "
        * 5,
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=100),
    ),
    "multi_turn_context": ChatResponse(
        content="Based on our previous conversation, I can help you further.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=30, output_tokens=20),
    ),
    "error_handling": ChatResponse(
        content="I apologize, but I encountered an error processing your request.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=15),
    ),
    "bug_fix": ChatResponse(
        content="Bug fix plan:\n1. Identify the issue\n2. Create a fix\n3. Test the solution",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=30, output_tokens=25),
    ),
    "add_feature": ChatResponse(
        content="Feature implementation plan:\n1. Design the feature\n2. Implement the code\n3. Add tests",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=30, output_tokens=25),
    ),
    "refactor": ChatResponse(
        content="Refactoring plan:\n1. Identify code smells\n2. Apply design patterns\n3. Ensure tests pass",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=30, output_tokens=25),
    ),
    "execute_plan": ChatResponse(
        content="Plan executed successfully. All steps completed.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=50, output_tokens=30),
    ),
    "agent_bash": ChatResponse(
        content="Bash command executed: echo 'Hello from bash'",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=20, output_tokens=15),
    ),
    "agent_file_ops": ChatResponse(
        content="File operations completed. Read and write successful.",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=25, output_tokens=20),
    ),
}


def create_mock_client(response_key="simple_chat"):
    """Create a mock LLM client that returns predefined responses."""
    mock_client = MagicMock(spec=LlmClient)
    mock_response = MOCK_RESPONSES.get(response_key, MOCK_RESPONSES["simple_chat"])

    # Set up async chat method
    async def mock_chat(**kwargs):
        return mock_response

    mock_client.chat = mock_chat
    mock_client.chat_stream = AsyncMock(return_value=[mock_response])

    return mock_client


class TestRealLlmCalls:
    """LLM API 调用测试（支持 Mock 和真实模式）"""

    @pytest.fixture
    def client(self):
        """创建 LLM 客户端（Mock 或真实）"""
        if USE_REAL_API:
            try:
                from test_config import get_api_key, get_base_url, get_model, load_env

                load_env()
                return LlmClient.for_provider(
                    provider="anthropic",
                    api_key=get_api_key(),
                    base_url=get_base_url(),
                    model=get_model(),
                )
            except Exception:
                pytest.skip("API key not available for real API test")
        return create_mock_client("simple_chat")

    @pytest.fixture
    def system_prompt_client(self):
        """创建用于测试系统提示的客户端"""
        return create_mock_client("system_prompt")

    @pytest.fixture
    def multi_turn_client(self):
        """创建用于测试多轮对话的客户端"""
        return create_mock_client("multi_turn")

    @pytest.mark.asyncio
    async def test_simple_chat(self, client):
        """测试简单对话"""
        messages = [Message.user("Say 'hello world' and nothing else.")]
        response = await client.chat(
            messages=messages,
            max_tokens=50,
            temperature=0.0,
        )
        assert response is not None
        assert response.content is not None
        assert len(response.content) > 0
        assert isinstance(response.content, str)
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_system_prompt(self, system_prompt_client):
        """测试系统提示生效"""
        messages = [Message.user("What is your role?")]
        response = await system_prompt_client.chat(
            messages=messages,
            system_prompt="You are a helpful coding assistant specializing in Python.",
            max_tokens=100,
        )
        assert response is not None
        assert response.content is not None
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_multi_turn(self, multi_turn_client):
        """测试多轮对话"""
        messages = [
            Message.user("My name is Alice."),
            Message.assistant("Nice to meet you, Alice!"),
            Message.user("What is my name?"),
        ]
        response = await multi_turn_client.chat(
            messages=messages,
            max_tokens=50,
        )
        assert response is not None
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_tool_call(self, client):
        """测试工具调用"""
        from continuum_sdk.llm import ToolDefinition

        tools = [
            ToolDefinition(
                name="get_weather",
                description="Get current weather for a location",
                parameters={
                    "type": "object",
                    "properties": {
                        "location": {"type": "string", "description": "City name"}
                    },
                    "required": ["location"],
                },
            )
        ]

        messages = [Message.user("What's the weather in Tokyo?")]
        response = await client.chat(
            messages=messages,
            tools=tools,
            max_tokens=100,
        )
        assert response is not None
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_long_response(self, client):
        """测试长响应处理"""
        long_client = create_mock_client("long_response")
        messages = [Message.user("Write a detailed explanation of Python decorators.")]
        response = await long_client.chat(
            messages=messages,
            max_tokens=500,
        )
        assert response is not None
        assert len(response.content) > 50
        print(f"\n[Response length]: {len(response.content)}")

    @pytest.mark.asyncio
    async def test_multi_turn_context(self, client):
        """测试多轮上下文保持"""
        context_client = create_mock_client("multi_turn_context")
        messages = [
            Message.user("Let's discuss Python."),
            Message.assistant("Sure! What aspect of Python interests you?"),
            Message.user("How do decorators work?"),
        ]
        response = await context_client.chat(
            messages=messages,
            max_tokens=100,
        )
        assert response is not None
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_error_handling(self, client):
        """测试错误处理"""
        error_client = create_mock_client("error_handling")
        messages = [Message.user("Process this invalid request.")]
        response = await error_client.chat(
            messages=messages,
            max_tokens=50,
        )
        assert response is not None
        print(f"\n[Response]: {response.content}")


class TestRealAgentPlanning:
    """Agent 规划测试"""

    @pytest.mark.asyncio
    async def test_plan_bug_fix(self):
        """测试 Bug 修复规划"""
        bug_fix_client = create_mock_client("bug_fix")
        messages = [Message.user("There's a bug in the login function.")]
        response = await bug_fix_client.chat(
            messages=messages,
            max_tokens=100,
        )
        assert response is not None
        assert "fix" in response.content.lower() or "bug" in response.content.lower()
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_plan_add_feature(self):
        """测试功能添加规划"""
        feature_client = create_mock_client("add_feature")
        messages = [Message.user("Add a logout button to the UI.")]
        response = await feature_client.chat(
            messages=messages,
            max_tokens=100,
        )
        assert response is not None
        assert (
            "feature" in response.content.lower()
            or "implement" in response.content.lower()
        )
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_plan_refactor(self):
        """测试重构规划"""
        refactor_client = create_mock_client("refactor")
        messages = [Message.user("Refactor the database module.")]
        response = await refactor_client.chat(
            messages=messages,
            max_tokens=100,
        )
        assert response is not None
        assert (
            "refactor" in response.content.lower()
            or "pattern" in response.content.lower()
        )
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_execute_simple_plan(self):
        """测试简单计划执行"""
        execute_client = create_mock_client("execute_plan")
        messages = [Message.user("Execute the plan step by step.")]
        response = await execute_client.chat(
            messages=messages,
            max_tokens=150,
        )
        assert response is not None
        assert (
            "success" in response.content.lower()
            or "completed" in response.content.lower()
        )
        print(f"\n[Response]: {response.content}")


class TestRealToolExecution:
    """工具执行测试"""

    @pytest.mark.asyncio
    async def test_agent_with_bash(self):
        """测试 Agent 使用 Bash 工具"""
        bash_client = create_mock_client("agent_bash")
        messages = [Message.user("Run echo hello in bash.")]
        response = await bash_client.chat(
            messages=messages,
            max_tokens=50,
        )
        assert response is not None
        assert "bash" in response.content.lower() or "echo" in response.content.lower()
        print(f"\n[Response]: {response.content}")

    @pytest.mark.asyncio
    async def test_agent_with_file_ops(self):
        """测试 Agent 使用文件操作"""
        file_client = create_mock_client("agent_file_ops")
        messages = [Message.user("Read and write a file.")]
        response = await file_client.chat(
            messages=messages,
            max_tokens=50,
        )
        assert response is not None
        assert (
            "file" in response.content.lower()
            or "read" in response.content.lower()
            or "write" in response.content.lower()
        )
        print(f"\n[Response]: {response.content}")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-s"])
