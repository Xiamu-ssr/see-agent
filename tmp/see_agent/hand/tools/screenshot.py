"""ScreenshotTool -- capture the current screen via the Eye subsystem."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Any, Callable

from see_agent.hand.tool import Tool, ToolResult, ToolResultImage

if TYPE_CHECKING:
    from see_agent.eye.base import BaseEye, Screenshot

logger = logging.getLogger(__name__)


class ScreenshotTool(Tool):
    """Take a screenshot of the current screen.

    Unlike most other tools this one requires an ``eye`` (``BaseEye``) instance
    to be injected at construction time so it can delegate the actual capture.

    An optional ``scale_fn`` callback can be provided to resize the screenshot
    before returning it (e.g. for LLM-compatible resolutions).
    """

    def __init__(
        self,
        eye: BaseEye,
        scale_fn: Callable[[Screenshot], Screenshot] | None = None,
    ) -> None:
        self._eye = eye
        self._scale_fn = scale_fn

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

    async def execute(self, **_kwargs: Any) -> ToolResult:
        logger.info("screenshot: capturing current screen")
        screenshot = await self._eye.capture()
        if self._scale_fn is not None:
            screenshot = self._scale_fn(screenshot)
        return ToolResult(
            text=f"截图完成 ({screenshot.width}x{screenshot.height})",
            images=[
                ToolResultImage(
                    base64=screenshot.base64,
                    mime_type=screenshot.mime_type,
                    detail=screenshot.detail,
                )
            ],
        )
