"""Unit tests for AgentLoop with fully mocked components (see_agent/agent/loop.py)."""

from __future__ import annotations

import asyncio
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


def _make_call_user_response(question: str = "Need help?") -> BrainResponse:
    """Create a BrainResponse that calls the 'call_user' tool."""
    raw = MagicMock()
    raw.model_dump.return_value = {
        "role": "assistant",
        "content": "Asking user.",
        "tool_calls": [
            {
                "id": "tc_cu",
                "type": "function",
                "function": {
                    "name": "call_user",
                    "arguments": f'{{"question": "{question}"}}',
                },
            }
        ],
    }
    return BrainResponse(
        content="Asking user.",
        tool_calls=[
            ToolCallInfo(
                id="tc_cu",
                name="call_user",
                arguments={"question": question},
            )
        ],
        raw=raw,
    )


_DEFAULT_SCREEN_TOOLS = {
    "screenshot": True, "click": True, "type_text": True,
    "scroll": True, "drag": True, "hotkey": True,
}


def _build_loop(
    brain: Any,
    eye: Any,
    registry: Any,
    max_steps: int = 5,
    on_step: Any = None,
    scaling_enabled: bool = False,
    session_root: Path | None = None,
    tmp_path: Path | None = None,
) -> AgentLoop:
    """Construct an AgentLoop with the given mocked components."""
    # Ensure registry._tools has screen tools so capture is called.
    if not isinstance(getattr(registry, "_tools", None), dict):
        registry._tools = dict(_DEFAULT_SCREEN_TOOLS)
    config: dict[str, Any] = {
        "language": "en",
        "max_steps": max_steps,
        "max_images": 5,
        "screenshot_interval_ms": 0,  # no real waiting in tests
        "tool_delay_ms": 0,
        "scaling_enabled": scaling_enabled,
    }
    # Default session_root to tmp_path/"sessions" when not explicitly given
    if session_root is None and tmp_path is not None:
        session_root = tmp_path / "sessions"
        session_root.mkdir(parents=True, exist_ok=True)
    return AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        on_step=on_step,
        session_root=session_root,
    )


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

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
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
        loop = _build_loop(brain, eye, registry, max_steps=max_steps, tmp_path=tmp_path)

        if True:
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

        loop = _build_loop(
            brain, eye, registry, max_steps=10,
            on_step=step_callback, tmp_path=tmp_path,
        )

        if True:
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

        loop = _build_loop(brain, eye, registry, max_steps=20, tmp_path=tmp_path)

        if True:
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
            on_step=capture_step, scaling_enabled=True, tmp_path=tmp_path,
        )

        # Patch _maybe_scale to return the screenshot as-is (it already
        # carries screen_width/screen_height for coordinate mapping).
        loop._maybe_scale = lambda s: s  # type: ignore[assignment]

        if True:
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

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
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

        loop2 = _build_loop(brain2, eye2, registry, max_steps=10, tmp_path=tmp_path)

        if True:
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

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
            result1 = await loop.run("Task 1")
            session_id = result1.session_id

            # Resume
            brain2 = AsyncMock()
            brain2.chat = AsyncMock(
                return_value=_make_finished_response("done2"),
            )
            eye2 = AsyncMock()
            eye2.capture = AsyncMock(return_value=_make_screenshot())
            loop2 = _build_loop(brain2, eye2, registry, max_steps=10, tmp_path=tmp_path)
            await loop2.run("Task 2", session_id=session_id)

            session = SessionStore.load(
                session_id, root_dir=tmp_path / "sessions",
            )

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

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
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

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
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

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
            result = await loop.run("Finish early")

        assert result.success is True
        assert result.summary == "bye"
        # The click after finished should NOT have been executed.
        registry.execute.assert_not_called()


# -------------------------------------------------------------------- #
# v2 behavior tests
# -------------------------------------------------------------------- #


