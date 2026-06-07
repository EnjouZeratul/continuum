"""
LLM Client

Real LLM API client implementations for multiple providers.

Features:
    - Unified interface for Anthropic, OpenAI, and Google Gemini
    - Streaming response support
    - Automatic error classification and retry handling
    - Custom endpoint support (OpenAI-compatible APIs)
    - Proxy configuration

Supported Providers:
    - Anthropic (Claude models)
    - OpenAI (GPT models)
    - Google (Gemini models)
    - Together AI
    - Groq
    - DeepSeek
    - Moonshot
    - Any OpenAI-compatible endpoint

Quick Start:
    >>> from continuum_sdk.llm import LlmClient, Message
    >>>
    >>> # Create client
    >>> client = LlmClient.for_provider("anthropic", api_key="your-key")
    >>>
    >>> # Send message
    >>> response = await client.chat([Message.user("Hello!")])
    >>> print(response.content)

Streaming Example:
    >>> async for chunk in client.chat_stream([Message.user("Tell me a story")]):
    ...     print(chunk.content, end="", flush=True)

Custom Endpoint:
    >>> client = LlmClient.for_provider(
    ...     "custom",
    ...     api_key="your-key",
    ...     base_url="https://api.custom.com/v1",
    ...     api_format="openai"
    ... )

See Also:
    Message: Conversation message type
    ChatResponse: Response container
    StreamChunk: Streaming chunk type
"""

import json
from abc import ABC, abstractmethod
from collections.abc import AsyncIterator
from typing import (
    Any,
)

import httpx

from ..config.providers import get_default_base_url, get_default_model
from .errors import (
    InvalidResponseError,
    classify_http_error,
)
from .types import (
    ChatResponse,
    Message,
    StreamChunk,
    ToolDefinition,
)


