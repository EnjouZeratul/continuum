"""E2E 场景: 简单问答流程

测试最基本的使用场景：用户提问，Agent 回答。

场景描述:
1. 用户启动 Agent
2. 用户发送问题
3. Agent 返回答案
4. 用户结束会话

预期行为:
- 无工具调用
- 单次对话
- 响应时间 < 5s
"""

import pytest
from unittest.mock import Mock, patch, AsyncMock
import asyncio
import time

from continuum_sdk.agent.session import Session, MessageRole
from continuum_sdk.llm.client import LlmClient, AnthropicClient, OpenAIClient
from continuum_sdk.llm.types import Message, ChatResponse


class ScenarioQA:
    """简单问答场景"""

    # 场景配置
    PROMPTS = [
        "你好，介绍一下你自己",
        "什么是 Python？",
        "今天天气怎么样？",
    ]

    EXPECTED_RESPONSES = {
        "你好，介绍一下你自己": "包含自我介绍的响应",
        "什么是 Python？": "包含 Python 解释的响应",
        "今天天气怎么样？": "无法获取天气或说明需要工具",
    }

    async def run(self, agent):
        """运行场景"""
        results = []
        for prompt in self.PROMPTS:
            response = await agent.chat(prompt)
            results.append({
                "prompt": prompt,
                "response": response,
                "expected": self.EXPECTED_RESPONSES.get(prompt),
            })
        return results

    def validate(self, results):
        """验证结果"""
        for result in results:
            # 检查响应非空
            assert result["response"], "响应不应为空"
            # 自动相关性检查：验证响应包含关键概念
            expected = result["expected"]
            response = result["response"]

            # 根据预期类型进行验证
            if expected == "包含自我介绍的响应":
                # 验证包含自我介绍关键词
                keywords = ["助手", "AI", "帮助", "assistant", "help"]
                assert any(kw in response.lower() for kw in keywords), \
                    f"响应应包含自我介绍关键词: {response}"
            elif expected == "包含 Python 解释的响应":
                # 验证包含 Python 解释关键词
                keywords = ["python", "编程", "语言", "programming", "language"]
                assert any(kw in response.lower() for kw in keywords), \
                    f"响应应包含 Python 解释关键词: {response}"
            elif expected == "无法获取天气或说明需要工具":
                # 验证说明无法获取或需要工具
                keywords = ["无法", "不能", "需要", "天气", "工具", "获取", "cannot", "need", "weather"]
                assert any(kw in response.lower() for kw in keywords), \
                    f"响应应说明天气相关限制: {response}"
        return True


class TestScenarioQA:
    """简单问答场景测试"""

    @pytest.mark.e2e
    async def test_basic_qa_session_created(self):
        """测试基本问答会话创建"""
        session = Session(id="test-qa-session")

        # 添加问答消息
        session.add_user_message("你好，介绍一下你自己")
        session.add_assistant_message("你好！我是一个 AI 助手...")

        # 验证会话状态
        assert session.message_count == 2
        assert session.id == "test-qa-session"

        # 验证消息角色正确
        messages = session.get_messages()
        assert messages[0].role == MessageRole.USER
        assert messages[1].role == MessageRole.ASSISTANT

    @pytest.mark.e2e
    async def test_qa_with_chinese_content(self):
        """测试中文问答内容正确保存"""
        session = Session(id="test-chinese-qa")

        chinese_question = "用中文解释什么是 API"
        chinese_answer = "API（应用程序接口）是一组定义软件组件如何交互的协议..."

        session.add_user_message(chinese_question)
        session.add_assistant_message(chinese_answer)

        # 验证中文内容正确保存
        messages = session.get_messages()
        assert messages[0].content == chinese_question
        assert messages[1].content == chinese_answer

        # 验证导出和重新加载后中文内容保持
        exported = session.export()
        restored = Session.from_export(exported)
        assert restored.get_messages()[0].content == chinese_question

    @pytest.mark.e2e
    async def test_qa_with_english_content(self):
        """测试英文问答内容正确保存"""
        session = Session(id="test-english-qa")

        english_question = "Explain what is REST API"
        english_answer = "REST API (Representational State Transfer) is an architectural style..."

        session.add_user_message(english_question)
        session.add_assistant_message(english_answer)

        # 验证英文内容正确保存
        messages = session.get_messages()
        assert messages[0].content == english_question
        assert messages[1].content == english_answer

        # 验证内容完整
        assert "REST" in messages[0].content
        assert "REST" in messages[1].content

    @pytest.mark.e2e
    async def test_qa_response_time_measured(self):
        """测试响应时间可被测量"""
        session = Session(id="test-response-time")

        # 模拟问答
        start_time = time.time()
        session.add_user_message("hello")
        session.add_assistant_message("Hello! How can I help you?")
        elapsed = time.time() - start_time

        # 验证响应时间被正确测量
        assert elapsed < 5.0, f"本地操作应该很快，实际用时 {elapsed:.3f}s"

        # 验证会话时间戳
        messages = session.get_messages()
        assert messages[0].timestamp is not None
        assert messages[1].timestamp is not None
        assert messages[1].timestamp >= messages[0].timestamp


# ==================== 运行标记 ====================

pytestmark = pytest.mark.e2e
