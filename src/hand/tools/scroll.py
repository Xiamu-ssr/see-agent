"""ScrollTool -- scroll at a given screen position."""

import logging
from typing import Any

import pyautogui

from src.hand.tool import Tool

pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

logger = logging.getLogger(__name__)


class ScrollTool(Tool):
    """Scroll at the specified screen coordinate."""

    @property
    def name(self) -> str:
        return "scroll"

    @property
    def description(self) -> str:
        return "在指定位置滚动。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "x": {"type": "integer", "description": "滚动位置横坐标"},
                "y": {"type": "integer", "description": "纵坐标"},
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                },
                "amount": {
                    "type": "integer",
                    "default": 3,
                    "description": "滚动格数",
                },
            },
            "required": ["x", "y", "direction"],
        }

    async def execute(self, **kwargs: Any) -> str:
        x: int = kwargs["x"]
        y: int = kwargs["y"]
        direction: str = kwargs["direction"]
        amount: int = kwargs.get("amount", 3)
        logger.info(
            "scroll(%d, %d, direction=%s, amount=%d)", x, y, direction, amount
        )

        # Move the mouse to the target position first so that the scroll
        # event occurs at the right location.
        pyautogui.moveTo(x, y)

        if direction in ("up", "down"):
            # PyAutoGUI scroll: positive = up, negative = down.
            clicks = amount if direction == "up" else -amount
            pyautogui.scroll(clicks)
        elif direction in ("left", "right"):
            # Horizontal scroll.  PyAutoGUI exposes ``hscroll`` on macOS.
            clicks = amount if direction == "right" else -amount
            if hasattr(pyautogui, "hscroll"):
                pyautogui.hscroll(clicks)
            else:
                # Fallback: no horizontal scroll support on this platform.
                logger.warning(
                    "hscroll not available; horizontal scroll ignored"
                )
                return f"水平滚动不支持当前平台 (direction={direction})"

        return f"已在 ({x}, {y}) 向{direction}滚动 {amount} 格"
