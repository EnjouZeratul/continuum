"""E2E 场景: 工具调用流程

测试 Agent 的工具调用能力。

场景描述:
1. 用户请求执行需要工具的操作
2. Agent 调用相应工具
3. 工具返回结果
4. Agent 根据结果回答用户

预期行为:
- 工具正确识别和调用
- 参数正确传递
- 结果正确处理
- 错误正确处理
"""

import pytest
from unittest.mock import Mock, patch
import asyncio
from pathlib import Path

from continuum_sdk.tools import (
    ReadTool,
    WriteTool,
    BashTool,
    ToolRegistry,
    CustomTool,
    tool,
    ToolResult,
    ToolError,
)
from continuum_sdk.tools.file_ops import read_file, write_file, edit_file


class ScenarioToolCalling:
    """工具调用场景"""

    # 场景配置
    TOOL_SCENARIOS = [
        {
            "prompt": "读取 README.md 文件",
            "expected_tool": "read_file",
            "expected_params": {"path": "README.md"},
        },
        {
            "prompt": "创建一个 test.txt 文件，内容是 hello world",
            "expected_tool": "write_file",
            "expected_params": {"path": "test.txt", "content": "hello world"},
        },
        {
            "prompt": "列出当前目录的文件",
            "expected_tool": "list_directory",
            "expected_params": {"path": "."},
        },
        {
            "prompt": "搜索所有 .py 文件",
            "expected_tool": "glob",
            "expected_params": {"pattern": "**/*.py"},
        },
        {
            "prompt": "运行 ls 命令",
            "expected_tool": "bash",
            "expected_params": {"command": "ls"},
        },
    ]

    async def run(self, agent, working_dir):
        """运行场景"""
        results = []

        for scenario in self.TOOL_SCENARIOS:
            response = await agent.chat(scenario["prompt"])
            results.append({
                "scenario": scenario,
                "response": response,
            })

        return results

    def validate(self, results):
        """验证结果"""
        for result in results:
            # 检查响应非空
            assert result["response"], "响应不应为空"

            # 工具调用验证：检查响应包含预期的工具结果特征
            scenario = result["scenario"]
            response = result["response"].lower()
            expected_tool = scenario["expected_tool"]

            # 根据工具类型验证响应特征
            if expected_tool == "read_file":
                # 读文件工具响应应包含文件内容或读取确认
                assert any(kw in response for kw in ["content", "内容", "read", "读取", "file", "文件"]), \
                    f"响应应反映读文件结果: {response}"

            elif expected_tool == "write_file":
                # 写文件工具响应应确认创建成功
                assert any(kw in response for kw in ["created", "written", "创建", "写入", "saved", "保存"]), \
                    f"响应应反映写文件结果: {response}"

            elif expected_tool == "list_directory":
                # 列目录工具响应应包含文件列表特征
                assert any(kw in response for kw in ["file", "files", "目录", "directory", "list", "列表"]), \
                    f"响应应反映目录列表结果: {response}"

            elif expected_tool == "glob":
                # glob 工具响应应包含搜索结果特征
                assert any(kw in response for kw in ["found", "搜索", "match", "匹配", "file", "py", ".py"]), \
                    f"响应应反映搜索结果: {response}"

            elif expected_tool == "bash":
                # bash 工具响应应包含命令执行结果
                assert any(kw in response for kw in ["executed", "output", "执行", "输出", "command", "命令"]), \
                    f"响应应反映命令执行结果: {response}"

        return True


