"""TypeTextTool -- type text via clipboard paste (supports Chinese)."""

import logging
import subprocess
from typing import Any

import pyautogui

from see_agent.hand.tool import Tool

pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

logger = logging.getLogger(__name__)


class TypeTextTool(Tool):
    """Type text at the current focus position.

    All text (both Chinese and English) is input via the system clipboard to
    avoid IME issues.  On macOS this uses ``pbcopy`` followed by Cmd+V.
    If the text ends with ``\\n``, an Enter key press is appended after paste.
    """

    @property
    def name(self) -> str:
        return "type_text"

    @property
    def description(self) -> str:
        return (
            "在当前焦点位置输入文字。中文通过剪贴板粘贴实现。"
            "如需按回车提交，在 text 末尾加 \\n。"
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "要输入的文字"},
            },
            "required": ["text"],
        }

    async def execute(self, **kwargs: Any) -> str:
        text: str = kwargs["text"]
        logger.info("type_text: %r", text)

        # Determine whether to press Enter after pasting.
        press_enter = text.endswith("\n")
        paste_text = text.rstrip("\n") if press_enter else text

        # Copy to clipboard via pbcopy (macOS).
        process = subprocess.Popen(
            ["pbcopy"],
            stdin=subprocess.PIPE,
        )
        process.communicate(paste_text.encode("utf-8"))

        # Paste from clipboard.
        pyautogui.hotkey("command", "v")

        # Press Enter if the original text ended with \n.
        if press_enter:
            pyautogui.press("enter")

        suffix = "（并按下回车）" if press_enter else ""
        return f"已输入文字: {paste_text}{suffix}"