class BaseLlmClient(ABC):
    """
    Abstract base class for LLM clients.
    """

    def __init__(
        self,
        api_key: str,
        base_url: str | None = None,
        timeout: float = 60.0,
        max_retries: int = 3,
        proxy: str | None = None,
    ):
        self.api_key = api_key
        self.base_url = base_url
        self.timeout = timeout
        self.max_retries = max_retries
        self.proxy = proxy

        # Build httpx client
        limits = httpx.Limits(max_connections=10, max_keepalive_connections=5)
        transport = httpx.HTTPTransport(proxy=proxy) if proxy else None

        self._client = httpx.AsyncClient(
            timeout=httpx.Timeout(timeout),
            limits=limits,
            transport=transport,
        )

    @abstractmethod
    async def chat(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> ChatResponse:
        """
        Send chat completion request.

        Args:
            messages: List of conversation messages
            model: Model to use (provider-specific)
            max_tokens: Maximum tokens to generate
            temperature: Sampling temperature
            system_prompt: System prompt
            tools: Available tools for function calling
            **kwargs: Additional provider-specific options

        Returns:
            ChatResponse with the model's reply
        """
        pass

    @abstractmethod
    async def chat_stream(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> AsyncIterator[StreamChunk]:
        """
        Send streaming chat completion request.

        Yields:
            StreamChunk objects as they arrive
        """
        pass  # pragma: no cover

    async def close(self):
        """Close the HTTP client."""
        await self._client.aclose()

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()


class AnthropicClient(BaseLlmClient):
    """
    Anthropic Claude API client.

    API Reference: https://docs.anthropic.com/claude/reference
    """

    DEFAULT_BASE_URL = "https://api.anthropic.com/v1"

    def __init__(
        self,
        api_key: str,
        base_url: str | None = None,
        default_model: str | None = None,
        **kwargs,
    ):
        super().__init__(api_key, base_url or self.DEFAULT_BASE_URL, **kwargs)
        self.default_model = default_model or get_default_model("anthropic")
        self.provider = "anthropic"

    def _build_headers(self) -> dict[str, str]:
        return {
            "x-api-key": self.api_key,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
        }

    async def chat(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> ChatResponse:
        """Send chat request to Anthropic Claude API."""
        # Build the correct URL: if base_url doesn't contain /v1, add it
        if self.base_url.endswith("/v1") or self.base_url.endswith("/v1/"):
            url = f"{self.base_url.rstrip('/')}/messages"
        else:
            url = f"{self.base_url.rstrip('/')}/v1/messages"

        # Build request body
        body: dict[str, Any] = {
            "model": model or self.default_model,
            "max_tokens": max_tokens,
            "messages": [m.to_anthropic_format() for m in messages],
            "temperature": temperature,
        }

        if system_prompt:
            body["system"] = system_prompt

        if tools:
            body["tools"] = [t.to_anthropic_format() for t in tools]

        # Add any extra kwargs
        body.update(kwargs)

        # Send request
        response = await self._client.post(
            url,
            headers=self._build_headers(),
            json=body,
        )

        if response.status_code != 200:
            raise classify_http_error(
                response.status_code,
                response.text,
                self.provider,
            )

        try:
            data = response.json()
            return ChatResponse.from_anthropic(data)
        except (json.JSONDecodeError, KeyError) as e:
            raise InvalidResponseError(
                f"Failed to parse Anthropic response: {e}",
                provider=self.provider,
                response_data=response.text,
            )

    async def chat_stream(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> AsyncIterator[StreamChunk]:
        """Send streaming chat request to Anthropic Claude API."""
        # Build correct URL: if base_url doesn't contain /v1, add it
        if self.base_url.endswith("/v1") or self.base_url.endswith("/v1/"):
            url = f"{self.base_url.rstrip('/')}/messages"
        else:
            url = f"{self.base_url.rstrip('/')}/v1/messages"

        body: dict[str, Any] = {
            "model": model or self.default_model,
            "max_tokens": max_tokens,
            "messages": [m.to_anthropic_format() for m in messages],
            "temperature": temperature,
            "stream": True,
        }

        if system_prompt:
            body["system"] = system_prompt

        if tools:
            body["tools"] = [t.to_anthropic_format() for t in tools]

        body.update(kwargs)

        async with self._client.stream(
            "POST",
            url,
            headers=self._build_headers(),
            json=body,
        ) as response:
            if response.status_code != 200:
                error_text = await response.aread()
                raise classify_http_error(
                    response.status_code,
                    error_text.decode(),
                    self.provider,
                )

            async for line in response.aiter_lines():  # pragma: no branch  # pragma: no branch
                if not line.startswith("data: "):
                    continue

                data_str = line[6:]  # Remove "data: " prefix
                if data_str.strip() == "[DONE]":
                    break

                try:
                    data = json.loads(data_str)
                except json.JSONDecodeError:
                    continue

                event_type = data.get("type", "")

                if event_type == "content_block_delta":
                    delta = data.get("delta", {})
                    if delta.get("type") == "text_delta":
                        yield StreamChunk(content=delta.get("text", ""))

                elif event_type == "message_stop":
                    yield StreamChunk(finish_reason="stop")
                    break  # pragma: no cover


class OpenAIClient(BaseLlmClient):
    """
    OpenAI GPT API client.

    API Reference: https://platform.openai.com/docs/api-reference
    """

    DEFAULT_BASE_URL = "https://api.openai.com/v1"

    def __init__(
        self,
        api_key: str,
        base_url: str | None = None,
        default_model: str | None = None,
        **kwargs,
    ):
        super().__init__(api_key, base_url or self.DEFAULT_BASE_URL, **kwargs)
        self.default_model = default_model or get_default_model("openai")
        self.provider = "openai"

    def _build_headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

    async def chat(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> ChatResponse:
        """Send chat request to OpenAI API."""
        url = f"{self.base_url}/chat/completions"

        # Build messages list
        api_messages = []
        if system_prompt:
            api_messages.append({"role": "system", "content": system_prompt})
        api_messages.extend([m.to_openai_format() for m in messages])

        body: dict[str, Any] = {
            "model": model or self.default_model,
            "messages": api_messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        }

        if tools:
            body["tools"] = [t.to_openai_format() for t in tools]

        body.update(kwargs)

        response = await self._client.post(
            url,
            headers=self._build_headers(),
            json=body,
        )

        if response.status_code != 200:
            raise classify_http_error(
                response.status_code,
                response.text,
                self.provider,
            )

        try:
            data = response.json()
            return ChatResponse.from_openai(data)
        except (json.JSONDecodeError, KeyError) as e:
            raise InvalidResponseError(
                f"Failed to parse OpenAI response: {e}",
                provider=self.provider,
                response_data=response.text,
            )

    async def chat_stream(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> AsyncIterator[StreamChunk]:
        """Send streaming chat request to OpenAI API."""
        url = f"{self.base_url}/chat/completions"

        api_messages = []
        if system_prompt:
            api_messages.append({"role": "system", "content": system_prompt})
        api_messages.extend([m.to_openai_format() for m in messages])

        body: dict[str, Any] = {
            "model": model or self.default_model,
            "messages": api_messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "stream": True,
        }

        if tools:
            body["tools"] = [t.to_openai_format() for t in tools]

        body.update(kwargs)

        async with self._client.stream(
            "POST",
            url,
            headers=self._build_headers(),
            json=body,
        ) as response:
            if response.status_code != 200:
                error_text = await response.aread()
                raise classify_http_error(
                    response.status_code,
                    error_text.decode(),
                    self.provider,
                )

            async for line in response.aiter_lines():  # pragma: no branch
                if not line.startswith("data: "):
                    continue

                data_str = line[6:]
                if data_str.strip() == "[DONE]":
                    yield StreamChunk(finish_reason="stop")
                    break

                try:
                    data = json.loads(data_str)
                except json.JSONDecodeError:
                    continue

                choices = data.get("choices", [])
                if choices:
                    delta = choices[0].get("delta", {})
                    content = delta.get("content", "")
                    finish_reason = choices[0].get("finish_reason")

                    if content:
                        yield StreamChunk(content=content)
                    if finish_reason:  # pragma: no branch
                        yield StreamChunk(finish_reason=finish_reason)


class GeminiClient(BaseLlmClient):
    """
    Google Gemini API client.

    API Reference: https://ai.google.dev/tutorials/python_quickstart
    """

    DEFAULT_BASE_URL = "https://generativelanguage.googleapis.com/v1beta"

    def __init__(
        self,
        api_key: str,
        base_url: str | None = None,
        default_model: str | None = None,
        **kwargs,
    ):
        super().__init__(api_key, base_url or self.DEFAULT_BASE_URL, **kwargs)
        self.default_model = default_model or get_default_model("google")
        self.provider = "gemini"

    def _build_url(self, model: str, method: str = "generateContent") -> str:
        return f"{self.base_url}/models/{model}:{method}?key={self.api_key}"

    async def chat(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> ChatResponse:
        """Send chat request to Google Gemini API."""
        model_name = model or self.default_model
        url = self._build_url(model_name, "generateContent")

        # Build contents
        contents = [m.to_gemini_format() for m in messages]

        body: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": max_tokens,
                "temperature": temperature,
            },
        }

        if system_prompt:
            body["systemInstruction"] = {"parts": [{"text": system_prompt}]}

        if tools:
            body["tools"] = [
                {"functionDeclarations": [t.to_gemini_format() for t in tools]}
            ]

        body.update(kwargs)

        response = await self._client.post(url, json=body)

        if response.status_code != 200:
            raise classify_http_error(
                response.status_code,
                response.text,
                self.provider,
            )

        try:
            data = response.json()
            return ChatResponse.from_gemini(data, model_name)
        except (json.JSONDecodeError, KeyError) as e:
            raise InvalidResponseError(
                f"Failed to parse Gemini response: {e}",
                provider=self.provider,
                response_data=response.text,
            )

    async def chat_stream(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> AsyncIterator[StreamChunk]:
        """Send streaming chat request to Google Gemini API."""
        model_name = model or self.default_model
        url = self._build_url(model_name, "streamGenerateContent")

        contents = [m.to_gemini_format() for m in messages]

        body: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": max_tokens,
                "temperature": temperature,
            },
        }

        if system_prompt:
            body["systemInstruction"] = {"parts": [{"text": system_prompt}]}

        body.update(kwargs)

        async with self._client.stream("POST", url, json=body) as response:
            if response.status_code != 200:
                error_text = await response.aread()
                raise classify_http_error(
                    response.status_code,
                    error_text.decode(),
                    self.provider,
                )

            buffer = ""
            async for chunk in response.aiter_text():
                buffer += chunk

                # Gemini returns JSON array elements
                while "},{" in buffer:
                    idx = buffer.index("},{") + 1
                    part = buffer[:idx]
                    buffer = buffer[idx:]

                    try:
                        # Remove array brackets if present
                        if part.startswith("["):  # pragma: no branch
                            part = part[1:]
                        if part.endswith("]"):  # pragma: no branch
                            part = part[:-1]  # pragma: no cover

                        data = json.loads(part)

                        candidates = data.get("candidates", [])
                        if candidates:
                            parts = candidates[0].get("content", {}).get("parts", [])
                            if parts:
                                text = parts[0].get("text", "")
                                if text:
                                    yield StreamChunk(content=text)

                            finish = candidates[0].get("finishReason")
                            if finish:
                                yield StreamChunk(finish_reason=finish)
                    except json.JSONDecodeError:
                        continue


class CustomClient(BaseLlmClient):
    """
    Custom/OpenAI-compatible API client.

    For endpoints that follow OpenAI's API format but use different URLs.
    """

    def __init__(
        self,
        api_key: str,
        base_url: str,
        default_model: str = "default",
        **kwargs,
    ):
        super().__init__(api_key, base_url, **kwargs)
        self.default_model = default_model
        self.provider = "custom"

    def _build_headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

    async def chat(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> ChatResponse:
        """Send chat request to custom OpenAI-compatible API."""
        url = f"{self.base_url}/chat/completions"

        api_messages = []
        if system_prompt:
            api_messages.append({"role": "system", "content": system_prompt})
        api_messages.extend([m.to_openai_format() for m in messages])

        body: dict[str, Any] = {
            "model": model or self.default_model,
            "messages": api_messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        }

        body.update(kwargs)

        response = await self._client.post(
            url,
            headers=self._build_headers(),
            json=body,
        )

        if response.status_code != 200:
            raise classify_http_error(
                response.status_code,
                response.text,
                self.provider,
            )

        try:
            data = response.json()
            return ChatResponse.from_openai(data)
        except (json.JSONDecodeError, KeyError) as e:
            raise InvalidResponseError(
                f"Failed to parse response: {e}",
                provider=self.provider,
                response_data=response.text,
            )

    async def chat_stream(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> AsyncIterator[StreamChunk]:
        """Send streaming chat request to custom API."""
        url = f"{self.base_url}/chat/completions"

        api_messages = []
        if system_prompt:
            api_messages.append({"role": "system", "content": system_prompt})
        api_messages.extend([m.to_openai_format() for m in messages])

        body: dict[str, Any] = {
            "model": model or self.default_model,
            "messages": api_messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "stream": True,
        }

        body.update(kwargs)

        async with self._client.stream(
            "POST",
            url,
            headers=self._build_headers(),
            json=body,
        ) as response:
            if response.status_code != 200:
                error_text = await response.aread()
                raise classify_http_error(
                    response.status_code,
                    error_text.decode(),
                    self.provider,
                )

            async for line in response.aiter_lines():  # pragma: no branch
                if not line.startswith("data: "):
                    continue

                data_str = line[6:]
                if data_str.strip() == "[DONE]":
                    yield StreamChunk(finish_reason="stop")
                    break

                try:
                    data = json.loads(data_str)
                    choices = data.get("choices", [])
                    if choices:
                        delta = choices[0].get("delta", {})
                        content = delta.get("content", "")
                        if content:
                            yield StreamChunk(content=content)
                except json.JSONDecodeError:
                    continue


class LlmClient:
    """
    Unified LLM client factory.

    Creates the appropriate client based on provider or api_format.

    Usage:
        >>> client = LlmClient.for_provider("anthropic", api_key="...")
        >>> response = await client.chat([Message.user("Hello")])

        >>> # Custom provider with OpenAI format
        >>> client = LlmClient.for_provider("custom", api_key="...",
        ...     base_url="https://api.custom.com/v1", api_format="openai")
    """

    @staticmethod
    def for_provider(
        provider: str,
        api_key: str,
        base_url: str | None = None,
        model: str | None = None,
        api_format: str | None = None,
        **kwargs,
    ) -> BaseLlmClient:
        """
        Create client for specified provider.

        Args:
            provider: Provider name (anthropic, openai, gemini, google,
                      together, groq, deepseek, moonshot, custom, etc.)
            api_key: API key for authentication
            base_url: Optional custom base URL (uses default if not provided)
            model: Default model to use
            api_format: API format override ("anthropic", "openai", "google")
                        If not specified, uses provider's default format
            **kwargs: Additional client options

        Returns:
            Appropriate LLM client instance
        """
        from continuum_sdk.config.providers import (
            BUILTIN_PROVIDERS,
        )

        provider_lower = provider.lower()

        # Get provider info for defaults
        provider_info = BUILTIN_PROVIDERS.get(provider_lower)

        # Determine base_url
        if base_url is None and provider_info:
            base_url = provider_info.default_base_url

        # Determine model
        if model is None and provider_info:
            model = provider_info.default_model

        # Determine api_format
        if api_format is None and provider_info:
            format_enum = provider_info.api_format
            api_format = format_enum.value if format_enum else "openai"

        api_format_lower = (api_format or "openai").lower()

        # Route by api_format, not provider name
        if api_format_lower == "anthropic":
            return AnthropicClient(
                api_key=api_key,
                base_url=base_url,
                default_model=model,  # AnthropicClient internally dynamically gets the default
                **kwargs,
            )
        elif api_format_lower == "openai":
            # OpenAI format: works for OpenAI, Together, Groq, DeepSeek, etc.
            # Get the corresponding default model based on provider name
            provider_for_default = provider if provider in ("openai", "deepseek", "together", "groq", "azure") else "openai"
            return OpenAIClient(
                api_key=api_key,
                base_url=base_url or get_default_base_url(provider_for_default) or "https://api.openai.com/v1",
                default_model=model or get_default_model(provider_for_default),
                **kwargs,
            )
        elif api_format_lower in ("google", "gemini"):
            return GeminiClient(
                api_key=api_key,
                base_url=base_url,
                default_model=model or get_default_model("google"),
                **kwargs,
            )
        else:
            # Unknown format, fall back to OpenAI-compatible
            if not base_url:
                raise ValueError(
                    f"base_url is required for unknown provider/format: {provider}/{api_format}"
                )
            return CustomClient(
                api_key=api_key,
                base_url=base_url,
                default_model=model,  # CustomClient requires user to provide model or use "default"
                **kwargs,
            )
