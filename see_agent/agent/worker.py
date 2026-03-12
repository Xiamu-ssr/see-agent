"""Agent worker process — runs a single agent with persistent loop.

This module is the entry point for agent subprocesses spawned by
AgentSupervisor. The worker is long-lived: it idles when no messages
are pending and wakes up via SIGUSR1 from the supervisor.

Usage::

    python -m see_agent.agent.worker <agent_id> <sock_path>
"""

from __future__ import annotations

import asyncio
import json
import logging
import signal
import sys
from pathlib import Path
from typing import Any

from see_agent.eye.base import BaseEye, Screenshot
from see_agent.ipc.message import Message

logger = logging.getLogger(__name__)


# ── Inbox helpers ────────────────────────────────────────────────────

def _read_cursor(agent_dir: Path) -> int:
    """Read last_read_id from inbox_cursor.json. Returns 0 if missing."""
    cursor_path = agent_dir / "inbox_cursor.json"
    if cursor_path.exists():
        try:
            data = json.loads(cursor_path.read_text())
            return data.get("last_read_id", 0)
        except (json.JSONDecodeError, KeyError):
            pass
    return 0


def _write_cursor(agent_dir: Path, last_read_id: int) -> None:
    """Persist the cursor."""
    cursor_path = agent_dir / "inbox_cursor.json"
    cursor_path.write_text(json.dumps({"last_read_id": last_read_id}))


def _drain_inbox(agent_dir: Path, cursor: int) -> tuple[list[Message], int]:
    """Read all messages from inbox.jsonl with msg_id > cursor.

    Returns (messages, new_cursor).
    """
    inbox_path = agent_dir / "inbox.jsonl"
    if not inbox_path.exists():
        return [], cursor

    messages: list[Message] = []
    new_cursor = cursor
    for line in inbox_path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            data = json.loads(line)
        except json.JSONDecodeError:
            continue
        msg_id = data.get("msg_id", 0)
        if msg_id <= cursor:
            continue
        messages.append(Message.from_json(line))
        new_cursor = max(new_cursor, msg_id)

    return messages, new_cursor


# ── Worker main loop ─────────────────────────────────────────────────

async def _run_worker(agent_id: str, sock_path: str) -> None:
    """Main async entry point — long-lived worker with inbox drain loop."""
    from see_agent.agent.loop import AgentLoop
    from see_agent.agent.runtime import AgentRuntime
    from see_agent.brain.openai_client import OpenAIBrain
    from see_agent.config import AGENTS_DIR, load_agent_config, setup_logging
    from see_agent.hand.tool import ToolRegistry
    from see_agent.memory import MarkdownMemoryBackend

    setup_logging()

    # Worker reads config itself.
    config: dict[str, Any] = load_agent_config(agent_id)
    agent_dir = AGENTS_DIR / agent_id

    # Inject internal fields.
    config["_agent_id"] = agent_id
    config["_agent_dir"] = str(agent_dir)
    config["_session_dir"] = str(agent_dir / "session")
    config["_memory_dir"] = str(agent_dir / "memory")

    logger.info("Worker starting: agent=%s sock=%s", agent_id, sock_path)

    # ── Build LLM brain ──
    llm_cfg = config["llm"]
    brain = OpenAIBrain(
        base_url=llm_cfg["base_url"],
        api_key=llm_cfg["api_key"],
        model=llm_cfg["model"],
    )

    # ── Build tool registry (local tools only for now) ──
    registry = ToolRegistry()

    from see_agent.hand.tools.finished import FinishedTool
    from see_agent.hand.tools.shell import ShellTool
    from see_agent.hand.tools.wait import WaitTool

    registry.register(ShellTool())
    registry.register(WaitTool())
    registry.register(FinishedTool())

    # Memory tools.
    memory_dir = agent_dir / "memory"
    memory_dir.mkdir(exist_ok=True)
    from see_agent.hand.tools.memory import MemorySearchTool, WriteMemoryTool

    mem_backend = MarkdownMemoryBackend(memory_dir=memory_dir)
    registry.register(MemorySearchTool(mem_backend), source="memory")
    registry.register(WriteMemoryTool(mem_backend), source="memory")

    # Apply tool filtering.
    denied_tools = config.get("tools", {}).get("disabled", [])
    for name in denied_tools:
        registry._tools.pop(name, None)
        registry._sources.pop(name, None)

    # ── Session directory ──
    session_dir = Path(config["_session_dir"])
    session_dir.mkdir(parents=True, exist_ok=True)

    # ── Eye (null for now — screenshots go through remote tools) ──
    eye = _NullEye()

    # ── Build AgentLoop + Runtime ──
    loop = AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        agent_id=agent_id,
        session_dir=session_dir,
        owner_display=config.get("_owner_display"),
    )

    runtime = AgentRuntime(agent_id, loop)

    # ── Wake event — set by SIGUSR1 from Supervisor ──
    wake_event = asyncio.Event()

    def _on_sigusr1(*_: Any) -> None:
        wake_event.set()

    signal.signal(signal.SIGUSR1, _on_sigusr1)

    logger.info("Worker ready, entering inbox drain loop: agent=%s", agent_id)

    # ── Main loop: drain inbox → dispatch → sleep ──
    cursor = _read_cursor(agent_dir)
    heartbeat_seconds = 300  # 5 min idle heartbeat

    while True:
        wake_event.clear()

        # Drain inbox.
        messages, new_cursor = _drain_inbox(agent_dir, cursor)

        if messages:
            cursor = new_cursor
            _write_cursor(agent_dir, cursor)
            for msg in messages:
                try:
                    await runtime.handle_message(msg)
                except Exception:
                    logger.exception("Error handling message for agent %s", agent_id)
        else:
            # No messages — idle wait for SIGUSR1 or heartbeat timeout.
            try:
                await asyncio.wait_for(wake_event.wait(), timeout=heartbeat_seconds)
            except asyncio.TimeoutError:
                # Heartbeat timeout — could do periodic checks here.
                logger.debug("Heartbeat: agent=%s idle", agent_id)


class _NullEye(BaseEye):
    """Minimal eye for subprocess mode."""

    async def capture(self) -> Screenshot:
        import base64 as b64

        pixel = b64.b64encode(b"\x00").decode()
        return Screenshot(
            base64=pixel, width=1, height=1,
            scale_factor=1.0, image=None,
        )


def main() -> None:
    """CLI entry point: python -m see_agent.agent.worker <agent_id> <sock_path>."""
    if len(sys.argv) < 3:
        print(
            "Usage: python -m see_agent.agent.worker <agent_id> <sock_path>",
            file=sys.stderr,
        )
        sys.exit(1)

    agent_id = sys.argv[1]
    sock_path = sys.argv[2]

    asyncio.run(_run_worker(agent_id, sock_path))


if __name__ == "__main__":
    main()
