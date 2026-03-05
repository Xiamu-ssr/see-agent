"""Unit tests for Tool base class and ToolRegistry (src/hand/tool.py)."""

from typing import Any

import pytest

from src.hand.tool import Tool, ToolRegistry

# -------------------------------------------------------------------- #
# Dummy tool for testing
# -------------------------------------------------------------------- #


class DummyTool(Tool):
    """A trivial tool implementation for testing purposes."""

    @property
    def name(self) -> str:
        return "dummy"

    @property
    def description(self) -> str:
        return "A dummy tool that echoes its input."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to echo back.",
                },
            },
            "required": ["text"],
        }

    async def execute(self, **kwargs: Any) -> str:
        return f"echo: {kwargs.get('text', '')}"


class AnotherTool(Tool):
    """A second tool to test multi-registration."""

    @property
    def name(self) -> str:
        return "another"

    @property
    def description(self) -> str:
        return "Another tool."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {},
        }

    async def execute(self, **kwargs: Any) -> str:
        return "done"


# -------------------------------------------------------------------- #
# Tests for Tool
# -------------------------------------------------------------------- #


class TestTool:
    """Tests for the Tool abstract base class (via DummyTool)."""

    def test_tool_schema(self):
        """to_openai_schema() generates the expected OpenAI function-calling format."""
        tool = DummyTool()
        schema = tool.to_openai_schema()

        assert schema["type"] == "function"
        assert "function" in schema

        func = schema["function"]
        assert func["name"] == "dummy"
        assert func["description"] == "A dummy tool that echoes its input."
        assert func["parameters"]["type"] == "object"
        assert "text" in func["parameters"]["properties"]
        assert func["parameters"]["required"] == ["text"]


# -------------------------------------------------------------------- #
# Tests for ToolRegistry
# -------------------------------------------------------------------- #


class TestToolRegistry:
    """Tests for ToolRegistry."""

    def test_registry_register_and_get(self):
        """Register a tool and retrieve it by name."""
        registry = ToolRegistry()
        tool = DummyTool()
        registry.register(tool)

        retrieved = registry.get("dummy")
        assert retrieved is tool
        assert retrieved.name == "dummy"

    def test_registry_duplicate(self):
        """Registering a tool with the same name twice raises ValueError."""
        registry = ToolRegistry()
        registry.register(DummyTool())

        with pytest.raises(ValueError, match="already registered"):
            registry.register(DummyTool())

    def test_registry_unknown_tool(self):
        """Getting an unregistered tool name raises KeyError."""
        registry = ToolRegistry()

        with pytest.raises(KeyError, match="Unknown tool"):
            registry.get("nonexistent")

    def test_registry_get_schemas(self):
        """get_openai_schemas() returns a list of schemas for all registered tools."""
        registry = ToolRegistry()
        registry.register(DummyTool())
        registry.register(AnotherTool())

        schemas = registry.get_openai_schemas()
        assert isinstance(schemas, list)
        assert len(schemas) == 2

        names = {s["function"]["name"] for s in schemas}
        assert names == {"dummy", "another"}

        # Each schema has the correct top-level structure
        for schema in schemas:
            assert schema["type"] == "function"
            assert "function" in schema
            assert "name" in schema["function"]
            assert "description" in schema["function"]
            assert "parameters" in schema["function"]

    @pytest.mark.asyncio
    async def test_registry_execute(self):
        """execute() routes to the correct tool and returns its result."""
        registry = ToolRegistry()
        registry.register(DummyTool())

        result = await registry.execute("dummy", {"text": "hello world"})
        assert result == "echo: hello world"

    @pytest.mark.asyncio
    async def test_registry_execute_unknown(self):
        """execute() raises KeyError for an unknown tool name."""
        registry = ToolRegistry()

        with pytest.raises(KeyError, match="Unknown tool"):
            await registry.execute("nonexistent", {})
