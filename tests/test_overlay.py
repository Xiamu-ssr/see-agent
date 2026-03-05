"""Tests for overlay dispatch and loop integration."""

from __future__ import annotations

import base64
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from src.agent.loop import AgentLoop, _show_overlay
from src.brain.base import BrainResponse, ToolCallInfo
from src.eye.base import Screenshot

# -------------------------------------------------------------------- #
# Helpers
# -------------------------------------------------------------------- #

FAKE_B64 = base64.b64encode(b"\x89PNG fake image data 000").decode("ascii")


def _make_screenshot(b64: str = FAKE_B64) -> Screenshot:
    return Screenshot(base64=b64, width=800, height=600, scale_factor=1.0)


def _make_finished_response(summary: str = "Task done.") -> BrainResponse:
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
            ToolCallInfo(id=f"tc_{step_id}", name="click", arguments={"x": 100, "y": 200})
        ],
        raw=raw,
    )


# -------------------------------------------------------------------- #
# _show_overlay dispatch tests
# -------------------------------------------------------------------- #


class TestShowOverlay:
    """Verify _show_overlay dispatches to the correct overlay method."""

    def test_click(self):
        overlay = MagicMock()
        _show_overlay(overlay, "click", {"x": 100, "y": 200})
        overlay.show_click.assert_called_once_with(100, 200, False)

    def test_click_double(self):
        overlay = MagicMock()
        _show_overlay(overlay, "click", {"x": 50, "y": 60, "double": True})
        overlay.show_click.assert_called_once_with(50, 60, True)

    def test_type_text(self):
        overlay = MagicMock()
        _show_overlay(overlay, "type_text", {"text": "hello"})
        overlay.show_type.assert_called_once_with("hello")

    def test_drag(self):
        overlay = MagicMock()
        _show_overlay(overlay, "drag", {"start_x": 10, "start_y": 20, "end_x": 30, "end_y": 40})
        overlay.show_drag.assert_called_once_with(10, 20, 30, 40)

    def test_scroll(self):
        overlay = MagicMock()
        _show_overlay(overlay, "scroll", {"x": 100, "y": 200, "direction": "down", "amount": 5})
        overlay.show_scroll.assert_called_once_with(100, 200, "down", 5)

    def test_scroll_default_amount(self):
        overlay = MagicMock()
        _show_overlay(overlay, "scroll", {"x": 100, "y": 200, "direction": "up"})
        overlay.show_scroll.assert_called_once_with(100, 200, "up", 3)

    def test_hotkey(self):
        overlay = MagicMock()
        _show_overlay(overlay, "hotkey", {"keys": ["cmd", "v"]})
        overlay.show_hotkey.assert_called_once_with(["cmd", "v"])

    def test_shell(self):
        overlay = MagicMock()
        _show_overlay(overlay, "shell", {"command": "date"})
        overlay.show_shell.assert_called_once_with("date")

    def test_wait(self):
        overlay = MagicMock()
        _show_overlay(overlay, "wait", {"seconds": 3})
        overlay.show_wait.assert_called_once_with(3)

    def test_screenshot(self):
        overlay = MagicMock()
        _show_overlay(overlay, "screenshot", {})
        overlay.show_screenshot.assert_called_once()

    def test_call_user(self):
        overlay = MagicMock()
        _show_overlay(overlay, "call_user", {"question": "password?"})
        overlay.show_call_user.assert_called_once_with("password?")

    def test_finished(self):
        overlay = MagicMock()
        _show_overlay(overlay, "finished", {"summary": "done"})
        overlay.show_finished.assert_called_once_with("done")

    def test_unknown_tool_no_error(self):
        """Unknown tools should not raise."""
        overlay = MagicMock()
        _show_overlay(overlay, "unknown_tool", {"a": 1})
        # No assertion on calls — just verify no exception.

    def test_overlay_error_swallowed(self):
        """Errors from overlay methods should not propagate."""
        overlay = MagicMock()
        overlay.show_click.side_effect = RuntimeError("boom")
        _show_overlay(overlay, "click", {"x": 1, "y": 2})
        # Should not raise.


