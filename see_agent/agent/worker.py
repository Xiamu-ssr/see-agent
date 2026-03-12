"""Agent worker process — runs a single AgentLoop with UDS-based IPC.

This module is the entry point for agent subprocesses spawned by
AgentSupervisor.  It connects to the AgentRouter via UDS and uses remote
proxies for screen tools and team communication.

Usage::

    python -m see_agent.agent.worker <agent_id> <sock_path>
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
    agent_id: str,
    sock_path: str,
) -> None:
    """Main async entry point for the worker subprocess."""
    from see_agent.brain.openai_client import OpenAIBrain
    from see_agent.config import AGENTS_DIR, load_agent_config, setup_logging
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
    from see_agent.memory import MarkdownMemoryBackend

    setup_logging()

    # Worker reads config itself — no runtime_config.json needed.
    config: dict[str, Any] = load_agent_config(agent_id)
    agent_dir = AGENTS_DIR / agent_id

    # Inject internal fields that the loop/tools need.
    config["_agent_id"] = agent_id
    config["_agent_dir"] = str(agent_dir)
    config["_session_dir"] = str(agent_dir / "session")
    config["_memory_dir"] = str(agent_dir / "memory")

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

        # Screen tools (remote) — default to enabled.
        screen_access = config.get("screen", {}).get("enabled", True)
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
        denied_tools = config.get("tools", {}).get("disabled", [])
        for name in denied_tools:
            registry._tools.pop(name, None)
            registry._sources.pop(name, None)

        # Memory tools.
        memory_dir = str(agent_dir / "memory")
        from see_agent.hand.tools.memory import MemorySearchTool, WriteMemoryTool

        mem_backend = MarkdownMemoryBackend(memory_dir=Path(memory_dir))
        registry.register(MemorySearchTool(mem_backend), source="memory")
        registry.register(WriteMemoryTool(mem_backend), source="memory")

        # Session directory.
        session_dir = Path(config["_session_dir"])
        session_dir.mkdir(parents=True, exist_ok=True)

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
            session_dir=session_dir,
            owner_display=config.get("_owner_display"),
            task_board=remote_board,
        )

        # Task comes from inbox or resumed session.
        task = ""
        meta_path = session_dir / "meta.json"
        if meta_path.exists():
            import json as _json
            meta = _json.loads(meta_path.read_text())
            task = meta.get("current_task", "")

        result = await loop.run(task)
        logger.info(
            "Worker finished: agent=%s success=%s steps=%d",
            agent_id, result.success, result.total_steps,
        )

        # Write result to a JSON file for the parent to collect.
        result_path = agent_dir / "session" / "result.json"
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
    """CLI entry point: python -m see_agent.agent.worker <agent_id> <sock_path>."""
    if len(sys.argv) < 3:
        print(
            "Usage: python -m see_agent.agent.worker "
            "<agent_id> <sock_path>",
            file=sys.stderr,
        )
        sys.exit(1)

    agent_id = sys.argv[1]
    sock_path = sys.argv[2]

    asyncio.run(_run_worker(agent_id, sock_path))


if __name__ == "__main__":
    main()
