"""MCP (Model Context Protocol) integration — connect external tool servers.

Requires the optional ``mcp`` package::

    pip install see-agent[mcp]
"""

from __future__ import annotations

import logging
import os
import re
from typing import Any

from see_agent.hand.tool import Tool, ToolRegistry, ToolResult

logger = logging.getLogger(__name__)

_ENV_VAR_RE = re.compile(r"\$\{(\w+)\}")


def _expand_env(value: str, env: dict[str, str] | None = None) -> str:
    """Replace ``${VAR}`` placeholders with environment variable values."""
    combined = {**os.environ, **(env or {})}

    def _replace(match: re.Match[str]) -> str:
        var = match.group(1)
        return combined.get(var) or match.group(0)

    return _ENV_VAR_RE.sub(_replace, value)


class MCPClient:
    """Connect to a single MCP server (stdio or HTTP).

    Parameters:
        name: Logical name for this server (used in tool namespacing).
        command: The command to start the stdio server (list of strings).
        args: Additional arguments appended to the command.
        url: HTTP URL for HTTP-based MCP servers (mutually exclusive with command).
        env: Extra environment variables for the subprocess.
        global_env: Global env vars from config ``"env"`` section.
    """

    def __init__(
        self,
        name: str,
        *,
        command: str | None = None,
        args: list[str] | None = None,
        url: str | None = None,
        env: dict[str, str] | None = None,
        global_env: dict[str, str] | None = None,
    ) -> None:
        self.name = name
        self._command = command
        self._args = args or []
        self._url = url
        self._env = env or {}
        self._global_env = global_env or {}
        self._session: Any = None
        self._client: Any = None

    async def connect(self) -> None:
        """Establish connection to the MCP server."""
        try:
            from mcp import ClientSession  # type: ignore[import-untyped]  # noqa: I001
            from mcp.client.stdio import StdioServerParameters, stdio_client  # type: ignore[import-untyped]
        except ImportError as exc:
            raise ImportError(
                "mcp package is required for MCP support. "
                "Install with: pip install see-agent[mcp]"
            ) from exc

        if self._command:
            # Expand env vars in command and args
            env = {
                k: _expand_env(v, self._global_env)
                for k, v in self._env.items()
            }
            server_params = StdioServerParameters(
                command=_expand_env(self._command, self._global_env),
                args=[_expand_env(a, self._global_env) for a in self._args],
                env={**os.environ, **env},
            )
            self._transport = stdio_client(server_params)
            read, write = await self._transport.__aenter__()
            self._session = ClientSession(read, write)
            await self._session.__aenter__()
            await self._session.initialize()
            logger.info("MCP server '%s' connected (stdio)", self.name)
        else:
            raise ValueError(f"MCP server '{self.name}': must specify 'command'")

    async def disconnect(self) -> None:
        """Close the MCP server connection."""
        if self._session is not None:
            try:
                await self._session.__aexit__(None, None, None)
            except Exception:
                logger.warning("Error closing MCP session '%s'", self.name, exc_info=True)
        if hasattr(self, "_transport") and self._transport is not None:
            try:
                await self._transport.__aexit__(None, None, None)
            except Exception:
                logger.warning("Error closing MCP transport '%s'", self.name, exc_info=True)

    async def list_tools(self) -> list[dict[str, Any]]:
        """List available tools from the MCP server."""
        if self._session is None:
            raise RuntimeError(f"MCP server '{self.name}' not connected")
        result = await self._session.list_tools()
        tools = []
        for tool in result.tools:
            tools.append({
                "name": tool.name,
                "description": tool.description or "",
                "parameters": tool.inputSchema or {"type": "object", "properties": {}},
            })
        return tools

    async def call_tool(self, name: str, args: dict[str, Any]) -> Any:
        """Call a tool on the MCP server."""
        if self._session is None:
            raise RuntimeError(f"MCP server '{self.name}' not connected")
        result = await self._session.call_tool(name, args)
        return result


class MCPToolWrapper(Tool):
    """Wraps an MCP tool as a local Tool for the registry.

    Tool name format: ``mcp__{server_name}__{tool_name}``
    """

    def __init__(
        self,
        server_name: str,
        tool_name: str,
        description: str,
        input_schema: dict[str, Any],
        client: MCPClient,
    ) -> None:
        self._server_name = server_name
        self._tool_name = tool_name
        self._description = description
        self._input_schema = input_schema
        self._client = client

    @property
    def name(self) -> str:
        return f"mcp__{self._server_name}__{self._tool_name}"

    @property
    def description(self) -> str:
        return self._description

    @property
    def parameters(self) -> dict[str, Any]:
        return self._input_schema

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call_tool(self._tool_name, kwargs)
        # Extract text from MCP result
        text_parts = []
        for content in getattr(result, "content", []):
            if hasattr(content, "text"):
                text_parts.append(content.text)
            else:
                text_parts.append(str(content))
        return ToolResult(text="\n".join(text_parts) if text_parts else str(result))


class MCPManager:
    """Manage multiple MCP server connections.

    Parameters:
        server_configs: Dict of server_name → server config from ``config.json``.
        global_env: Global env vars from config ``"env"`` section.
    """

    def __init__(
        self,
        server_configs: dict[str, dict[str, Any]],
        global_env: dict[str, str] | None = None,
    ) -> None:
        self._clients: dict[str, MCPClient] = {}
        for name, cfg in server_configs.items():
            self._clients[name] = MCPClient(
                name=name,
                command=cfg.get("command"),
                args=cfg.get("args", []),
                url=cfg.get("url"),
                env=cfg.get("env", {}),
                global_env=global_env,
            )

    async def connect_all(self) -> None:
        """Connect to all configured MCP servers."""
        for name, client in self._clients.items():
            try:
                await client.connect()
            except Exception:
                logger.exception("Failed to connect MCP server '%s'", name)

    async def disconnect_all(self) -> None:
        """Disconnect from all MCP servers."""
        for name, client in self._clients.items():
            try:
                await client.disconnect()
            except Exception:
                logger.exception("Failed to disconnect MCP server '%s'", name)

    async def register_tools(self, registry: ToolRegistry) -> None:
        """Discover tools from all connected servers and register them."""
        for name, client in self._clients.items():
            try:
                tools = await client.list_tools()
                for tool_def in tools:
                    wrapper = MCPToolWrapper(
                        server_name=name,
                        tool_name=tool_def["name"],
                        description=tool_def.get("description", ""),
                        input_schema=tool_def.get("parameters", {}),
                        client=client,
                    )
                    try:
                        registry.register(wrapper)
                        logger.info("Registered MCP tool: %s", wrapper.name)
                    except ValueError:
                        logger.warning("Duplicate MCP tool name: %s", wrapper.name)
            except Exception:
                logger.exception("Failed to list tools from MCP server '%s'", name)
