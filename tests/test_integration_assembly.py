"""Integration tests for component assembly in CLI and server.

Verify that ``_build_components`` (CLI) and ``_run_agent`` (server) wire up
the ``AgentLoop`` correctly — no ``TypeError`` on construction or on the
``loop.run()`` call signature.  Heavy dependencies (Eye, Brain, screenshots)
are replaced with lightweight mocks so nothing touches real hardware or LLM.
"""

from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from see_agent.brain.base import BrainResponse, ToolCallInfo

# -------------------------------------------------------------------- #
# Shared helpers
# -------------------------------------------------------------------- #

FAKE_CONFIG: dict = {
    "llm": {
        "base_url": "https://test.example.com/v1",
        "api_key": "sk-test-key",
        "model": "test-model",
    },
    "language": "zh",
    "max_steps": 3,
    "max_images": 2,
    "screenshot_interval_ms": 0,
    "show_overlay": False,
    "scaling_enabled": False,
}


def _make_fake_screenshot() -> MagicMock:
    """Return a MagicMock that quacks like a Screenshot."""
    shot = MagicMock()
    shot.base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJ"
    shot.width = 800
    shot.height = 600
    shot.detail = "low"
    shot.mime_type = "image/webp"
    shot.screen_width = None
    shot.screen_height = None
    shot.save = MagicMock(return_value=None)
    return shot


def _make_finished_response() -> BrainResponse:
    return BrainResponse(
        content="Done.",
        tool_calls=[
            ToolCallInfo(id="call_1", name="finished", arguments={"summary": "ok"})
        ],
        raw=MagicMock(
            model_dump=MagicMock(
                return_value={
                    "role": "assistant",
                    "content": "Done.",
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "finished",
                                "arguments": '{"summary":"ok"}',
                            },
                        }
                    ],
                }
            )
        ),
    )


def _make_mock_eye() -> MagicMock:
    eye = MagicMock()
    eye.capture = AsyncMock(return_value=_make_fake_screenshot())
    return eye


def _make_mock_brain() -> MagicMock:
    brain = MagicMock()
    brain.chat = AsyncMock(return_value=_make_finished_response())
    return brain


# Patch targets — CLI uses lazy imports so we patch at source modules.
_PATCHES_EYE = "see_agent.eye.mac.MacEye"
_PATCHES_BRAIN = "see_agent.brain.openai_client.OpenAIBrain"
_PATCHES_REGISTRY = "see_agent.hand.tools.create_registry"

# Server patches — target the names where they are used in the chat module,
# because top-level imports bind the name at import time.
_SRV_EYE = "see_agent.server.routes.chat.MacEye"
_SRV_BRAIN = "see_agent.server.routes.chat.OpenAIBrain"
_SRV_REGISTRY = "see_agent.server.routes.chat.create_registry"


# -------------------------------------------------------------------- #
# Server: _run_agent
# -------------------------------------------------------------------- #


class TestServerRunAgent:
    """Tests for ``src.server.routes.chat._run_agent``."""

    @patch(_SRV_REGISTRY, return_value=MagicMock())
    @patch(_SRV_BRAIN, return_value=_make_mock_brain())
    @patch(_SRV_EYE, return_value=_make_mock_eye())
    @pytest.mark.asyncio
    async def test_run_agent_no_type_error(self, _eye, _brain, _reg):
        """_run_agent should construct AgentLoop and call run() without TypeError."""
        from see_agent.server.models import TaskStatus
        from see_agent.server.routes.chat import _run_agent

        tasks: dict[str, TaskStatus] = {}
        await _run_agent("test123", "say hello", FAKE_CONFIG, tasks, {})

        assert "test123" in tasks
        assert tasks["test123"].status == "completed"

    @patch(_SRV_REGISTRY, return_value=MagicMock())
    @patch(_SRV_BRAIN, return_value=_make_mock_brain())
    @patch(_SRV_EYE, return_value=_make_mock_eye())
    @pytest.mark.asyncio
    async def test_run_agent_summary_propagates(self, _eye, _brain, _reg):
        """The finished-tool summary should appear in the final TaskStatus."""
        from see_agent.server.models import TaskStatus
        from see_agent.server.routes.chat import _run_agent

        tasks: dict[str, TaskStatus] = {}
        await _run_agent("t1", "greet", FAKE_CONFIG, tasks, {})

        assert tasks["t1"].summary == "ok"

    @patch(_SRV_REGISTRY, return_value=MagicMock())
    @patch(_SRV_BRAIN, return_value=_make_mock_brain())
    @patch(_SRV_EYE, return_value=_make_mock_eye())
    @pytest.mark.asyncio
    async def test_run_agent_broadcasts_sentinel(self, _eye, _brain, _reg):
        """After completion, _run_agent must broadcast None sentinel to subscribers."""
        from see_agent.server.models import TaskStatus
        from see_agent.server.routes.chat import _run_agent

        queue: asyncio.Queue[dict | None] = asyncio.Queue()
        subscribers: dict = {"done1": [queue]}
        tasks: dict[str, TaskStatus] = {}

        await _run_agent("done1", "finish", FAKE_CONFIG, tasks, subscribers)

        sentinel = await asyncio.wait_for(queue.get(), timeout=1.0)
        assert sentinel is None
