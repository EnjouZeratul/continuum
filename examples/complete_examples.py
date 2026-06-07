"""
Continuum 使用示例集

包含各种场景的完整代码示例。
"""

# ==================== 基础用法 ====================

# 示例1: 最简单的 Agent
from continuum_sdk import Agent

agent = Agent()
result = agent.run("解释什么是递归")
print(result)


# 示例2: 带配置的 Agent
agent = Agent(
    name="code-reviewer",
    model="claude-sonnet-4-6",
    provider="anthropic",
    system_prompt="你是一个专业的代码审查助手。输出简洁、具体的审查意见。"
)


# 示例3: 异步响应
async def async_example():
    agent = Agent()
    result = await agent.arun("讲一个短故事")
    print(result)


# ==================== 会话管理 ====================

# 示例4: 会话保存与恢复
from continuum_sdk import Session

session = Session()
session.add_user_message("帮我记住：项目名是 Continuum")
session.add_assistant_message("好的，我记住了项目名是 Continuum。")
session.save("project_context")

# 稍后恢复
session = Session.load("project_context")


# 示例5: Checkpoint 回滚
from continuum_sdk.agent import CheckpointClient

session = Session()
checkpoint_client = CheckpointClient()
checkpoint_id = checkpoint_client.save("my-session", {"state": "before-refactor"})

# 执行操作
session.add_user_message("重构这个函数")

# 如果需要回滚
restored = checkpoint_client.load("my-session", checkpoint_id)


# ==================== 自定义工具 ====================

# 示例6: 装饰器方式创建工具
from continuum_sdk.tools import tool

@tool(name="weather", description="获取城市天气")
async def get_weather(city: str) -> str:
    # 实际实现会调用天气API
    return f"{city} 天气: 晴，25°C"

# 自动注册


# 示例7: 类方式创建工具
from continuum_sdk.tools import CustomTool

class CalculatorTool(CustomTool):
    @property
    def name(self) -> str:
        return "calculator"

    @property
    def description(self) -> str:
        return "执行数学计算"

    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "数学表达式，如 '2+3*4'"
                }
            },
            "required": ["expression"]
        }

    async def execute(self, **kwargs) -> str:
        expression = kwargs["expression"]
        try:
            result = eval(expression)  # 注意: 生产环境需安全处理
            return str(result)
        except Exception as e:
            return f"错误: {e}"


# 示例8: 危险工具（需要确认）
@tool(
    name="delete_files",
    description="删除匹配的文件",
    is_dangerous=True,
    requires_confirmation=True
)
async def delete_files(pattern: str) -> str:
    import glob
    files = glob.glob(pattern)
    for f in files:
        os.remove(f)
    return f"已删除 {len(files)} 个文件"


# ==================== 记忆系统 ====================

# 示例9: 分层记忆
from continuum_sdk.api import MemorySystem

memory = MemorySystem(session_id="example-session")

# Working: 当前对话上下文
memory.store("working", "用户刚刚请求生成测试用例")

# Session: 会话级别事实
memory.store("session", "项目使用 pytest 作为测试框架")

# Project: 项目知识
memory.store("project", "运行测试: pytest tests/")

# 查询
results = memory.query("如何运行测试?", tier="project")
for entry in results:
    print(f"- {entry['content']}")


# 示例10: 记忆查询
results = memory.query("如何运行测试?", tier="project")
for entry in results:
    print(f"- {entry['content']}")


# ==================== 工作流 ====================

# 示例11: 简单工作流
from continuum_sdk.workflow import DAG, Node

dag = DAG(id="code-refactor-flow")

dag.add(Node("analyze", func=lambda: "分析代码库结构"))
dag.add(Node("refactor", func=lambda: "重构主模块").depends_on("analyze"))
dag.add(Node("test", func=lambda: "运行测试验证").depends_on("refactor"))

# 执行
async def run_workflow():
    result = await dag.execute()
    print(f"状态: {result.status.value}")
    print(f"执行顺序: {result.execution_order()}")


