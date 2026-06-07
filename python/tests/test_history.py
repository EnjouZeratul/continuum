"""
Comprehensive tests for the history browser module.

history 模块的完整测试覆盖。
"""

import json
import tempfile
from datetime import datetime, timedelta
from pathlib import Path

import pytest

from continuum_sdk.agent.history import (
    HistoryBrowser,
    HistoryFilter,
    HistoryStatistics,
    SearchResult,
    SortOrder,
    browse_session,
    browse_session_file,
)
from continuum_sdk.agent.session import Message, MessageRole, Session


class TestHistoryFilter:
    """Tests for HistoryFilter dataclass.

    HistoryFilter 数据类测试。
    """

    def test_default_values(self):
        """Test that all default values are None or False.

        测试所有默认值为 None 或 False。
        """
        filter_obj = HistoryFilter()
        assert filter_obj.role is None
        assert filter_obj.start_time is None
        assert filter_obj.end_time is None
        assert filter_obj.keyword is None
        assert filter_obj.case_sensitive is False
        assert filter_obj.use_regex is False
        assert filter_obj.limit is None
        assert filter_obj.offset == 0

    def test_custom_values(self):
        """Test setting custom filter values.

        测试设置自定义过滤值。
        """
        now = datetime.now()
        filter_obj = HistoryFilter(
            role=MessageRole.USER,
            start_time=now - timedelta(hours=1),
            end_time=now,
            keyword="test",
            case_sensitive=True,
            use_regex=True,
            limit=10,
            offset=5,
        )
        assert filter_obj.role == MessageRole.USER
        assert filter_obj.case_sensitive is True
        assert filter_obj.use_regex is True
        assert filter_obj.limit == 10
        assert filter_obj.offset == 5


class TestHistoryStatistics:
    """Tests for HistoryStatistics dataclass.

    HistoryStatistics 数据类测试。
    """

    def test_default_values(self):
        """Test default statistics are zero/empty.

        测试默认统计值为零或空。
        """
        stats = HistoryStatistics()
        assert stats.total_messages == 0
        assert stats.user_messages == 0
        assert stats.assistant_messages == 0
        assert stats.system_messages == 0
        assert stats.tool_messages == 0
        assert stats.first_message_time is None
        assert stats.last_message_time is None
        assert stats.average_message_length == 0.0
        assert stats.total_characters == 0
        assert stats.session_duration_seconds == 0.0

    def test_custom_values(self):
        """Test setting custom statistics values.

        测试设置自定义统计值。
        """
        now = datetime.now()
        stats = HistoryStatistics(
            total_messages=100,
            user_messages=50,
            assistant_messages=40,
            system_messages=5,
            tool_messages=5,
            first_message_time=now - timedelta(hours=1),
            last_message_time=now,
            average_message_length=150.5,
            total_characters=15050,
            session_duration_seconds=3600.0,
        )
        assert stats.total_messages == 100
        assert stats.session_duration_seconds == 3600.0


class TestSearchResult:
    """Tests for SearchResult dataclass.

    SearchResult 数据类测试。
    """

    def test_search_result_creation(self):
        """Test creating a search result.

        测试创建搜索结果。
        """
        msg = Message(role=MessageRole.USER, content="Hello World")
        result = SearchResult(
            message=msg,
            index=0,
            match_position=0,
            match_length=5,
            matched_text="Hello",
        )
        assert result.message == msg
        assert result.index == 0
        assert result.match_position == 0
        assert result.match_length == 5
        assert result.matched_text == "Hello"


class TestSortOrder:
    """Tests for SortOrder enum.

    SortOrder 枚举测试。
    """

    def test_enum_values(self):
        """Test enum values are correct.

        测试枚举值正确。
        """
        assert SortOrder.ASCENDING.value == "asc"
        assert SortOrder.DESCENDING.value == "desc"

    def test_enum_members(self):
        """Test enum member count.

        测试枚举成员数量。
        """
        members = list(SortOrder)
        assert len(members) == 2


