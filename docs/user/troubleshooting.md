# Continuum 故障排除手册

> 适用版本: v1.0.0+
> 更新时间: 2026-05-30

---

## 目录

1. [常见错误](#常见错误)
2. [配置问题](#配置问题)
3. [连接问题](#连接问题)
4. [工具执行问题](#工具执行问题)
5. [性能问题](#性能问题)
6. [调试技巧](#调试技巧)

---

## 常见错误

### 错误代码速查表

| 错误码 | 类型 | 说明 | 解决方案 |
|--------|------|------|----------|
| E001 | Config | API Key 未配置 | 配置环境变量 |
| E002 | Config | 模型不支持 | 检查模型名称 |
| E003 | Network | 连接超时 | 检查网络/代理 |
| E004 | Network | SSL 错误 | 检查证书配置 |
| E005 | Tool | 工具不存在 | 检查工具名称 |
| E006 | Tool | 参数错误 | 验证参数格式 |
| E007 | LLM | Token 超限 | 增加预算/压缩上下文 |
| E008 | LLM | 内容过滤 | 检查提示内容 |
| E009 | Memory | 存储失败 | 检查磁盘空间 |
| E010 | Session | 会话损坏 | 恢复 checkpoint |

---

## 配置问题

### 1. API Key 未配置

**症状**: `ConfigError: API key not found`

**诊断**:
```bash
# 检查环境变量
echo $ANTHROPIC_API_KEY
echo $CONTINUUM_API_KEY
```

**解决方案**:
```bash
# 方式1: 设置环境变量
export ANTHROPIC_API_KEY=your-key-here

# 方式2: 使用配置文件
continuum config init
# 编辑 ~/.continuum/config.toml
```

### 2. 配置文件路径错误

**症状**: `ConfigError: Config file not found`

**诊断**:
```bash
# 检查配置文件位置
ls ~/.continuum/config.toml
ls ~/.sh/config.toml  # 旧版路径
```

**解决方案**:
```bash
# 初始化新配置
continuum config init

# 或手动创建
mkdir -p ~/.continuum
cp config.example.toml ~/.continuum/config.toml
```

### 3. 模型名称错误

**症状**: `LLMError: Model not found`

**诊断**:
```python
from continuum_sdk import Config
config = Config.from_env()
print(config.model)  # 检查当前模型
```

**解决方案**:
```bash
# 使用正确模型名
export CONTINUUM_MODEL=claude-sonnet-4-6

# 或在配置文件中修改
[providers.anthropic]
model = "claude-sonnet-4-6"
```

**支持模型列表**:
- `claude-opus-4-7`
- `claude-sonnet-4-6`
- `claude-haiku-4-5`

---

## 连接问题

### 1. 连接超时

**症状**: `NetworkError: Connection timeout`

**诊断**:
```bash
# 测试 API 连通性
curl -v https://api.anthropic.com/v1/messages

# 检查代理设置
echo $HTTP_PROXY
echo $HTTPS_PROXY
```

**解决方案**:
```bash
# 方式1: 增加超时时间
export CONTINUUM_TIMEOUT=60

# 方式2: 配置代理
export HTTPS_PROXY=http://proxy.example.com:8080

# 方式3: 使用自定义 URL
export CONTINUUM_BASE_URL=https://your-proxy.example.com
```

### 2. SSL 证书错误

**症状**: `NetworkError: SSL certificate verify failed`

**诊断**:
```bash
# 检查证书
openssl s_client -connect api.anthropic.com:443
```

**解决方案**:
```bash
# 方式1: 更新证书
pip install --upgrade certifi

# 方式2: 禁用验证 (仅测试环境)
export CONTINUUM_SSL_VERIFY=false
```

### 3. DNS 解析失败

**症状**: `NetworkError: DNS resolution failed`

**诊断**:
```bash
# 检查 DNS
nslookup api.anthropic.com
```

**解决方案**:
```bash
# 配置 DNS 或使用 IP
export CONTINUUM_BASE_URL=https://直接IP地址
```

---

## 工具执行问题

### 1. 工具不存在

**症状**: `ToolExecutionError: Tool 'xxx' not found`

**诊断**:
```python
from continuum_sdk.tools import get_registry
registry = get_registry()
print(registry.list_names())  # 列出所有可用工具
```

**解决方案**:
```python
# 检查工具名称拼写
# 内置工具: read_file, write_file, grep, glob, bash...

# 注册自定义工具
from continuum_sdk.tools import register_tool
register_tool(MyCustomTool())
```

### 2. 参数格式错误

**症状**: `ToolExecutionError: Invalid parameters`

**诊断**:
```python
# 查看工具参数 schema
tool = registry.get("read_file")
print(tool.parameters_schema())
```

**解决方案**:
```python
# 使用正确参数格式
result = tools.read_file(
    path="file.txt",  # 字符串路径
    offset=0,         # 整数偏移
    limit=100         # 整数限制
)
```

### 3. 权限不足

**症状**: `ToolExecutionError: Permission denied`

**诊断**:
```bash
# 检查文件权限
ls -la /path/to/file

# 检查用户权限
whoami
```

**解决方案**:
```bash
# 修改文件权限
chmod 644 /path/to/file

# 或使用允许的目录
cd /allowed/directory
```

### 4. 工具超时

**症状**: `ToolExecutionError: Timeout`

**解决方案**:
```python
# 增加超时时间
result = tools.bash(
    command="long-running-command",
    timeout_ms=60000  # 60秒
)
```

---

## 性能问题

### 1. 响应缓慢

**症状**: Agent 响应时间超过 10 秒

**诊断**:
```python
# 启用性能追踪
from continuum_sdk.observability import enable_tracing
enable_tracing()

agent = Agent()
result = agent.run("任务")

report = agent.get_performance_report()
print(report)
```

**解决方案**:
- 使用更快模型 (Haiku)
- 增加并发执行
- 压缩上下文历史
- 启用提示缓存

### 2. 内存占用高

**症状**: 内存超过 1GB

**诊断**:
```python
from continuum_sdk.debug import memory_profiler
memory_profiler.start()
# 执行操作
report = memory_profiler.get_report()
print(f"内存使用: {report.current_memory}MB")
```

**解决方案**:
```python
# 清理会话历史
session.prune_messages(keep_last=20)

# 禁用不必要的功能
agent = Agent(
    enable_memory=False,  # 禁用记忆
    enable_history=False  # 禁用历史
)
```

### 3. Token 超限

**症状**: `LLMError: Token limit exceeded`

**诊断**:
```python
stats = agent.get_token_stats()
print(f"输入: {stats.input_tokens}, 限制: {stats.max_input}")
```

**解决方案**:
```python
# 增加预算
agent.set_token_budget(max_input=200000)

# 或压缩上下文
from continuum_sdk.context import ContextCompressor
compressed = ContextCompressor().compress(messages)
```

---

## 调试技巧

### 1. 启用详细日志

```python
import logging

# 启用 DEBUG 级别日志
logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger("continuum_sdk")
logger.setLevel(logging.DEBUG)
```

### 2. 请求/响应日志

```python
# 启用请求追踪
agent = Agent(enable_request_logging=True)

# 查看请求详情
logs = agent.get_request_logs()
for log in logs:
    print(f"Request: {log.request}")
    print(f"Response: {log.response}")
```

### 3. 交互式调试

```python
# 使用调试模式
agent = Agent(debug_mode=True)

# 单步执行
agent.run_step("步骤1")
agent.run_step("步骤2")

# 查看当前状态
print(agent.get_state())
```

### 4. 错误堆栈分析

```python
import traceback

try:
    result = agent.run("任务")
except Exception as e:
    traceback.print_exc()
    
    # 获取详细错误信息
    from continuum_sdk.errors import get_error_details
    details = get_error_details(e)
    print(details)
```

---

## 常见问题 Q&A

### Q: 如何重置配置？

```bash
continuum config reset
```

### Q: 如何查看版本信息？

```bash
continuum --version
python -c "import continuum_sdk; print(continuum_sdk.__version__)"
```

### Q: 如何导出调试信息？

```python
from continuum_sdk.debug import export_debug_info
export_debug_info("debug_report.json")
```

### Q: 如何回滚会话？

```python
session = Session.load("backup_session")
# 或使用 checkpoint
session.restore(last_checkpoint)
```

---

## 获取帮助

如果以上方案未能解决问题：

1. 查看 [GitHub Issues](https://github.com/anthropics/continuum/issues)
2. 提交新 Issue 并附上:
   - 错误信息完整输出
   - 配置文件内容 (移除敏感信息)
   - 操作步骤复述
3. 加入社区讨论

---

## 参考资源

- [最佳实践指南](./best_practices.md)
- [性能调优指南](./performance_guide.md)
- [常见问题](./faq.md)

---

*Continuum - 让问题解决变得简单*