# -------------------------------------------------------------------- #
# Loop integration — overlay called and dismissed
# -------------------------------------------------------------------- #


class TestLoopOverlayIntegration:
    """Verify AgentLoop correctly calls overlay show methods.

    With ``setSharingType_(0)`` the overlay window is invisible to
    screenshots, so ``dismiss()`` is no longer called during the loop.
    The overlay persists until replaced by the next ``show_*`` call.
    """

    @pytest.mark.asyncio
    async def test_overlay_show_on_tool_execution(self, tmp_path):
        """Overlay show_click and show_finished should be called."""
        # click → finished
        responses = [
            _make_click_response(1),
            _make_finished_response("OK"),
        ]
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=responses)

        eye = AsyncMock()
        call_count = 0

        async def varying_capture():
            nonlocal call_count
            call_count += 1
            raw = f"PNG fake capture {call_count}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=varying_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value="Clicked")

        overlay = MagicMock()

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config={"max_steps": 10, "max_images": 5, "screenshot_interval_ms": 0},
            overlay=overlay,
        )

        with patch("src.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("click then finish")

        assert result.success is True

        # show_click called for the click step.
        overlay.show_click.assert_called_once_with(100, 200, False)

        # dismiss is NOT called during the loop (setSharingType hides overlay).
        overlay.dismiss.assert_not_called()

        # show_finished called for the finished step.
        overlay.show_finished.assert_called_once_with("OK")

    @pytest.mark.asyncio
    async def test_overlay_persists_through_screenshot(self, tmp_path):
        """Overlay is NOT dismissed before eye.capture (setSharingType hides it)."""
        brain = AsyncMock()
        brain.chat = AsyncMock(side_effect=[
            _make_click_response(1),
            _make_finished_response("done"),
        ])

        call_order: list[str] = []

        eye = AsyncMock()

        async def tracking_capture():
            call_order.append("capture")
            raw = f"PNG {len(call_order)}".encode()
            return _make_screenshot(b64=base64.b64encode(raw).decode("ascii"))

        eye.capture = AsyncMock(side_effect=tracking_capture)

        registry = AsyncMock()
        registry.get_openai_schemas.return_value = []
        registry.execute = AsyncMock(return_value="Clicked")

        overlay = MagicMock()

        def tracking_show_click(x, y, double=False):
            call_order.append("show_click")

        overlay.show_click = MagicMock(side_effect=tracking_show_click)

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config={"max_steps": 10, "max_images": 5, "screenshot_interval_ms": 0},
            overlay=overlay,
        )

        with patch("src.agent.loop.SCREENSHOTS_DIR", tmp_path):
            await loop.run("click then finish")

        # show_click should appear before the post-tool capture,
        # with NO dismiss in between.
        assert "show_click" in call_order
        assert "dismiss" not in call_order
        click_idx = call_order.index("show_click")
        captures_after = [i for i, v in enumerate(call_order) if v == "capture" and i > click_idx]
        assert len(captures_after) >= 1, "capture must happen after show_click"

    @pytest.mark.asyncio
    async def test_no_overlay_no_errors(self, tmp_path):
        """When overlay is None, the loop should work without errors."""
        brain = AsyncMock()
        brain.chat = AsyncMock(return_value=_make_finished_response("OK"))

        eye = AsyncMock()
        eye.capture = AsyncMock(return_value=_make_screenshot())

        registry = MagicMock()
        registry.get_openai_schemas.return_value = []

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config={"max_steps": 5, "max_images": 5, "screenshot_interval_ms": 0},
        )

        with patch("src.agent.loop.SCREENSHOTS_DIR", tmp_path):
            result = await loop.run("hello")

        assert result.success is True
        assert result.summary == "OK"
