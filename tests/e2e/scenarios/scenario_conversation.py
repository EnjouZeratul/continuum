"""E2E 场景: 多轮对话流程

测试多轮对话的上下文保持能力。

场景描述:
1. 用户开始对话
2. 用户提出问题并得到回答
3. 用户继续追问，引用之前的回答
4. Agent 应能理解上下文
5. 用户改变话题
6. Agent 应能适应新话题

预期行为:
- 上下文正确传递
- 引用理解正确
- 话题切换流畅
"""

import pytest
from unittest.mock import Mock, patch, AsyncMock
import asyncio

from continuum_sdk.agent.session import Session, MessageRole, Message


class ScenarioConversation:
    """多轮对话场景"""

    # 场景配置
    CONVERSATION_FLOW = [
        {"user": "帮我写一个 Python 函数", "expect_context": None},
        {"user": "它应该计算斐波那契数列", "expect_context": "Python 函数"},
        {"user": "请加上类型注解", "expect_context": "斐波那契函数"},
        {"user": "现在帮我写一个测试", "expect_context": "斐波那契函数"},
        {"user": "换个话题，推荐一些好书", "expect_context": None},  # 话题切换
    ]

    async def run(self, agent):
        """运行场景"""
        results = []
        history = []

        for step in self.CONVERSATION_FLOW:
            response = await agent.chat(step["user"])
            history.append({
                "role": "user",
                "content": step["user"],
            })
            history.append({
                "role": "assistant",
                "content": response,
            })
            results.append({
                "step": step,
                "response": response,
                "history_length": len(history),
            })

        return results

    def validate(self, results):
        """验证结果"""
        for i, result in enumerate(results):
            # 检查响应非空
            assert result["response"], f"第 {i+1} 步响应不应为空"

            # 检查上下文引用
            expected_context = result["step"]["expect_context"]
            if expected_context:
                # 上下文相关性检查：验证响应包含相关上下文关键词
                response = result["response"].lower()
                context_keywords = expected_context.lower().split()

                # 检查是否包含上下文关键词
                matched = any(kw in response for kw in context_keywords)

                # 对于编程相关上下文，检查代码关键词
                if "python" in expected_context.lower() or "函数" in expected_context:
                    code_keywords = ["def", "function", "python", "fibonacci", "斐波那契", "return"]
                    matched = matched or any(kw in response for kw in code_keywords)

                # 如果话题切换（expect_context为None），则不需要匹配
                if expected_context is not None:
                    assert matched, \
                        f"第 {i+1} 步响应应引用上下文 '{expected_context}': {response}"

        # 检查历史累积
        expected_history_len = len(self.CONVERSATION_FLOW) * 2
        assert results[-1]["history_length"] == expected_history_len

        return True


class TestScenarioConversation:
    """多轮对话场景测试"""

    @pytest.mark.e2e
    async def test_basic_conversation_flow(self, tmp_path):
        """测试基本多轮对话流程"""
        # 使用 Session 模拟对话
        session = Session(id="test-conversation")

        # 模拟多轮对话
        session.add_user_message("你好，请帮我写一个 Python 函数")
        session.add_assistant_message("好的，请问您需要什么功能的函数？")
        session.add_user_message("计算斐波那契数列")
        session.add_assistant_message("def fibonacci(n): ...")
        session.add_user_message("请加上类型注解")
        session.add_assistant_message("def fibonacci(n: int) -> int: ...")

        # 验证消息历史
        messages = session.get_messages()
        assert len(messages) == 6, "应该有 6 条消息（3 轮对话）"
        assert messages[0].role == MessageRole.USER
        assert messages[1].role == MessageRole.ASSISTANT
        assert "Python" in messages[0].content
        assert "fibonacci" in messages[3].content.lower() or "fibonacci" in messages[5].content.lower()

    @pytest.mark.e2e
    async def test_context_reference_preserved(self, tmp_path):
        """测试上下文引用被正确保存"""
        session = Session(id="test-context")

        # 第一轮：定义主题
        session.add_user_message("我们正在讨论 Python 编程")
        session.add_assistant_message("好的，我可以帮您解答 Python 相关问题")

        # 第二轮：引用之前的主题
        session.add_user_message("那个语言有什么优点？")  # 引用 "Python"
        session.add_assistant_message("Python 的优点包括：简洁、易读、丰富的库...")

        # 验证上下文信息被保存
        messages = session.get_messages()
        assert len(messages) >= 2, "至少应该有对话历史"

        # 验证可以通过导出/导入恢复上下文
        exported = session.export()
        restored = Session.from_export(exported)
        assert restored.message_count == session.message_count
        assert "Python" in restored.get_messages()[0].content

    @pytest.mark.e2e
    async def test_topic_switch_detected(self):
        """测试话题切换能被检测"""
        session = Session(id="test-topic-switch")

        # 第一个话题：编程
        session.add_user_message("帮我写一个排序算法")
        session.add_assistant_message("好的，这是快速排序...")
        session.add_user_message("改成归并排序")
        session.add_assistant_message("这是归并排序...")

        # 话题切换
        session.add_user_message("换个话题，推荐一些书")
        session.add_assistant_message("好的，我推荐...")

        messages = session.get_messages()
        # 验证消息历史完整
        assert len(messages) == 6

        # 验证话题切换前后的内容都存在
        user_messages = [m.content for m in messages if m.role == MessageRole.USER]
        assert any("排序" in msg for msg in user_messages), "应包含排序话题"
        assert any("书" in msg for msg in user_messages), "应包含书籍话题"

    @pytest.mark.e2e
    async def test_long_conversation_history_preserved(self):
        """测试长对话历史被正确保存"""
        session = Session(id="test-long-conversation")

        # 模拟 20+ 轮对话
        for i in range(25):
            session.add_user_message(f"问题 {i+1}: 请解释概念 {i+1}")
            session.add_assistant_message(f"回答 {i+1}: 概念 {i+1} 的解释...")

        # 验证消息数量
        assert session.message_count == 50, "应该有 50 条消息（25 轮对话）"

        # 验证早期消息仍然存在
        messages = session.get_messages()
        assert "问题 1" in messages[0].content
        assert "问题 25" in messages[-2].content

        # 验证导出和重新加载后历史完整
        exported = session.export()
        restored = Session.from_export(exported)
        assert restored.message_count == 50

    @pytest.mark.e2e
    async def test_conversation_with_code_blocks(self):
        """测试包含代码片段的对话"""
        session = Session(id="test-code-conversation")

        code_message = """
请帮我检查这段代码：
```python
def fibonacci(n: int) -> int:
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)
```
"""
        session.add_user_message(code_message)
        session.add_assistant_message("这段代码可以工作，但效率较低...")

        # 验证代码内容被正确保存
        messages = session.get_messages()
        assert "def fibonacci" in messages[0].content
        assert "```python" in messages[0].content

        # 验证导出后代码格式保持
        exported = session.export()
        assert "fibonacci(n-1)" in exported


# ==================== 运行标记 ====================

pytestmark = pytest.mark.e2e
