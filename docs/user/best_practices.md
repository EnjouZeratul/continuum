# Continuum 最佳实践指南

> 适用版本: v1.0.0+
> 更新时间: 2026-05-30

---

## 目录

1. [Agent 设计原则](#agent-设计原则)
2. [工具开发最佳实践](#工具开发最佳实践)
3. [会话管理策略](#会话管理策略)
4. [记忆系统使用](#记忆系统使用)
5. [错误处理模式](#错误处理模式)
6. [性能优化建议](#性能优化建议)

---

## Agent 设计原则

### 1. 单一职责原则

每个 Agent 应专注于一个明确的任务领域：

```python
# Good: 专注单一领域
code_agent = Agent(
    name="code-assistant",
    specialty="code-review"
)

# Bad: 过于宽泛
generic_agent = Agent(
    name="do-everything",
    specialty="all"  # 难以优化
)
```

### 2. 配置优先级

遵循环境变量优先级规则：

```
CONTINUUM_* > PROVIDER_SPECIFIC_* > DEFAULT
```

```python
# 推荐：使用环境变量
import os
os.environ["CONTINUUM_API_KEY"] = "your-key"
agent = Agent()  # 自动加载

# 不推荐：硬编码
agent = Agent(api_key="hardcoded-key")  # 安全风险
```

### 3. 模型选择策略

根据任务复杂度选择合适模型：

| 任务类型 | 推荐模型 | 理由 |
|----------|----------|------|
| 简单问答 | claude-haiku-4-5 | 速度快、成本低 |
| 代码生成 | claude-sonnet-4-6 | 平衡性能与成本 |
| 复杂分析 | claude-opus-4-7 | 最高质量 |
| 批量处理 | claude-haiku-4-5 | 并行效率高 |

---

## 工具开发最佳实践

### 1. 参数验证

所有工具必须验证输入参数：

```python
@tool(name="safe_write", description="安全写入文件")
async def safe_write_file(path: str, content: str) -> str:
    # 验证路径
    if not path or ".." in path:
        raise ToolError("Invalid path")
    
    # 验证内容大小
    if len(content) > 10_000_000:  # 10MB
        raise ToolError("Content too large")
    
    # 执行操作
    return f"Written {len(content)} bytes to {path}"
```

### 2. 错误处理

使用结构化错误类型：

```python
from continuum_sdk.errors import ToolExecutionError

@tool(name="api_call", description="调用外部API")
async def call_api(url: str) -> str:
    try:
        response = await httpx.get(url)
        return response.text
    except httpx.TimeoutError:
        raise ToolExecutionError(
            tool_name="api_call",
            message=f"Timeout calling {url}",
            recoverable=True  # 可重试
        )
    except httpx.HTTPError as e:
        raise ToolExecutionError(
            tool_name="api_call",
            message=str(e),
            recoverable=False
        )
```

### 3. 危险操作标记

标记需要确认的危险操作：

```python
@tool(
    name="delete_files",
    description="删除文件",
    is_dangerous=True,
    requires_confirmation=True
)
async def delete_files(pattern: str) -> str:
    # 用户会收到确认提示
    return f"Deleted files matching {pattern}"
```

### 4. 资源清理

确保工具正确清理资源：

```python
@tool(name="process_file", description="处理文件")
async def process_file(path: str) -> str:
    f = open(path, "r")
    try:
        content = f.read()
        # 处理逻辑
        return result
    finally:
        f.close()  # 确保关闭
```

---

## 会话管理策略

### 1. 会话持久化

关键会话应及时保存：

```python
session = Session()

# 重要操作后立即保存
session.add_user_message("重要指令")
session.save("critical_session")

# 定期自动保存
session.enable_auto_save(interval=60)  # 每60秒
```

### 2. Checkpoint 使用

复杂任务使用 checkpoint 支持回滚：

```python
from continuum_sdk.agent import CheckpointClient

client = CheckpointClient()
session_id = "refactor-session"

# 任务开始前创建 checkpoint
state = {"messages": [...], "context": {...}}
checkpoint_id = client.save(session_id, state)

# 执行任务
result = agent.run("重构代码")

# 如果失败，加载 checkpoint
if not result.success:
    restored = client.load(session_id, checkpoint_id)
```

### 3. 会话命名规范

使用有意义的会话名称：

```python
# Good: 描述性命名
session.save("code-review-pr-123")

# Bad: 随机命名
session.save("session_abc123")
```

---

## 记忆系统使用

### 1. 分层存储策略

根据信息类型选择合适层级：

| 信息类型 | 存储层级 | 示例 |
|----------|----------|------|
| 事件记录 | episodic | "用户刚才说偏好暗色模式" |
| 知识事实 | semantic | "项目使用 Python 3.11" |
| 操作技能 | procedural | "测试命令: pytest tests/" |

```python
# Illustrative - requires session context
from continuum_sdk.api import MemorySystem

memory = MemorySystem(session_id="my-session")

# Working: 当前对话上下文
memory.store("working", "用户请求生成测试")

# Session: 会话级别事实
memory.store("session", "API版本: v2.0")

# Project: 项目知识
memory.store("project", "部署流程: git push && npm run deploy")
```

### 2. 记忆查询优化

使用精确查询减少噪音：

```python
# Good: 精确查询
results = memory.query(
    "如何运行测试?",
    limit=5,
    tier="session"
)

# Bad: 模糊查询
results = memory.query("测试")  # 可能返回太多结果
```

---

## 错误处理模式

### 1. 分层错误处理

根据错误类型采取不同策略：

```python
from continuum_sdk.errors import (
    ConfigError,
    ToolExecutionError,
    LLMError,
    NetworkError
)

try:
    result = agent.run("复杂任务")
except ConfigError:
    # 配置问题 - 提示用户检查
    print("请检查 API Key 配置")
except ToolExecutionError as e:
    if e.recoverable:
        # 可恢复 - 自动重试
        result = agent.run("复杂任务", retry=True)
    else:
        # 不可恢复 - 通知用户
        print(f"工具执行失败: {e.message}")
except LLMError:
    # LLM 问题 - 可能需要切换模型
    agent.switch_model("claude-opus-4-7")
except NetworkError:
    # 网络问题 - 等待后重试
    await asyncio.sleep(5)
    result = agent.run("复杂任务")
```

### 2. 重试策略

实现智能重试：

```python
async def run_with_retry(agent, task, max_retries=3):
    for attempt in range(max_retries):
        try:
            return agent.run(task)
        except RecoverableError as e:
            if attempt < max_retries - 1:
                await asyncio.sleep(2 ** attempt)  # 指数退避
                continue
            raise
```

---

## 性能优化建议

### 1. 并行执行

独立任务并行执行：

```python
import asyncio

# 并行执行多个独立任务
tasks = [
    agent.run("分析模块A"),
    agent.run("分析模块B"),
    agent.run("分析模块C"),
]

results = await asyncio.gather(*tasks)
```

### 2. 流式响应

使用流式响应减少等待：

```python
# 流式获取响应
async for chunk in agent.stream("生成长文档"):
    print(chunk, end="", flush=True)
```

### 3. 缓存利用

利用提示缓存降低成本：

```python
# 重复使用的系统提示会被缓存
agent = Agent(
    system_prompt="你是一个专业的代码审查助手..."  # 缓存命中率高
)
```

### 4. Token 管理

监控和管理 Token 使用：

```python
# 设置 Token 预算
agent.set_token_budget(max_input=100000, max_output=4000)

# 监控使用量
stats = agent.get_token_stats()
print(f"输入: {stats.input_tokens}, 输出: {stats.output_tokens}")
print(f"成本: ${stats.cost:.4f}")
```

---

## 安全最佳实践

### 1. API Key 管理

- 使用环境变量，不硬编码
- 使用 `${VAR}` 语法引用，不直接写入
- 定期轮换密钥

### 2. 输入验证

所有外部输入必须验证：

```python
from continuum_sdk.security import InputValidator

validator = InputValidator()

# 验证用户输入
clean_input = validator.validate(user_input)
```

### 3. PII 保护

敏感数据自动清洗：

```python
from continuum_sdk.security import PiiScrubber

scrubber = PiiScrubber()

# 清洗日志
safe_log = scrubber.scrub(log_content)
# "email: user@example.com" → "email: [REDACTED]"
```

---

## 参考资源

- [API 文档](../python/README.md)
- [工具使用指南](./tools_guide.md)
- [常见问题](./faq.md)
- [性能调优指南](./performance_guide.md)

---

*Continuum - 让 AI Agent 开发变得专业*