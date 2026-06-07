"""Agent 性能基准测试"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import statistics
import time
from collections.abc import Callable


# 简单基准测试框架（避免pytest-benchmark依赖）
class BenchResult:
    def __init__(self, name: str):
        self.name = name
        self.times: list[float] = []
        self.result = None

    def add(self, elapsed: float):
        self.times.append(elapsed)

    @property
    def mean(self) -> float:
        return statistics.mean(self.times) if self.times else 0

    @property
    def median(self) -> float:
        return statistics.median(self.times) if self.times else 0

    @property
    def stdev(self) -> float:
        return statistics.stdev(self.times) if len(self.times) > 1 else 0

    @property
    def min(self) -> float:
        return min(self.times) if self.times else 0

    @property
    def max(self) -> float:
        return max(self.times) if self.times else 0


def bench(func: Callable, iterations: int = 100) -> BenchResult:
    """运行基准测试"""
    result = BenchResult(func.__name__)

    # 预热
    func()

    # 正式测试
    for _ in range(iterations):
        start = time.perf_counter()
        func()
        elapsed = time.perf_counter() - start
        result.add(elapsed)

    return result


def format_result(result: BenchResult) -> str:
    """格式化结果"""
    return f"""
{result.name}:
  Iterations: {len(result.times)}
  Mean:   {result.mean*1000:.4f} ms
  Median: {result.median*1000:.4f} ms
  StdDev: {result.stdev*1000:.4f} ms
  Min:    {result.min*1000:.4f} ms
  Max:    {result.max*1000:.4f} ms
"""


# ========================================
# Agent 基准测试
# ========================================


def bench_agent_creation():
    """测试 Agent 创建时间"""
    from continuum_sdk import Agent

    return Agent()


def bench_agent_start():
    """测试 Agent 启动时间"""
    from continuum_sdk import Agent

    agent = Agent()
    agent.start()
    return agent


def bench_agent_run():
    """测试 Agent run 方法时间"""
    from continuum_sdk import Agent

    agent = Agent()
    agent.start()
    return agent.run("test task", auto_start=False)


def bench_agent_chat():
    """测试 Agent chat 方法时间"""
    from continuum_sdk import Agent

    agent = Agent()
    agent.start()
    return agent.chat("hello")


def bench_agent_with_config():
    """测试带配置的 Agent 创建时间"""
    from continuum_sdk import Agent, Config

    config = Config(provider="anthropic", model="claude-sonnet-4-6")
    return Agent(config=config)


def bench_agent_create_session():
    """测试会话创建时间"""
    from continuum_sdk import Agent

    agent = Agent()
    return agent.create_session()


def bench_agent_register_tool():
    """测试工具注册时间"""
    from continuum_sdk import Agent

    agent = Agent()
    for i in range(10):
        agent.register_tool(f"tool_{i}", lambda x: x)
    return agent


def run_agent_benchmarks():
    """运行所有 Agent 基准测试"""
    print("=" * 50)
    print("Agent Performance Benchmarks")
    print("=" * 50)

    tests = [
        ("Agent Creation", bench_agent_creation, 1000),
        ("Agent with Config", bench_agent_with_config, 1000),
        ("Agent Start", bench_agent_start, 100),
        ("Agent Run", bench_agent_run, 100),
        ("Agent Chat", bench_agent_chat, 100),
        ("Create Session", bench_agent_create_session, 1000),
        ("Register 10 Tools", bench_agent_register_tool, 100),
    ]

    results = []
    for name, func, iterations in tests:
        print(f"\nRunning: {name}...")
        func.__name__ = name
        result = bench(func, iterations)
        results.append(result)
        print(format_result(result))

    return results


if __name__ == "__main__":
    run_agent_benchmarks()