def _make_screenshot_response(step_id: int = 1, b64: str = FAKE_B64) -> BrainResponse:
    """Create a BrainResponse that calls the 'screenshot' tool."""
    raw = MagicMock()
    raw.model_dump.return_value = {
        "role": "assistant",
        "content": f"Taking screenshot {step_id}.",
        "tool_calls": [
            {
                "id": f"tc_ss_{step_id}",
                "type": "function",
                "function": {"name": "screenshot", "arguments": "{}"},
            }
        ],
    }
    return BrainResponse(
        content=f"Taking screenshot {step_id}.",
        tool_calls=[
            ToolCallInfo(id=f"tc_ss_{step_id}", name="screenshot", arguments={})
        ],
        raw=raw,
    )


class TestAgentLoopV2Behavior:
    """Tests for v2 ReAct loop — screenshot tool, memory, no-screenshot warning."""

    @pytest.mark.asyncio
    async def test_no_screenshot_warning_injected(self, tmp_path):
        """After MAX_STEPS_WITHOUT_SCREENSHOT steps with no screenshot,
        a system hint should be injected."""
        from see_agent.agent.loop import MAX_STEPS_WITHOUT_SCREENSHOT

        # Need enough click responses to trigger the warning, then a finish.
        # Use different coordinates to avoid repeated-action abort.
        num_clicks = MAX_STEPS_WITHOUT_SCREENSHOT + 1

        def _make_varied_click(i):
            raw = MagicMock()
            raw.model_dump.return_value = {
                "role": "assistant",
                "content": f"Click {i}.",
                "tool_calls": [{
                    "id": f"tc_{i}",
                    "type": "function",
                    "function": {
                        "name": "click",
                        "arguments": f'{{"x": {100 + i * 50}, "y": {200 + i * 50}}}',
                    },
                }],
            }
            return BrainResponse(
                content=f"Click {i}.",
                tool_calls=[ToolCallInfo(
                    id=f"tc_{i}", name="click",
                    arguments={"x": 100 + i * 50, "y": 200 + i * 50},
                )],
                raw=raw,
            )

        responses = [
            _make_varied_click(i) for i in range(1, num_clicks + 1)
        ] + [_make_finished_response("done")]

        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=responses)

        eye = AsyncMock()
        call_count = 0

        async def varying_capture():
            nonlocal call_count
            call_count += 1
            raw = f"PNG {call_count}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=varying_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked"))

        loop = _build_loop(brain, eye, registry, max_steps=num_clicks + 2, tmp_path=tmp_path)

        if True:
            result = await loop.run("click a lot")

        assert result.success is True

        # Verify the LLM received a hint about not taking screenshots.
        # The hint should appear in messages sent to brain.chat after step 5.
        found_hint = False
        for call_args in brain.chat.call_args_list:
            messages = call_args[0][0]
            for msg in messages:
                content = msg.get("content", "")
                if isinstance(content, str) and "screenshot" in content.lower():
                    if "have not taken" in content:
                        found_hint = True
                        break
        assert found_hint, "Expected no-screenshot warning hint in messages"

    @pytest.mark.asyncio
    async def test_screenshot_tool_images_saved_to_disk(self, tmp_path):
        """Screenshot tool returning ToolResult with images saves to disk."""
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=[
            _make_screenshot_response(1),
            _make_finished_response("done"),
        ])

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        # Return a ToolResult with an image
        img_b64 = base64.b64encode(b"FAKE_WEBP_DATA_12345").decode()
        from see_agent.hand.tool import ToolResultImage

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(
            text="Screenshot taken",
            images=[ToolResultImage(base64=img_b64)],
        ))

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)

        if True:
            result = await loop.run("take screenshot")

        assert result.success is True
        # Verify a webp file was written in the session's screenshots dir.
        screenshots_dir = Path(result.task_dir) / "screenshots"
        screenshots = list(screenshots_dir.glob("*.webp")) if screenshots_dir.exists() else []
        # At least the initial screenshot + the tool-returned one
        assert len(screenshots) >= 2

    @pytest.mark.asyncio
    async def test_screenshot_hash_no_progress_detection(self, tmp_path):
        """Repeated identical screenshots trigger no-progress warning."""
        from see_agent.agent.loop import NO_PROGRESS_LIMIT

        same_b64 = base64.b64encode(b"IDENTICAL_IMAGE_DATA").decode()
        from see_agent.hand.tool import ToolResultImage

        # Need NO_PROGRESS_LIMIT + 1 screenshot tool calls with same image
        num_ss = NO_PROGRESS_LIMIT + 1
        responses = [
            _make_screenshot_response(i) for i in range(1, num_ss + 1)
        ] + [_make_finished_response("done")]

        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=responses)

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(
            text="Screenshot taken",
            images=[ToolResultImage(base64=same_b64)],
        ))

        loop = _build_loop(brain, eye, registry, max_steps=num_ss + 2, tmp_path=tmp_path)

        if True:
            result = await loop.run("screenshot loop")

        assert result.success is True

        # Verify "screen has not changed" hint was injected
        found_warning = False
        for call_args in brain.chat.call_args_list:
            messages = call_args[0][0]
            for msg in messages:
                content = msg.get("content", "")
                if isinstance(content, str) and "has not changed" in content:
                    found_warning = True
                    break
        assert found_warning, "Expected no-progress warning in messages"

    @pytest.mark.asyncio
    async def test_tool_delay_ms_respected(self, tmp_path):
        """tool_delay_ms should cause delay between tool executions."""
        import time

        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=[
            _make_click_response(1),
            _make_click_response(2),
            _make_finished_response("done"),
        ])

        eye = AsyncMock()
        ctr = 0

        async def vc():
            nonlocal ctr
            ctr += 1
            raw = f"PNG {ctr}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=vc)

        exec_times: list[float] = []

        async def timed_execute(name, args):
            exec_times.append(time.monotonic())
            return ToolResult(text=f"{name} ok")

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(side_effect=timed_execute)

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 100,
            "scaling_enabled": False,
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("click twice")

        assert result.success is True
        assert len(exec_times) >= 2
        # At least ~80ms gap (allowing some tolerance)
        gap_ms = (exec_times[1] - exec_times[0]) * 1000
        assert gap_ms >= 80, f"Expected >=80ms gap, got {gap_ms:.0f}ms"

    @pytest.mark.asyncio
    async def test_save_memory_called_on_finished(self, tmp_path):
        """Memory.add() should be called when task finishes."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        memory = MagicMock()
        memory.search = MagicMock(return_value=[])
        memory.add = MagicMock()

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "skills_dirs": [],
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, memory=memory,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test memory")

        assert result.success is True
        memory.add.assert_called_once()
        # Messages should not contain base64 data
        saved_msgs = memory.add.call_args[0][0]
        for msg in saved_msgs:
            content = msg.get("content", "")
            if isinstance(content, str):
                assert "base64," not in content

    @pytest.mark.asyncio
    async def test_save_memory_failure_non_fatal(self, tmp_path):
        """Memory.add() failure should not crash the loop."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        memory = MagicMock()
        memory.search = MagicMock(return_value=[])
        memory.add = MagicMock(side_effect=RuntimeError("mem0 crash"))

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "skills_dirs": [],
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, memory=memory,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test memory fail")

        assert result.success is True
        assert result.summary == "done"

    @pytest.mark.asyncio
    async def test_memory_search_failure_non_fatal(self, tmp_path):
        """Memory.search() failure should not crash the loop."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        memory = MagicMock()
        memory.search = MagicMock(side_effect=Exception("search broken"))
        memory.add = MagicMock()

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "skills_dirs": [],
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, memory=memory,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test search fail")

        assert result.success is True

    @pytest.mark.asyncio
    async def test_skills_injected_into_prompt(self, tmp_path):
        """Skills from skills_dirs should appear in the system prompt."""
        # Create a SKILL.md
        skill_dir = tmp_path / "skills" / "open-url"
        skill_dir.mkdir(parents=True)
        (skill_dir / "SKILL.md").write_text(
            "---\nname: open-url\ndescription: Open a URL in browser.\n---\nStep 1: open Safari."
        )

        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "skills_dirs": [str(tmp_path / "skills")],
        }
        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)
        loop._config = config

        if True:
            result = await loop.run("test skills")

        assert result.success is True
        # Verify system prompt contains the skill
        call_args = brain.chat.call_args
        messages = call_args[0][0]
        system_msg = messages[0]["content"]
        assert "open-url" in system_msg

    @pytest.mark.asyncio
    async def test_save_memory_on_max_steps(self, tmp_path):
        """Memory.add() should be called even when max_steps is reached."""
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=lambda msgs, tools: _make_click_response())

        eye = AsyncMock()
        ctr = 0

        async def vc():
            nonlocal ctr
            ctr += 1
            raw = f"PNG {ctr}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=vc)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked"))

        memory = MagicMock()
        memory.search = MagicMock(return_value=[])
        memory.add = MagicMock()

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 1,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "skills_dirs": [],
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, memory=memory,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test max steps memory")

        assert result.success is False
        memory.add.assert_called_once()

    @pytest.mark.asyncio
    async def test_save_memory_on_no_tool_calls(self, tmp_path):
        """Memory.add() should be called when brain returns text-only (no tool calls)."""
        raw = MagicMock()
        raw.model_dump.return_value = {
            "role": "assistant",
            "content": "Just thinking, no actions.",
        }
        text_response = BrainResponse(
            content="Just thinking, no actions.",
            tool_calls=[],
            raw=raw,
        )

        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=text_response)

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        memory = MagicMock()
        memory.search = MagicMock(return_value=[])
        memory.add = MagicMock()

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "skills_dirs": [],
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, memory=memory,
            session_root=tmp_path / "sessions",
        )

        if True:
            await loop.run("test no tool calls memory")

        memory.add.assert_called_once()

    @pytest.mark.asyncio
    async def test_mcp_connect_called_on_first_run(self, tmp_path):
        """MCP manager connect_all and register_tools called on first run."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        mcp_manager = AsyncMock()
        mcp_manager.connect_all = AsyncMock()
        mcp_manager.register_tools = AsyncMock()

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)
        loop._mcp_manager = mcp_manager

        if True:
            await loop.run("test mcp")

        mcp_manager.connect_all.assert_called_once()
        mcp_manager.register_tools.assert_called_once()

    @pytest.mark.asyncio
    async def test_compact_not_triggered_when_disabled(self, tmp_path):
        """Compaction should not crash when compact.enabled=False."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "compact": {"enabled": False},
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test compact disabled")

        assert result.success is True

    @pytest.mark.asyncio
    async def test_compact_triggered_over_threshold(self, tmp_path):
        """Compaction should trigger when token estimate exceeds threshold."""
        # Need enough steps to accumulate messages before compaction triggers.
        # clicks build up context, then compaction triggers, then finished.
        ctr = 0

        def _make_varied_click(i: int) -> BrainResponse:
            raw = MagicMock()
            raw.model_dump.return_value = {
                "role": "assistant",
                "content": f"Click {i}.",
                "tool_calls": [{
                    "id": f"tc_{i}",
                    "type": "function",
                    "function": {
                        "name": "click",
                        "arguments": f'{{"x": {100 + i * 50}, "y": {200 + i * 50}}}',
                    },
                }],
            }
            return BrainResponse(
                content=f"Click {i}.",
                tool_calls=[ToolCallInfo(
                    id=f"tc_{i}", name="click",
                    arguments={"x": 100 + i * 50, "y": 200 + i * 50},
                )],
                raw=raw,
            )

        responses = [
            _make_varied_click(1),
            _make_varied_click(2),
            _make_varied_click(3),
            _make_varied_click(4),
            _make_varied_click(5),
            _make_varied_click(6),
            _make_varied_click(7),
            _make_finished_response("done"),
        ]

        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=responses)
        brain.summarize.return_value = "Summary of earlier conversation."

        eye = AsyncMock()

        async def vc():
            nonlocal ctr
            ctr += 1
            raw = f"PNG {ctr}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=vc)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value=ToolResult(text="Clicked"))

        # Low context_window to trigger compaction after a few steps.
        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 20,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "compact": {
                "enabled": True,
                "context_window": 100,
                "target_ratio": 0.5,
            },
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test compact triggered")

        assert result.success is True
        assert brain.summarize.call_count >= 1

    @pytest.mark.asyncio
    async def test_user_queue_drained_before_chat(self, tmp_path):
        """Pre-filled user_queue messages appear in LLM input."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        user_queue: asyncio.Queue[str] = asyncio.Queue()
        await user_queue.put("change direction please")

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 10,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config=config, user_queue=user_queue,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test queue")

        assert result.success is True
        # The injected message should appear in the messages sent to brain.
        call_args = brain.chat.call_args
        messages = call_args[0][0]
        all_text = str(messages)
        assert "[用户插入消息] change direction please" in all_text

    @pytest.mark.asyncio
    async def test_user_queue_none_no_crash(self, tmp_path):
        """user_queue=None should not cause any crash."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("done"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = _build_loop(brain, eye, registry, max_steps=10, tmp_path=tmp_path)
        assert loop._user_queue is None  # default

        if True:
            result = await loop.run("test no queue")

        assert result.success is True


# -------------------------------------------------------------------- #
# Phase 3: agent_id and session_root propagation
# -------------------------------------------------------------------- #


class TestAgentLoopTeamParams:
    """Tests for agent_id and session_root parameters."""

    def test_agent_id_stored(self):
        """agent_id param is stored on the loop."""
        brain = AsyncMock()
        eye = AsyncMock()
        registry = MagicMock()
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config={"max_steps": 1}, agent_id="alice",
        )
        assert loop._agent_id == "alice"

    def test_session_root_stored(self, tmp_path):
        """session_root param is stored on the loop."""
        brain = AsyncMock()
        eye = AsyncMock()
        registry = MagicMock()
        root = tmp_path / "custom_sessions"
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config={"max_steps": 1}, session_root=root,
        )
        assert loop._session_root == root

    def test_team_bus_stored(self):
        """team_bus param is stored on the loop."""
        brain = AsyncMock()
        eye = AsyncMock()
        registry = MagicMock()
        bus = MagicMock()
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config={"max_steps": 1}, team_bus=bus, agent_id="alice",
        )
        assert loop._team_bus is bus

    def test_drain_team_bus(self):
        """_drain_team_bus drains messages from team bus into context."""
        from see_agent.agent.context import ConversationContext
        from see_agent.team.bus import BusMessage, TeamBus

        brain = AsyncMock()
        eye = AsyncMock()
        registry = MagicMock()
        bus = TeamBus(Path("/tmp/test_bus_drain"))
        bus.register("alice")
        bus.register("bob")
        bus.send(BusMessage(sender="bob", recipient="alice", content="hello"))

        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config={"max_steps": 1}, team_bus=bus, agent_id="alice",
        )
        ctx = ConversationContext("system prompt")
        count = loop._drain_team_bus(ctx)
        assert count == 1
        msgs = ctx.get_messages()
        # Should have system + user reply with teammate message.
        user_msgs = [m for m in msgs if m.get("role") == "user"]
        assert any("[teammate bob]" in str(m.get("content", "")) for m in user_msgs)

    def test_drain_team_bus_no_bus(self):
        """_drain_team_bus with no bus returns 0."""
        from see_agent.agent.context import ConversationContext

        brain = AsyncMock()
        eye = AsyncMock()
        registry = MagicMock()
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry,
            config={"max_steps": 1},
        )
        ctx = ConversationContext("system prompt")
        assert loop._drain_team_bus(ctx) == 0

class TestCallUserTeamMode:
    """call_user sends to owner when in team mode."""

    @pytest.mark.asyncio
    async def test_call_user_sends_to_owner(self, tmp_path):
        """In team mode, call_user sends question to owner via bus."""
        from see_agent.team.bus import TeamBus

        bus = TeamBus(tmp_path / "team")
        bus.register("owner")
        bus.register("agent1")

        call_user_response = _make_call_user_response()
        finished_response = _make_finished_response("Done.")

        brain = AsyncMock()
        brain.chat = AsyncMock(
            side_effect=[call_user_response, finished_response],
        )

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []
        registry._tools = {"screenshot": True, "click": True}

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config={"max_steps": 5, "tool_delay_ms": 0},
            agent_id="agent1",
            team_bus=bus,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("test task")

        assert result.success is True
        # Owner should have received the question.
        owner_msgs = bus.drain("owner")
        assert len(owner_msgs) == 1
        assert owner_msgs[0].sender == "agent1"


class TestScreenshotSkip:
    """Text-only agents skip initial screenshot."""

    @pytest.mark.asyncio
    async def test_no_screen_tools_skips_capture(self, tmp_path):
        """When registry has no screen tools, eye.capture is never called."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("ok"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []
        # No screen tools registered.
        registry._tools = {"send_message": True, "list_tasks": True}

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config={"max_steps": 5, "tool_delay_ms": 0},
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("text task")

        assert result.success is True
        eye.capture.assert_not_called()


