"""Unit tests for TeamBus."""

import json

import pytest

from see_agent.team.bus import BusMessage, TeamBus


@pytest.fixture
def bus(tmp_path):
    b = TeamBus(tmp_path)
    b.register("alice")
    b.register("bob")
    return b


class TestTeamBus:

    def test_register_creates_queue(self, bus):
        assert "alice" in bus._queues
        assert "bob" in bus._queues

    def test_send_to_recipient(self, bus):
        bus.send(BusMessage(sender="alice", recipient="bob", content="hi"))
        messages = bus.drain("bob")
        assert len(messages) == 1
        assert messages[0].content == "hi"
        assert messages[0].sender == "alice"

    def test_drain_empty(self, bus):
        assert bus.drain("alice") == []

    def test_broadcast(self, bus):
        bus.broadcast("alice", "hello everyone")
        msgs_bob = bus.drain("bob")
        msgs_alice = bus.drain("alice")
        assert len(msgs_bob) == 1
        assert msgs_bob[0].content == "hello everyone"
        # Sender should not receive broadcast.
        assert len(msgs_alice) == 0

    def test_send_unknown_recipient(self, bus):
        # Should not crash, just log warning.
        bus.send(
            BusMessage(sender="alice", recipient="ghost", content="x")
        )

    def test_audit_log(self, bus, tmp_path):
        bus.send(BusMessage(sender="alice", recipient="bob", content="log"))
        log_path = tmp_path / "messages.jsonl"
        assert log_path.exists()
        lines = log_path.read_text().strip().splitlines()
        assert len(lines) == 1
        entry = json.loads(lines[0])
        assert entry["sender"] == "alice"
        assert entry["content"] == "log"

    def test_multiple_messages_ordered(self, bus):
        for i in range(5):
            bus.send(
                BusMessage(
                    sender="alice", recipient="bob", content=f"msg{i}",
                )
            )
        msgs = bus.drain("bob")
        assert [m.content for m in msgs] == [
            f"msg{i}" for i in range(5)
        ]

    def test_get_queue(self, bus):
        q = bus.get_queue("alice")
        assert q is not None
