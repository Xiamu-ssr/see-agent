"""Tests for the unified Message dataclass."""

from see_agent.ipc.message import Message


class TestMessage:
    """Tests for Message serialization and properties."""

    def test_round_trip_json(self):
        msg = Message(
            source="user",
            sender="alice",
            content="hello",
            priority="normal",
            metadata={"team_id": "t1"},
        )
        raw = msg.to_json()
        restored = Message.from_json(raw)
        assert restored.source == "user"
        assert restored.sender == "alice"
        assert restored.content == "hello"
        assert restored.priority == "normal"
        assert restored.metadata == {"team_id": "t1"}
        assert restored.timestamp == msg.timestamp

    def test_format_prefix(self):
        msg = Message(source="leader", sender="bob", content="do X")
        assert msg.format_prefix() == "[leader bob]"

    def test_is_steer(self):
        normal = Message(source="user", sender="u", content="hi")
        steer = Message(source="user", sender="u", content="stop!", priority="steer")
        assert not normal.is_steer
        assert steer.is_steer

    def test_is_shutdown(self):
        msg = Message(source="system", sender="system", content="shutdown")
        assert msg.is_shutdown
        not_shutdown = Message(source="system", sender="system", content="hello")
        assert not not_shutdown.is_shutdown

    def test_default_priority(self):
        msg = Message(source="teammate", sender="x", content="y")
        assert msg.priority == "normal"

    def test_from_json_defaults(self):
        """Missing optional fields get defaults."""
        import json

        raw = json.dumps({"source": "user", "sender": "u", "content": "hi"})
        msg = Message.from_json(raw)
        assert msg.priority == "normal"
        assert msg.metadata == {}

    def test_timestamp_auto_generated(self):
        msg = Message(source="user", sender="u", content="hi")
        assert msg.timestamp  # non-empty
        assert "T" in msg.timestamp  # ISO format
