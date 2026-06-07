# Continuum 性能调优指南

> 适用版本: v1.0.0+
> 更新时间: 2026-05-30

---

## 目录

1. [性能基准](#性能基准)
2. [Token 优化](#token-优化)
3. [并发与并行](#并发与并行)
4. [内存管理](#内存管理)
5. [缓存策略](#缓存策略)
6. [监控与调试](#监控与调试)

---

## 性能基准

### 硬件推荐配置

| 场景 | CPU | 内存 | 存储 |
|------|-----|------|------|
| 开发测试 | 4核 | 8GB | SSD |
| 生产环境 | 8核+ | 16GB+ | NVMe SSD |
| 高并发 | 16核+ | 32GB+ | NVMe SSD |

### 性能指标

| 操作 | 目标延迟 | 说明 |
|------|----------|------|
| Agent 响应首字 | <500ms | 流式首字节 |
| 工具执行 | <1s | 本地工具 |
| 会话保存 | <100ms | 持久化 |
| 记忆查询 | <50ms | 向量检索 |

---

## Token 优化

### 1. 模型选择优化

根据任务选择合适模型：

```python
# 简单任务用 Haiku
simple_agent = Agent(model="claude-haiku-4-5")

# 复杂任务用 Sonnet/Opus
complex_agent = Agent(model="claude-sonnet-4-6")
```

**成本对比** (每百万 Token):

| 模型 | 输入成本 | 输出成本 |
|------|----------|----------|
| Haiku | $0.25 | $1.25 |
| Sonnet | $3.00 | $15.00 |
| Opus | $15.00 | $75.00 |

### 2. 上下文压缩

自动压缩长对话：

```python
from continuum_sdk.context import ContextCompressor

compressor = ContextCompressor()

# 压缩历史消息
compressed = compressor.compress(
    messages,
    strategy="summary",  # 或 "sliding_window"
    max_tokens=10000
)
```

### 3. 系统提示优化

简洁的系统提示减少 Token：

```python
# Good: 简洁有效
system_prompt = "你是代码审查助手。输出简洁的审查意见。"

# Bad: 冗长描述
system_prompt = """你是一个非常专业且经验丰富的代码审查助手，
你的主要职责是帮助开发者审查代码，找出潜在问题..."""  # 太长
```

### 4. 缓存利用

利用提示缓存降低成本：

```python
# 可缓存的系统提示放在前面
agent = Agent(
    system_prompt="固定的系统提示...",  # 会被缓存
)

# 缓存命中时成本降低 90%
```

---

## 并发与并行

### 1. 异步执行

使用 async/await 提高并发：

```python
import asyncio

async def process_multiple_files(files):
    tasks = [process_file(f) for f in files]
    results = await asyncio.gather(*tasks)
    return results
```

### 2. 并行任务执行

独立任务并行执行：

```python
from continuum_sdk.agent import IntelligentAgent, AgentMode

agent = IntelligentAgent(mode=AgentMode.AUTONOMOUS)

# 并行执行多个独立任务
plan = await agent.plan_batch([
    "分析模块A",
    "分析模块B", 
    "分析模块C"
])

results = await agent.execute_batch(plan)
```

### 3. 连接池

复用 HTTP 连接：

```python
from continuum_sdk.llm import LLMClient

# 配置连接池
client = LLMClient(
    max_connections=10,
    keepalive_timeout=30
)
```

---

## 内存管理

### 1. 会话内存优化

大型会话及时清理：

```python
session = Session()

# 定期清理旧消息
session.prune_messages(keep_last=50)

# 或设置自动清理
session.enable_auto_prune(max_messages=100)
```

### 2. 记忆存储优化

分层存储减少内存占用：

```python
memory = MemorySystem(session_id="perf-session")

# 工作记忆 - 内存中
memory.store("working", "临时信息")

# 长期记忆 - 磁盘持久化
memory.store("project", "重要信息")
```

### 3. 大文件处理

流式处理大文件：

```python
# 不要一次性加载
content = tools.read_file("large_file.txt")  # 可能OOM

# 使用分页读取
for chunk in tools.read_file_chunks("large_file.txt", chunk_size=1000):
    process(chunk)
```

---

## 缓存策略

### 1. 提示缓存

Anthropic 支持提示缓存：

```python
agent = Agent(
    model="claude-sonnet-4-6",
    enable_cache=True  # 自动缓存系统提示
)

# 查看缓存命中率
stats = agent.get_cache_stats()
print(f"缓存命中率: {stats.hit_rate:.1%}")
```

### 2. 提示缓存

利用 API 提示缓存降低延迟和成本：

```python
# Anthropic 提示缓存（需要系统提示固定）
agent = Agent(
    model="claude-sonnet-4-6",
    system_prompt="固定的系统提示..."  # 会被自动缓存
)

# 相同系统提示的后续请求会命中缓存
result1 = agent.run("任务1")
result2 = agent.run("任务2")  # 系统提示使用缓存
```

提示缓存优势：
- 首字节延迟降低 80%+
- Token 成本降低 90%

### 3. 向量检索优化

使用合适的距离度量和索引：

```python
from continuum_sdk.rag import InMemoryVectorStore, DistanceMetric

# 根据数据特性选择距离度量
store = InMemoryVectorStore(metric=DistanceMetric.COSINE)  # 文本常用
# store = InMemoryVectorStore(metric=DistanceMetric.EUCLIDEAN)  # 图像常用
```
```

---

## 监控与调试

### 1. 性能监控

启用性能追踪：

```python
from continuum_sdk.observability import enable_tracing

enable_tracing()

agent = Agent()
result = agent.run("任务")

# 查看性能报告
report = agent.get_performance_report()
print(report)
# 输出:
# Total time: 2.5s
# - LLM call: 1.8s (72%)
# - Tool execution: 0.5s (20%)
# - Overhead: 0.2s (8%)
```

### 2. Token 统计

实时监控 Token 使用：

```python
agent = Agent()

# 启用统计
agent.enable_token_tracking()

# 执行任务
result = agent.run("任务")

# 查看统计
stats = agent.get_token_stats()
print(f"输入: {stats.input_tokens}")
print(f"输出: {stats.output_tokens}")
print(f"成本: ${stats.estimated_cost:.4f}")
```

### 3. 慢查询分析

识别性能瓶颈：

```python
from continuum_sdk.debug import slow_query_log

# 记录超过阈值的操作
slow_query_log(threshold_ms=1000)

# 查看慢查询
logs = get_slow_queries()
for log in logs:
    print(f"{log.operation}: {log.duration}ms")
```

### 4. 内存分析

检测内存泄漏：

```python
from continuum_sdk.debug import memory_profiler

memory_profiler.start()

# 执行操作
for i in range(1000):
    agent.run(f"任务 {i}")

# 分析内存使用
report = memory_profiler.get_report()
print(f"峰值内存: {report.peak_memory}MB")
print(f"内存泄漏: {report.leaks}")
```

---

## 性能调优清单

### 启动前检查

- [ ] 选择合适模型
- [ ] 配置连接池
- [ ] 启用提示缓存
- [ ] 设置 Token 预算

### 运行时监控

- [ ] Token 使用统计
- [ ] 响应延迟监控
- [ ] 内存使用监控
- [ ] 缓存命中率

### 优化后验证

- [ ] 延迟是否降低
- [ ] 成本是否降低
- [ ] 内存是否稳定
- [ ] 吞吐是否提升

---

## 参考资源

- [最佳实践指南](./best_practices.md)
- [故障排除手册](./troubleshooting.md)
- [API 文档](../python/README.md)

---

*Continuum - 让性能优化变得简单*