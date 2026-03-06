"""Tool base class, ToolResult data types, and ToolRegistry."""

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)


# -------------------------------------------------------------------- #
# Tool result data types
# -------------------------------------------------------------------- #


@dataclass
class ToolResultImage:
    """An image returned as part of a tool result."""

    base64: str
    mime_type: str = "image/webp"
    detail: str = "high"


@dataclass
class ToolResult:
    """Rich result from tool execution, supporting text + optional images."""

    text: str
    images: list[ToolResultImage] = field(default_factory=list)


# -------------------------------------------------------------------- #
# Tool ABC
# -------------------------------------------------------------------- #


class Tool(ABC):
    """Abstract base class that every tool must implement.

    Each concrete tool is self-contained: it carries its own name, description,
    parameter schema, and execution logic.  ``to_openai_schema()`` converts the
    tool metadata into the OpenAI function-calling format so that callers never
    need to construct the schema manually.
    """

    @property
    @abstractmethod
    def name(self) -> str:
        """Unique tool name used in LLM function calling."""
        ...

    @property
    @abstractmethod
    def description(self) -> str:
        """Human-readable description sent to the LLM."""
        ...

    @property
    @abstractmethod
    def parameters(self) -> dict[str, Any]:
        """JSON Schema describing the tool's parameters."""
        ...

    @abstractmethod
    async def execute(self, **kwargs: Any) -> str | ToolResult:
        """Run the tool with the given arguments and return a result.

        May return a plain ``str`` (backward-compatible) or a :class:`ToolResult`
        for richer responses including images.

        All implementations must be async-safe even if the underlying operation
        is synchronous (wrap with ``asyncio.to_thread`` when appropriate).
        """
        ...

    def to_openai_schema(self) -> dict[str, Any]:
        """Generate the OpenAI function-calling tool definition.

        Returns a dict of the form::

            {
                "type": "function",
                "function": {
                    "name": ...,
                    "description": ...,
                    "parameters": ...
                }
            }
        """
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            },
        }


class ToolRegistry:
    """Registry that holds all available tools and routes execution requests.

    Usage::

        registry = ToolRegistry()
        registry.register(ClickTool())
        schemas = registry.get_openai_schemas()   # pass to LLM API
        result = await registry.execute("click", {"x": 100, "y": 200})
    """

    def __init__(self) -> None:
        self._tools: dict[str, Tool] = {}

    def register(self, tool: Tool) -> None:
        """Register a tool instance.  Raises ``ValueError`` on duplicate names."""
        if tool.name in self._tools:
            raise ValueError(f"Tool '{tool.name}' is already registered")
        self._tools[tool.name] = tool
        logger.debug("Registered tool: %s", tool.name)

    def get(self, name: str) -> Tool:
        """Return the tool with the given *name*.

        Raises ``KeyError`` if no such tool is registered.
        """
        try:
            return self._tools[name]
        except KeyError:
            raise KeyError(f"Unknown tool: '{name}'") from None

    def get_openai_schemas(self) -> list[dict[str, Any]]:
        """Return OpenAI function-calling definitions for all registered tools."""
        return [tool.to_openai_schema() for tool in self._tools.values()]

    async def execute(self, name: str, args: dict[str, Any]) -> ToolResult:
        """Look up the tool by *name* and execute it with *args*.

        Returns a :class:`ToolResult`.  If the tool returns a plain ``str``
        it is automatically wrapped.

        Raises ``KeyError`` for unknown tools and propagates any exception
        raised by the tool itself.
        """
        tool = self.get(name)
        logger.info("Executing tool '%s' with args: %s", name, args)
        try:
            raw = await tool.execute(**args)
            if isinstance(raw, ToolResult):
                result = raw
            else:
                result = ToolResult(text=str(raw))
            logger.info("Tool '%s' result: %s", name, result.text)
            return result
        except Exception:
            logger.exception("Tool '%s' raised an exception", name)
            raise
