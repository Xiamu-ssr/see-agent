"""Unit tests for session persistence (see_agent/session/)."""

from __future__ import annotations

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
