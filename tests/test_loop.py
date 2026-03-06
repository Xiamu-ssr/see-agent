"""Unit tests for AgentLoop with fully mocked components (see_agent/agent/loop.py)."""

from __future__ import annotations

import base64
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from see_agent.agent.loop import AgentLoop, RunResult, StepEvent
from see_agent.brain.base import BrainResponse, ToolCallInfo
from see_agent.eye.base import Screenshot
from see_agent.hand.tool import ToolResult

# -------------------------------------------------------------------- #
# Helpers
# -------------------------------------------------------------------- #

# Valid base64 that decodes cleanly (encode some known bytes).
FAKE_B64 = base64.b64encode(b"\x89PNG fake image data 000").decode("ascii")


def _make_screenshot(b64: str = FAKE_B64) -> Screenshot:
    """Create a minimal Screenshot for testing."""
    return Screenshot(base64=b64, width=800, height=600, scale_factor=1.0)


def _make_finished_response(summary: str = "Task done.") -> BrainResponse:
    """Create a BrainResponse that calls the 'finished' tool."""
    raw = MagicMock()
    raw.model_dump.return_value = {
        "role": "assistant",
        "content": "Finishing up.",
        "tool_calls": [
            {
                "id": "tc_fin",
                "type": "function",
                "function": {"name": "finished", "arguments": f'{{"summary": "{summary}"}}'},
            }
        ],
    }
    return BrainResponse(
        content="Finishing up.",
        tool_calls=[
            ToolCallInfo(id="tc_fin", name="finished", arguments={"summary": summary})
        ],
        raw=raw,
    )


def _make_click_response(step_id: int = 1) -> BrainResponse:
    """Create a BrainResponse that calls a 'click' tool (non-finished)."""
    raw = MagicMock()
    raw.model_dump.return_value = {
        "role": "assistant",
        "content": f"Clicking step {step_id}.",
        "tool_calls": [
            {
                "id": f"tc_{step_id}",
                "type": "function",
                "function": {"name": "click", "arguments": '{"x": 100, "y": 200}'},
            }
        ],
    }
    return BrainResponse(
        content=f"Clicking step {step_id}.",
        tool_calls=[
            ToolCallInfo(
                id=f"tc_{step_id}",
                name="click",
                arguments={"x": 100, "y": 200},
            )
        ],
        raw=raw,
    )


def _build_loop(
    brain: Any,
    eye: Any,
    registry: Any,
    max_steps: int = 5,
    on_step: Any = None,
    scaling_enabled: bool = False,
) -> AgentLoop:
    """Construct an AgentLoop with the given mocked components."""
    config: dict[str, Any] = {
        "language": "en",
        "max_steps": max_steps,
        "max_images": 5,
        "screenshot_interval_ms": 0,  # no real waiting in tests
        "tool_delay_ms": 0,
        "scaling_enabled": scaling_enabled,
    }
    return AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        on_step=on_step,
    )


def _patch_sessions(tmp_path: Path):
    """Return a patch context manager that redirects SessionStore to tmp_path."""
    return patch("see_agent.session.store.SESSIONS_DIR", tmp_path / "sessions")


@pytest.fixture(autouse=True)
def _setup_sessions_dir(tmp_path: Path):
    """Ensure sessions subdir exists for every test."""
    (tmp_path / "sessions").mkdir(exist_ok=True)


# -------------------------------------------------------------------- #
# Tests
# -------------------------------------------------------------------- #


