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

from src.agent.loop import AgentLoop, RunResult
from src.brain.base import BrainResponse, ToolCallInfo

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
_PATCHES_EYE = "src.eye.mac.MacEye"
_PATCHES_BRAIN = "src.brain.openai_client.OpenAIBrain"
_PATCHES_REGISTRY = "src.hand.tools.create_registry"


# -------------------------------------------------------------------- #
# CLI: _build_components
# -------------------------------------------------------------------- #


class TestCliBuildComponents:
    """Tests for ``src.cli.main._build_components``."""

    @patch(_PATCHES_REGISTRY, return_value=MagicMock())
    @patch(_PATCHES_BRAIN, return_value=_make_mock_brain())
    @patch(_PATCHES_EYE, return_value=_make_mock_eye())
    def test_returns_agent_loop(self, _eye, _brain, _reg):
        """_build_components should return an AgentLoop without TypeError."""
        from src.cli.main import _build_components

        loop = _build_components(FAKE_CONFIG)

        assert isinstance(loop, AgentLoop)

    @patch(_PATCHES_REGISTRY, return_value=MagicMock())
    @patch(_PATCHES_BRAIN, return_value=_make_mock_brain())
    @patch(_PATCHES_EYE, return_value=_make_mock_eye())
    def test_loop_run_accepts_task_only(self, _eye, _brain, _reg):
        """loop.run(task) must work with a single positional str argument."""
        from src.cli.main import _build_components

        loop = _build_components(FAKE_CONFIG)
        result = asyncio.run(loop.run("hello"))

        assert isinstance(result, RunResult)
        assert isinstance(result.summary, str)

    @patch(_PATCHES_REGISTRY, return_value=MagicMock())
    @patch(_PATCHES_BRAIN, return_value=_make_mock_brain())
    @patch(_PATCHES_EYE, return_value=_make_mock_eye())
    def test_config_values_propagate(self, _eye, _brain, _reg):
        """AgentLoop should read max_steps / max_images from config."""
        from src.cli.main import _build_components

        loop = _build_components(FAKE_CONFIG)

        assert loop._max_steps == FAKE_CONFIG["max_steps"]
        assert loop._max_images == FAKE_CONFIG["max_images"]


# -------------------------------------------------------------------- #
# Server: _run_agent
# -------------------------------------------------------------------- #


class TestServerRunAgent:
    """Tests for ``src.server.routes.chat._run_agent``."""

    @patch(_PATCHES_REGISTRY, return_value=MagicMock())
    @patch(_PATCHES_BRAIN, return_value=_make_mock_brain())
    @patch(_PATCHES_EYE, return_value=_make_mock_eye())
    @pytest.mark.asyncio
    async def test_run_agent_no_type_error(self, _eye, _brain, _reg):
        """_run_agent should construct AgentLoop and call run() without TypeError."""
        from src.server.models import TaskStatus
        from src.server.routes.chat import _run_agent

        tasks: dict[str, TaskStatus] = {}
        await _run_agent("test123", "say hello", FAKE_CONFIG, tasks, {})

        assert "test123" in tasks
        assert tasks["test123"].status == "completed"

    @patch(_PATCHES_REGISTRY, return_value=MagicMock())
    @patch(_PATCHES_BRAIN, return_value=_make_mock_brain())
    @patch(_PATCHES_EYE, return_value=_make_mock_eye())
    @pytest.mark.asyncio
    async def test_run_agent_summary_propagates(self, _eye, _brain, _reg):
        """The finished-tool summary should appear in the final TaskStatus."""
        from src.server.models import TaskStatus
        from src.server.routes.chat import _run_agent

        tasks: dict[str, TaskStatus] = {}
        await _run_agent("t1", "greet", FAKE_CONFIG, tasks, {})

        assert tasks["t1"].summary == "ok"

    @patch(_PATCHES_REGISTRY, return_value=MagicMock())
    @patch(_PATCHES_BRAIN, return_value=_make_mock_brain())
    @patch(_PATCHES_EYE, return_value=_make_mock_eye())
    @pytest.mark.asyncio
    async def test_run_agent_broadcasts_sentinel(self, _eye, _brain, _reg):
        """After completion, _run_agent must broadcast None sentinel to subscribers."""
        from src.server.models import TaskStatus
        from src.server.routes.chat import _run_agent

        queue: asyncio.Queue[dict | None] = asyncio.Queue()
        subscribers: dict = {"done1": [queue]}
        tasks: dict[str, TaskStatus] = {}

        await _run_agent("done1", "finish", FAKE_CONFIG, tasks, subscribers)

        sentinel = await asyncio.wait_for(queue.get(), timeout=1.0)
        assert sentinel is None
