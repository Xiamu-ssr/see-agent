"""AgentRuntime — collect/steer message dispatcher for a single agent.

v4: Sits between the inbox watcher and AgentLoop.  Incoming messages
are either batched (collect) for the next turn or injected into the
running turn (steer).

The runtime manages the agent's lifecycle:
- Idle: messages are batched and trigger a new turn.
- Running: steer messages go directly to the inject queue,
  collect messages are queued for the *next* turn.
"""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from see_agent.agent.loop import AgentLoop
    from see_agent.ipc.message import Message

logger = logging.getLogger(__name__)


class AgentRuntime:
    """Dispatch incoming messages to an AgentLoop.

    Parameters:
        agent_id: The agent's unique identifier.
        loop: The AgentLoop instance (already configured with tools/brain).
    """

    def __init__(self, agent_id: str, loop: AgentLoop) -> None:
        self._agent_id = agent_id
        self._loop = loop
        self._pending: list[Message] = []
        self._inject: list[Message] = []
        self._running = False
        self._turn_lock = asyncio.Lock()
        self.drain_interrupts: Any | None = None  # Set by worker.

    def enqueue(self, msg: Message) -> None:
        """Route a message without starting a turn.

        - steer → inject queue (if running) or pending (if idle)
        - collect → pending queue
        """
        if msg.is_steer and self._running:
            self._inject.append(msg)
            logger.debug(
                "Steer message injected into running turn: %s",
                msg.format_prefix(),
            )
        else:
            self._pending.append(msg)
            logger.debug(
                "Message queued (%s, %s): %s",
                "steer-idle" if msg.is_steer else "collect",
                "busy" if self._running else "idle",
                msg.format_prefix(),
            )

    async def flush(self) -> None:
        """If idle and there are pending messages, start a turn.

        Call this after enqueuing one or more messages.
        """
        if self._running or not self._pending:
            return
        await self._run_turn()

    async def _run_turn(self) -> None:
        """Execute one agent turn with all pending messages."""
        async with self._turn_lock:
            self._running = True
            try:
                batch = list(self._pending)
                self._pending.clear()
                logger.info(
                    "Starting turn: %d message(s), agent=%s",
                    len(batch), self._agent_id,
                )
                await self._loop.run_one_turn(
                    messages=batch,
                    inject_queue=self._inject,
                    drain_interrupts=self.drain_interrupts,
                )
            finally:
                self._running = False
                # Move any unconsumed inject messages to pending
                # so they get processed in the next turn.
                if self._inject:
                    logger.info(
                        "Moving %d unconsumed steer message(s) to pending",
                        len(self._inject),
                    )
                    self._pending.extend(self._inject)
                    self._inject.clear()

        # If messages arrived while running, start another turn.
        if self._pending:
            await self._run_turn()

    @property
    def pending_count(self) -> int:
        """Number of messages waiting for the next turn."""
        return len(self._pending)

    @property
    def inject_count(self) -> int:
        """Number of steer messages waiting to be injected."""
        return len(self._inject)
