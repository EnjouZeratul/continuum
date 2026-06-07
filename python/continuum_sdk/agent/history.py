"""History Browser Module

Browse and search session message history with rich filtering capabilities.

Features:
    - Time-based filtering (date range, last N hours/days)
    - Role-based filtering (user, assistant, system, tool)
    - Content search (keyword, regex)
    - Pagination support
    - Statistics (message counts, token usage)
    - Export capabilities

Quick Start:
    >>> from continuum_sdk.agent.history import HistoryBrowser
    >>> from continuum_sdk.agent.session import Session
    >>>
    >>> session = Session.load("my-session.json")
    >>> browser = HistoryBrowser(session)
    >>>
    >>> # Get recent messages
    >>> recent = browser.get_recent(limit=10)
    >>>
    >>> # Search by keyword
    >>> results = browser.search("Python")
    >>>
    >>> # Filter by role
    >>> user_msgs = browser.filter_by_role("user")
    >>>
    >>> # Get statistics
    >>> stats = browser.get_statistics()
    >>> print(f"Total messages: {stats['total_messages']}")

Time Range Filtering:
    >>> from datetime import datetime, timedelta
    >>>
    >>> # Last hour
    >>> start = datetime.now() - timedelta(hours=1)
    >>> recent = browser.get_range(start, datetime.now())
    >>>
    >>> # Yesterday
    >>> yesterday = datetime.now() - timedelta(days=1)
    >>> msgs = browser.get_range(yesterday.start, yesterday.end)

Search Options:
    >>> # Simple keyword search
    >>> results = browser.search("error")
    >>>
    >>> # Case-insensitive search
    >>> results = browser.search("python", case_sensitive=False)
    >>>
    >>> # Regex search
    >>> results = browser.search_regex(r"def \\w+\\(")

Export:
    >>> # Export to JSON
    >>> browser.export_json("history.json")
    >>>
    >>> # Export to Markdown
    >>> browser.export_markdown("history.md")
"""

import json
import re
from collections.abc import Iterator
from dataclasses import dataclass
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path

from .session import Message, MessageRole, Session


@dataclass
class HistoryFilter:
    """Filter criteria for history queries."""

    role: MessageRole | None = None
    start_time: datetime | None = None
    end_time: datetime | None = None
    keyword: str | None = None
    case_sensitive: bool = False
    use_regex: bool = False
    limit: int | None = None
    offset: int = 0


@dataclass
class HistoryStatistics:
    """Statistics about session history."""

    total_messages: int = 0
    user_messages: int = 0
    assistant_messages: int = 0
    system_messages: int = 0
    tool_messages: int = 0
    first_message_time: datetime | None = None
    last_message_time: datetime | None = None
    average_message_length: float = 0.0
    total_characters: int = 0
    session_duration_seconds: float = 0.0


@dataclass
class SearchResult:
    """A single search result."""

    message: Message
    index: int
    match_position: int
    match_length: int
    matched_text: str


class SortOrder(Enum):
    """Sort order for history queries."""

    ASCENDING = "asc"
    DESCENDING = "desc"


