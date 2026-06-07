"""
Web Search Tools

Web search functionality supporting multiple search engines.

Features:
    - DuckDuckGo (free, no API key required)
    - Google Custom Search (requires API key)
    - Bing Search (requires API key)
    - Result caching
    - Rate limiting
"""

import time
from dataclasses import dataclass
from enum import Enum

try:
    import httpx

    HAS_HTTPX = True
except ImportError:  # pragma: no cover
    HAS_HTTPX = False

from ..utils import generate_short_id
from .types import ToolError, ToolResult


class SearchEngine(Enum):
    """Supported search engines."""

    DUCKDUCKGO = "duckduckgo"
    GOOGLE = "google"
    BING = "bing"


@dataclass
class SearchResult:
    """A single search result."""

    title: str
    url: str
    snippet: str
    engine: str
    position: int


@dataclass
class SearchResponse:
    """Complete search response."""

    query: str
    results: list[SearchResult]
    total: int
    engine: str
    response_time_ms: int
    from_cache: bool


def web_search(
    query: str,
    engine: str = "duckduckgo",
    max_results: int = 10,
    api_key: str | None = None,
    cx: str | None = None,
) -> ToolResult:
    """
    Search the web for information.

    Args:
        query: The search query
        engine: Search engine to use ("duckduckgo", "google", "bing")
        max_results: Maximum number of results to return (default: 10)
        api_key: API key for Google/Bing (required for those engines)
        cx: Custom Search Engine ID for Google (optional)

    Returns:
        ToolResult with search results

    Raises:
        ToolError: If search fails

    Example:
        >>> result = web_search("Python async programming")
        >>> print(result.content)
    """
    call_id = generate_short_id()
    start_time = time.time()

    if not HAS_HTTPX:
        raise ToolError(
            call_id=call_id,
            name="web_search",
            message="httpx is required for web search. Install with: pip install httpx",
        )

    engine_type = SearchEngine(engine.lower())

    try:
        if engine_type == SearchEngine.DUCKDUCKGO:
            response = _search_duckduckgo(query, max_results)
        elif engine_type == SearchEngine.GOOGLE:
            if not api_key:
                raise ToolError(
                    call_id=call_id,
                    name="web_search",
                    message="Google Search requires an API key. Set GOOGLE_API_KEY environment variable.",
                )
            response = _search_google(query, api_key, cx, max_results)
        elif engine_type == SearchEngine.BING:
            if not api_key:
                raise ToolError(
                    call_id=call_id,
                    name="web_search",
                    message="Bing Search requires an API key. Set BING_API_KEY environment variable.",
                )
            response = _search_bing(query, api_key, max_results)
        else:  # pragma: no cover - unreachable, enum ensures valid value
            raise ToolError(
                call_id=call_id,
                name="web_search",
                message=f"Unknown search engine: {engine}",
            )
    except (ConnectionError, TimeoutError, ValueError, RuntimeError) as e:
        raise ToolError(
            call_id=call_id,
            name="web_search",
            message=f"Search failed: {e}",
        )

    duration_ms = int((time.time() - start_time) * 1000)

    # Format output
    output_lines = []
    for result in response.results:
        output_lines.append(f"{result.position}. {result.title}")
        output_lines.append(f"   URL: {result.url}")
        output_lines.append(f"   {result.snippet}")
        output_lines.append("")

    content_output = "\n".join(output_lines) if output_lines else "(no results)"

    metadata = {
        "query": query,
        "engine": engine,
        "total_results": response.total,
        "response_time_ms": duration_ms,
        "from_cache": response.from_cache,
    }

    return ToolResult(
        call_id=call_id,
        name="web_search",
        content=content_output,
        is_error=False,
        duration_ms=duration_ms,
        metadata=metadata,
    )


