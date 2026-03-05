"""DragTool -- drag from one coordinate to another."""

import logging
from typing import Any

import pyautogui

from src.hand.tool import Tool

pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

logger = logging.getLogger(__name__)


class DragTool(Tool):
    """Drag from ``(start_x, start_y)`` to ``(end_x, end_y)``."""

    @property
    def name(self) -> str:
        return "drag"

    @property
    def description(self) -> str:
        return "从一个坐标拖拽到另一个坐标。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "start_x": {"type": "integer"},
                "start_y": {"type": "integer"},
                "end_x": {"type": "integer"},
                "end_y": {"type": "integer"},
            },
            "required": ["start_x", "start_y", "end_x", "end_y"],
        }

    async def execute(self, **kwargs: Any) -> str:
        start_x: int = kwargs["start_x"]
        start_y: int = kwargs["start_y"]
        end_x: int = kwargs["end_x"]
        end_y: int = kwargs["end_y"]
        logger.info(
            "drag(%d, %d) -> (%d, %d)", start_x, start_y, end_x, end_y
        )

        # Manual mouseDown → moveTo → mouseUp to work around a pyautogui
        # bug where drag() passes button='primary' (instead of 'left') to
        # _mouseMoveDrag, which triggers an AssertionError on macOS.
        pyautogui.moveTo(start_x, start_y)
        pyautogui.mouseDown(button="left")
        pyautogui.moveTo(end_x, end_y, duration=0.5)
        pyautogui.mouseUp(button="left")

        return (
            f"已从 ({start_x}, {start_y}) 拖拽到 ({end_x}, {end_y})"
        )
