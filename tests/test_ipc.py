"""Tests for IPC infrastructure: protocol, client, router, screen manager."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import patch

import pytest

from see_agent.ipc.protocol import (
    BOARD_CREATE,
    BOARD_LIST,
    BUS_DRAIN,
    BUS_SEND,
    SCREEN_ACQUIRE,
    SCREEN_RELEASE,
    RPCRequest,
    RPCResponse,
)

# -------------------------------------------------------------------- #
# Protocol
# -------------------------------------------------------------------- #


class TestProtocol:
    def test_rpc_request_to_dict(self):
        req = RPCRequest(id=1, method="bus.send", params={"sender": "a"})
        d = req.to_dict()
        assert d == {"id": 1, "method": "bus.send", "params": {"sender": "a"}}

    def test_rpc_response_result(self):
        resp = RPCResponse(id=1, result={"status": "ok"})
        d = resp.to_dict()
        assert d == {"id": 1, "result": {"status": "ok"}}

    def test_rpc_response_error(self):
        resp = RPCResponse(id=2, error="Unknown method")
        d = resp.to_dict()
        assert d == {"id": 2, "error": "Unknown method"}


# -------------------------------------------------------------------- #
# ScreenManager
# -------------------------------------------------------------------- #


class TestScreenManager:
    @pytest.mark.asyncio
    async def test_acquire_release(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        assert await mgr.acquire("team:alice") is True
        assert mgr.is_holder("team:alice") is True
        assert mgr.is_holder("team:bob") is False

        await mgr.release("team:alice")
        assert mgr.is_holder("team:alice") is False

    @pytest.mark.asyncio
    async def test_acquire_queues(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        assert await mgr.acquire("team:alice") is True
        assert await mgr.acquire("team:bob") is False
        assert mgr.is_holder("team:alice") is True

        # Release alice → bob should be granted.
        await mgr.release("team:alice")
        assert mgr.is_holder("team:bob") is True

    @pytest.mark.asyncio
    async def test_reacquire_same_holder(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        assert await mgr.acquire("team:alice") is True
        assert await mgr.acquire("team:alice") is True  # same holder

    @pytest.mark.asyncio
    async def test_touch_resets_idle(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        await mgr.acquire("team:alice")
        assert mgr._lease is not None
        old_used = mgr._lease.last_used_at
        import time
        time.sleep(0.01)
        mgr.touch("team:alice")
        assert mgr._lease.last_used_at > old_used

    @pytest.mark.asyncio
    async def test_get_status_empty(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        status = mgr.get_status()
        assert status["holder"] is None
        assert status["queue_length"] == 0

    @pytest.mark.asyncio
    async def test_get_status_with_lease(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        await mgr.acquire("team:alice")
        status = mgr.get_status()
        assert status["holder"] == "team:alice"

    @pytest.mark.asyncio
    async def test_start_stop(self):
        from see_agent.screen.manager import ScreenManager

        mgr = ScreenManager()
        await mgr.start()
        assert mgr._check_task is not None
        await mgr.stop()
        assert mgr._check_task is None


# -------------------------------------------------------------------- #
# AgentRouter (UDS server)
# -------------------------------------------------------------------- #


@pytest.fixture
def run_dir():
    """Use a short path for UDS sockets (macOS 104-char limit)."""
    import tempfile

    d = Path(tempfile.mkdtemp(prefix="sa_"))
    yield d
    import shutil

    shutil.rmtree(d, ignore_errors=True)


@pytest.fixture
def teams_dir(tmp_path):
    d = tmp_path / "teams"
    d.mkdir()
    return d


class TestAgentRouter:
    @pytest.mark.asyncio
    async def test_router_bus_send_writes_audit_log(self, run_dir, teams_dir):
        """v3.5: bus.send writes to audit log, drain returns empty."""
        from see_agent.ipc.router import AgentRouter

        with patch("see_agent.ipc.router.RUN_DIR", run_dir), \
             patch("see_agent.ipc.router.TEAMS_DIR", teams_dir):
            router = AgentRouter("test-team")
            await router.start()

            try:
                from see_agent.ipc.client import UDSClient

                client = UDSClient(router.sock_path)
                await client.connect()

                # Send writes to audit log.
                result = await client.call(
                    BUS_SEND,
                    sender="alice", recipient="bob",
                    content="hello bob",
                )
                assert result["status"] == "ok"

                # Verify audit log written.
                import json
                log_path = teams_dir / "test-team" / "messages.jsonl"
                assert log_path.exists()
                entry = json.loads(
                    log_path.read_text().strip().splitlines()[0],
                )
                assert entry["sender"] == "alice"
                assert entry["content"] == "hello bob"

                # Drain returns empty (v3.5: delivery via MessageRouter).
                result = await client.call(BUS_DRAIN, agent_id="bob")
                assert result["messages"] == []

                await client.close()
            finally:
                await router.stop()

    @pytest.mark.asyncio
    async def test_router_board_operations(self, run_dir, teams_dir):
        from see_agent.ipc.router import AgentRouter

        with patch("see_agent.ipc.router.RUN_DIR", run_dir), \
             patch("see_agent.ipc.router.TEAMS_DIR", teams_dir):
            router = AgentRouter("test-team")
            await router.start()

            try:
                from see_agent.ipc.client import UDSClient

                client = UDSClient(router.sock_path)
                await client.connect()

                # Create a task.
                result = await client.call(
                    BOARD_CREATE,
                    title="Fix bug", description="Fix the login bug",
                    created_by="alice",
                )
                assert "id" in result
                task_id = result["id"]

                # List tasks.
                result = await client.call(BOARD_LIST)
                assert len(result["tasks"]) == 1
                assert result["tasks"][0]["title"] == "Fix bug"

                # Claim task.
                result = await client.call(
                    "board.claim", task_id=task_id, agent_id="alice",
                )
                assert result["status"] == "claimed"

                # Complete task.
                result = await client.call(
                    "board.complete",
                    task_id=task_id, agent_id="alice",
                    result="Fixed it",
                )
                assert result["status"] == "done"

                await client.close()
            finally:
                await router.stop()

    @pytest.mark.asyncio
    async def test_router_screen_acquire_release(self, run_dir, teams_dir):
        from see_agent.ipc.router import AgentRouter

        with patch("see_agent.ipc.router.RUN_DIR", run_dir), \
             patch("see_agent.ipc.router.TEAMS_DIR", teams_dir):
            router = AgentRouter("test-team")
            await router.start()

            try:
                from see_agent.ipc.client import UDSClient

                client = UDSClient(router.sock_path)
                await client.connect()

                # Acquire screen.
                result = await client.call(
                    SCREEN_ACQUIRE, agent_id="alice",
                )
                assert result["granted"] is True

                # Release screen.
                result = await client.call(
                    SCREEN_RELEASE, agent_id="alice",
                )
                assert result["status"] == "ok"

                await client.close()
            finally:
                await router.stop()

    @pytest.mark.asyncio
    async def test_router_unknown_method(self, run_dir, teams_dir):
        from see_agent.ipc.router import AgentRouter

        with patch("see_agent.ipc.router.RUN_DIR", run_dir), \
             patch("see_agent.ipc.router.TEAMS_DIR", teams_dir):
            router = AgentRouter("test-team")
            await router.start()

            try:
                from see_agent.ipc.client import UDSClient

                client = UDSClient(router.sock_path)
                await client.connect()

                with pytest.raises(RuntimeError, match="Unknown method"):
                    await client.call("nonexistent.method")

                await client.close()
            finally:
                await router.stop()


# -------------------------------------------------------------------- #
# UDSClient
# -------------------------------------------------------------------- #


class TestUDSClient:
    @pytest.mark.asyncio
    async def test_not_connected_raises(self):
        from see_agent.ipc.client import UDSClient

        client = UDSClient(Path("/nonexistent.sock"))
        with pytest.raises(RuntimeError, match="not connected"):
            await client.call("test")
