"""WaitTool -- pause execution for a specified duration."""

import asyncio
import logging
from typing import Any

from see_agent.hand.tool import Tool

logger = logging.getLogger(__name__)


class WaitTool(Tool):
    """Wait (sleep) for a given number of seconds."""

    @property
    def name(self) -> str:
        return "wait"

    @property
    def description(self) -> str:
        return "等待指定秒数，用于等待页面加载或动画完成。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "seconds": {"type": "number", "default": 2},
            },
        }

    async def execute(self, **kwargs: Any) -> str:
        seconds: float = kwargs.get("seconds", 2)
        logger.info("wait: %.1fs", seconds)
        await asyncio.sleep(seconds)
        return f"已等待 {seconds} 秒"