class TestHistoryBrowser:
    """Tests for HistoryBrowser class.

    HistoryBrowser 类测试。
    """

    @pytest.fixture
    def session(self):
        """Create a session with messages for testing.

        创建包含消息的会话用于测试。
        """
        session = Session(id="test-session")
        session.add_system_message("You are a helpful assistant.")
        session.add_user_message("What is Python?")
        session.add_assistant_message("Python is a programming language.")
        session.add_user_message("What about JavaScript?")
        session.add_assistant_message("JavaScript is also a programming language.")
        # Add tool message using add_message with TOOL role
        session.add_message(MessageRole.TOOL, '{"tool": "search", "result": "found"}')
        return session

    @pytest.fixture
    def browser(self, session):
        """Create a history browser for testing.

        创建历史浏览器用于测试。
        """
        return HistoryBrowser(session)

    def test_init(self, session):
        """Test browser initialization.

        测试浏览器初始化。
        """
        browser = HistoryBrowser(session)
        assert browser._session == session
        assert browser._messages == []

    def test_refresh(self, browser):
        """Test refreshing messages from session.

        测试从会话刷新消息。
        """
        assert browser._messages == []
        browser.refresh()
        assert len(browser._messages) == 6

    def test_ensure_messages(self, browser):
        """Test lazy loading of messages.

        测试消息的延迟加载。
        """
        # First call should load messages
        messages = browser._ensure_messages()
        assert len(messages) == 6

        # Second call should return same cached messages
        messages2 = browser._ensure_messages()
        assert messages is messages2

    def test_get_all_ascending(self, browser):
        """Test getting all messages in ascending order.

        测试按升序获取所有消息。
        """
        messages = browser.get_all(order=SortOrder.ASCENDING)
        assert len(messages) == 6
        assert messages[0].role == MessageRole.SYSTEM
        assert messages[-1].role == MessageRole.TOOL

    def test_get_all_descending(self, browser):
        """Test getting all messages in descending order.

        测试按降序获取所有消息。
        """
        messages = browser.get_all(order=SortOrder.DESCENDING)
        assert len(messages) == 6
        assert messages[0].role == MessageRole.TOOL
        assert messages[-1].role == MessageRole.SYSTEM

    def test_get_all_returns_copy(self, browser):
        """Test that get_all returns a copy, not the original.

        测试 get_all 返回副本而非原始列表。
        """
        messages = browser.get_all()
        messages.clear()
        # Original should still have messages
        assert len(browser._ensure_messages()) == 6

    def test_get_recent(self, browser):
        """Test getting recent messages.

        测试获取最近的消息。
        """
        recent = browser.get_recent(limit=2)
        assert len(recent) == 2
        # Most recent first
        assert recent[0].role == MessageRole.TOOL
        assert recent[1].role == MessageRole.ASSISTANT

    def test_get_recent_default_limit(self, browser):
        """Test get_recent default limit is 10.

        测试 get_recent 默认限制为 10。
        """
        recent = browser.get_recent()
        assert len(recent) == 6  # We only have 6 messages

    def test_get_range(self, browser):
        """Test getting messages in time range.

        测试获取时间范围内的消息。
        """
        now = datetime.now()
        start = now - timedelta(hours=1)
        end = now + timedelta(hours=1)
        messages = browser.get_range(start, end)
        assert len(messages) == 6

    def test_get_range_no_messages(self, browser):
        """Test time range with no messages.

        测试无消息的时间范围。
        """
        now = datetime.now()
        start = now - timedelta(days=10)
        end = now - timedelta(days=9)
        messages = browser.get_range(start, end)
        assert len(messages) == 0

    def test_get_last_hours(self, browser):
        """Test getting messages from last N hours.

        测试获取过去 N 小时的消息。
        """
        messages = browser.get_last_hours(hours=1)
        assert len(messages) == 6

    def test_get_last_days(self, browser):
        """Test getting messages from last N days.

        测试获取过去 N 天的消息。
        """
        messages = browser.get_last_days(days=1)
        assert len(messages) == 6

    def test_filter_by_role_enum(self, browser):
        """Test filtering by role using MessageRole enum.

        测试使用 MessageRole 枚举按角色过滤。
        """
        user_messages = browser.filter_by_role(MessageRole.USER)
        assert len(user_messages) == 2
        for msg in user_messages:
            assert msg.role == MessageRole.USER

    def test_filter_by_role_string(self, browser):
        """Test filtering by role using string.

        测试使用字符串按角色过滤。
        """
        assistant_messages = browser.filter_by_role("assistant")
        assert len(assistant_messages) == 2
        for msg in assistant_messages:
            assert msg.role == MessageRole.ASSISTANT

    def test_filter_by_role_with_limit(self, browser):
        """Test filtering by role with limit.

        测试带限制的角色过滤。
        """
        user_messages = browser.filter_by_role(MessageRole.USER, limit=1)
        assert len(user_messages) == 1

    def test_search_case_sensitive(self, browser):
        """Test case-sensitive keyword search.

        测试区分大小写的关键字搜索。
        """
        results = browser.search("Python", case_sensitive=True)
        assert len(results) == 2
        for result in results:
            assert "Python" in result.matched_text

    def test_search_case_insensitive(self, browser):
        """Test case-insensitive keyword search.

        测试不区分大小写的关键字搜索。
        """
        results = browser.search("python", case_sensitive=False)
        assert len(results) == 2

    def test_search_with_limit(self, browser):
        """Test search with result limit.

        测试带结果限制的搜索。
        """
        results = browser.search("language", case_sensitive=False, limit=1)
        assert len(results) == 1

    def test_search_no_match(self, browser):
        """Test search with no matches.

        测试无匹配结果的搜索。
        """
        results = browser.search("nonexistent_keyword_xyz", case_sensitive=True)
        assert len(results) == 0

    def test_search_regex(self, browser):
        """Test regex pattern search.

        测试正则表达式搜索。
        """
        results = browser.search_regex(r"What \w+ \w+\?")
        assert len(results) == 2

    def test_search_regex_with_limit(self, browser):
        """Test regex search with limit.

        测试带限制的正则搜索。
        """
        results = browser.search_regex(r"What", limit=1)
        assert len(results) == 1

    def test_apply_filter_role(self, browser):
        """Test applying filter with role.

        测试应用带角色的过滤。
        """
        filter_obj = HistoryFilter(role=MessageRole.USER)
        messages = browser.apply_filter(filter_obj)
        assert len(messages) == 2
        for msg in messages:
            assert msg.role == MessageRole.USER

    def test_apply_filter_time_range(self, browser):
        """Test applying filter with time range.

        测试应用带时间范围的过滤。
        """
        now = datetime.now()
        filter_obj = HistoryFilter(
            start_time=now - timedelta(hours=1),
            end_time=now + timedelta(hours=1),
        )
        messages = browser.apply_filter(filter_obj)
        assert len(messages) == 6

    def test_apply_filter_keyword(self, browser):
        """Test applying filter with keyword.

        测试应用带关键字的过滤。
        """
        filter_obj = HistoryFilter(keyword="Python")
        messages = browser.apply_filter(filter_obj)
        assert len(messages) == 2

    def test_apply_filter_regex(self, browser):
        """Test applying filter with regex.

        测试应用带正则的过滤。
        """
        filter_obj = HistoryFilter(keyword=r"What \w+", use_regex=True)
        messages = browser.apply_filter(filter_obj)
        assert len(messages) == 2

    def test_apply_filter_pagination(self, browser):
        """Test applying filter with pagination.

        测试应用带分页的过滤。
        """
        filter_obj = HistoryFilter(limit=2, offset=1)
        messages = browser.apply_filter(filter_obj)
        assert len(messages) == 2

    def test_apply_filter_combined(self, browser):
        """Test applying combined filters.

        测试应用组合过滤。
        """
        now = datetime.now()
        filter_obj = HistoryFilter(
            role=MessageRole.USER,
            start_time=now - timedelta(hours=1),
            end_time=now + timedelta(hours=1),
            keyword="Python",
            case_sensitive=True,
        )
        messages = browser.apply_filter(filter_obj)
        assert len(messages) == 1

    def test_iterate(self, browser):
        """Test iterating over messages.

        测试迭代消息。
        """
        count = 0
        for _msg in browser.iterate():
            count += 1
        assert count == 6

    def test_iterate_descending(self, browser):
        """Test iterating in descending order.

        测试降序迭代。
        """
        messages = list(browser.iterate(order=SortOrder.DESCENDING))
        assert messages[0].role == MessageRole.TOOL
        assert messages[-1].role == MessageRole.SYSTEM

    def test_get_statistics(self, browser):
        """Test getting history statistics.

        测试获取历史统计。
        """
        stats = browser.get_statistics()
        assert stats.total_messages == 6
        assert stats.user_messages == 2
        assert stats.assistant_messages == 2
        assert stats.system_messages == 1
        assert stats.tool_messages == 1
        assert stats.first_message_time is not None
        assert stats.last_message_time is not None
        assert stats.average_message_length > 0
        assert stats.total_characters > 0

    def test_get_statistics_empty(self):
        """Test statistics for empty session.

        测试空会话的统计。
        """
        empty_session = Session(id="empty")
        browser = HistoryBrowser(empty_session)
        stats = browser.get_statistics()
        assert stats.total_messages == 0
        assert stats.session_duration_seconds == 0.0

    def test_get_message_at_valid(self, browser):
        """Test getting message at valid index.

        测试获取有效索引的消息。
        """
        msg = browser.get_message_at(0)
        assert msg is not None
        assert msg.role == MessageRole.SYSTEM

    def test_get_message_at_invalid(self, browser):
        """Test getting message at invalid index.

        测试获取无效索引的消息。
        """
        assert browser.get_message_at(-1) is None
        assert browser.get_message_at(100) is None

    def test_get_context_around(self, browser):
        """Test getting context around a message.

        测试获取消息周围的上下文。
        """
        # Get context around index 2 (first assistant message)
        context = browser.get_context_around(2, before=1, after=1)
        assert len(context) == 3
        assert context[0].role == MessageRole.USER  # Before
        assert context[1].role == MessageRole.ASSISTANT  # Center
        assert context[2].role == MessageRole.USER  # After

    def test_get_context_around_boundaries(self, browser):
        """Test context around at boundaries.

        测试边界处的上下文。
        """
        # First message
        context = browser.get_context_around(0, before=5, after=1)
        assert len(context) == 2  # Can't go before 0

        # Last message
        context = browser.get_context_around(5, before=1, after=5)
        assert len(context) == 2  # Can't go past last

    def test_export_json(self, browser):
        """Test exporting to JSON file.

        测试导出到 JSON 文件。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "history.json"
            browser.export_json(path)

            assert path.exists()

            with open(path, encoding="utf-8") as f:
                data = json.load(f)

            assert data["session_id"] == "test-session"
            assert data["message_count"] == 6
            assert "messages" in data
            assert len(data["messages"]) == 6
            assert "statistics" in data

    def test_export_json_creates_directories(self, browser):
        """Test that export creates parent directories.

        测试导出时创建父目录。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "nested" / "dirs" / "history.json"
            browser.export_json(path)

            assert path.exists()
            assert path.parent.exists()

    def test_export_markdown(self, browser):
        """Test exporting to Markdown file.

        测试导出到 Markdown 文件。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "history.md"
            browser.export_markdown(path)

            assert path.exists()

            content = path.read_text(encoding="utf-8")
            assert "# Session History: test-session" in content
            assert "[USER]" in content
            assert "[ASSISTANT]" in content
            assert "[SYSTEM]" in content
            assert "[TOOL]" in content

    def test_export_text(self, browser):
        """Test exporting to plain text file.

        测试导出到纯文本文件。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "history.txt"
            browser.export_text(path)

            assert path.exists()

            content = path.read_text(encoding="utf-8")
            assert "[user]" in content
            assert "[assistant]" in content

    def test_export_unicode(self, browser):
        """Test exporting content with Unicode characters.

        测试导出包含 Unicode 字符的内容。
        """
        # Add a message with Unicode
        browser._session.add_user_message("Hello 世界! 🌍")

        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "unicode.json"
            browser.export_json(path)

            content = path.read_text(encoding="utf-8")
            assert "世界" in content


