"""Unit tests for session persistence (see_agent/session/)."""

from __future__ import annotations

import base64
import json
from pathlib import Path

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
        config = {"llm": {"model": "gpt-4o"}, "max_steps": 50}
        session = SessionStore.create("Open Safari", config, root_dir=sessions_dir)

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
        created = SessionStore.create(
            "Test task", {"llm": {"model": "m"}, "max_steps": 10},
            root_dir=sessions_dir,
        )
        loaded = SessionStore.load(created.id, root_dir=sessions_dir)

        assert loaded.id == created.id
        assert loaded.task == "Test task"

    def test_load_nonexistent_raises(self, sessions_dir: Path) -> None:
        with pytest.raises(FileNotFoundError):
            SessionStore.load("nonexistent_session_id", root_dir=sessions_dir)

    def test_list_sessions(self, sessions_dir: Path) -> None:
        SessionStore.create("Task 1", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        SessionStore.create("Task 2", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        sessions = SessionStore.list(root_dir=sessions_dir)

        assert len(sessions) == 2
        assert all(isinstance(s, SessionSummary) for s in sessions)

    def test_list_sessions_with_status_filter(self, sessions_dir: Path) -> None:
        s1 = SessionStore.create("Task 1", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        s1.update_meta(status="completed")
        SessionStore.create("Task 2", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        completed = SessionStore.list(status="completed", root_dir=sessions_dir)
        running = SessionStore.list(status="running", root_dir=sessions_dir)

        assert len(completed) == 1
        assert len(running) == 1

    def test_delete_session(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Delete me", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session_dir = session.dir
        SessionStore.delete(session.id, root_dir=sessions_dir)

        assert not session_dir.exists()

    def test_clean_empty_sessions(self, sessions_dir: Path) -> None:
        s1 = SessionStore.create("Empty", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        s2 = SessionStore.create("Has screenshots", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        # Write a fake screenshot in s2
        (s2.screenshots_dir / "step_000.webp").write_bytes(b"fake")

        deleted, freed = SessionStore.clean(keep_days=0, empty_only=True, root_dir=sessions_dir)

        assert deleted == 1  # only the empty one
        assert not s1.dir.exists()
        assert s2.dir.exists()


class TestSession:
    """Tests for Session instance methods."""

    def test_append_and_read_messages(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)

        session.append_message({"type": "user_task", "text": "hello"})
        session.append_message({"type": "assistant", "content": "hi"})

        messages = session.read_messages()
        assert len(messages) == 2
        assert messages[0]["type"] == "user_task"
        assert messages[0]["text"] == "hello"
        assert "ts" in messages[0]
        assert messages[1]["type"] == "assistant"

    def test_update_meta(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)

        session.update_meta(status="completed", total_steps=5, summary="Done")

        assert session.status == "completed"
        meta = json.loads((session.dir / "meta.json").read_text())
        assert meta["status"] == "completed"
        assert meta["total_steps"] == 5
        assert meta["summary"] == "Done"

    def test_screenshot_path(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)

        path = session.screenshot_path(3)
        assert path.name == "step_003.webp"
        assert path.parent == session.screenshots_dir

    def test_next_step_number_empty(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        assert session.next_step_number() == 0

    def test_next_step_number_with_screenshots(
        self, sessions_dir: Path,
    ) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
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
        session = SessionStore.create(
            "Search weather", {"llm": {"model": "m"}}, root_dir=sessions_dir,
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
        session = SessionStore.create(
            "Test", {"llm": {"model": "m"}}, root_dir=sessions_dir,
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
        session = SessionStore.create(
            "Test", {"llm": {"model": "m"}}, root_dir=sessions_dir,
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
        session = SessionStore.create(
            "Test", {"llm": {"model": "m"}}, root_dir=sessions_dir,
        )
        # Simulate 3 existing screenshots from prior run.
        for i in range(3):
            (session.screenshots_dir / f"step_{i:03d}.webp").write_bytes(
                b"fake"
            )
        assert session.next_step_number() == 3


class TestLogSystemPrompt:
    """Tests for Session.log_system_prompt()."""

    def test_log_system_prompt_writes_file(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.log_system_prompt("You are a helpful assistant.")
        log_file = session.dir / "system_prompt_log.md"
        assert log_file.exists()
        content = log_file.read_text()
        assert "You are a helpful assistant." in content

    def test_log_system_prompt_skips_duplicate(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.log_system_prompt("Same prompt")
        session.log_system_prompt("Same prompt")
        content = (session.dir / "system_prompt_log.md").read_text()
        assert content.count("Same prompt") == 1

    def test_log_system_prompt_appends_on_change(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.log_system_prompt("Prompt A")
        session.log_system_prompt("Prompt B")
        content = (session.dir / "system_prompt_log.md").read_text()
        assert "Prompt A" in content
        assert "Prompt B" in content


class TestSessionLogging:
    """Tests for Session.setup_logging() / teardown_logging()."""

    def test_setup_logging_creates_file(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.setup_logging()
        try:
            import logging as _logging
            _logging.getLogger("see_agent.agent").warning("test log entry")
            log_file = session.dir / "session.log"
            assert log_file.exists()
            assert "test log entry" in log_file.read_text()
        finally:
            session.teardown_logging()

    def test_teardown_logging_removes_handler(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.setup_logging()
        handler = session._log_handler
        assert handler is not None
        import logging as _logging
        assert handler in _logging.getLogger("see_agent.agent").handlers
        session.teardown_logging()
        assert handler not in _logging.getLogger("see_agent.agent").handlers
        assert session._log_handler is None


class TestSessionMsgId:
    """Tests for msg_id on JSONL entries."""

    def test_append_message_includes_msg_id(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.append_message({"type": "user_task", "text": "a"})
        session.append_message({"type": "assistant", "content": "b"})
        session.append_message({"type": "user_reply", "text": "c"})
        messages = session.read_messages()
        assert messages[0]["msg_id"] == 1
        assert messages[1]["msg_id"] == 2
        assert messages[2]["msg_id"] == 3

    def test_msg_counter_restored_on_read(self, sessions_dir: Path) -> None:
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.append_message({"type": "user_task", "text": "a"})
        session.append_message({"type": "assistant", "content": "b"})
        # Simulate reload: read restores counter
        session.read_messages()
        session.append_message({"type": "user_reply", "text": "c"})
        messages = session.read_messages()
        assert messages[2]["msg_id"] == 3


class TestSessionLoggingBug1:
    """Tests for Bug 1 fix — session.log captures DEBUG after global WARNING."""

    def test_setup_logging_captures_debug_after_global_warning(
        self, sessions_dir: Path,
    ) -> None:
        import logging as _logging

        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)

        lgr = _logging.getLogger("see_agent.agent")
        lgr.setLevel(_logging.WARNING)  # simulate config.py global setting
        try:
            session.setup_logging()
            lgr.debug("debug-level-test-entry")
            log_file = session.dir / "session.log"
            assert log_file.exists()
            assert "debug-level-test-entry" in log_file.read_text()
        finally:
            session.teardown_logging()

    def test_teardown_restores_logger_levels(self, sessions_dir: Path) -> None:
        import logging as _logging

        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)

        lgr = _logging.getLogger("see_agent.brain")
        lgr.setLevel(_logging.WARNING)
        session.setup_logging()
        assert lgr.level == _logging.DEBUG
        session.teardown_logging()
        assert lgr.level == _logging.WARNING


class TestRestoreCompact:
    """Tests for restoring sessions with compact entries."""

    def test_restore_handles_compact_entry(self, sessions_dir: Path) -> None:
        """Sessions with compact entry should inject summary on restore."""
        session = SessionStore.create("Test", {"llm": {"model": "m"}}, root_dir=sessions_dir)

        session.append_message({"type": "system", "content": "sys"})
        session.append_message({"type": "user_task", "text": "old task", "detail": "high"})
        session.append_message({"type": "assistant", "content": "old response"})
        session.append_message({
            "type": "compact",
            "summary": "Earlier we discussed opening Safari.",
            "first_kept_msg_id": 3,
        })
        session.append_message({"type": "user_reply", "text": "new question"})

        ctx = session.restore_context("sys", max_images=5)
        msgs = ctx.get_messages()
        # Should have: system + summary + new question
        summaries = [m for m in msgs if "[Conversation Summary]" in str(m.get("content", ""))]
        assert len(summaries) == 1
        assert "opening Safari" in summaries[0]["content"]


class TestSessionEdgeCases:
    """Additional edge cases for session management."""

    def test_corrupted_meta_json_skipped_in_list(self, sessions_dir: Path) -> None:
        """Sessions with invalid meta.json should be skipped in list()."""
        bad_dir = sessions_dir / "bad_session"
        bad_dir.mkdir()
        (bad_dir / "meta.json").write_text("{invalid json")

        # Also create a valid session.
        good = SessionStore.create("valid task", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        sessions = SessionStore.list(root_dir=sessions_dir)

        # Should not crash. The good session should be listed.
        ids = [s.id for s in sessions]
        assert good.id in ids

    def test_create_session_id_uniqueness(self, sessions_dir: Path) -> None:
        """Two sessions created sequentially should have different IDs."""
        s1 = SessionStore.create("task1", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        s2 = SessionStore.create("task2", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        assert s1.id != s2.id

    def test_append_message_unicode_roundtrip(
        self, sessions_dir: Path,
    ) -> None:
        """CJK + emoji content should survive write/read JSONL roundtrip."""
        session = SessionStore.create("测试", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        session.append_message({"type": "user_task", "text": "打开Safari搜索🍓"})
        messages = session.read_messages()
        assert messages[0]["text"] == "打开Safari搜索🍓"

    def test_restore_context_empty_session(self, sessions_dir: Path) -> None:
        """Empty JSONL session restore_context returns context with system only."""
        session = SessionStore.create("empty", {"llm": {"model": "m"}}, root_dir=sessions_dir)
        ctx = session.restore_context("System prompt.", max_images=5)
        msgs = ctx.get_messages()
        assert len(msgs) == 1
        assert msgs[0]["role"] == "system"


class TestSessionStoreRootDir:
    """Tests for root_dir parameter on SessionStore methods."""

    def test_create_with_root_dir(self, tmp_path: Path) -> None:
        custom_root = tmp_path / "custom_sessions"
        custom_root.mkdir()
        session = SessionStore.create("task", {"llm": {}}, root_dir=custom_root)
        assert session.dir.parent == custom_root

    def test_load_with_root_dir(self, tmp_path: Path) -> None:
        custom_root = tmp_path / "custom_sessions"
        custom_root.mkdir()
        session = SessionStore.create("task", {"llm": {}}, root_dir=custom_root)
        loaded = SessionStore.load(session.id, root_dir=custom_root)
        assert loaded.id == session.id

    def test_list_with_root_dir(self, tmp_path: Path) -> None:
        custom_root = tmp_path / "custom_sessions"
        custom_root.mkdir()
        SessionStore.create("task", {"llm": {}}, root_dir=custom_root)
        sessions = SessionStore.list(root_dir=custom_root)
        assert len(sessions) == 1
