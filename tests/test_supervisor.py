"""Tests for AgentSupervisor."""

from __future__ import annotations

import json
from unittest.mock import MagicMock, patch

import pytest

from see_agent.ipc.message import Message
from see_agent.server.supervisor import AgentSupervisor

FAKE_CONFIG = {
    "llm": {"base_url": "http://x", "api_key": "k", "model": "m"},
    "agent": {"max_steps": 5},
}


@pytest.fixture()
def dirs(tmp_path):
    agents = tmp_path / "agents"
    agents.mkdir()
    with (
        patch("see_agent.server.supervisor.AGENTS_DIR", agents),
        patch("see_agent.config.AGENTS_DIR", agents),
    ):
        yield tmp_path


class TestAgentSupervisorBasic:
    """Basic init/query operations."""

    def test_init_empty(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        assert sup.running_agents == []

    def test_is_running_nonexistent(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        assert not sup.is_running("ghost")

    def test_stop_nonexistent_is_noop(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        sup.stop_agent("ghost")  # should not raise

    def test_stop_all_empty(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        sup.stop_all()  # should not raise


class TestAgentSupervisorProcessLifecycle:
    """Process start/stop without actually spawning real subprocesses."""

    def test_start_agent_creates_process(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None  # "running"
        mock_proc.pid = 12345

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc):
            sock = sup.start_agent("alice")

        # Should return a sock path in /tmp/.
        assert "see-agent-alice" in str(sock)

        # Agent dir should exist.
        assert (dirs / "agents" / "alice").is_dir()

        # Should be tracked as running.
        assert sup.is_running("alice")
        assert "alice" in sup.running_agents

    def test_start_already_running_returns_existing(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None
        mock_proc.pid = 111

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc) as popen:
            sock1 = sup.start_agent("bob")
            sock2 = sup.start_agent("bob")

        assert sock1 == sock2
        popen.assert_called_once()  # Only one Popen call.

    def test_start_restarts_exited_process(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        dead_proc = MagicMock()
        dead_proc.poll.return_value = 1  # exited
        dead_proc.pid = 222

        new_proc = MagicMock()
        new_proc.poll.return_value = None
        new_proc.pid = 333

        popen_path = "see_agent.server.supervisor.subprocess.Popen"
        with patch(popen_path, side_effect=[dead_proc, new_proc]):
            sup.start_agent("eve")
            # First start registers dead_proc; second call should detect exit and restart.
            sup.start_agent("eve")

        assert sup.is_running("eve")

    def test_stop_terminates_process(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None
        mock_proc.pid = 444

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc):
            sup.start_agent("alice")

        sup.stop_agent("alice")
        mock_proc.terminate.assert_called_once()
        assert not sup.is_running("alice")

    def test_stop_all_kills_all(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        procs = []
        for name in ("a", "b", "c"):
            p = MagicMock()
            p.poll.return_value = None
            p.pid = hash(name) % 10000
            procs.append(p)

        with patch("see_agent.server.supervisor.subprocess.Popen", side_effect=procs):
            for name in ("a", "b", "c"):
                sup.start_agent(name)

        sup.stop_all()
        for p in procs:
            p.terminate.assert_called_once()
        assert sup.running_agents == []

    def test_is_running_detects_exited(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None
        mock_proc.pid = 555

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc):
            sup.start_agent("alice")

        assert sup.is_running("alice")

        # Simulate process exit.
        mock_proc.poll.return_value = 0
        assert not sup.is_running("alice")


class TestAgentSupervisorMessaging:
    """Message sending via inbox."""

    def test_send_to_writes_inbox(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None
        mock_proc.pid = 666

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc):
            msg = Message(sender="lanxuan", content="hello agent")
            sup.send_to("alice", msg)

        inbox = dirs / "agents" / "alice" / "inbox.jsonl"
        assert inbox.exists()
        lines = inbox.read_text().strip().splitlines()
        assert len(lines) == 1
        data = json.loads(lines[0])
        assert data["content"] == "hello agent"
        assert data["sender"] == "lanxuan"

    def test_send_to_auto_starts_agent(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None
        mock_proc.pid = 777

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc) as popen:
            msg = Message(sender="u", content="wake up")
            sup.send_to("bob", msg)

        popen.assert_called_once()  # Agent was started.
        assert sup.is_running("bob")

    def test_send_multiple_messages_appends(self, dirs):
        sup = AgentSupervisor(FAKE_CONFIG)
        mock_proc = MagicMock()
        mock_proc.poll.return_value = None
        mock_proc.pid = 888

        with patch("see_agent.server.supervisor.subprocess.Popen", return_value=mock_proc):
            for i in range(3):
                sup.send_to("alice", Message(sender="u", content=f"msg{i}"))

        inbox = dirs / "agents" / "alice" / "inbox.jsonl"
        lines = inbox.read_text().strip().splitlines()
        assert len(lines) == 3
        contents = [json.loads(line)["content"] for line in lines]
        assert contents == ["msg0", "msg1", "msg2"]
