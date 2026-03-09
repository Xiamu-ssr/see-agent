"""Simple event bus for lifecycle hooks.

Supported events: ``before_task``, ``after_task``, ``before_compact``,
``after_compact``.
"""

from __future__ import annotations

import asyncio
import logging
from typing import Any, Callable, Coroutine

logger = logging.getLogger(__name__)

Handler = Callable[..., Coroutine[Any, Any, None]]


class HookBus:
    """Lightweight async event bus for agent lifecycle hooks."""

    def __init__(self) -> None:
        self._handlers: dict[str, list[Handler]] = {}

    def on(self, event: str, handler: Handler) -> None:
        """Register *handler* for *event*."""
        self._handlers.setdefault(event, []).append(handler)

    async def emit(self, event: str, **kwargs: Any) -> None:
        """Fire all handlers for *event*.

        Handler errors are logged but do not propagate, ensuring that one
        broken handler cannot crash the agent loop.
        """
        for handler in self._handlers.get(event, []):
            try:
                result = handler(**kwargs)
                if asyncio.iscoroutine(result):
                    await result
            except Exception:
                logger.exception("Hook handler error for event '%s'", event)
