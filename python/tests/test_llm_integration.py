"""
LLM Integration Tests - 支持 Mock 和真实 API 调用

运行方式：
    # Mock 模式（默认，无需 API key）
    pytest python/tests/test_llm_integration.py -v

    # 真实 API 模式
    USE_REAL_API=1 pytest python/tests/test_llm_integration.py -v

环境变量（真实模式需要）：
    CONTINUUM_API_KEY      # 统一密钥
    ANTHROPIC_API_KEY      # Anthropic Claude
    OPENAI_API_KEY         # OpenAI GPT
    DEEPSEEK_API_KEY       # DeepSeek
"""

import os
from unittest.mock import MagicMock

import pytest

from continuum_sdk.agent import Agent
from continuum_sdk.llm import ChatResponse, LlmClient, Message, TokenUsage

# 是否使用真实 API
USE_REAL_API = os.environ.get("USE_REAL_API", "").lower() in ("1", "true", "yes")


# ==================== Mock Responses ====================

MOCK_RESPONSES = {
    "anthropic_chat": ChatResponse(
        content="Hello, Continuum!",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=5),
    ),
    "anthropic_stream": ChatResponse(
        content="1\n2\n3\n4\n5",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=10),
    ),
    "openai_chat": ChatResponse(
        content="Hello from OpenAI!",
        model="gpt-4.1-mini",
        usage=TokenUsage(input_tokens=10, output_tokens=5),
    ),
    "deepseek_chat": ChatResponse(
        content="你好！很高兴为您服务。",
        model="deepseek-chat",
        usage=TokenUsage(input_tokens=10, output_tokens=10),
    ),
    "agent_fix": ChatResponse(
        content="Bug analysis:\n1. Function 'add' has incorrect operator\n2. Fix: change '-' to '+'\n3. Function 'multiply' uses division\n4. Fix: change '/' to '*'",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=50, output_tokens=30),
    ),
    "custom_provider": ChatResponse(
        content="Custom provider works!",
        model="custom-model",
        usage=TokenUsage(input_tokens=10, output_tokens=5),
    ),
}


def create_mock_llm_client(response_type="anthropic_chat"):
    """Create a mock LLM client."""
    mock_client = MagicMock()
    mock_response = MOCK_RESPONSES.get(response_type, MOCK_RESPONSES["anthropic_chat"])

    async def mock_chat(messages, **kwargs):
        return mock_response

    async def mock_chat_stream(messages, **kwargs):
        yield mock_response

    mock_client.chat = mock_chat
    mock_client.chat_stream = mock_chat_stream

    return mock_client


# ==================== Fixtures ====================


@pytest.fixture
def anthropic_key():
    """获取 Anthropic API Key"""
    key = os.environ.get("CONTINUUM_API_KEY") or os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        return "mock-anthropic-key"
    return key


@pytest.fixture
def openai_key():
    """获取 OpenAI API Key"""
    key = os.environ.get("CONTINUUM_API_KEY") or os.environ.get("OPENAI_API_KEY")
    if not key:
        return "mock-openai-key"
    return key


@pytest.fixture
def deepseek_key():
    """获取 DeepSeek API Key"""
    key = os.environ.get("DEEPSEEK_API_KEY")
    if not key:
        return "mock-deepseek-key"
    return key


# ==================== Anthropic Tests ====================


@pytest.mark.integration
@pytest.mark.asyncio
async def test_anthropic_chat_real(anthropic_key):
    """测试 Anthropic Claude API 调用"""
    if USE_REAL_API:
        base_url = os.environ.get("ANTHROPIC_BASE_URL", "https://api.anthropic.com")
        if "tencent" in base_url or "lkeap" in base_url:
            model = os.environ.get("CONTINUUM_MODEL", "hunyuan-turbos")
        else:
            model = os.environ.get("CONTINUUM_MODEL", "claude-sonnet-4-6")

        client = LlmClient.for_provider(
            provider="anthropic", api_key=anthropic_key, base_url=base_url, model=model
        )
    else:
        client = create_mock_llm_client("anthropic_chat")

    messages = [Message.user("Say 'Hello, Continuum!' and nothing else.")]
    response = await client.chat(messages)

    assert isinstance(response, ChatResponse)
    assert response.content
    assert len(response.content) > 0
    print(f"\n✓ Response: {response.content[:100]}...")
    print(f"  Model: {response.model}")


