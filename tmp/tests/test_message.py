"""Tests for the unified Message dataclass."""

from see_agent.ipc.message import Message


class TestMessage:
    """Tests for Message serialization and properties."""

    def test_round_trip_json(self):
        msg = Message(
            sender="alice",
            content="hello",
            priority="collect",
            metadata={"team_id": "t1"},
        )
        raw = msg.to_json()
        restored = Message.from_json(raw)
        assert restored.sender == "alice"
        assert restored.content == "hello"
        assert restored.priority == "collect"
        assert restored.metadata == {"team_id": "t1"}
        assert restored.timestamp == msg.timestamp

    def test_format_prefix(self):
        msg = Message(sender="bob", content="do X")
        assert msg.format_prefix() == "[bob]"

    def test_is_steer(self):
        collect = Message(sender="u", content="hi")
        steer = Message(sender="u", content="stop!", priority="steer")
        assert not collect.is_steer
        assert steer.is_steer

    def test_is_shutdown(self):
        msg = Message(sender="system", content="shutdown")
        assert msg.is_shutdown
        not_shutdown = Message(sender="system", content="hello")
        assert not not_shutdown.is_shutdown

    def test_default_priority(self):
        msg = Message(sender="x", content="y")
        assert msg.priority == "collect"

    def test_from_json_defaults(self):
        """Missing optional fields get defaults."""
        import json

        raw = json.dumps({"sender": "u", "content": "hi"})
        msg = Message.from_json(raw)
        assert msg.priority == "collect"
        assert msg.metadata == {}

    def test_timestamp_auto_generated(self):
        msg = Message(sender="u", content="hi")
        assert msg.timestamp  # non-empty
        assert "T" in msg.timestamp  # ISO format
