"""HotkeyTool -- press a keyboard shortcut combination."""

import logging
from typing import Any

import pyautogui

from src.hand.tool import Tool

pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

logger = logging.getLogger(__name__)

# pyautogui only recognises certain key names.  LLMs frequently emit
# synonyms that pyautogui silently ignores, so we normalise first.
KEY_ALIASES: dict[str, str] = {
    "control": "ctrl",
    "cmd": "command",
    "option": "alt",
    "enter": "return",
    "esc": "escape",
}


class HotkeyTool(Tool):
    """Press a keyboard shortcut (e.g. Cmd+C, Ctrl+Alt+Delete)."""

    @property
    def name(self) -> str:
        return "hotkey"

    @property
    def description(self) -> str:
        return (
            "按下快捷键组合。例如 ['command','c'] 表示 Cmd+C。"
            "键名请用: command, ctrl, alt, shift, return, escape, "
            "tab, space, delete, fn, up, down, left, right。"
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "keys": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": (
                        "按键列表。使用 pyautogui 识别的键名: "
                        "command, ctrl, alt, shift, return, escape 等"
                    ),
                },
            },
            "required": ["keys"],
        }

    async def execute(self, **kwargs: Any) -> str:
        keys: list[str] = kwargs["keys"]
        normalised = [KEY_ALIASES.get(k.lower(), k) for k in keys]
        if normalised != keys:
            logger.info("hotkey: %s (normalised from %s)", normalised, keys)
        else:
            logger.info("hotkey: %s", keys)
        pyautogui.hotkey(*normalised)
        combo = "+".join(normalised)
        return f"已按下快捷键: {combo}"