# 示例12: 并行步骤
dag = DAG(id="parallel-analysis")

# 并行分析多个模块
dag.add(Node("analyze-a", func=lambda: "模块A分析完成"))
dag.add(Node("analyze-b", func=lambda: "模块B分析完成"))
dag.add(Node("analyze-c", func=lambda: "模块C分析完成"))

# 汇总（依赖所有分析）
dag.add(Node("summarize", func=lambda: "汇总完成").depends_on("analyze-a", "analyze-b", "analyze-c"))


# ==================== MCP 集成 ====================

# 示例13: MCP 文件系统
from continuum_sdk.tools import create_mcp_registry

registry = create_mcp_registry(
    ["filesystem"],
    root_path="/path/to/project"
)

# 获取工具
tools = registry.get_tools()
for tool in tools:
    print(f"{tool.name}: {tool.description}")

# 执行
result = await registry.execute("filesystem/read_file", path="README.md")


# 示例14: MCP 多服务器
registry = create_mcp_registry(
    ["filesystem", "github"],
    root_path="/project",
    github_token="your-token"  # 可选
)


# ==================== 错误处理 ====================

# 示例15: 完整错误处理
from continuum_sdk.errors import (
    ContinuumError,
    ConfigError,
    ToolExecutionError,
    LLMError,
    NetworkError
)

async def robust_execution(agent, task: str):
    max_retries = 3

    for attempt in range(max_retries):
        try:
            return agent.run(task)

        except ConfigError as e:
            print(f"配置错误，请检查API Key: {e}")
            raise  # 不可恢复

        except NetworkError as e:
            if attempt < max_retries - 1:
                print(f"网络错误，重试 {attempt + 1}/{max_retries}")
                await asyncio.sleep(2 ** attempt)
                continue
            raise

        except ToolExecutionError as e:
            if e.recoverable and attempt < max_retries - 1:
                print(f"工具执行失败，重试中...")
                continue
            print(f"工具 {e.tool_name} 执行失败: {e.message}")
            raise

        except LLMError as e:
            if "token" in str(e).lower():
                print("Token 超限，尝试压缩上下文")
                # 可以尝试压缩或切换模型
            raise

        except ContinuumError as e:
            print(f"未知错误: {e}")
            raise


# ==================== 性能优化 ====================

# 示例16: 并行执行
import asyncio

async def parallel_tasks():
    agent1 = Agent()
    agent2 = Agent()
    agent3 = Agent()

    tasks = [
        agent1.arun("分析模块 A"),
        agent2.arun("分析模块 B"),
        agent3.arun("分析模块 C"),
    ]

    results = await asyncio.gather(*tasks)
    return results


# 示例17: 成本控制
agent = Agent(model="claude-haiku-4-5")  # 使用更便宜的模型

result = agent.run("复杂任务")
# 使用更便宜的模型可降低成本


# 示例18: 缓存利用
agent = Agent(
    model="claude-sonnet-4-6",
    enable_cache=True,
    system_prompt="固定的系统提示..."  # 会被缓存
)

# 多次调用相同提示会命中缓存
for i in range(10):
    agent.run(f"处理任务 {i}")


# ==================== 智能代理 ====================

# 示例19: 自主模式
from continuum_sdk.agent import IntelligentAgent, AgentMode

agent = IntelligentAgent(
    mode=AgentMode.AUTONOMOUS
)

plan = await agent.plan("重构认证模块，提高安全性")
print(plan.to_dict())

result = await agent.execute(plan)
print(f"完成 {result.completed_steps}/{result.total_steps}")


# 示例20: 交互模式
agent = IntelligentAgent(
    mode=AgentMode.INTERACTIVE
)

# 每一步都会询问确认
async for step in agent.run_interactive("部署到生产环境"):
    print(f"步骤: {step.description}")
    confirm = input("继续? (y/n): ")
    if confirm.lower() != 'y':
        break


if __name__ == "__main__":
    import asyncio
    asyncio.run(async_example())
