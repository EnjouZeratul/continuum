"""
LLM Types

Type definitions for LLM client interactions.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class MessageRole(Enum):
    """Message role in conversation."""

    USER = "user"
    ASSISTANT = "assistant"
    SYSTEM = "system"
    TOOL = "tool"


@dataclass
class Message:
    """
    A single message in a conversation.

    Attributes:
        role: The role of the message sender
        content: The text content of the message
        name: Optional name for tool messages
        tool_call_id: Optional tool call ID for tool responses
    """

    role: MessageRole
    content: str
    name: str | None = None
    tool_call_id: str | None = None

    def to_anthropic_format(self) -> dict[str, Any]:
        """Convert to Anthropic API format."""
        return {
            "role": self.role.value,
            "content": self.content,
        }

    def to_openai_format(self) -> dict[str, Any]:
        """Convert to OpenAI API format."""
        msg = {"role": self.role.value, "content": self.content}
        if self.name:
            msg["name"] = self.name
        if self.tool_call_id:
            msg["tool_call_id"] = self.tool_call_id
        return msg

    def to_gemini_format(self) -> dict[str, Any]:
        """Convert to Google Gemini API format."""
        # Gemini uses "model" instead of "assistant"
        role = "model" if self.role == MessageRole.ASSISTANT else self.role.value
        return {
            "role": role,
            "parts": [{"text": self.content}],
        }

    @classmethod
    def user(cls, content: str) -> "Message":
        """Create a user message."""
        return cls(role=MessageRole.USER, content=content)

    @classmethod
    def assistant(cls, content: str) -> "Message":
        """Create an assistant message."""
        return cls(role=MessageRole.ASSISTANT, content=content)

    @classmethod
    def system(cls, content: str) -> "Message":
        """Create a system message."""
        return cls(role=MessageRole.SYSTEM, content=content)


@dataclass
class TokenUsage:
    """Token usage statistics."""

    input_tokens: int = 0
    output_tokens: int = 0
    total_tokens: int = 0

    def __post_init__(self):
        if self.total_tokens == 0:
            self.total_tokens = self.input_tokens + self.output_tokens


@dataclass
class ChatResponse:
    """
    Response from a chat completion.

    Attributes:
        content: The text content of the response
        model: The model used for generation
        usage: Token usage statistics
        finish_reason: Why the generation stopped
        response_id: Unique response identifier
        tool_calls: List of tool calls if any
    """

    content: str
    model: str
    usage: TokenUsage
    finish_reason: str = "stop"
    response_id: str = ""
    tool_calls: list[dict[str, Any]] = field(default_factory=list)

    @classmethod
    def from_anthropic(cls, data: dict[str, Any]) -> "ChatResponse":
        """Create from Anthropic API response."""
        content = ""
        if data.get("content"):  # pragma: no branch
            for block in data["content"]:
                if block.get("type") == "text":  # pragma: no branch
                    content = block.get("text", "")
                    break
            else:  # pragma: no cover
                # No text block found - rare edge case
                pass

        return cls(
            content=content,
            model=data.get("model", ""),
            usage=TokenUsage(
                input_tokens=data.get("usage", {}).get("input_tokens", 0),
                output_tokens=data.get("usage", {}).get("output_tokens", 0),
            ),
            finish_reason=data.get("stop_reason", "stop"),
            response_id=data.get("id", ""),
        )

    @classmethod
    def from_openai(cls, data: dict[str, Any]) -> "ChatResponse":
        """Create from OpenAI API response."""
        choice = data.get("choices", [{}])[0]
        message = choice.get("message", {})

        return cls(
            content=message.get("content", ""),
            model=data.get("model", ""),
            usage=TokenUsage(
                input_tokens=data.get("usage", {}).get("prompt_tokens", 0),
                output_tokens=data.get("usage", {}).get("completion_tokens", 0),
                total_tokens=data.get("usage", {}).get("total_tokens", 0),
            ),
            finish_reason=choice.get("finish_reason", "stop"),
            response_id=data.get("id", ""),
            tool_calls=message.get("tool_calls", []),
        )

    @classmethod
    def from_gemini(cls, data: dict[str, Any], model: str) -> "ChatResponse":
        """Create from Google Gemini API response."""
        candidate = data.get("candidates", [{}])[0]
        content = ""
        if candidate.get("content", {}).get("parts"):
            content = candidate["content"]["parts"][0].get("text", "")

        return cls(
            content=content,
            model=model,
            usage=TokenUsage(
                input_tokens=data.get("usageMetadata", {}).get("promptTokenCount", 0),
                output_tokens=data.get("usageMetadata", {}).get(
                    "candidatesTokenCount", 0
                ),
                total_tokens=data.get("usageMetadata", {}).get("totalTokenCount", 0),
            ),
            finish_reason=candidate.get("finishReason", "STOP"),
        )


@dataclass
class StreamChunk:
    """
    A single chunk in a streaming response.

    Attributes:
        content: The text content delta
        finish_reason: Why the stream ended (if it ended)
        tool_calls: Tool call deltas if any
    """

    content: str = ""
    finish_reason: str | None = None
    tool_calls: list[dict[str, Any]] = field(default_factory=list)


@dataclass
class ToolDefinition:
    """
    Definition of a tool that can be called by the LLM.

    Attributes:
        name: Tool name
        description: Tool description
        parameters: JSON Schema for parameters
    """

    name: str
    description: str
    parameters: dict[str, Any]

    def to_anthropic_format(self) -> dict[str, Any]:
        """Convert to Anthropic tool format."""
        return {
            "name": self.name,
            "description": self.description,
            "input_schema": self.parameters,
        }

    def to_openai_format(self) -> dict[str, Any]:
        """Convert to OpenAI tool format."""
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            },
        }

    def to_gemini_format(self) -> dict[str, Any]:
        """Convert to Gemini function declaration format."""
        return {
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters,
        }