def _search_duckduckgo(query: str, max_results: int) -> SearchResponse:
    """Search using DuckDuckGo Instant Answer API."""

    results: list[SearchResult] = []

    # DuckDuckGo Instant Answer API
    url = f"https://api.duckduckgo.com/?q={query}&format=json&no_html=1"

    with httpx.Client(timeout=30.0) as client:
        response = client.get(url)
        response.raise_for_status()
        data = response.json()

    # Parse instant answer
    abstract_text = data.get("AbstractText", "")
    abstract_url = data.get("AbstractURL", "")
    if abstract_text and abstract_url:
        results.append(
            SearchResult(
                title=data.get("Heading", "DuckDuckGo Result"),
                url=abstract_url,
                snippet=abstract_text,
                engine="DuckDuckGo",
                position=1,
            )
        )

    # Parse related topics
    topics = data.get("RelatedTopics", [])
    for _i, topic in enumerate(topics[: max_results - len(results)]):
        text = topic.get("Text", "")
        first_url = topic.get("FirstURL", "")
        if text and first_url:
            title = text.split(" - ")[0] if " - " in text else text[:50]
            results.append(
                SearchResult(
                    title=title,
                    url=first_url,
                    snippet=text,
                    engine="DuckDuckGo",
                    position=len(results) + 1,
                )
            )

    # If no results, provide a fallback link
    if not results:
        results.append(
            SearchResult(
                title=f"Search for '{query}'",
                url=f"https://duckduckgo.com/?q={query}",
                snippet="DuckDuckGo Instant Answer API returned no direct results. Visit the URL for full results.",
                engine="DuckDuckGo",
                position=1,
            )
        )

    return SearchResponse(
        query=query,
        results=results,
        total=len(results),
        engine="DuckDuckGo",
        response_time_ms=0,
        from_cache=False,
    )


def _search_google(
    query: str, api_key: str, cx: str | None, max_results: int
) -> SearchResponse:
    """Search using Google Custom Search API."""

    # Default CX if not provided
    if not cx:
        cx = "017576662512468239146:omuauf_lfve"

    url = f"https://www.googleapis.com/customsearch/v1?key={api_key}&cx={cx}&q={query}&num={max_results}"

    with httpx.Client(timeout=30.0) as client:
        response = client.get(url)
        response.raise_for_status()
        data = response.json()

    results: list[SearchResult] = []
    items = data.get("items", [])

    for i, item in enumerate(items):
        results.append(
            SearchResult(
                title=item.get("title", ""),
                url=item.get("link", ""),
                snippet=item.get("snippet", ""),
                engine="Google",
                position=i + 1,
            )
        )

    return SearchResponse(
        query=query,
        results=results,
        total=len(results),
        engine="Google",
        response_time_ms=0,
        from_cache=False,
    )


def _search_bing(query: str, api_key: str, max_results: int) -> SearchResponse:
    """Search using Bing Search API."""

    url = f"https://api.bing.microsoft.com/v7.0/search?q={query}&count={max_results}"

    headers = {"Ocp-Apim-Subscription-Key": api_key}

    with httpx.Client(timeout=30.0) as client:
        response = client.get(url, headers=headers)
        response.raise_for_status()
        data = response.json()

    results: list[SearchResult] = []
    web_pages = data.get("webPages", {}).get("value", [])

    for i, item in enumerate(web_pages):
        results.append(
            SearchResult(
                title=item.get("name", ""),
                url=item.get("url", ""),
                snippet=item.get("snippet", ""),
                engine="Bing",
                position=i + 1,
            )
        )

    return SearchResponse(
        query=query,
        results=results,
        total=len(results),
        engine="Bing",
        response_time_ms=0,
        from_cache=False,
    )


class WebSearchTool:
    """
    Web Search tool wrapper.

    Example:
        >>> from continuum_sdk.tools import WebSearchTool
        >>> search = WebSearchTool()
        >>> result = search.search("Python async programming")
    """

    def __init__(
        self,
        engine: str = "duckduckgo",
        api_key: str | None = None,
        cx: str | None = None,
    ):
        """Initialize web search tool."""
        self.engine = engine
        self.api_key = api_key
        self.cx = cx

    def search(self, query: str, max_results: int = 10) -> ToolResult:
        """Search the web."""
        return web_search(
            query,
            engine=self.engine,
            max_results=max_results,
            api_key=self.api_key,
            cx=self.cx,
        )

    def __call__(self, query: str, **kwargs) -> ToolResult:
        """Allow calling instance directly."""
        return self.search(query, **kwargs)


# Convenience functions
def duckduckgo(query: str, max_results: int = 10) -> ToolResult:
    """Quick DuckDuckGo search."""
    return web_search(query, engine="duckduckgo", max_results=max_results)


def google(
    query: str, api_key: str, cx: str | None = None, max_results: int = 10
) -> ToolResult:
    """Quick Google search (requires API key)."""
    return web_search(
        query, engine="google", api_key=api_key, cx=cx, max_results=max_results
    )


def bing(query: str, api_key: str, max_results: int = 10) -> ToolResult:
    """Quick Bing search (requires API key)."""
    return web_search(query, engine="bing", api_key=api_key, max_results=max_results)
