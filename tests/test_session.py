"""Unit tests for session persistence (see_agent/session/)."""

from __future__ import annotations

import base64
import json
from pathlib import Path
from unittest.mock import patch

import pytest

from see_agent.session.store import SessionStore, SessionSummary


@pytest.fixture
def sessions_dir(tmp_path: Path) -> Path:
    """Provide a temporary sessions directory."""
    d = tmp_path / "sessions"
    d.mkdir()
    return d


class TestSessionStore:
    """Tests for SessionStore static methods."""

    def test_create_session(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            config = {"llm": {"model": "gpt-4o"}, "max_steps": 50}
            session = SessionStore.create("Open Safari", config)

        assert session.id
        assert session.task == "Open Safari"
        assert session.status == "running"
        assert session.dir.exists()
        assert (session.dir / "meta.json").exists()
        assert (session.dir / "messages.jsonl").exists()
        assert (session.dir / "screenshots").is_dir()

        meta = json.loads((session.dir / "meta.json").read_text())
        assert meta["task"] == "Open Safari"
        assert meta["config_snapshot"]["model"] == "gpt-4o"

    def test_load_session(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            created = SessionStore.create("Test task", {"llm": {"model": "m"}, "max_steps": 10})
            loaded = SessionStore.load(created.id)

        assert loaded.id == created.id
        assert loaded.task == "Test task"

    def test_load_nonexistent_raises(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            with pytest.raises(FileNotFoundError):
                SessionStore.load("nonexistent_session_id")

    def test_list_sessions(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            SessionStore.create("Task 1", {"llm": {"model": "m"}})
            SessionStore.create("Task 2", {"llm": {"model": "m"}})
            sessions = SessionStore.list()

        assert len(sessions) == 2
        assert all(isinstance(s, SessionSummary) for s in sessions)

    def test_list_sessions_with_status_filter(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            s1 = SessionStore.create("Task 1", {"llm": {"model": "m"}})
            s1.update_meta(status="completed")
            SessionStore.create("Task 2", {"llm": {"model": "m"}})
            completed = SessionStore.list(status="completed")
            running = SessionStore.list(status="running")

        assert len(completed) == 1
        assert len(running) == 1

    def test_delete_session(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create("Delete me", {"llm": {"model": "m"}})
            session_dir = session.dir
            SessionStore.delete(session.id)

        assert not session_dir.exists()

    def test_clean_empty_sessions(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            s1 = SessionStore.create("Empty", {"llm": {"model": "m"}})
            s2 = SessionStore.create("Has screenshots", {"llm": {"model": "m"}})
            # Write a fake screenshot in s2
            (s2.screenshots_dir / "step_000.webp").write_bytes(b"fake")

            deleted, freed = SessionStore.clean(keep_days=0, empty_only=True)

        assert deleted == 1  # only the empty one
        assert not s1.dir.exists()
        assert s2.dir.exists()


class TestSession:
    """Tests for Session instance methods."""

    def test_append_and_read_messages(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create("Test", {"llm": {"model": "m"}})

        session.append_message({"type": "user_task", "text": "hello"})
        session.append_message({"type": "assistant", "content": "hi"})

        messages = session.read_messages()
        assert len(messages) == 2
        assert messages[0]["type"] == "user_task"
        assert messages[0]["text"] == "hello"
        assert "ts" in messages[0]
        assert messages[1]["type"] == "assistant"

    def test_update_meta(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create("Test", {"llm": {"model": "m"}})

        session.update_meta(status="completed", total_steps=5, summary="Done")

        assert session.status == "completed"
        meta = json.loads((session.dir / "meta.json").read_text())
        assert meta["status"] == "completed"
        assert meta["total_steps"] == 5
        assert meta["summary"] == "Done"

    def test_screenshot_path(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create("Test", {"llm": {"model": "m"}})

        path = session.screenshot_path(3)
        assert path.name == "step_003.webp"
        assert path.parent == session.screenshots_dir

    def test_next_step_number_empty(self, sessions_dir: Path) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create("Test", {"llm": {"model": "m"}})
        assert session.next_step_number() == 0

    def test_next_step_number_with_screenshots(
        self, sessions_dir: Path,
    ) -> None:
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create("Test", {"llm": {"model": "m"}})
        (session.screenshots_dir / "step_000.webp").write_bytes(b"a")
        (session.screenshots_dir / "step_001.webp").write_bytes(b"b")
        (session.screenshots_dir / "step_005.webp").write_bytes(b"c")
        assert session.next_step_number() == 6


class TestRestoreContext:
    """Tests for Session.restore_context (resume support)."""

    SYSTEM_PROMPT = "You are a test assistant."
    FAKE_B64 = base64.b64encode(b"\x89PNG fake").decode("ascii")

    def _make_session_with_history(
        self, sessions_dir: Path,
    ) -> "SessionStore":
        """Create a session with a realistic JSONL history."""
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create(
                "Search weather", {"llm": {"model": "m"}},
            )

        # Write a fake screenshot for step_000.
        (session.screenshots_dir / "step_000.webp").write_bytes(
            b"\x89PNG fake screenshot"
        )
        (session.screenshots_dir / "step_001.webp").write_bytes(
            b"\x89PNG fake screenshot 2"
        )

        # Simulate a realistic JSONL conversation.
        session.append_message({
            "type": "system", "content": self.SYSTEM_PROMPT,
        })
        session.append_message({
            "type": "user_task",
            "text": "Search weather",
            "screenshot": "step_000.webp",
            "detail": "high",
        })
        session.append_message({
            "type": "assistant",
            "content": "I will click the browser.",
            "tool_calls": [
                {"id": "tc_1", "name": "click", "args": '{"x":100,"y":50}'},
            ],
        })
        session.append_message({
            "type": "tool_result",
            "tool_call_id": "tc_1",
            "result": "Clicked",
            "screenshot": "step_001.webp",
        })
        session.append_message({
            "type": "screenshot",
            "screenshot": "step_001.webp",
            "detail": "high",
        })
        session.append_message({
            "type": "assistant",
            "content": "The weather is 25°C.",
        })
        return session

    def test_restore_context_has_history(
        self, sessions_dir: Path,
    ) -> None:
        """Restored context contains the full conversation history."""
        session = self._make_session_with_history(sessions_dir)
        ctx = session.restore_context(
            self.SYSTEM_PROMPT, max_images=5,
        )
        msgs = ctx.get_messages()
        # system + user_task + assistant(click) + tool + screenshot
        # + assistant(weather)
        assert len(msgs) >= 5

        # The system message is present.
        assert msgs[0]["role"] == "system"
        assert msgs[0]["content"] == self.SYSTEM_PROMPT

        # The user task is present with text.
        assert msgs[1]["role"] == "user"
        content = msgs[1]["content"]
        assert isinstance(content, list)
        assert any(
            p.get("text") == "Search weather"
            for p in content if isinstance(p, dict)
        )

        # An assistant message with weather data is present.
        assistant_msgs = [m for m in msgs if m["role"] == "assistant"]
        assert any("25°C" in (m.get("content") or "") for m in assistant_msgs)

    def test_restore_no_duplicate_system_message(
        self, sessions_dir: Path,
    ) -> None:
        """Restored context has exactly one system message, not two."""
        session = self._make_session_with_history(sessions_dir)
        ctx = session.restore_context(self.SYSTEM_PROMPT, max_images=5)
        msgs = ctx.get_messages()
        system_msgs = [m for m in msgs if m["role"] == "system"]
        assert len(system_msgs) == 1

    def test_restore_on_append_not_called_during_replay(
        self, sessions_dir: Path,
    ) -> None:
        """on_append must not fire during replay (no JSONL re-write)."""
        session = self._make_session_with_history(sessions_dir)
        original_line_count = len(session.read_messages())

        callback_calls: list[dict] = []

        def on_append(msg: dict) -> None:
            callback_calls.append(msg)
            session.append_message(msg)

        ctx = session.restore_context(
            self.SYSTEM_PROMPT, max_images=5, on_append=on_append,
        )
        # No calls during replay.
        assert len(callback_calls) == 0

        # But new messages DO fire the callback.
        ctx.add_user_reply("What was the temperature?")
        assert len(callback_calls) == 1
        assert callback_calls[0]["type"] == "user_reply"

        # And the JSONL now has one more line.
        new_count = len(session.read_messages())
        assert new_count == original_line_count + 1

    def test_restore_with_missing_screenshot(
        self, sessions_dir: Path,
    ) -> None:
        """Missing screenshot files degrade gracefully to placeholder."""
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create(
                "Test", {"llm": {"model": "m"}},
            )
        # Write JSONL referencing a screenshot that doesn't exist.
        session.append_message({"type": "system", "content": "sys"})
        session.append_message({
            "type": "user_task",
            "text": "Do something",
            "screenshot": "step_000.webp",  # file doesn't exist!
            "detail": "high",
        })

        ctx = session.restore_context("sys", max_images=5)
        msgs = ctx.get_messages()
        # Should not crash. The user_task should still be there.
        assert len(msgs) >= 2
        user_msg = msgs[1]
        assert user_msg["role"] == "user"

    def test_restore_respects_max_images(
        self, sessions_dir: Path,
    ) -> None:
        """Only the most recent N screenshots have base64 in the context."""
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create(
                "Test", {"llm": {"model": "m"}},
            )
        session.append_message({"type": "system", "content": "sys"})
        # Add 5 screenshot messages, each with a real file.
        for i in range(5):
            ref = f"step_{i:03d}.webp"
            (session.screenshots_dir / ref).write_bytes(
                f"PNG fake {i}".encode()
            )
            session.append_message({
                "type": "screenshot",
                "screenshot": ref,
                "detail": "high",
            })

        # Restore with max_images=2 — sliding window should prune old ones.
        ctx = session.restore_context("sys", max_images=2)
        msgs = ctx.get_messages()

        # Count actual image_url parts.
        image_count = 0
        for m in msgs:
            content = m.get("content")
            if isinstance(content, list):
                for p in content:
                    if isinstance(p, dict) and p.get("type") == "image_url":
                        image_count += 1
        assert image_count == 2

    def test_screenshot_numbering_continues(
        self, sessions_dir: Path,
    ) -> None:
        """On resume, screenshot numbering picks up where it left off."""
        with patch("see_agent.session.store.SESSIONS_DIR", sessions_dir):
            session = SessionStore.create(
                "Test", {"llm": {"model": "m"}},
            )
        # Simulate 3 existing screenshots from prior run.
        for i in range(3):
            (session.screenshots_dir / f"step_{i:03d}.webp").write_bytes(
                b"fake"
            )
        assert session.next_step_number() == 3