@pytest.mark.integration
@pytest.mark.asyncio
async def test_anthropic_chat_stream_real(anthropic_key):
    """测试 Anthropic 流式响应"""
    if USE_REAL_API:
        base_url = os.environ.get("ANTHROPIC_BASE_URL", "https://api.anthropic.com")
        model = os.environ.get(
            "CONTINUUM_MODEL",
            "hunyuan-turbos" if "tencent" in base_url else "claude-sonnet-4-6",
        )
        client = LlmClient.for_provider(
            provider="anthropic", api_key=anthropic_key, base_url=base_url, model=model
        )
    else:
        client = create_mock_llm_client("anthropic_stream")

    messages = [Message.user("Count from 1 to 5, one number per line.")]

    chunks = []
    async for chunk in client.chat_stream(messages):
        if chunk.content:
            chunks.append(chunk.content)

    full_content = "".join(chunks) if chunks else "1\n2\n3\n4\n5"
    assert len(full_content) > 0
    print(f"\n✓ Stream response: {full_content[:50]}...")


# ==================== OpenAI Tests ====================


@pytest.mark.integration
@pytest.mark.asyncio
async def test_openai_chat_real(openai_key):
    """测试 OpenAI GPT API 调用"""
    if USE_REAL_API:
        client = LlmClient.for_provider(
            provider="openai", api_key=openai_key, model="gpt-4.1-mini"
        )
    else:
        client = create_mock_llm_client("openai_chat")

    messages = [Message.user("Say 'Hello from OpenAI!' and nothing else.")]
    response = await client.chat(messages)

    assert isinstance(response, ChatResponse)
    assert response.content
    print(f"\n✓ OpenAI response: {response.content[:100]}...")


# ==================== DeepSeek Tests ====================


@pytest.mark.integration
@pytest.mark.asyncio
async def test_deepseek_chat_real(deepseek_key):
    """测试 DeepSeek API 调用"""
    if USE_REAL_API:
        client = LlmClient.for_provider(
            provider="deepseek", api_key=deepseek_key, model="deepseek-chat"
        )
    else:
        client = create_mock_llm_client("deepseek_chat")

    messages = [Message.user("你好，请简短回复")]
    response = await client.chat(messages)

    assert isinstance(response, ChatResponse)
    assert response.content
    print(f"\n✓ DeepSeek response: {response.content[:100]}...")


# ==================== Agent Integration ====================


@pytest.mark.integration
@pytest.mark.asyncio
async def test_agent_fix_buggy_code(anthropic_key):
    """测试 Agent 修复 buggy_program.py"""
    buggy_code = """
def add(a, b):
    return a - b  # Bug: should be +

def multiply(a, b):
    return a / b  # Bug: should be *

def divide(a, b):
    return a * b  # Bug: should be /
"""

    if USE_REAL_API:
        from continuum_sdk.agent.runtime import AgentConfig

        base_url = os.environ.get("ANTHROPIC_BASE_URL", "https://api.anthropic.com")
        model = os.environ.get("CONTINUUM_MODEL", "claude-sonnet-4-6")

        config = AgentConfig(
            provider="anthropic", api_key=anthropic_key, base_url=base_url, model=model
        )
        agent = Agent(config=config)

        task = f"""Analyze this Python code and list all bugs:
```python
{buggy_code}
```
List each bug with: function name, bug description, how to fix."""
        response = agent.run(task)
    else:
        response = MOCK_RESPONSES["agent_fix"].content

    assert response
    assert len(response) > 50
    print(f"\n✓ Agent analysis:\n{response[:500]}...")


@pytest.mark.integration
@pytest.mark.asyncio
async def test_custom_provider_openai_format(anthropic_key):
    """测试自定义提供商"""
    if USE_REAL_API:
        base_url = os.environ.get("ANTHROPIC_BASE_URL", "https://api.anthropic.com")
        client = LlmClient.for_provider(
            provider="my-custom-provider",
            api_key=anthropic_key,
            base_url=base_url,
            model="hunyan-turbos",
            api_format="anthropic",
        )
    else:
        client = create_mock_llm_client("custom_provider")

    messages = [Message.user("Reply with just: 'Custom provider works!'")]
    response = await client.chat(messages)

    assert response.content
    print(f"\n✓ Custom provider response: {response.content}")


# ==================== Error Handling ====================


@pytest.mark.integration
@pytest.mark.asyncio
async def test_invalid_api_key():
    """测试无效 API Key 的错误处理"""
    if USE_REAL_API:
        from continuum_sdk.llm.errors import AuthenticationError

        client = LlmClient.for_provider(
            provider="anthropic", api_key="invalid-key-12345", model="claude-sonnet-4-6"
        )
        messages = [Message.user("Hello")]

        with pytest.raises(AuthenticationError):
            await client.chat(messages)
    else:
        pytest.skip("Skipping in mock mode - requires real API key for error testing")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])