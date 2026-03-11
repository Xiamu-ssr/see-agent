"""Tests for AgentRuntime collect/steer logic."""

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


def _msg(content: str = "hi", priority: str = "normal", source: str = "user") -> Message:
    return Message(source=source, sender="u", content=content, priority=priority)


class TestAgentRuntime:
    """Tests for message dispatching."""

    @pytest.mark.asyncio
    async def test_normal_message_triggers_turn(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        msg = _msg("hello")
        await rt.handle_message(msg)
        loop.run_one_turn.assert_called_once()
        call_kwargs = loop.run_one_turn.call_args.kwargs
        assert msg in call_kwargs["messages"]

    @pytest.mark.asyncio
    async def test_steer_message_queues_inject_without_triggering_turn(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        steer = _msg("urgent!", priority="steer")
        await rt.handle_message(steer)
        # Steer message while idle: queued in inject, no turn started.
        loop.run_one_turn.assert_not_called()
        assert rt.inject_count == 1
        assert rt._inject[0].content == "urgent!"

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

        # Start a turn in background.
        task = asyncio.create_task(rt.handle_message(_msg("first")))
        await barrier.wait()  # Wait until turn is running.

        # Now send a steer message while busy.
        await rt.handle_message(_msg("interrupt!", priority="steer"))
        assert rt.inject_count == 1

        proceed.set()
        await task

    @pytest.mark.asyncio
    async def test_normal_during_running_queues_pending(self):
        """Normal messages during a running turn are queued as pending."""
        loop = _make_loop()
        barrier = asyncio.Event()
        proceed = asyncio.Event()

        async def slow_turn(**kwargs):
            barrier.set()
            await proceed.wait()

        loop.run_one_turn = AsyncMock(side_effect=slow_turn)
        rt = AgentRuntime("alice", loop)

        task = asyncio.create_task(rt.handle_message(_msg("first")))
        await barrier.wait()

        # Send normal message while busy — should be queued.
        await rt.handle_message(_msg("second"))
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

        task = asyncio.create_task(rt.handle_message(_msg("first")))
        await barrier.wait()

        # Queue a pending message.
        await rt.handle_message(_msg("second"))

        # Let first turn finish — should auto-start second turn.
        proceed.set()
        await task
        assert call_count == 2

    @pytest.mark.asyncio
    async def test_inject_queue_passed_to_loop(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        await rt.handle_message(_msg("go"))
        call_kwargs = loop.run_one_turn.call_args.kwargs
        assert "inject_queue" in call_kwargs
        assert call_kwargs["inject_queue"] is rt._inject

    @pytest.mark.asyncio
    async def test_batch_includes_accumulated_pending(self):
        """When idle, pending messages from before are included in batch."""
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        # Manually add pending (simulating messages that arrived between turns).
        rt._pending.append(_msg("queued1"))
        rt._pending.append(_msg("queued2"))

        await rt.handle_message(_msg("trigger"))
        call_kwargs = loop.run_one_turn.call_args.kwargs
        msgs = call_kwargs["messages"]
        assert len(msgs) == 3  # trigger + 2 queued
        contents = [m.content for m in msgs]
        assert contents == ["trigger", "queued1", "queued2"]
