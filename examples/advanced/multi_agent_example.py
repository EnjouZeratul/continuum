"""
多 Agent 协作示例

本示例展示如何使用 Continuum SDK 构建多 Agent 系统：
- Agent 专业化分工
- Agent 间通信与协作
- 任务路由与分发
- 结果汇总

运行方式:
    python multi_agent_example.py

依赖:
    pip install continuum-agent-sdk
"""

import asyncio
from dataclasses import dataclass
from enum import Enum
from typing import Optional, List, Dict, Any


class AgentRole(Enum):
    """Agent 角色"""
    COORDINATOR = "coordinator"   # 协调者
    RESEARCHER = "researcher"      # 研究员
    CODER = "coder"               # 程序员
    REVIEWER = "reviewer"         # 审核员
    WRITER = "writer"             # 写作者


@dataclass
class AgentMessage:
    """Agent 间消息"""
    from_agent: str
    to_agent: str
    content: str
    task_id: str
    metadata: Optional[Dict[str, Any]] = None


class Agent:
    """简化 Agent 类"""

    def __init__(self, name: str, role: AgentRole, capabilities: List[str]):
        self.name = name
        self.role = role
        self.capabilities = capabilities
        self.inbox: List[AgentMessage] = []
        self.outbox: List[AgentMessage] = []

    def can_handle(self, task_type: str) -> bool:
        """检查是否能处理某类任务"""
        return task_type in self.capabilities

    def receive(self, message: AgentMessage):
        """接收消息"""
        self.inbox.append(message)

    def send(self, to_agent: str, content: str, task_id: str):
        """发送消息"""
        self.outbox.append(AgentMessage(
            from_agent=self.name,
            to_agent=to_agent,
            content=content,
            task_id=task_id,
        ))

    async def process(self) -> Optional[str]:
        """处理收到的消息"""
        if not self.inbox:
            return None

        message = self.inbox.pop(0)

        # 根据角色处理
        if self.role == AgentRole.RESEARCHER:
            return f"[{self.name}] 研究结果: {message.content} 的分析完成"

        elif self.role == AgentRole.CODER:
            return f"[{self.name}] 代码实现:\n```python\n# {message.content}\npass\n```"

        elif self.role == AgentRole.REVIEWER:
            return f"[{self.name}] 审核意见: {message.content} - 需要改进"

        elif self.role == AgentRole.WRITER:
            return f"[{self.name}] 文档: {message.content} 的说明已生成"

        return f"[{self.name}] 已处理: {message.content}"


class MultiAgentSystem:
    """多 Agent 系统"""

    def __init__(self):
        self.agents: Dict[str, Agent] = {}
        self.task_counter = 0

    def register_agent(self, agent: Agent):
        """注册 Agent"""
        self.agents[agent.name] = agent
        print(f"✓ Agent '{agent.name}' 已注册 (角色: {agent.role.value})")

    def route_task(self, task_type: str, content: str) -> Optional[Agent]:
        """路由任务到合适的 Agent"""
        for agent in self.agents.values():
            if agent.can_handle(task_type):
                return agent
        return None

    async def broadcast(self, from_agent: str, content: str, task_id: str):
        """广播消息"""
        for name, agent in self.agents.items():
            if name != from_agent:
                agent.receive(AgentMessage(
                    from_agent=from_agent,
                    to_agent=name,
                    content=content,
                    task_id=task_id,
                ))

    async def run_task(self, task_type: str, content: str) -> Dict[str, Any]:
        """运行任务"""
        self.task_counter += 1
        task_id = f"task_{self.task_counter}"

        print(f"\n--- 任务 {task_id}: {task_type} ---")

        # 找到合适的 Agent
        agent = self.route_task(task_type, content)
        if not agent:
            print(f"  ✗ 无法找到能处理 '{task_type}' 的 Agent")
            return {"task_id": task_id, "status": "failed", "reason": "no_agent"}

        # 发送任务
        agent.receive(AgentMessage(
            from_agent="system",
            to_agent=agent.name,
            content=content,
            task_id=task_id,
        ))

        # 处理任务
        result = await agent.process()
        print(f"  → {result}")

        return {"task_id": task_id, "status": "completed", "result": result}

    async def collaborate(self, task: str) -> Dict[str, Any]:
        """多 Agent 协作"""
        self.task_counter += 1
        task_id = f"collab_{self.task_counter}"

        print(f"\n=== 协作任务 {task_id}: {task} ===")

        results = []

        # 1. 研究员分析
        if "researcher" in self.agents:
            researcher = self.agents["researcher"]
            researcher.receive(AgentMessage(
                from_agent="coordinator",
                to_agent="researcher",
                content=task,
                task_id=task_id,
            ))
            result = await researcher.process()
            results.append(result)
            print(f"  [1/4] 研究: 完成")

        # 2. 程序员实现
        if "coder" in self.agents:
            coder = self.agents["coder"]
            coder.receive(AgentMessage(
                from_agent="researcher",
                to_agent="coder",
                content=task,
                task_id=task_id,
            ))
            result = await coder.process()
            results.append(result)
            print(f"  [2/4] 编码: 完成")

        # 3. 审核员检查
        if "reviewer" in self.agents:
            reviewer = self.agents["reviewer"]
            reviewer.receive(AgentMessage(
                from_agent="coder",
                to_agent="reviewer",
                content=task,
                task_id=task_id,
            ))
            result = await reviewer.process()
            results.append(result)
            print(f"  [3/4] 审核: 完成")

        # 4. 写作者文档
        if "writer" in self.agents:
            writer = self.agents["writer"]
            writer.receive(AgentMessage(
                from_agent="reviewer",
                to_agent="writer",
                content=task,
                task_id=task_id,
            ))
            result = await writer.process()
            results.append(result)
            print(f"  [4/4] 文档: 完成")

        return {"task_id": task_id, "status": "completed", "results": results}


