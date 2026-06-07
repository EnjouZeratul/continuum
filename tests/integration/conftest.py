"""集成测试 fixtures 和配置

提供测试所需的共享配置和 mock 对象。
"""

import pytest
import asyncio
import tempfile
import os
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock


# ==================== 异步测试支持 ====================

@pytest.fixture(scope="session")
def event_loop():
    """创建 session 级别的 event loop"""
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


# ==================== 工作目录 ====================

@pytest.fixture
def temp_working_dir():
    """临时工作目录"""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def sample_project_dir(temp_working_dir):
    """包含示例文件的临时目录"""
    # 创建示例文件
    (temp_working_dir / "main.py").write_text("print('hello')")
    (temp_working_dir / "config.json").write_text("{\"name\": \"test\"}")
    (temp_working_dir / "README.md").write_text("# Test Project")
    yield temp_working_dir


# ==================== Mock Session ====================

@pytest.fixture
def mock_session_config():
    """Mock session 配置"""
    return {
        "name": "test-session",
        "working_dir": ".",
        "model": "claude-3-haiku",
        "max_tokens": 4096,
    }


@pytest.fixture
def mock_agent_config():
    """Mock agent 配置"""
    return {
        "tools_enabled": True,
        "memory_enabled": True,
        "auto_save": True,
    }


# ==================== Mock Responses ====================

@pytest.fixture
def mock_llm_response():
    """Mock LLM ChatResponse object"""
    from continuum_sdk.llm import ChatResponse, TokenUsage
    return ChatResponse(
        content="Test response from LLM",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=20),
    )


@pytest.fixture
def mock_llm_client(mock_llm_response):
    """Mock LLM client with pre-configured responses"""
    mock_client = MagicMock()
    mock_client.chat = AsyncMock(return_value=mock_llm_response)

    async def mock_stream(*args, **kwargs):
        """Mock streaming response"""
        from continuum_sdk.llm import StreamChunk
        chunks = ["Hello", " ", "world", "!"]
        for chunk in chunks:
            yield StreamChunk(content=chunk)

    mock_client.chat_stream = mock_stream
    return mock_client


@pytest.fixture
def mock_tool_result():
    """Mock 工具执行结果"""
    return {
        "success": True,
        "output": "工具执行成功",
        "error": None,
    }


# ==================== CLI fixtures ====================

@pytest.fixture
def cli_args_base():
    """CLI 基础参数"""
    return {
        "model": "claude-3-haiku",
        "no_tools": False,
        "verbose": False,
    }


@pytest.fixture
def cli_args_session():
    """CLI session 参数"""
    return {
        "name": "cli-test-session",
        "resume": None,
        "list": False,
    }


# ==================== 测试数据 ====================

@pytest.fixture
def sample_messages():
    """示例消息历史"""
    return [
        {"role": "user", "content": "你好"},
        {"role": "assistant", "content": "你好！有什么可以帮助你的？"},
        {"role": "user", "content": "请帮我写一个 Python 函数"},
        {"role": "assistant", "content": "好的，请告诉我你需要什么功能？"},
    ]


@pytest.fixture
def sample_tool_definitions():
    """示例工具定义"""
    return [
        {
            "name": "read_file",
            "description": "读取文件内容",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "文件路径"}
                },
                "required": ["path"]
            }
        },
        {
            "name": "write_file",
            "description": "写入文件内容",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }
        }
    ]


# ==================== 清理 ====================

@pytest.fixture(autouse=True)
def cleanup_test_sessions():
    """自动清理测试会话"""
    yield
    # 清理逻辑在测试后执行
    # 实际实现需要访问 SessionManager