"""Abstract base class for LLM backends and shared data types."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ToolCallInfo:
    """Parsed representation of a single tool call from the LLM response."""

    id: str
    name: str
    arguments: dict[str, Any]


@dataclass
class BrainResponse:
    """Standardised response returned by all Brain implementations.

    Attributes:
        content: The text portion of the assistant message, or ``None`` when the
            model only returned tool calls without accompanying text.
        tool_calls: A (possibly empty) list of parsed tool call requests.
        raw: The raw API response message object so that callers can, e.g.,
            serialise it back into the conversation history with
            ``raw.model_dump()``.
    """

    content: str | None
    tool_calls: list[ToolCallInfo] = field(default_factory=list)
    raw: Any = None  # openai ChatCompletionMessage or equivalent


class BaseBrain(ABC):
    """Abstract interface that every LLM backend must implement."""

    @abstractmethod
    async def chat(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
    ) -> BrainResponse:
        """Send *messages* and *tools* to the LLM and return a parsed response.

        Parameters:
            messages: The full conversation history in OpenAI message format.
            tools: Tool definitions in OpenAI function-calling format.

        Returns:
            A :class:`BrainResponse` containing the assistant's text and any
            tool calls.
        """
        ...