async def basic_multi_agent():
    """基础多 Agent 示例"""
    print("=== 基础多 Agent 示例 ===\n")

    # 创建系统
    system = MultiAgentSystem()

    # 注册专业化 Agent
    system.register_agent(Agent(
        name="researcher",
        role=AgentRole.RESEARCHER,
        capabilities=["research", "analyze", "investigate"],
    ))

    system.register_agent(Agent(
        name="coder",
        role=AgentRole.CODER,
        capabilities=["code", "implement", "develop"],
    ))

    system.register_agent(Agent(
        name="reviewer",
        role=AgentRole.REVIEWER,
        capabilities=["review", "audit", "check"],
    ))

    # 运行任务
    await system.run_task("research", "分析 Python 异步编程模式")
    await system.run_task("code", "实现一个简单的异步任务队列")
    await system.run_task("review", "审核代码质量和性能")


async def collaborative_workflow():
    """协作工作流示例"""
    print("\n=== 协作工作流示例 ===\n")

    system = MultiAgentSystem()

    # 注册完整的协作团队
    system.register_agent(Agent(
        name="researcher",
        role=AgentRole.RESEARCHER,
        capabilities=["research", "analyze"],
    ))

    system.register_agent(Agent(
        name="coder",
        role=AgentRole.CODER,
        capabilities=["code", "implement"],
    ))

    system.register_agent(Agent(
        name="reviewer",
        role=AgentRole.REVIEWER,
        capabilities=["review", "audit"],
    ))

    system.register_agent(Agent(
        name="writer",
        role=AgentRole.WRITER,
        capabilities=["write", "document"],
    ))

    # 运行协作任务
    result = await system.collaborate("实现一个 REST API 服务")

    print(f"\n协作完成:")
    print(f"  任务数: {len(result['results'])}")
    print(f"  状态: {result['status']}")


async def task_routing():
    """任务路由示例"""
    print("\n=== 任务路由示例 ===\n")

    system = MultiAgentSystem()

    # 注册不同能力的 Agent
    system.register_agent(Agent(
        name="python_expert",
        role=AgentRole.CODER,
        capabilities=["python", "data_analysis"],
    ))

    system.register_agent(Agent(
        name="frontend_expert",
        role=AgentRole.CODER,
        capabilities=["javascript", "css", "html"],
    ))

    system.register_agent(Agent(
        name="devops_expert",
        role=AgentRole.CODER,
        capabilities=["docker", "kubernetes", "ci_cd"],
    ))

    # 路由任务
    await system.run_task("python", "数据分析脚本")
    await system.run_task("javascript", "前端组件")
    await system.run_task("docker", "容器化部署")


async def main():
    """运行所有示例"""
    await basic_multi_agent()
    await collaborative_workflow()
    await task_routing()
    print("\n✓ 所有多 Agent 示例完成")


if __name__ == "__main__":
    asyncio.run(main())
