"""CallUserTool -- marker tool to request human assistance.

Similar to :class:`FinishedTool`, the agent loop intercepts this tool by name
and pauses to wait for user input rather than routing through the registry.
The ``execute`` method is provided for completeness and simply returns the
question string.
"""

import logging
from typing import Any

from src.hand.tool import Tool

logger = logging.getLogger(__name__)


class CallUserTool(Tool):
    """Ask the user for help when the agent cannot proceed on its own."""

    @property
    def name(self) -> str:
        return "call_user"

    @property
    def description(self) -> str:
        return "遇到无法解决的问题（需要密码、验证码等），请求用户帮助。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "需要用户回答的问题",
                },
            },
            "required": ["question"],
        }

    async def execute(self, **kwargs: Any) -> str:
        question: str = kwargs["question"]
        logger.info("call_user: %s", question)
        return question
