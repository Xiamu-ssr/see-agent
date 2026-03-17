"""ScreenManager — global screen lease management for multi-process agents.

The screen is a physically exclusive resource. Two agents interleaving
screen operations at the step level will interfere with each other.
ScreenManager provides 10-minute leases so one agent can work on the
screen uninterrupted.
"""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class ScreenLease:
    """A screen lease held by one agent."""

    holder: str  # "team_id:agent_id"
    started_at: float
    last_used_at: float
    max_duration: int = 600  # 10 minutes
    idle_timeout: int = 300  # 5 minutes idle → auto-release


class ScreenManager:
    """Global screen lease manager (singleton in main process).

    Only one agent may hold the screen lease at a time. Others queue up
    and are notified when the lease becomes available.
    """

    def __init__(self) -> None:
        self._lease: ScreenLease | None = None
        self._queue: list[tuple[str, asyncio.Event]] = []
        self._check_task: asyncio.Task[None] | None = None

    async def start(self) -> None:
        """Start the periodic lease expiry checker."""
        self._check_task = asyncio.create_task(self._expiry_checker())

    async def stop(self) -> None:
        """Stop the expiry checker and clear state."""
        if self._check_task is not None:
            self._check_task.cancel()
            try:
                await self._check_task
            except asyncio.CancelledError:
                pass
            self._check_task = None
        self._lease = None
        # Wake up any waiters so they can exit cleanly.
        for _, event in self._queue:
            event.set()
        self._queue.clear()

    def is_holder(self, holder_id: str) -> bool:
        """Check whether *holder_id* currently holds the lease."""
        return self._lease is not None and self._lease.holder == holder_id

    async def acquire(self, holder_id: str, duration: int = 600) -> bool:
        """Request the screen lease.

        Returns ``True`` if granted immediately, ``False`` if queued.
        """
        if self._lease is None:
            self._lease = ScreenLease(
                holder=holder_id,
                started_at=time.time(),
                last_used_at=time.time(),
                max_duration=duration,
            )
            logger.info(
                "Screen lease granted to %s (%ds)", holder_id, duration,
            )
            return True

        if self._lease.holder == holder_id:
            return True  # already held

        # Enqueue
        event = asyncio.Event()
        self._queue.append((holder_id, event))
        logger.info(
            "Screen lease queued for %s (pos %d)",
            holder_id, len(self._queue),
        )
        return False

    async def release(self, holder_id: str) -> None:
        """Release the lease and grant to next waiter."""
        if self._lease is not None and self._lease.holder == holder_id:
            logger.info("Screen lease released by %s", holder_id)
            self._lease = None
            await self._grant_next()

    def touch(self, holder_id: str) -> None:
        """Reset idle timer — called whenever the holder uses a screen tool."""
        if self._lease is not None and self._lease.holder == holder_id:
            self._lease.last_used_at = time.time()

    def get_status(self) -> dict[str, object]:
        """Return current lease status for the API."""
        if self._lease is None:
            return {
                "holder": None,
                "started_at": None,
                "idle_seconds": 0,
                "queue_length": len(self._queue),
            }
        return {
            "holder": self._lease.holder,
            "started_at": self._lease.started_at,
            "idle_seconds": int(time.time() - self._lease.last_used_at),
            "queue_length": len(self._queue),
        }

    # ------------------------------------------------------------------ #
    # Internal
    # ------------------------------------------------------------------ #

    async def _grant_next(self) -> None:
        """Grant lease to the next waiter in the queue."""
        while self._queue:
            holder_id, event = self._queue.pop(0)
            self._lease = ScreenLease(
                holder=holder_id,
                started_at=time.time(),
                last_used_at=time.time(),
            )
            event.set()
            logger.info(
                "Screen lease granted to %s (from queue)", holder_id,
            )
            return

    async def _expiry_checker(self) -> None:
        """Periodically check for expired or idle leases."""
        while True:
            await asyncio.sleep(10)
            if self._lease is None:
                continue
            now = time.time()
            if now - self._lease.started_at > self._lease.max_duration:
                logger.info(
                    "Screen lease expired for %s (max duration)",
                    self._lease.holder,
                )
                self._lease = None
                await self._grant_next()
            elif now - self._lease.last_used_at > self._lease.idle_timeout:
                logger.info(
                    "Screen lease expired for %s (idle timeout)",
                    self._lease.holder,
                )
                self._lease = None
                await self._grant_next()
