"""Unit tests for AgentLoop with fully mocked components (src/agent/loop.py)."""

from __future__ import annotations

import base64
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from see_agent.agent.loop import AgentLoop, RunResult, StepEvent
from see_agent.brain.base import BrainResponse, ToolCallInfo
from see_agent.eye.base import Screenshot

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
        "scaling_enabled": scaling_enabled,
    }
    return AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        on_step=on_step,
    )


# -------------------------------------------------------------------- #
# Tests
# -------------------------------------------------------------------- #


class TestAgentLoop:
    """Tests for the AgentLoop orchestrator."""

    @pytest.mark.asyncio
    async def test_run_simple_task(self, tmp_path):
        """Brain returns 'finished' on the first response -- loop completes with summary."""
        # -- Arrange --
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("All done!"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = _build_loop(brain, eye, registry, max_steps=10)

        # Patch SCREENSHOTS_DIR to use tmp_path so we don't pollute the real fs.
        with patch("see_agent.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("Open Safari")

        # -- Assert --
        assert isinstance(result, RunResult)
        assert result.summary == "All done!"
        assert result.success is True
        # Brain was called exactly once
        brain.chat.assert_called_once()
        # Eye captured the initial screenshot
        assert eye.capture.call_count >= 1

    @pytest.mark.asyncio
    async def test_run_max_steps(self, tmp_path):
        """Brain never returns 'finished' -- loop stops at max_steps."""
        # -- Arrange --
        brain = AsyncMock()
        # Always return a non-finished click action
        brain.chat = AsyncMock(side_effect=lambda msgs, tools: _make_click_response())

        eye = AsyncMock()
        # Return a *different* screenshot each call to avoid no-progress abort
        call_count = 0

        async def varying_capture():
            nonlocal call_count
            call_count += 1
            # Each capture returns valid but distinct base64 data
            raw = f"PNG fake screenshot data {call_count}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=varying_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value="Clicked (100, 200)")

        max_steps = 3
        loop = _build_loop(brain, eye, registry, max_steps=max_steps)

        with patch("see_agent.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("Do something forever")

        # -- Assert --
        assert isinstance(result, RunResult)
        assert "Max steps" in result.summary
        assert str(max_steps) in result.summary
        assert result.success is False
        # Brain should have been called max_steps times
        assert brain.chat.call_count == max_steps

    @pytest.mark.asyncio
    async def test_on_step_callback(self, tmp_path):
        """The on_step callback is invoked with a StepEvent for each tool execution."""
        # -- Arrange --
        # First call -> click, second call -> finished
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
        registry.execute = AsyncMock(return_value="Clicked (100, 200)")

        step_callback = AsyncMock()

        loop = _build_loop(brain, eye, registry, max_steps=10, on_step=step_callback)

        with patch("see_agent.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("Click then finish")

        # -- Assert --
        assert isinstance(result, RunResult)
        assert result.summary == "Done after one click."
        assert result.success is True
        # The callback should have been called once (for the click step; finished
        # returns immediately without going through the tool-execute + callback path).
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
        # Always return the same click at the same coordinate.
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
        registry.execute = AsyncMock(return_value="Clicked (100, 200)")

        loop = _build_loop(brain, eye, registry, max_steps=20)

        with patch("see_agent.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("Click forever")

        assert isinstance(result, RunResult)
        assert result.success is False
        assert "repeated action" in result.summary.lower()

    @pytest.mark.asyncio
    async def test_scaling_maps_coordinates(self, tmp_path):
        """When scaling is enabled, tool args should be scaled to screen coords."""
        # Return screenshots already marked as "scaled" so that the loop
        # applies coordinate mapping without needing real image resize.
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
        registry.execute = AsyncMock(return_value="Clicked")

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

        with patch("see_agent.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("click scaled")

        assert result.success is True

        # The registry.execute should have been called with scaled coords,
        # not the raw model coords (100, 200).
        exec_call = registry.execute.call_args
        exec_args = exec_call[0][1]  # positional arg #1 is the args dict
        # Model coords are 100, 200 in a 1280x800 model space
        # Screen is 1728x1080 → x = round(100*1728/1280) = 135
        #                      → y = round(200*1080/800) = 270
        assert exec_args["x"] == round(100 * 1728 / 1280)
        assert exec_args["y"] == round(200 * 1080 / 800)

        # StepEvent should carry screen_tool_args.
        assert len(step_events) == 1
        assert step_events[0].screen_tool_args is not None
        assert step_events[0].screen_tool_args["x"] == exec_args["x"]