class TestScenarioToolCalling:
    """工具调用场景测试"""

    @pytest.mark.e2e
    async def test_read_file_tool_returns_content(self, tmp_path):
        """测试读文件工具返回内容"""
        # 创建测试文件
        test_file = tmp_path / "test_read.txt"
        test_file.write_text("Hello, World!", encoding="utf-8")

        # 使用 ReadTool 读取
        reader = ReadTool()
        result = reader.read(str(test_file))

        # 验证结果
        assert result.is_error is False
        assert result.content == "Hello, World!"
        assert result.name == "read"
        assert result.metadata["path"] == str(test_file)

    @pytest.mark.e2e
    async def test_write_file_tool_creates_file(self, tmp_path):
        """测试写文件工具创建文件"""
        test_file = tmp_path / "test_write.txt"

        # 使用 WriteTool 写入
        writer = WriteTool(backup=False)
        result = writer.write(str(test_file), "Test content from tool")

        # 验证文件已创建
        assert test_file.exists()
        assert test_file.read_text(encoding="utf-8").strip() == "Test content from tool"

        # 验证工具结果
        assert result.is_error is False
        assert "Successfully wrote" in result.content

    @pytest.mark.e2e
    async def test_bash_tool_executes_command(self, tmp_path):
        """测试 Bash 工具执行命令"""
        from continuum_sdk.tools.bash import BashTool

        bash = BashTool(default_working_dir=str(tmp_path))

        # 执行简单命令
        result = bash.run("echo 'Hello from bash'")

        # 验证执行结果
        assert result.is_error is False
        assert "Hello from bash" in result.content

    @pytest.mark.e2e
    async def test_tool_chain_sequential_execution(self, tmp_path):
        """测试工具链顺序执行"""
        # 第一步：写入文件
        writer = WriteTool(backup=False)
        test_file = tmp_path / "chain_test.txt"
        write_result = writer.write(str(test_file), "Initial content")

        assert write_result.is_error is False, "写入应该成功"

        # 第二步：读取文件
        reader = ReadTool()
        read_result = reader.read(str(test_file))

        assert read_result.is_error is False
        assert read_result.content == "Initial content"

        # 第三步：编辑文件
        edit_result = edit_file(str(test_file), "Initial", "Modified")

        assert edit_result.is_error is False
        assert edit_result.metadata["replacements"] == 1

        # 第四步：验证修改
        final_content = test_file.read_text(encoding="utf-8")
        assert "Modified content" in final_content

    @pytest.mark.e2e
    async def test_tool_error_handling_returns_error(self, tmp_path):
        """测试工具错误处理"""
        reader = ReadTool()

        # 尝试读取不存在的文件
        with pytest.raises(ToolError) as exc_info:
            reader.read(str(tmp_path / "nonexistent.txt"))

        # 验证错误信息
        assert "not found" in str(exc_info.value).lower() or "File not found" in str(exc_info.value)

    @pytest.mark.e2e
    async def test_tool_with_confirmation_flag(self):
        """测试需要确认的工具标记"""
        # 创建一个需要确认的自定义工具
        @tool(
            name="dangerous_delete",
            description="Delete files",
            requires_confirmation=True,
            is_dangerous=True,
        )
        async def delete_files(path: str) -> str:
            return f"Deleted {path}"

        # 验证工具属性
        assert delete_files.name == "dangerous_delete"
        assert delete_files.requires_confirmation is True
        assert delete_files.is_dangerous is True
        assert delete_files.description == "Delete files"

    @pytest.mark.e2e
    async def test_custom_tool_registration_and_execution(self):
        """测试自定义工具注册和执行"""
        registry = ToolRegistry()

        # 创建自定义工具
        @tool(name="add_numbers", description="Add two numbers")
        async def add_numbers(a: int, b: int) -> str:
            return str(a + b)

        # 注册工具
        registry.register(add_numbers)

        # 验证注册成功
        assert registry.has_tool("add_numbers")
        assert "add_numbers" in registry.list_names()

        # 执行工具
        result = await registry.execute("add_numbers", a=5, b=3)
        assert result == "8"

        # 获取工具元数据
        meta = registry.get_meta("add_numbers")
        assert meta["name"] == "add_numbers"
        assert meta["description"] == "Add two numbers"
        assert "properties" in meta["parameters"]


# ==================== 运行标记 ====================

pytestmark = pytest.mark.e2e
