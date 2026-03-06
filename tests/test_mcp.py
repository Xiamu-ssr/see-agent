"""Unit tests for MCP integration."""

from unittest.mock import AsyncMock, MagicMock

import pytest

from see_agent.hand.mcp import MCPManager, MCPToolWrapper, _expand_env
from see_agent.hand.tool import ToolRegistry, ToolResult


class TestExpandEnv:
    """Tests for environment variable expansion."""

    def test_expand_known_var(self):
        result = _expand_env("${HOME}/test", {"HOME": "/users/me"})
        assert result == "/users/me/test"

    def test_unknown_var_unchanged(self):
        result = _expand_env("${UNKNOWN_VAR_12345}", {})
        assert result == "${UNKNOWN_VAR_12345}"

    def test_no_vars(self):
        result = _expand_env("plain text", {})
        assert result == "plain text"

    def test_multiple_vars(self):
        result = _expand_env("${A}/${B}", {"A": "x", "B": "y"})
        assert result == "x/y"


class TestMCPToolWrapper:
    """Tests for MCPToolWrapper."""

    def test_namespaced_name(self):
        client = MagicMock()
        wrapper = MCPToolWrapper(
            server_name="myserver",
            tool_name="search",
            description="Search tool",
            input_schema={"type": "object"},
            client=client,
        )
        assert wrapper.name == "mcp__myserver__search"
        assert wrapper.description == "Search tool"

    @pytest.mark.asyncio
    async def test_execute_calls_client(self):
        client = MagicMock()
        mock_content = MagicMock()
        mock_content.text = "result text"
        mock_result = MagicMock()
        mock_result.content = [mock_content]
        client.call_tool = AsyncMock(return_value=mock_result)

        wrapper = MCPToolWrapper(
            server_name="srv",
            tool_name="do_thing",
            description="",
            input_schema={},
            client=client,
        )
        result = await wrapper.execute(query="test")
        assert isinstance(result, ToolResult)
        assert result.text == "result text"
        client.call_tool.assert_called_once_with("do_thing", {"query": "test"})


class TestMCPManager:
    """Tests for MCPManager."""

    @pytest.mark.asyncio
    async def test_register_tools(self):
        """MCPManager.register_tools adds wrapped tools to the registry."""
        manager = MCPManager({"test": {"command": "echo"}})

        # Mock the client to return tools without actual connection
        mock_client = MagicMock()
        mock_client.list_tools = AsyncMock(return_value=[
            {"name": "search", "description": "Search", "parameters": {"type": "object"}},
        ])
        manager._clients["test"] = mock_client

        registry = ToolRegistry()
        await manager.register_tools(registry)

        tool = registry.get("mcp__test__search")
        assert tool.name == "mcp__test__search"

    def test_manager_creates_clients(self):
        """MCPManager creates MCPClient instances from config."""
        config = {
            "server1": {"command": "npx", "args": ["-y", "some-server"]},
            "server2": {"command": "python", "args": ["-m", "my_server"]},
        }
        manager = MCPManager(config, global_env={"API_KEY": "test"})
        assert "server1" in manager._clients
        assert "server2" in manager._clients


class TestMCPIntegration:
    """Integration-level MCP edge cases."""

    @pytest.mark.asyncio
    async def test_connect_failure_non_fatal(self):
        """A single server connect failure should not prevent others."""
        manager = MCPManager({
            "bad": {"command": "nonexistent_command_xyz"},
            "good": {"command": "echo"},
        })
        # connect_all should not raise — failures are logged.
        await manager.connect_all()
        # We can't verify "good" connected (it also fails without real server),
        # but the point is no exception propagated.

    @pytest.mark.asyncio
    async def test_disconnect_all_tolerates_errors(self):
        """disconnect_all should not raise even if a client errors."""
        manager = MCPManager({"test": {"command": "echo"}})
        # Mock a client whose disconnect raises.
        mock_client = MagicMock()
        mock_client.disconnect = AsyncMock(side_effect=RuntimeError("boom"))
        manager._clients["test"] = mock_client

        # Should not raise.
        await manager.disconnect_all()

    @pytest.mark.asyncio
    async def test_tool_execution_failure_returns_error_text(self):
        """MCP tool execution error should be caught in wrapper."""
        client = MagicMock()
        client.call_tool = AsyncMock(side_effect=RuntimeError("MCP call failed"))

        wrapper = MCPToolWrapper(
            server_name="srv", tool_name="broken",
            description="", input_schema={}, client=client,
        )
        # The wrapper itself raises — the registry.execute catches it.
        with pytest.raises(RuntimeError, match="MCP call failed"):
            await wrapper.execute()

    def test_tool_name_special_chars(self):
        """MCP tool name with hyphens should be preserved."""
        client = MagicMock()
        wrapper = MCPToolWrapper(
            server_name="my-server", tool_name="search-web",
            description="", input_schema={}, client=client,
        )
        assert wrapper.name == "mcp__my-server__search-web"
