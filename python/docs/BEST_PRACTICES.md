# Continuum SDK Best Practices

This guide covers best practices for using Continuum SDK effectively in your applications.

## Table of Contents

- [Configuration](#configuration)
- [Agent Usage](#agent-usage)
- [Tool Registration](#tool-registration)
- [Session Management](#session-management)
- [Error Handling](#error-handling)
- [Memory Management](#memory-management)
- [Security](#security)
- [Performance](#performance)

---

## Configuration

### Use Environment Variables

Store API keys in environment variables, never in code:

```python
# Good / 好
import os
from continuum import Config

config = Config(
    provider="anthropic",
    api_key=os.environ.get("ANTHROPIC_API_KEY")
)

# Bad - hardcoded keys / 坏 - 硬编码密钥
config = Config(
    provider="anthropic",
    api_key="sk-ant-api03-..."  # Never do this! / 永远不要这样做!
)
```

### Use Configuration Files

For complex configurations, use TOML files:

```python
from continuum import load_config

config = load_config("config.toml")
```

`config.toml`:
```toml
[agent]
provider = "anthropic"
model = "claude-sonnet-4-6"
max_tokens = 4096
temperature = 0.7

[security]
base_dir = "/workspace"
allowed_extensions = [".py", ".md", ".txt"]
```

### Provider Selection

Choose providers based on your needs:

| Use Case | Recommended Provider | Reason |
|----------|---------------------|--------|
| Code generation | Anthropic Claude | Best code understanding |
| Fast responses | Groq | Lowest latency |
| Cost-effective | DeepSeek | Best value |
| Chinese language | GLM, KIMI | Optimized for Chinese |

---

## Agent Usage

### Streaming for Long Responses

Use async for better UX:

```python
from continuum import Agent

agent = Agent()

# Good - async execution / 好 - 异步执行
result = await agent.arun("Explain quantum computing")

# Good for short responses (sync) / 好 - 短响应（同步）
result = agent.run("What is 2+2?")
```

### Reuse Agent Instances

Create one agent and reuse it:

```python
# Good / 好
agent = Agent()
result1 = agent.run("Task 1")
result2 = agent.run("Task 2")

# Bad - creates new connection each time / 坏 - 每次创建新连接
result1 = Agent().run("Task 1")
result2 = Agent().run("Task 2")
```

### Use Context Managers

For resource cleanup:

```python
from continuum import Agent

async with Agent() as agent:
    result = await agent.arun("Hello")
# Automatic cleanup / 自动清理
```

---

## Tool Registration

### Validate Inputs

Always validate tool inputs:

```python
import ast
from continuum import Agent

def safe_calculate(expression: str) -> float:
    """Safely evaluate math expressions. / 安全计算数学表达式。"""
    try:
        # Use ast.literal_eval for safety / 使用 ast.literal_eval 保证安全
        return ast.literal_eval(expression)
    except (ValueError, SyntaxError):
        raise ValueError(f"Invalid expression: {expression}")

agent = Agent()
agent.register_tool(
    "calculator",
    safe_calculate,
    description="Evaluate safe math expressions",
    parameters={
        "type": "object",
        "properties": {
            "expression": {
                "type": "string",
                "description": "Math expression to evaluate"
            }
        },
        "required": ["expression"]
    }
)
```

### Descriptive Tool Names

Use clear, descriptive names:

```python
# Good / 好
agent.register_tool("search_documentation", ...)
agent.register_tool("get_weather_data", ...)

# Bad / 坏
agent.register_tool("search", ...)
agent.register_tool("get", ...)
```

### Tool Error Handling

Handle errors gracefully:

```python
def robust_tool(input_data: str) -> str:
    try:
        result = process(input_data)
        return result
    except ConnectionError:
        return "Error: Unable to connect to service"
    except ValueError as e:
        return f"Error: Invalid input - {e}"
```

---

## Session Management

### Save Important Sessions

```python
from continuum import Agent

agent = Agent()
session = agent.create_session()

# After important work / 重要工作后
session_id = session.save()
print(f"Session saved: {session_id}")
```

### Recovery Pattern

```python
from continuum import Session, Agent

def continue_conversation(session_path: str):
    try:
        session = Session.load(session_path)
        return session
    except FileNotFoundError:
        print("Session not found, creating new")
        return Agent().create_session()
```

---

## Error Handling

### Catch Specific Exceptions

```python
from continuum_sdk import RateLimitError, AuthenticationError, ConfigError

async def safe_chat(agent, prompt):
    try:
        return await agent.arun(prompt)
    except RateLimitError as e:
        await asyncio.sleep(e.retry_after)
        return await agent.arun(prompt)
    except AuthenticationError:
        raise ConfigError("Check your API key")
    except Exception as e:
        logger.error(f"LLM error: {e}")
        raise
```

### Retry with Exponential Backoff

```python
import asyncio
from continuum_sdk import RateLimitError

async def with_retry(agent, prompt, max_retries=3):
    for attempt in range(max_retries):
        try:
            return await agent.arun(prompt)
        except RateLimitError as e:
            if attempt == max_retries - 1:
                raise
            backoff = 2 ** attempt
            await asyncio.sleep(backoff)
```

---

## Memory Management

### Use Appropriate Tiers

```python
from continuum_sdk.memory import Memory, MemoryTier

memory = Memory(session_id="session-123")

# Working memory - current task / 工作记忆 - 当前任务
memory.remember("current file: main.py", tier=MemoryTier.WORKING)

# Session memory - conversation context / 会话记忆 - 对话上下文
memory.remember("user prefers dark mode", tier=MemoryTier.SESSION)

# Project memory - long-term project info / 项目记忆 - 长期项目信息
memory.remember("database: PostgreSQL", tier=MemoryTier.LONG_TERM)
```

### Regular Cleanup

```python
from continuum_sdk.memory import Memory, MemoryTier

memory = Memory(session_id="cleanup-session")

# Periodic cleanup / 定期清理
def cleanup_session():
    stats = memory.stats()
    if stats.get(MemoryTier.WORKING, 0) > 100:
        memory.clear(MemoryTier.WORKING)
```

---

## Security

### Validate Paths

```python
from continuum_sdk.security import PathValidator, SecurityError

validator = PathValidator(project_root="/workspace")

def safe_read(path: str) -> str:
    if not validator.is_valid(path):
        raise SecurityError(f"Access denied: {path}")
    return open(path).read()
```

### Check Permissions

```python
from continuum_sdk.security import PermissionChecker, Permission

checker = PermissionChecker()

def safe_write(path: str, content: str):
    if not checker.can_write(path):
        raise PermissionError(f"Cannot write to: {path}")
    # Proceed with write / 继续写入
```

### Log Operations

```python
from continuum_sdk.security import AuditLogger, AuditOperation, AuditResult

audit = AuditLogger("audit.json")

def logged_operation(path: str, operation: str):
    audit.log(
        operation=AuditOperation.WRITE,
        path=path,
        result=AuditResult.SUCCESS,
        user="agent",
        metadata={"timestamp": datetime.now().isoformat()}
    )
```

---

## Performance

### Batch Operations

```python
# Good - batch / 好 - 批量
prompts = ["Task 1", "Task 2", "Task 3"]
results = await asyncio.gather(*[
    agent.arun(prompt) for prompt in prompts
])

# Bad - sequential / 坏 - 顺序
results = []
for prompt in prompts:
    results.append(await agent.arun(prompt))
```

### Use Connection Pooling

```python
from continuum import Agent

# Single agent instance per process / 每个进程一个 Agent 实例
agent = Agent()

# Reuse for multiple requests / 复用于多个请求
async def handle_request(prompt: str):
    return await agent.arun(prompt)
```

### Cache Configurations

```python
from functools import lru_cache
from continuum import Config

@lru_cache(maxsize=1)
def get_config() -> Config:
    return Config.from_env()

# Subsequent calls return cached config / 后续调用返回缓存的配置
config = get_config()
```

---

## Testing

### Mock LLM Responses

```python
import pytest
from unittest.mock import AsyncMock, patch
from continuum import Agent

@pytest.mark.asyncio
async def test_agent_response():
    agent = Agent()
    
    with patch.object(agent._client, "chat", 
                      new_callable=AsyncMock) as mock_chat:
        mock_chat.return_value = "Test response"
        
        result = await agent.arun("Hello")
        assert result == "Test response"
```

### Test Error Handling

```python
import pytest
from unittest.mock import patch
from continuum_sdk import RateLimitError
from continuum import Agent

@pytest.mark.asyncio
async def test_rate_limit_handling():
    agent = Agent()
    
    with patch.object(agent._client, "chat") as mock_chat:
        mock_chat.side_effect = [
            RateLimitError(retry_after=1),
            "Success"
        ]
        
        result = await agent.arun("Test")
        assert result == "Success"
        assert mock_chat.call_count == 2
```

---

## Summary

1. **Configuration**: Use environment variables and config files
2. **Agent**: Stream for long responses, reuse instances
3. **Tools**: Validate inputs, handle errors gracefully
4. **Security**: Always validate paths and log operations
5. **Performance**: Batch operations, cache configurations
6. **Testing**: Mock external dependencies, test error paths

Following these practices will help you build robust, secure, and efficient applications with Continuum SDK.
