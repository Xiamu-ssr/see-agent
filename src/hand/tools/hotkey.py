"""HotkeyTool -- press a keyboard shortcut combination."""

import logging
from typing import Any

import pyautogui

from src.hand.tool import Tool

pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

logger = logging.getLogger(__name__)


class HotkeyTool(Tool):
    """Press a keyboard shortcut (e.g. Cmd+C, Ctrl+Alt+Delete)."""

    @property
    def name(self) -> str:
        return "hotkey"

    @property
    def description(self) -> str:
        return "按下快捷键组合。例如 ['command','c'] 表示 Cmd+C。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "keys": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "按键列表",
                },
            },
            "required": ["keys"],
        }

    async def execute(self, **kwargs: Any) -> str:
        keys: list[str] = kwargs["keys"]
        logger.info("hotkey: %s", keys)
        pyautogui.hotkey(*keys)
        combo = "+".join(keys)
        return f"已按下快捷键: {combo}"
