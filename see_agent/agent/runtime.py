"""AgentRuntime — collect/steer message dispatcher for a single agent.

v3.5: Sits between the UDS connection and AgentLoop.  Incoming messages
are either queued (collect/normal) or injected into the running turn
(steer).

The runtime manages the agent's lifecycle:
- Idle: messages are queued in ``_pending``.
- Running: a turn is in progress; steer messages go to ``_inject``,
  normal messages are queued for the *next* turn.
"""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING

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

    async def handle_message(self, msg: Message) -> None:
        """Route an incoming message.

        - **steer** messages are placed in ``_inject`` so the currently
          running turn picks them up.
        - **normal** messages trigger a new turn if idle, or queue for
          the next turn if busy.
        """
        if msg.is_steer:
            if self._running:
                # Agent busy — inject into current ReAct loop.
                self._inject.append(msg)
                logger.debug("Steer message injected: %s", msg.format_prefix())
                return
            # Agent idle — treat as normal message.
            logger.debug("Steer message (idle, treating as normal): %s", msg.format_prefix())

        if self._running:
            # Busy — queue for next turn.
            self._pending.append(msg)
            logger.debug("Message queued (busy): %s", msg.format_prefix())
            return

        # Idle — start a new turn with this message + any pending.
        await self._run_turn(msg)

    async def _run_turn(self, trigger: Message) -> None:
        """Execute one agent turn with *trigger* + queued messages."""
        async with self._turn_lock:
            self._running = True
            try:
                batch = [trigger, *self._pending]
                self._pending.clear()
                await self._loop.run_one_turn(
                    messages=batch,
                    inject_queue=self._inject,
                )
            finally:
                self._running = False

        # If messages arrived while we were running, start a new turn.
        if self._pending:
            next_msg = self._pending.pop(0)
            await self._run_turn(next_msg)

    @property
    def pending_count(self) -> int:
        """Number of messages waiting for the next turn."""
        return len(self._pending)

    @property
    def inject_count(self) -> int:
        """Number of steer messages waiting to be injected."""
        return len(self._inject)
