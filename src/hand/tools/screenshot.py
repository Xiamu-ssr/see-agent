"""ScreenshotTool -- capture the current screen via the Eye subsystem."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Any

from src.hand.tool import Tool

if TYPE_CHECKING:
    from src.eye.base import BaseEye

logger = logging.getLogger(__name__)


class ScreenshotTool(Tool):
    """Take a screenshot of the current screen.

    Unlike most other tools this one requires an ``eye`` (``BaseEye``) instance
    to be injected at construction time so it can delegate the actual capture.
    """

    def __init__(self, eye: BaseEye) -> None:
        self._eye = eye

    @property
    def name(self) -> str:
        return "screenshot"

    @property
    def description(self) -> str:
        return "截取当前屏幕截图，用于观察当前界面状态。在不确定当前状态时使用。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {},
            "required": [],
        }

    async def execute(self, **_kwargs: Any) -> str:
        logger.info("screenshot: capturing current screen")
        screenshot = await self._eye.capture()
        # The actual base64 image data is attached to the conversation context
        # by the agent loop -- here we just return a confirmation string along
        # with basic metadata so the LLM knows the capture succeeded.
        return (
            f"截图完成 ({screenshot.width}x{screenshot.height})"
        )
