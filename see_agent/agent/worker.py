"""Agent worker process — runs a single AgentLoop with UDS-based IPC.

This module is the entry point for agent subprocesses spawned by
TeamManager.  It connects to the AgentRouter via UDS and uses remote
proxies (RemoteBus, RemoteBoard, RemoteScreen*) instead of local
in-process objects.

Usage::

    python -m see_agent.agent.worker <config_json> <sock_path> <task>
"""

from __future__ import annotations

import asyncio
import json
import logging
import sys
from pathlib import Path
from typing import Any

from see_agent.eye.base import BaseEye, Screenshot

logger = logging.getLogger(__name__)


async def _run_worker(
    config_path: str,
    sock_path: str,
    task: str,
) -> None:
    """Main async entry point for the worker subprocess."""
    from see_agent.brain.openai_client import OpenAIBrain
    from see_agent.config import setup_logging
    from see_agent.hand.tool import ToolRegistry
    from see_agent.ipc.client import UDSClient
    from see_agent.ipc.remote_tools import (
        RemoteBoard,
        RemoteBus,
        RemoteClickTool,
        RemoteDragTool,
        RemoteHotkeyTool,
        RemoteScreenAcquireTool,
        RemoteScreenReleaseTool,
        RemoteScreenshotTool,
        RemoteScrollTool,
        RemoteTypeTextTool,
    )
    from see_agent.memory.file_backend import FileMemory

    setup_logging()

    # Load config written by TeamManager.
    config: dict[str, Any] = json.loads(Path(config_path).read_text())
    agent_id: str = config["_agent_id"]
    screen_access: bool = config.get("_screen_access", True)

    logger.info("Worker starting: agent=%s sock=%s", agent_id, sock_path)

    # Connect to main process.
    client = UDSClient(Path(sock_path))
    await client.connect()

    try:
        # Build LLM brain.
        llm_cfg = config["llm"]
        brain = OpenAIBrain(
            base_url=llm_cfg["base_url"],
            api_key=llm_cfg["api_key"],
            model=llm_cfg["model"],
        )

        # Build remote proxies.
        remote_bus = RemoteBus(client, agent_id)
        remote_board = RemoteBoard(client)

        # Build tool registry with remote tools.
        registry = ToolRegistry()

        # Register non-screen tools that run locally in subprocess.
        from see_agent.hand.tools.finished import FinishedTool
        from see_agent.hand.tools.shell import ShellTool
        from see_agent.hand.tools.wait import WaitTool

        registry.register(ShellTool())
        registry.register(WaitTool())
        registry.register(FinishedTool())

        # Screen tools (remote) — only if agent has screen access.
        if screen_access:
            registry.register(RemoteScreenshotTool(client, agent_id))
            registry.register(RemoteClickTool(client, agent_id))
            registry.register(RemoteTypeTextTool(client, agent_id))
            registry.register(RemoteScrollTool(client, agent_id))
            registry.register(RemoteDragTool(client, agent_id))
            registry.register(RemoteHotkeyTool(client, agent_id))
            registry.register(
                RemoteScreenAcquireTool(client, agent_id), source="screen",
            )
            registry.register(
                RemoteScreenReleaseTool(client, agent_id), source="screen",
            )

        # Team tools (using remote bus and board).
        from see_agent.hand.tools.team_tools import (
            AssignTaskTool,
            ClaimTaskTool,
            CompleteTaskTool,
            CreateTaskTool,
            ListTasksTool,
            SendMessageTool,
            UpdateTaskTool,
        )

        leader_id = config.get("_leader_id")
        registry.register(
            SendMessageTool(remote_bus, agent_id, leader_id=leader_id),
            source="team",
        )
        registry.register(ListTasksTool(remote_board), source="team")
        registry.register(
            CreateTaskTool(remote_board, agent_id), source="team",
        )
        registry.register(
            ClaimTaskTool(remote_board, agent_id), source="team",
        )
        registry.register(
            CompleteTaskTool(remote_board, agent_id), source="team",
        )
        registry.register(UpdateTaskTool(remote_board), source="team")
        registry.register(AssignTaskTool(remote_board), source="team")

        # Apply agent-level tool filtering.
        denied_tools = config.get("_denied_tools", [])
        for name in denied_tools:
            registry._tools.pop(name, None)
            registry._sources.pop(name, None)

        # Memory.
        memory_dir = config.get("_memory_dir")
        memory = FileMemory(memory_dir=Path(memory_dir)) if memory_dir else None

        # Session root.
        session_root = Path(config["_session_root"])
        session_root.mkdir(parents=True, exist_ok=True)

        # Eye — we need a dummy eye for AgentLoop signature.
        # In subprocess mode, screenshots go through RemoteScreenshotTool,
        # but AgentLoop still expects an eye for its initial capture.
        # We use a NullEye that returns empty screenshots.
        eye = _NullEye()

        # Build AgentLoop.
        from see_agent.agent.loop import AgentLoop

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config=config,
            agent_id=agent_id,
            session_root=session_root,
            team_bus=remote_bus,
            memory=memory,
            owner_display=config.get("_owner_display"),
            task_board=remote_board,
        )

        result = await loop.run(task)
        logger.info(
            "Worker finished: agent=%s success=%s steps=%d",
            agent_id, result.success, result.total_steps,
        )

        # Write result to a JSON file for the parent to collect.
        result_path = Path(config["_result_path"])
        result_path.write_text(
            json.dumps({
                "summary": result.summary,
                "success": result.success,
                "total_steps": result.total_steps,
                "elapsed_seconds": result.elapsed_seconds,
                "session_id": result.session_id,
            }),
        )

    finally:
        await client.close()


class _NullEye(BaseEye):
    """Minimal eye implementation for subprocess mode.

    The actual screenshot capture goes through RemoteScreenshotTool.
    This exists only to satisfy AgentLoop's constructor.
    """

    async def capture(self) -> Screenshot:
        # Return a minimal 1x1 transparent screenshot.
        import base64 as b64

        pixel = b64.b64encode(b"\x00").decode()
        return Screenshot(
            base64=pixel,
            width=1,
            height=1,
            scale_factor=1.0,
            image=None,
        )


def main() -> None:
    """CLI entry point: python -m see_agent.agent.worker <config> <sock> <task>."""
    if len(sys.argv) < 4:
        print(
            "Usage: python -m see_agent.agent.worker "
            "<config_json> <sock_path> <task>",
            file=sys.stderr,
        )
        sys.exit(1)

    config_path = sys.argv[1]
    sock_path = sys.argv[2]
    task = sys.argv[3]

    asyncio.run(_run_worker(config_path, sock_path, task))


if __name__ == "__main__":
    main()