class TestAgentLoop:
    """Tests for the AgentLoop orchestrator."""

    @pytest.mark.asyncio
    async def test_run_simple_task(self, tmp_path):
        """Brain returns 'finished' on the first response -- loop completes with summary."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("All done!"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = _build_loop(brain, eye, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            result = await loop.run("Open Safari")

        assert isinstance(result, RunResult)
        assert result.summary == "All done!"
        assert result.success is True
        assert result.session_id  # session was created
        brain.chat.assert_called_once()
        assert eye.capture.call_count >= 1

    @pytest.mark.asyncio
    async def test_run_max_steps(self, tmp_path):
        """Brain never returns 'finished' -- loop stops at max_steps."""
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=lambda msgs, tools: _make_click_response())

        eye = AsyncMock()
        call_count = 0

        async def varying_capture():
            nonlocal call_count
            call_count += 1
            raw = f"PNG fake screenshot data {call_count}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=varying_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked (100, 200)"))

        max_steps = 3
        loop = _build_loop(brain, eye, registry, max_steps=max_steps)

        with _patch_sessions(tmp_path):
            result = await loop.run("Do something forever")

        assert isinstance(result, RunResult)
        assert "Max steps" in result.summary
        assert str(max_steps) in result.summary
        assert result.success is False
        assert brain.chat.call_count == max_steps

    @pytest.mark.asyncio
    async def test_on_step_callback(self, tmp_path):
        """The on_step callback is invoked with a StepEvent for each tool execution."""
        responses = [
            _make_click_response(step_id=1),
            _make_finished_response("Done after one click."),
        ]
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=responses)

        eye = AsyncMock()
        call_count = 0

        async def varying_capture():
            nonlocal call_count
            call_count += 1
            raw = f"PNG fake capture data {call_count}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=varying_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked (100, 200)"))

        step_callback = AsyncMock()

        loop = _build_loop(brain, eye, registry, max_steps=10, on_step=step_callback)

        with _patch_sessions(tmp_path):
            result = await loop.run("Click then finish")

        assert isinstance(result, RunResult)
        assert result.summary == "Done after one click."
        assert result.success is True
        step_callback.assert_called_once()

        event: StepEvent = step_callback.call_args[0][0]
        assert isinstance(event, StepEvent)
        assert event.step == 1
        assert event.tool_name == "click"
        assert event.tool_args == {"x": 100, "y": 200}
        assert event.tool_result == "Clicked (100, 200)"
        assert event.max_steps == 10

    @pytest.mark.asyncio
    async def test_repeated_action_abort(self, tmp_path):
        """Agent stuck in identical clicks should abort after REPEAT_ABORT_LIMIT."""
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=lambda msgs, tools: _make_click_response(step_id=1))

        eye = AsyncMock()
        call_count = 0

        async def varying_capture():
            nonlocal call_count
            call_count += 1
            raw = f"PNG fake screenshot data {call_count}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=varying_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked (100, 200)"))

        loop = _build_loop(brain, eye, registry, max_steps=20)

        with _patch_sessions(tmp_path):
            result = await loop.run("Click forever")

        assert isinstance(result, RunResult)
        assert result.success is False
        assert "repeated action" in result.summary.lower()

    @pytest.mark.asyncio
    async def test_scaling_maps_coordinates(self, tmp_path):
        """When scaling is enabled, tool args should be scaled to screen coords."""
        def _make_scaled_screenshot():
            raw = f"PNG fake {id(object())}".encode()
            return Screenshot(
                base64=base64.b64encode(raw).decode("ascii"),
                width=1280,
                height=800,
                mime_type="image/png",
                screen_width=1728,
                screen_height=1080,
            )

        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=[
            _make_click_response(1),
            _make_finished_response("done"),
        ])

        eye = AsyncMock()
        eye.capture = AsyncMock(side_effect=lambda: _make_scaled_screenshot())

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked"))

        step_events: list[StepEvent] = []

        async def capture_step(event: StepEvent) -> None:
            step_events.append(event)

        loop = _build_loop(
            brain, eye, registry, max_steps=10,
            on_step=capture_step, scaling_enabled=True,
        )

        # Patch _maybe_scale to return the screenshot as-is (it already
        # carries screen_width/screen_height for coordinate mapping).
        loop._maybe_scale = lambda s: s  # type: ignore[assignment]

        with _patch_sessions(tmp_path):
            result = await loop.run("click scaled")

        assert result.success is True

        exec_call = registry.execute.call_args
        exec_args = exec_call[0][1]
        assert exec_args["x"] == round(100 * 1728 / 1280)
        assert exec_args["y"] == round(200 * 1080 / 800)

        assert len(step_events) == 1
        assert step_events[0].screen_tool_args is not None
        assert step_events[0].screen_tool_args["x"] == exec_args["x"]

    @pytest.mark.asyncio
    async def test_resume_restores_conversation_history(self, tmp_path):
        """When resuming a session, LLM receives the full message history."""
        (tmp_path / "sessions").mkdir(exist_ok=True)

        # Phase 1: run a task that finishes, creating a real session.
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("Weather is 25°C"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = _build_loop(brain, eye, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            result1 = await loop.run("Search weather")

        assert result1.success is True
        session_id = result1.session_id
        assert session_id

        # Phase 2: resume the same session with a new question.
        brain2 = AsyncMock()
        brain2.chat = AsyncMock(
            return_value=_make_finished_response("It was 25°C."),
        )

        eye2 = AsyncMock()
        eye2.capture = AsyncMock(return_value=_make_screenshot())

        loop2 = _build_loop(brain2, eye2, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            result2 = await loop2.run(
                "What was the temperature?", session_id=session_id,
            )

        assert result2.success is True
        assert result2.session_id == session_id

        # Verify: the LLM received messages from the first run.
        call_args = brain2.chat.call_args
        messages_sent = call_args[0][0]  # first positional arg

        # Must have more than just system + user_task (i.e. history present).
        assert len(messages_sent) > 3

        # The first run's user task text should be in there.
        all_text = str(messages_sent)
        assert "Search weather" in all_text

    @pytest.mark.asyncio
    async def test_resume_no_duplicate_system_in_jsonl(self, tmp_path):
        """Resuming must not write a duplicate system message to JSONL."""
        from see_agent.session.store import SessionStore

        (tmp_path / "sessions").mkdir(exist_ok=True)

        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))
        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())
        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = _build_loop(brain, eye, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            result1 = await loop.run("Task 1")
            session_id = result1.session_id

            # Resume
            brain2 = AsyncMock()
            brain2.chat = AsyncMock(
                return_value=_make_finished_response("done2"),
            )
            eye2 = AsyncMock()
            eye2.capture = AsyncMock(return_value=_make_screenshot())
            loop2 = _build_loop(brain2, eye2, registry, max_steps=10)
            await loop2.run("Task 2", session_id=session_id)

            session = SessionStore.load(session_id)

        messages = session.read_messages()
        system_msgs = [m for m in messages if m.get("type") == "system"]
        assert len(system_msgs) == 1

    @pytest.mark.asyncio
    async def test_multi_tool_serial_execution(self, tmp_path):
        """Multiple tool calls in one response are all executed serially."""
        raw = MagicMock()
        raw.model_dump.return_value = {
            "role": "assistant",
            "content": "I'll click and type.",
            "tool_calls": [
                {
                    "id": "tc_a",
                    "type": "function",
                    "function": {"name": "click", "arguments": '{"x": 10, "y": 20}'},
                },
                {
                    "id": "tc_b",
                    "type": "function",
                    "function": {"name": "type_text", "arguments": '{"text": "hi"}'},
                },
            ],
        }
        multi_response = BrainResponse(
            content="I'll click and type.",
            tool_calls=[
                ToolCallInfo(id="tc_a", name="click", arguments={"x": 10, "y": 20}),
                ToolCallInfo(id="tc_b", name="type_text", arguments={"text": "hi"}),
            ],
            raw=raw,
        )

        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=[
            multi_response,
            _make_finished_response("done"),
        ])

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        exec_order: list[str] = []

        async def mock_execute(name, args):
            exec_order.append(name)
            return ToolResult(text=f"{name} ok")

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(side_effect=mock_execute)

        loop = _build_loop(brain, eye, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            result = await loop.run("Multi tool test")

        assert result.success is True
        assert exec_order == ["click", "type_text"]

    @pytest.mark.asyncio
    async def test_pure_text_response_ends_loop(self, tmp_path):
        """When LLM returns no tool_calls, the loop ends gracefully."""
        raw = MagicMock()
        raw.model_dump.return_value = {
            "role": "assistant",
            "content": "I'm done thinking, no actions needed.",
        }
        text_response = BrainResponse(
            content="I'm done thinking, no actions needed.",
            tool_calls=[],
            raw=raw,
        )

        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=text_response)

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = _build_loop(brain, eye, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            await loop.run("Just think")

        # Loop ends at step budget since no finished was called, but
        # only 1 brain.chat call should have been made (exits on no tool_calls).
        assert brain.chat.call_count == 1

    @pytest.mark.asyncio
    async def test_finished_in_multi_tool_stops_immediately(self, tmp_path):
        """If 'finished' appears mid-batch, the loop returns immediately."""
        raw = MagicMock()
        raw.model_dump.return_value = {
            "role": "assistant",
            "content": "Finishing.",
            "tool_calls": [
                {
                    "id": "tc_fin",
                    "type": "function",
                    "function": {"name": "finished", "arguments": '{"summary": "bye"}'},
                },
                {
                    "id": "tc_extra",
                    "type": "function",
                    "function": {"name": "click", "arguments": '{"x": 1, "y": 2}'},
                },
            ],
        }
        response = BrainResponse(
            content="Finishing.",
            tool_calls=[
                ToolCallInfo(id="tc_fin", name="finished", arguments={"summary": "bye"}),
                ToolCallInfo(id="tc_extra", name="click", arguments={"x": 1, "y": 2}),
            ],
            raw=raw,
        )

        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=response)

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="clicked"))

        loop = _build_loop(brain, eye, registry, max_steps=10)

        with _patch_sessions(tmp_path):
            result = await loop.run("Finish early")

        assert result.success is True
        assert result.summary == "bye"
        # The click after finished should NOT have been executed.
        registry.execute.assert_not_called()