class HistoryBrowser:
    """
    Browse and analyze session message history.

    Provides rich filtering, search, and analysis capabilities
    for session message history.

    Example:
        >>> session = Session.load("my-session.json")
        >>> browser = HistoryBrowser(session)
        >>> recent = browser.get_recent(10)
        >>> for msg in recent:
        ...     print(f"[{msg.role.value}] {msg.content[:50]}")
    """

    def __init__(self, session: Session):
        """
        Initialize history browser.

        Args:
            session: Session to browse
        """
        self._session = session
        self._messages: list[Message] = []

    def refresh(self) -> None:
        """Refresh messages from session."""
        self._messages = self._session.get_messages()

    def _ensure_messages(self) -> list[Message]:
        """Ensure messages are loaded."""
        if not self._messages:
            self.refresh()
        return self._messages

    def get_all(self, order: SortOrder = SortOrder.ASCENDING) -> list[Message]:
        """
        Get all messages.

        Args:
            order: Sort order (ascending by time)

        Returns:
            List of all messages
        """
        messages = self._ensure_messages()
        if order == SortOrder.DESCENDING:
            return list(reversed(messages))
        return messages.copy()

    def get_recent(self, limit: int = 10) -> list[Message]:
        """
        Get most recent messages.

        Args:
            limit: Maximum number of messages

        Returns:
            List of recent messages (most recent first)
        """
        messages = self._ensure_messages()
        return list(reversed(messages[-limit:]))

    def get_range(
        self,
        start_time: datetime,
        end_time: datetime,
        order: SortOrder = SortOrder.ASCENDING,
    ) -> list[Message]:
        """
        Get messages within time range.

        Args:
            start_time: Start of range
            end_time: End of range
            order: Sort order

        Returns:
            List of messages in range
        """
        messages = self._ensure_messages()
        result = [m for m in messages if start_time <= m.timestamp <= end_time]
        if order == SortOrder.DESCENDING:
            return list(reversed(result))
        return result

    def get_last_hours(self, hours: int) -> list[Message]:
        """
        Get messages from last N hours.

        Args:
            hours: Number of hours to look back

        Returns:
            List of messages in time range
        """
        start = datetime.now() - timedelta(hours=hours)
        return self.get_range(start, datetime.now())

    def get_last_days(self, days: int) -> list[Message]:
        """
        Get messages from last N days.

        Args:
            days: Number of days to look back

        Returns:
            List of messages in time range
        """
        start = datetime.now() - timedelta(days=days)
        return self.get_range(start, datetime.now())

    def filter_by_role(
        self,
        role: MessageRole | str,
        limit: int | None = None,
    ) -> list[Message]:
        """
        Filter messages by role.

        Args:
            role: Message role to filter
            limit: Maximum messages to return

        Returns:
            List of matching messages
        """
        if isinstance(role, str):
            role = MessageRole(role)

        messages = self._ensure_messages()
        result = [m for m in messages if m.role == role]

        if limit:
            result = result[-limit:]

        return result

    def search(
        self,
        keyword: str,
        case_sensitive: bool = False,
        limit: int | None = None,
    ) -> list[SearchResult]:
        """
        Search messages by keyword.

        Args:
            keyword: Keyword to search
            case_sensitive: Case sensitivity
            limit: Maximum results

        Returns:
            List of search results
        """
        messages = self._ensure_messages()
        results: list[SearchResult] = []

        search_text = keyword if case_sensitive else keyword.lower()

        for idx, msg in enumerate(messages):
            content = msg.content if case_sensitive else msg.content.lower()

            pos = content.find(search_text)
            if pos >= 0:
                results.append(
                    SearchResult(
                        message=msg,
                        index=idx,
                        match_position=pos,
                        match_length=len(keyword),
                        matched_text=msg.content[pos : pos + len(keyword)],
                    )
                )

        if limit:
            results = results[:limit]

        return results

    def search_regex(
        self,
        pattern: str,
        limit: int | None = None,
    ) -> list[SearchResult]:
        """
        Search messages by regex pattern.

        Args:
            pattern: Regex pattern
            limit: Maximum results

        Returns:
            List of search results
        """
        messages = self._ensure_messages()
        results: list[SearchResult] = []

        regex = re.compile(pattern)

        for idx, msg in enumerate(messages):
            match = regex.search(msg.content)
            if match:
                results.append(
                    SearchResult(
                        message=msg,
                        index=idx,
                        match_position=match.start(),
                        match_length=len(match.group()),
                        matched_text=match.group(),
                    )
                )

        if limit:
            results = results[:limit]

        return results

    def apply_filter(self, filter: HistoryFilter) -> list[Message]:
        """
        Apply complex filter to messages.

        Args:
            filter: Filter criteria

        Returns:
            List of matching messages
        """
        messages = self._ensure_messages()

        # Role filter
        if filter.role:
            messages = [m for m in messages if m.role == filter.role]

        # Time range filter
        if filter.start_time:
            messages = [m for m in messages if m.timestamp >= filter.start_time]
        if filter.end_time:
            messages = [m for m in messages if m.timestamp <= filter.end_time]

        # Keyword search
        if filter.keyword:
            if filter.use_regex:
                regex = re.compile(filter.keyword)
                messages = [m for m in messages if regex.search(m.content)]
            else:
                search_text = (
                    filter.keyword if filter.case_sensitive else filter.keyword.lower()
                )
                messages = [
                    m
                    for m in messages
                    if search_text
                    in (m.content if filter.case_sensitive else m.content.lower())
                ]

        # Pagination
        if filter.offset:
            messages = messages[filter.offset :]
        if filter.limit:
            messages = messages[: filter.limit]

        return messages

    def iterate(
        self,
        order: SortOrder = SortOrder.ASCENDING,
    ) -> Iterator[Message]:
        """
        Iterate over all messages.

        Args:
            order: Sort order

        Yields:
            Message objects
        """
        messages = self.get_all(order)
        yield from messages

    def get_statistics(self) -> HistoryStatistics:
        """
        Get statistics about history.

        Returns:
            HistoryStatistics with counts and metrics
        """
        messages = self._ensure_messages()

        stats = HistoryStatistics()
        stats.total_messages = len(messages)

        if not messages:
            return stats

        # Role counts
        stats.user_messages = sum(1 for m in messages if m.role == MessageRole.USER)
        stats.assistant_messages = sum(
            1 for m in messages if m.role == MessageRole.ASSISTANT
        )
        stats.system_messages = sum(1 for m in messages if m.role == MessageRole.SYSTEM)
        stats.tool_messages = sum(1 for m in messages if m.role == MessageRole.TOOL)

        # Time range
        stats.first_message_time = messages[0].timestamp
        stats.last_message_time = messages[-1].timestamp

        # Content stats
        total_chars = sum(len(m.content) for m in messages)
        stats.total_characters = total_chars
        stats.average_message_length = total_chars / len(messages) if messages else 0

        # Duration
        if stats.first_message_time and stats.last_message_time:  # pragma: no branch
            duration = stats.last_message_time - stats.first_message_time
            stats.session_duration_seconds = duration.total_seconds()

        return stats

    def get_message_at(self, index: int) -> Message | None:
        """
        Get message at specific index.

        Args:
            index: Message index

        Returns:
            Message or None if index out of range
        """
        messages = self._ensure_messages()
        if 0 <= index < len(messages):
            return messages[index]
        return None

    def get_context_around(
        self,
        index: int,
        before: int = 2,
        after: int = 2,
    ) -> list[Message]:
        """
        Get messages around a specific index.

        Args:
            index: Center index
            before: Messages before index
            after: Messages after index

        Returns:
            List of context messages
        """
        messages = self._ensure_messages()
        start = max(0, index - before)
        end = min(len(messages), index + after + 1)
        return messages[start:end]

    def export_json(self, path: str | Path) -> None:
        """
        Export history to JSON file.

        Args:
            path: Output file path
        """
        messages = self._ensure_messages()
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)

        data = {
            "session_id": self._session.id,
            "export_time": datetime.now().isoformat(),
            "message_count": len(messages),
            "messages": [m.to_dict() for m in messages],
            "statistics": {
                "total_messages": len(messages),
                "user_messages": sum(1 for m in messages if m.role == MessageRole.USER),
                "assistant_messages": sum(
                    1 for m in messages if m.role == MessageRole.ASSISTANT
                ),
            },
        }

        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

    def export_markdown(self, path: str | Path) -> None:
        """
        Export history to Markdown file.

        Args:
            path: Output file path
        """
        messages = self._ensure_messages()
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)

        lines = [
            f"# Session History: {self._session.id}",
            "",
            f"**Exported**: {datetime.now().isoformat()}",
            f"**Messages**: {len(messages)}",
            "",
            "---",
            "",
        ]

        for msg in messages:
            role_label = msg.role.value.upper()
            timestamp = msg.timestamp.strftime("%Y-%m-%d %H:%M:%S")
            lines.append(f"## [{role_label}] @ {timestamp}")
            lines.append("")
            lines.append(msg.content)
            lines.append("")
            lines.append("---")
            lines.append("")

        with open(path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))

    def export_text(self, path: str | Path) -> None:
        """
        Export history to plain text file.

        Args:
            path: Output file path
        """
        messages = self._ensure_messages()
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)

        lines = []
        for msg in messages:
            timestamp = msg.timestamp.strftime("%H:%M:%S")
            lines.append(f"[{timestamp}] [{msg.role.value}] {msg.content}")

        with open(path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))


def browse_session(session: Session) -> HistoryBrowser:
    """
    Create a history browser for a session.

    Args:
        session: Session to browse

    Returns:
        HistoryBrowser instance
    """
    return HistoryBrowser(session)


def browse_session_file(path: str | Path) -> HistoryBrowser:
    """
    Create a history browser from session file.

    Args:
        path: Session file path

    Returns:
        HistoryBrowser instance
    """
    session = Session.load(path)
    return HistoryBrowser(session)
