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


def _drain_inbox(
    agent_dir: Path, cursor: int, *, steer_only: bool = False,
) -> tuple[list[Message], int]:
    """Read messages from inbox.jsonl with msg_id > cursor.

    Args:
        steer_only: If True, only return steer-priority messages.
                    Cursor still only advances to the last *returned*
                    message (so non-steer messages are not skipped).

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
        msg = Message.from_json(line)
        if steer_only and not msg.is_steer:
            continue  # Skip but don't advance cursor past it.
        messages.append(msg)
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

    logger.info("Agent process starting: agent=%s sock=%s", agent_id, sock_path)

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

    # Team tools (only if agent belongs to a team).
    team_id = config.get("team_id")
    if team_id:
        from see_agent.hand.tools.send_message import SendMessageTool
        from see_agent.team.definition import TeamDefinition

        try:
            team_def = TeamDefinition.load(team_id)
            teammate_ids = [m["id"] for m in team_def.members]
            registry.register(
                SendMessageTool(agent_id, teammate_ids, AGENTS_DIR),
                source="team",
            )
            logger.info(
                "Registered team tool send_message for agent %s"
                " (team=%s, members=%s)",
                agent_id, team_id, teammate_ids,
            )
        except FileNotFoundError:
            logger.warning(
                "Agent %s has team_id=%s but team not found",
                agent_id, team_id,
            )

    # Apply tool filtering (re-read agent.json for hot-reload).
    denied_tools = config.get("tools", {}).get("disabled", [])
    for name in denied_tools:
        registry._tools.pop(name, None)
        registry._sources.pop(name, None)

    # Write actual tool list to disk for API to read.
    tools_manifest = [
        {"name": t.name, "description": t.description}
        for t in registry._tools.values()
    ]
    (agent_dir / "tools.json").write_text(
        json.dumps(tools_manifest, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

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
    # Use asyncio-safe signal handler to avoid interrupting httpx IO.
    wake_event = asyncio.Event()
    asyncio.get_running_loop().add_signal_handler(signal.SIGUSR1, wake_event.set)

    logger.info("Agent process ready, entering inbox drain loop: agent=%s", agent_id)

    # ── Main loop: drain inbox → batch enqueue → flush → sleep ──
    cursor = _read_cursor(agent_dir)
    heartbeat_seconds = 300  # 5 min idle heartbeat
    _turn_task: asyncio.Task[None] | None = None

    def _drain_interrupts() -> list[Message]:
        """Return any new interrupt (steer) messages since last check.

        Called by the ReAct loop before each LLM step. The loop doesn't
        need to know *where* interrupts come from — this is the seam.
        """
        steer_msgs, new_sc = _drain_inbox(
            agent_dir, _steer_cursor[0], steer_only=True,
        )
        if new_sc > _steer_cursor[0]:
            _steer_cursor[0] = new_sc
        return steer_msgs

    # Separate cursor for steer polling (list for nonlocal mutability).
    _steer_cursor = [_read_cursor(agent_dir)]

    # Wire drain_interrupts into runtime → loop.
    runtime.drain_interrupts = _drain_interrupts

    def _drain_and_enqueue() -> None:
        """Read all new inbox messages and enqueue collect ones.

        Steer messages are skipped here — they're handled by
        drain_interrupts() inside the ReAct loop.
        """
        nonlocal cursor
        messages, new_cursor = _drain_inbox(agent_dir, cursor)
        if not messages:
            return
        cursor = new_cursor
        _write_cursor(agent_dir, cursor)
        for msg in messages:
            if msg.is_steer:
                continue  # Handled by drain_interrupts.
            runtime.enqueue(msg)

    async def _maybe_flush() -> None:
        """Start a turn task if not already running."""
        nonlocal _turn_task
        if _turn_task and not _turn_task.done():
            return  # Turn already running; steer was injected by enqueue.
        _turn_task = asyncio.create_task(_run_flush())

    async def _run_flush() -> None:
        """Run flush in a loop until no more pending messages."""
        try:
            while runtime.pending_count > 0:
                await runtime.flush()
        except Exception:
            logger.exception("Error in turn for agent %s", agent_id)

    async def _inbox_watcher() -> None:
        """Continuously watch for SIGUSR1 and drain inbox."""
        while True:
            wake_event.clear()
            try:
                await asyncio.wait_for(
                    wake_event.wait(), timeout=heartbeat_seconds,
                )
            except asyncio.TimeoutError:
                logger.debug("Heartbeat: agent=%s idle", agent_id)
            # Signal or timeout — drain inbox and kick off turn.
            try:
                _drain_and_enqueue()
                await _maybe_flush()
            except Exception:
                logger.exception(
                    "Error draining inbox for agent %s", agent_id,
                )

    try:
        # Initial drain on startup.
        _drain_and_enqueue()
        await _maybe_flush()

        # Then run the watcher forever.
        await _inbox_watcher()
    except Exception:
        logger.exception("FATAL: agent process %s crashed in main loop", agent_id)
    finally:
        logger.error("Agent process %s exiting main loop — THIS SHOULD NOT HAPPEN", agent_id)


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

    # Catch ALL unhandled exceptions and log to stderr before dying.
    def _excepthook(exc_type, exc_value, exc_tb):  # type: ignore[no-untyped-def]
        import traceback
        msg = "".join(traceback.format_exception(exc_type, exc_value, exc_tb))
        print(f"[AGENT {agent_id}] UNHANDLED EXCEPTION:\n{msg}", file=sys.stderr, flush=True)

    sys.excepthook = _excepthook

    # Log signals that would kill us.
    import signal as _sig
    for sig_name in ("SIGTERM", "SIGHUP", "SIGINT"):
        sig = getattr(_sig, sig_name, None)
        if sig:
            _sig.signal(sig, lambda s, f, _n=sig_name: (
                print(f"[AGENT {agent_id}] Received {_n} — exiting", file=sys.stderr, flush=True),
                sys.exit(128 + s),
            ))

    asyncio.run(_run_worker(agent_id, sock_path))


if __name__ == "__main__":
    main()
