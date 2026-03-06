"""ClickTool -- click at a screen coordinate using PyAutoGUI."""

import logging
from typing import Any

import pyautogui

from see_agent.hand.tool import Tool

pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

logger = logging.getLogger(__name__)


class ClickTool(Tool):
    """Click at the specified logical-pixel coordinate on screen."""

    @property
    def name(self) -> str:
        return "click"

    @property
    def description(self) -> str:
        return "点击屏幕上的指定坐标。坐标是逻辑像素，左上角为 (0,0)。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "x": {"type": "integer", "description": "横坐标（逻辑像素）"},
                "y": {"type": "integer", "description": "纵坐标（逻辑像素）"},
                "button": {
                    "type": "string",
                    "enum": ["left", "right", "middle"],
                    "default": "left",
                },
                "double": {"type": "boolean", "default": False},
            },
            "required": ["x", "y"],
        }

    async def execute(self, **kwargs: Any) -> str:
        x: int = kwargs["x"]
        y: int = kwargs["y"]
        button: str = kwargs.get("button", "left")
        double: bool = kwargs.get("double", False)
        clicks = 2 if double else 1
        logger.info(
            "click(%d, %d, button=%s, clicks=%d)", x, y, button, clicks
        )
        pyautogui.click(x, y, button=button, clicks=clicks)
        action = "双击" if double else "点击"
        return f"已{action} ({x}, {y})，按钮={button}"