class TestBrowseSessionFunctions:
    """Tests for convenience functions.

    便捷函数测试。
    """

    def test_browse_session(self):
        """Test browse_session function.

        测试 browse_session 函数。
        """
        session = Session(id="test")
        session.add_user_message("Hello")

        browser = browse_session(session)
        assert isinstance(browser, HistoryBrowser)
        assert browser._session == session

    def test_browse_session_file(self):
        """Test browse_session_file function.

        测试 browse_session_file 函数。
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "session.json"

            # Create and save a session
            session = Session(id="file-test")
            session.add_user_message("Test message")
            session.save(path)

            # Browse from file
            browser = browse_session_file(path)
            assert isinstance(browser, HistoryBrowser)
            assert browser._session.id == "file-test"

            messages = browser.get_all()
            assert len(messages) == 1


class TestHistoryBrowserEdgeCases:
    """Edge case tests for HistoryBrowser.

    HistoryBrowser 边界情况测试。
    """

    def test_empty_session(self):
        """Test browser with empty session.

        测试空会话的浏览器。
        """
        session = Session(id="empty")
        browser = HistoryBrowser(session)

        assert len(browser.get_all()) == 0
        assert len(browser.get_recent()) == 0
        assert browser.get_statistics().total_messages == 0
        assert browser.get_message_at(0) is None

    def test_single_message(self):
        """Test browser with single message.

        测试单条消息的浏览器。
        """
        session = Session(id="single")
        session.add_user_message("Only message")
        browser = HistoryBrowser(session)

        recent = browser.get_recent(limit=10)
        assert len(recent) == 1
        assert recent[0].content == "Only message"

    def test_search_empty_content(self):
        """Test searching in messages with empty content.

        测试搜索空内容消息。
        """
        session = Session(id="empty-content")
        session.add_user_message("")  # Empty content
        session.add_user_message("Hello")
        browser = HistoryBrowser(session)

        results = browser.search("Hello")
        assert len(results) == 1

    def test_special_regex_characters(self):
        """Test search with special regex characters.

        测试包含特殊正则字符的搜索。
        """
        session = Session(id="regex-test")
        session.add_user_message("Price: $100 (approx)")
        session.add_user_message("Email: test@example.com")
        browser = HistoryBrowser(session)

        # Search for literal characters (not regex)
        results = browser.search("$100", case_sensitive=True)
        assert len(results) == 1

    def test_very_long_message(self):
        """Test handling very long messages.

        测试处理超长消息。
        """
        session = Session(id="long-message")
        long_content = "x" * 10000
        session.add_user_message(long_content)
        browser = HistoryBrowser(session)

        stats = browser.get_statistics()
        assert stats.total_characters == 10000
        assert stats.average_message_length == 10000.0

    def test_concurrent_refresh(self):
        """Test that refresh can be called multiple times safely.

        测试可以安全地多次调用刷新。
        """
        session = Session(id="concurrent")
        browser = HistoryBrowser(session)

        browser.refresh()
        first_messages = browser._messages.copy()

        browser.refresh()
        second_messages = browser._messages.copy()

        # Both should have same content
        assert len(first_messages) == len(second_messages)


class TestHistorySerialization:
    """Tests for history serialization/deserialization.

    历史序列化/反序列化测试。
    """

    def test_export_import_cycle(self):
        """Test that export/import preserves all data.

        测试导出/导入保留所有数据。
        """
        session = Session(id="cycle-test")
        session.add_system_message("System prompt")
        session.add_user_message("Question 1")
        session.add_assistant_message("Answer 1")
        session.add_user_message("Question 2")
        session.add_assistant_message("Answer 2")
        session.add_message(MessageRole.TOOL, '{"tool": "search", "query": "test"}')
        session.set_metadata("user_id", "12345")
        session.set_metadata("session_type", "test")

        browser = HistoryBrowser(session)

        with tempfile.TemporaryDirectory() as tmpdir:
            json_path = Path(tmpdir) / "export.json"

            # Export
            browser.export_json(json_path)

            # Read and verify
            with open(json_path, encoding="utf-8") as f:
                data = json.load(f)

            assert data["session_id"] == "cycle-test"
            assert data["message_count"] == 6
            assert len(data["messages"]) == 6

            # Verify message order and content
            roles = [m["role"] for m in data["messages"]]
            assert roles == ["system", "user", "assistant", "user", "assistant", "tool"]


class TestHistoryFilterErrors:
    """Error handling tests for history filtering.

    历史过滤错误处理测试。
    """

    def test_invalid_role_string(self):
        """Test filtering with invalid role string.

        测试使用无效角色字符串过滤。
        """
        session = Session(id="test")
        browser = HistoryBrowser(session)

        with pytest.raises(ValueError):
            browser.filter_by_role("invalid_role")

    def test_invalid_regex_pattern(self):
        """Test searching with invalid regex pattern.

        测试使用无效正则表达式搜索。
        """
        session = Session(id="test")
        session.add_user_message("Test")
        browser = HistoryBrowser(session)

        with pytest.raises(Exception):  # re.error
            browser.search_regex(r"[invalid(regex")


class TestGetRangeDescending:
    """Test get_range with DESCENDING order for coverage.
    测试 get_range 降序排序的覆盖。
    Covers line 207.
    """

    def test_get_range_descending_returns_reversed_results(self):
        """Test get_range with DESCENDING returns reversed order.

        测试 get_range 使用 DESCENDING 返回反向顺序。
        """
        session = Session(id="test-range-desc")
        session.add_user_message("First message")
        session.add_user_message("Second message")
        session.add_user_message("Third message")

        browser = HistoryBrowser(session)

        now = datetime.now()
        start = now - timedelta(hours=1)
        end = now + timedelta(hours=1)

        # Get messages in descending order
        messages_desc = browser.get_range(start, end, order=SortOrder.DESCENDING)

        assert len(messages_desc) == 3
        # In descending order, the last message should come first
        assert messages_desc[0].content == "Third message"
        assert messages_desc[1].content == "Second message"
        assert messages_desc[2].content == "First message"

        # Compare with ascending order
        messages_asc = browser.get_range(start, end, order=SortOrder.ASCENDING)
        assert messages_asc[0].content == "First message"
        assert messages_asc[-1].content == "Third message"

        # Verify that descending is actually reversed
        assert list(messages_desc) == list(reversed(messages_asc))


class TestSessionDurationCalculation:
    """Test session_duration_seconds calculation for coverage.
    测试 session_duration_seconds 计算的覆盖。
    Covers branch 428->432.
    """

    def test_session_duration_with_valid_timestamps(self):
        """Test session duration is calculated when timestamps exist.

        测试当时间戳存在时计算会话持续时间。
        """
        session = Session(id="duration-test")

        # Add messages - each will have its own timestamp
        session.add_user_message("Message 1")
        session.add_user_message("Message 2")
        session.add_user_message("Message 3")

        browser = HistoryBrowser(session)
        stats = browser.get_statistics()

        # Verify that duration is calculated
        assert stats.session_duration_seconds >= 0

        # Verify timestamps are set
        assert stats.first_message_time is not None
        assert stats.last_message_time is not None

        # The duration should be the difference between last and first
        if stats.first_message_time and stats.last_message_time:
            expected_duration = (
                stats.last_message_time - stats.first_message_time
            ).total_seconds()
            assert stats.session_duration_seconds == expected_duration


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
