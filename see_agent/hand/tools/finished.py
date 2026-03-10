"""FinishedTool -- marker tool indicating the task is complete.

The agent loop checks for this tool by name and exits the loop; it does
**not** route execution through the ToolRegistry.  The ``execute`` method
is provided for completeness and simply returns the summary string.
"""

import logging
from typing import Any

from see_agent.hand.tool import Tool, ToolResult

logger = logging.getLogger(__name__)


class FinishedTool(Tool):
    """Signal that the current task has been completed successfully."""

    @property
    def name(self) -> str:
        return "finished"

    @property
    def description(self) -> str:
        return "任务完成。必须调用此工具表示任务结束。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "任务完成的总结",
                },
            },
            "required": ["summary"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        summary: str = kwargs["summary"]
        logger.info("finished: %s", summary)
        return ToolResult(text=summary)
