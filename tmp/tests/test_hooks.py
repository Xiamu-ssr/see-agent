"""Unit tests for the HookBus lifecycle event system."""

import pytest

from see_agent.plugin.hooks import HookBus


class TestHookBus:
    """Tests for HookBus."""

    @pytest.mark.asyncio
    async def test_on_and_emit(self):
        bus = HookBus()
        received = []

        async def handler(**kwargs):
            received.append(kwargs)

        bus.on("before_task", handler)
        await bus.emit("before_task", task="hello")
        assert len(received) == 1
        assert received[0]["task"] == "hello"

    @pytest.mark.asyncio
    async def test_multiple_handlers(self):
        bus = HookBus()
        calls = []

        async def h1(**kwargs):
            calls.append("h1")

        async def h2(**kwargs):
            calls.append("h2")

        bus.on("after_task", h1)
        bus.on("after_task", h2)
        await bus.emit("after_task")
        assert calls == ["h1", "h2"]

    @pytest.mark.asyncio
    async def test_emit_unknown_event(self):
        """Emitting an event with no handlers is a no-op."""
        bus = HookBus()
        await bus.emit("nonexistent")  # should not raise

    @pytest.mark.asyncio
    async def test_error_isolation(self):
        """A handler error does not prevent other handlers from running."""
        bus = HookBus()
        calls = []

        async def bad_handler(**kwargs):
            raise RuntimeError("boom")

        async def good_handler(**kwargs):
            calls.append("ok")

        bus.on("before_compact", bad_handler)
        bus.on("before_compact", good_handler)
        await bus.emit("before_compact")
        assert calls == ["ok"]