class TestCachedEnvBlock:
    """Environment block caching in config."""

    @pytest.mark.asyncio
    async def test_cached_env_skips_collect(self, tmp_path):
        """When _cached_env_block is set, collect_environment is not called."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("ok"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []
        registry._tools = dict(_DEFAULT_SCREEN_TOOLS)

        config: dict[str, Any] = {
            "language": "en",
            "max_steps": 5,
            "max_images": 5,
            "screenshot_interval_ms": 0,
            "tool_delay_ms": 0,
            "scaling_enabled": False,
            "_cached_env_block": "Pre-collected env info",
        }
        loop = AgentLoop(
            brain=brain, eye=eye, registry=registry, config=config,
            session_root=tmp_path / "sessions",
        )

        with patch(
            "see_agent.agent.environment.collect_environment",
        ) as mock_collect:
            result = await loop.run("test cached env")

        assert result.success is True
        mock_collect.assert_not_called()
        # Cached env block should appear in messages sent to brain.
        call_args = brain.chat.call_args
        messages = call_args[0][0]
        all_text = str(messages)
        assert "Pre-collected env info" in all_text


class TestFinishedAutoMarkTasks:
    """finished tool auto-marks tasks on the board."""

    @pytest.mark.asyncio
    async def test_finished_auto_completes_tasks(self, tmp_path):
        """When agent finishes, its claimed tasks are marked done."""
        from see_agent.team.task_board import TaskBoard

        board = TaskBoard(tmp_path / "team")
        t1 = board.create_task(title="Task 1", created_by="leader")
        board.claim_task(t1.id, "alice")
        t2 = board.create_task(title="Task 2", created_by="leader")
        board.update_task(
            t2.id, assigned_to="alice", status="in_progress",
        )
        # Task owned by another agent — should NOT be touched.
        t3 = board.create_task(title="Task 3", created_by="leader")
        board.claim_task(t3.id, "bob")

        brain = AsyncMock()
        brain.chat = AsyncMock(
            return_value=_make_finished_response("All done."),
        )

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []
        registry._tools = dict(_DEFAULT_SCREEN_TOOLS)

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config={"max_steps": 5, "tool_delay_ms": 0},
            agent_id="alice",
            task_board=board,
            session_root=tmp_path / "sessions",
        )

        if True:
            result = await loop.run("do work")

        assert result.success is True

        tasks = board.list_tasks()
        by_id = {t.id: t for t in tasks}
        assert by_id[t1.id].status == "done"
        assert by_id[t2.id].status == "done"
        assert by_id[t3.id].status == "claimed"  # unchanged
