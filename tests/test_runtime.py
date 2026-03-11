"""Tests for AgentRuntime collect/steer logic."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from see_agent.agent.runtime import AgentRuntime
from see_agent.ipc.message import Message


def _make_loop() -> MagicMock:
    loop = MagicMock()
    loop.run_one_turn = AsyncMock()
    return loop


class TestAgentRuntime:
    """Tests for message dispatching."""

    @pytest.mark.asyncio
    async def test_normal_message_triggers_turn(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        msg = Message(source="user", sender="u", content="hi")
        await rt.handle_message(msg)
        loop.run_one_turn.assert_called_once()
        call_args = loop.run_one_turn.call_args
        assert msg in call_args.kwargs["messages"]

    @pytest.mark.asyncio
    async def test_steer_message_queues_inject(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        steer = Message(
            source="user", sender="u", content="urgent!", priority="steer",
        )
        await rt.handle_message(steer)
        # Steer messages don't trigger turns.
        loop.run_one_turn.assert_not_called()
        assert rt.inject_count == 1

    @pytest.mark.asyncio
    async def test_pending_count(self):
        loop = _make_loop()
        # Make run_one_turn slow so we can test queuing.
        import asyncio

        async def slow_turn(**kwargs):
            await asyncio.sleep(0.1)

        loop.run_one_turn = AsyncMock(side_effect=slow_turn)
        rt = AgentRuntime("alice", loop)

        # Send first message (starts turn).
        msg1 = Message(source="user", sender="u", content="first")
        task = asyncio.create_task(rt.handle_message(msg1))

        # Give the turn a moment to start.
        await asyncio.sleep(0.01)

        # Send second message while busy — should be queued.
        msg2 = Message(source="user", sender="u", content="second")
        # Can't await handle_message because it would also try to run_turn.
        rt._pending.append(msg2)
        assert rt.pending_count == 1

        await task

    @pytest.mark.asyncio
    async def test_inject_queue_passed_to_turn(self):
        loop = _make_loop()
        rt = AgentRuntime("alice", loop)
        msg = Message(source="user", sender="u", content="go")
        await rt.handle_message(msg)
        call_args = loop.run_one_turn.call_args
        assert "inject_queue" in call_args.kwargs
        # The inject queue is the runtime's _inject list.
        assert call_args.kwargs["inject_queue"] is rt._inject
