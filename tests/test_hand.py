"""Unit tests for Tool base class, ToolResult, and ToolRegistry (see_agent/hand/tool.py)."""

from typing import Any

import pytest

from see_agent.hand.tool import Tool, ToolRegistry, ToolResult, ToolResultImage

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

    async def execute(self, **kwargs: Any) -> ToolResult:
        return ToolResult(text=f"echo: {kwargs.get('text', '')}")


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

    async def execute(self, **kwargs: Any) -> ToolResult:
        return ToolResult(text="done")


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

    def test_register_with_source(self):
        """register() records the source tag."""
        registry = ToolRegistry()
        registry.register(DummyTool(), source="mcp")
        assert registry._sources["dummy"] == "mcp"

    def test_register_default_source(self):
        """Default source is 'builtin'."""
        registry = ToolRegistry()
        registry.register(DummyTool())
        assert registry._sources["dummy"] == "builtin"

    def test_get_filtered_allowed(self):
        """get_filtered with allowed returns only whitelisted tools."""
        registry = ToolRegistry()
        registry.register(DummyTool())
        registry.register(AnotherTool())
        filtered = registry.get_filtered(allowed=["dummy"])
        assert len(filtered) == 1
        assert filtered[0].name == "dummy"

    def test_get_filtered_denied(self):
        """get_filtered with denied excludes blacklisted tools."""
        registry = ToolRegistry()
        registry.register(DummyTool())
        registry.register(AnotherTool())
        filtered = registry.get_filtered(denied=["dummy"])
        assert len(filtered) == 1
        assert filtered[0].name == "another"

    def test_get_openai_schemas_filtered(self):
        """get_openai_schemas_filtered respects allowed/denied."""
        registry = ToolRegistry()
        registry.register(DummyTool())
        registry.register(AnotherTool())
        schemas = registry.get_openai_schemas_filtered(allowed=["another"])
        assert len(schemas) == 1
        assert schemas[0]["function"]["name"] == "another"

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
        """execute() routes to the correct tool and returns a ToolResult."""
        registry = ToolRegistry()
        registry.register(DummyTool())

        result = await registry.execute("dummy", {"text": "hello world"})
        assert isinstance(result, ToolResult)
        assert result.text == "echo: hello world"
        assert result.images == []

    @pytest.mark.asyncio
    async def test_registry_execute_unknown(self):
        """execute() raises KeyError for an unknown tool name."""
        registry = ToolRegistry()

        with pytest.raises(KeyError, match="Unknown tool"):
            await registry.execute("nonexistent", {})

    @pytest.mark.asyncio
    async def test_registry_execute_returns_tool_result(self):
        """A tool returning ToolResult is returned as-is."""
        registry = ToolRegistry()
        registry.register(DummyTool())

        result = await registry.execute("dummy", {"text": "wrap me"})
        assert isinstance(result, ToolResult)
        assert result.text == "echo: wrap me"

    @pytest.mark.asyncio
    async def test_registry_execute_passes_through_tool_result(self):
        """A tool returning ToolResult is passed through without wrapping."""

        class ImageTool(Tool):
            @property
            def name(self) -> str:
                return "img"

            @property
            def description(self) -> str:
                return "Returns an image."

            @property
            def parameters(self) -> dict[str, Any]:
                return {"type": "object", "properties": {}}

            async def execute(self, **kwargs: Any) -> ToolResult:
                return ToolResult(
                    text="image captured",
                    images=[ToolResultImage(base64="abc123", mime_type="image/png")],
                )

        registry = ToolRegistry()
        registry.register(ImageTool())

        result = await registry.execute("img", {})
        assert isinstance(result, ToolResult)
        assert result.text == "image captured"
        assert len(result.images) == 1
        assert result.images[0].base64 == "abc123"


# -------------------------------------------------------------------- #
# Tests for HotkeyTool key aliases
# -------------------------------------------------------------------- #


class TestHotkeyAliases:
    """Verify KEY_ALIASES normalisation in HotkeyTool."""

    def test_aliases_defined(self):
        from see_agent.hand.tools.hotkey import KEY_ALIASES

        assert KEY_ALIASES["control"] == "ctrl"
        assert KEY_ALIASES["cmd"] == "command"
        assert KEY_ALIASES["option"] == "alt"
        assert KEY_ALIASES["enter"] == "return"
        assert KEY_ALIASES["esc"] == "escape"

    @pytest.mark.asyncio
    async def test_normalise_control_to_ctrl(self):
        """'control' should be normalised to 'ctrl' before pyautogui."""
        from unittest.mock import patch

        from see_agent.hand.tools.hotkey import HotkeyTool

        tool = HotkeyTool()
        with patch("see_agent.hand.tools.hotkey.pyautogui") as mock_pag:
            result = await tool.execute(keys=["control", "c"])

        mock_pag.hotkey.assert_called_once_with("ctrl", "c")
        assert "ctrl+c" in result.text

    @pytest.mark.asyncio
    async def test_already_correct_keys_unchanged(self):
        """Keys that need no normalisation pass through unchanged."""
        from unittest.mock import patch

        from see_agent.hand.tools.hotkey import HotkeyTool

        tool = HotkeyTool()
        with patch("see_agent.hand.tools.hotkey.pyautogui") as mock_pag:
            await tool.execute(keys=["command", "v"])

        mock_pag.hotkey.assert_called_once_with("command", "v")

    @pytest.mark.asyncio
    async def test_case_insensitive(self):
        """Alias lookup should be case-insensitive."""
        from unittest.mock import patch

        from see_agent.hand.tools.hotkey import HotkeyTool

        tool = HotkeyTool()
        with patch("see_agent.hand.tools.hotkey.pyautogui") as mock_pag:
            await tool.execute(keys=["Control", "Esc"])

        mock_pag.hotkey.assert_called_once_with("ctrl", "escape")
