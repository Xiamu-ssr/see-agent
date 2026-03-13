"""Tests for AgentRuntime collect/steer logic (v4: enqueue + flush API)."""

from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, MagicMock

import pytest

from see_agent.agent.runtime import AgentRuntime
from see_agent.ipc.message import Message


def _make_loop() -> MagicMock:
    loop = MagicMock()
    loop.run_one_turn = AsyncMock()
    return loop


def _msg(content: str = "hi", priority: str = "collect") -> Message:
    return Message(sender="u", content=content, priority=priority)


class TestAgentRuntime:
    """Tests for message dispatching."""

    @pytest.mark.asyncio
    async def test_collect_message_triggers_turn(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        msg = _msg("hello")
        rt.enqueue(msg)
        await rt.flush()
        loop.run_one_turn.assert_called_once()
        call_kwargs = loop.run_one_turn.call_args.kwargs
        assert msg in call_kwargs["messages"]

    @pytest.mark.asyncio
    async def test_steer_idle_triggers_turn(self):
        """Steer message while idle is enqueued as pending and triggers turn."""
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        steer = _msg("urgent!", priority="steer")
        rt.enqueue(steer)
        await rt.flush()
        loop.run_one_turn.assert_called_once()
        call_kwargs = loop.run_one_turn.call_args.kwargs
        assert steer in call_kwargs["messages"]

    @pytest.mark.asyncio
    async def test_steer_during_running_goes_to_inject(self):
        """Steer messages during a running turn go to inject queue."""
        loop = _make_loop()
        barrier = asyncio.Event()
        proceed = asyncio.Event()

        async def slow_turn(**kwargs):
            barrier.set()
            await proceed.wait()

        loop.run_one_turn = AsyncMock(side_effect=slow_turn)
        rt = AgentRuntime("alice", loop)

        # Start a turn.
        rt.enqueue(_msg("first"))
        task = asyncio.create_task(rt.flush())
        await barrier.wait()

        # Send steer while busy — should go to inject.
        rt.enqueue(_msg("interrupt!", priority="steer"))
        assert rt.inject_count == 1

        proceed.set()
        await task

    @pytest.mark.asyncio
    async def test_collect_during_running_queues_pending(self):
        """Collect messages during a running turn are queued as pending."""
        loop = _make_loop()
        barrier = asyncio.Event()
        proceed = asyncio.Event()

        async def slow_turn(**kwargs):
            barrier.set()
            await proceed.wait()

        loop.run_one_turn = AsyncMock(side_effect=slow_turn)
        rt = AgentRuntime("alice", loop)

        rt.enqueue(_msg("first"))
        task = asyncio.create_task(rt.flush())
        await barrier.wait()

        # Enqueue collect while busy — should be pending.
        rt.enqueue(_msg("second"))
        assert rt.pending_count == 1

        proceed.set()
        await task

    @pytest.mark.asyncio
    async def test_pending_messages_auto_trigger_next_turn(self):
        """After a turn finishes, pending messages trigger a new turn."""
        loop = _make_loop()
        call_count = 0
        barrier = asyncio.Event()
        proceed = asyncio.Event()

        async def counting_turn(**kwargs):
            nonlocal call_count
            call_count += 1
            if call_count == 1:
                barrier.set()
                await proceed.wait()

        loop.run_one_turn = AsyncMock(side_effect=counting_turn)
        rt = AgentRuntime("alice", loop)

        rt.enqueue(_msg("first"))
        task = asyncio.create_task(rt.flush())
        await barrier.wait()

        # Queue a pending message.
        rt.enqueue(_msg("second"))

        # Let first turn finish — should auto-start second turn.
        proceed.set()
        await task
        assert call_count == 2

    @pytest.mark.asyncio
    async def test_inject_queue_passed_to_loop(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        rt.enqueue(_msg("go"))
        await rt.flush()
        call_kwargs = loop.run_one_turn.call_args.kwargs
        assert "inject_queue" in call_kwargs
        assert call_kwargs["inject_queue"] is rt._inject

    @pytest.mark.asyncio
    async def test_batch_collects_multiple_messages(self):
        """Multiple enqueued messages are batched into one turn."""
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        rt.enqueue(_msg("msg1"))
        rt.enqueue(_msg("msg2"))
        rt.enqueue(_msg("msg3"))
        await rt.flush()
        loop.run_one_turn.assert_called_once()
        call_kwargs = loop.run_one_turn.call_args.kwargs
        msgs = call_kwargs["messages"]
        assert len(msgs) == 3
        assert [m.content for m in msgs] == ["msg1", "msg2", "msg3"]

    @pytest.mark.asyncio
    async def test_flush_when_idle_no_pending_is_noop(self):
        """Flush with no pending messages does nothing."""
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        await rt.flush()
        loop.run_one_turn.assert_not_called()

    @pytest.mark.asyncio
    async def test_flush_when_busy_is_noop(self):
        """Flush while a turn is running does not start another."""
        loop = _make_loop()
        barrier = asyncio.Event()
        proceed = asyncio.Event()

        async def slow_turn(**kwargs):
            barrier.set()
            await proceed.wait()

        loop.run_one_turn = AsyncMock(side_effect=slow_turn)
        rt = AgentRuntime("alice", loop)

        rt.enqueue(_msg("first"))
        task = asyncio.create_task(rt.flush())
        await barrier.wait()

        # Enqueue and flush while busy — should not start new turn.
        rt.enqueue(_msg("second"))
        await rt.flush()  # Should be noop (busy).
        assert loop.run_one_turn.call_count == 1

        proceed.set()
        await task
        # But auto-trigger should have started a second turn.
        assert loop.run_one_turn.call_count == 2
