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

        # Move to the starting position, then drag to the end position.
        pyautogui.moveTo(start_x, start_y)
        pyautogui.drag(
            end_x - start_x,
            end_y - start_y,
            duration=0.5,
        )

        return (
            f"已从 ({start_x}, {start_y}) 拖拽到 ({end_x}, {end_y})"
        )
