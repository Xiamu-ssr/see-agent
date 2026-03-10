"""API routes — MCP server management."""

from __future__ import annotations

import logging
import subprocess
import sys
from typing import Any

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

from see_agent.config import save_config
from see_agent.server.schemas import McpInstallResponse

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/mcp", tags=["mcp"])


class InstallMcpRequest(BaseModel):
    name: str
    install_type: str  # "npm" | "pip" | "manual"
    package: str | None = None
    params: str | None = None
    command: str | None = None
    args: list[str] | None = None
    env: dict[str, str] | None = None


@router.post("/install")
async def install_mcp(body: InstallMcpRequest, request: Request) -> McpInstallResponse:
    """Install and configure an MCP server."""
    config = request.app.state.config
    if "mcp_servers" not in config:
        config["mcp_servers"] = {}

    server_cfg: dict[str, Any]

    if body.install_type == "npm":
        if not body.package:
            raise HTTPException(status_code=400, detail="package is required for npm install")
        args = ["-y", body.package]
        if body.params:
            args.extend(body.params.split())
        server_cfg = {
            "type": "stdio",
            "command": "npx",
            "args": args,
        }

    elif body.install_type == "pip":
        if not body.package:
            raise HTTPException(status_code=400, detail="package is required for pip install")
        result = subprocess.run(
            [sys.executable, "-m", "pip", "install", body.package],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=f"pip install failed: {result.stderr}")
        module_name = body.package.replace("-", "_")
        args = ["-m", module_name]
        if body.params:
            args.extend(body.params.split())
        server_cfg = {
            "type": "stdio",
            "command": "python",
            "args": args,
        }

    elif body.install_type == "manual":
        if not body.command:
            raise HTTPException(status_code=400, detail="command is required for manual install")
        server_cfg = {
            "type": "stdio",
            "command": body.command,
            "args": body.args or [],
        }
        if body.env:
            server_cfg["env"] = body.env

    else:
        raise HTTPException(status_code=400, detail=f"Unknown install_type: {body.install_type}")

    config["mcp_servers"][body.name] = server_cfg
    save_config(config)
    request.app.state.config = config
    return McpInstallResponse(status="ok", name=body.name, config=server_cfg)
