"""Tests for AgentSupervisor."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import patch

import pytest

from see_agent.ipc.message import Message
from see_agent.server.supervisor import AgentSupervisor


@pytest.fixture()
def dirs(tmp_path):
    agents = tmp_path / "agents"
    agents.mkdir()
    run = tmp_path / "run"
    run.mkdir()
    with (
        patch("see_agent.server.supervisor.AGENTS_DIR", agents),
        patch("see_agent.server.supervisor.RUN_DIR", run),
        patch("see_agent.config.AGENTS_DIR", agents),
        patch("see_agent.config.RUN_DIR", run),
    ):
        yield tmp_path


class TestAgentSupervisor:

    def test_init(self, dirs):
        config = {"llm": {"base_url": "http://x", "api_key": "k", "model": "m"}}
        sup = AgentSupervisor(config)
        assert sup.running_agents == []

    def test_is_running_false(self, dirs):
        config = {"llm": {"base_url": "http://x", "api_key": "k", "model": "m"}}
        sup = AgentSupervisor(config)
        assert not sup.is_running("nonexistent")

    def test_stop_nonexistent_noop(self, dirs):
        config = {"llm": {"base_url": "http://x", "api_key": "k", "model": "m"}}
        sup = AgentSupervisor(config)
        sup.stop_agent("nonexistent")  # should not raise

    def test_stop_all_empty(self, dirs):
        config = {"llm": {"base_url": "http://x", "api_key": "k", "model": "m"}}
        sup = AgentSupervisor(config)
        sup.stop_all()  # should not raise

    def test_send_to_writes_inbox(self, dirs):
        config = {"llm": {"base_url": "http://x", "api_key": "k", "model": "m"}}
        sup = AgentSupervisor(config)
        msg = Message(source="user", sender="user", content="hello")
        # Patch start_agent to avoid actually spawning a process.
        with patch.object(sup, "start_agent", return_value=Path("/tmp/fake.sock")):
            sup.send_to("alice", msg)
        run = dirs / "run"
        inbox = run / "agents" / "alice" / "inbox.jsonl"
        assert inbox.exists()
        content = inbox.read_text()
        assert "hello" in content
