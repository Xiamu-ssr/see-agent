"""OpenAI-protocol LLM backend (compatible with all OpenAI-API providers)."""

import json
import logging
from typing import Any, cast

from openai import AsyncOpenAI
from openai.types.chat import ChatCompletionMessageToolCall

from see_agent.brain.base import BaseBrain, BrainResponse, ToolCallInfo

logger = logging.getLogger(__name__)


class OpenAIBrain(BaseBrain):
    """Brain implementation that talks to any OpenAI-compatible endpoint.

    Parameters:
        base_url: The base URL of the API (e.g. ``https://api.openai.com/v1``).
        api_key: The bearer token / API key.
        model: Model identifier (e.g. ``gpt-4o``, ``claude-opus-4-6``).
    """

    def __init__(self, base_url: str, api_key: str, model: str) -> None:
        self._client = AsyncOpenAI(base_url=base_url, api_key=api_key)
        self._model = model

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def chat(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
    ) -> BrainResponse:
        """Call the chat completions endpoint and return a parsed response.

        Uses ``stream=False`` (v1 – no streaming) so that tool_calls are
        available as a complete list in the response.
        """
        logger.info(
            "LLM request: model=%s messages=%d tools=%d",
            self._model,
            len(messages),
            len(tools),
        )

        try:
            response = await self._client.chat.completions.create(
                model=self._model,
                messages=cast(Any, messages),
                tools=cast(Any, tools),
                max_tokens=4096,
                stream=False,
            )
        except Exception:
            logger.exception("LLM API call failed")
            raise

        message = response.choices[0].message

        # -- Parse tool calls (if any) ----------------------------------
        tool_calls: list[ToolCallInfo] = []

        if message.tool_calls:
            for tc in message.tool_calls:
                # We only send function tools, so we only expect function
                # tool calls back.  Skip any non-function variants.
                if not isinstance(tc, ChatCompletionMessageToolCall):
                    continue

                try:
                    arguments = json.loads(tc.function.arguments)
                except (json.JSONDecodeError, TypeError):
                    logger.warning(
                        "Failed to parse arguments for tool call %s (%s): %r",
                        tc.id,
                        tc.function.name,
                        tc.function.arguments,
                    )
                    arguments = {}

                tool_calls.append(
                    ToolCallInfo(
                        id=tc.id,
                        name=tc.function.name,
                        arguments=arguments,
                    )
                )

            logger.info(
                "LLM returned %d tool call(s): %s",
                len(tool_calls),
                ", ".join(f"{t.name}({t.arguments})" for t in tool_calls),
            )
        else:
            # Text-only response (no tool calls)
            logger.info(
                "LLM returned text only (no tool calls): %s",
                (message.content or "")[:120],
            )

        return BrainResponse(
            content=message.content,
            tool_calls=tool_calls,
            raw=message,
        )

    async def summarize(self, messages: list[dict[str, Any]]) -> str:
        """Summarize earlier conversation messages for context compaction."""
        formatted = self._format_for_summary(messages)
        summary_messages: list[dict[str, Any]] = [
            {
                "role": "system",
                "content": (
                    "You are a conversation summarizer. Condense the following "
                    "conversation into a concise summary preserving: key decisions, "
                    "actions taken, results observed, and current state. "
                    "Output only the summary, no preamble."
                ),
            },
            {"role": "user", "content": formatted},
        ]

        response = await self._client.chat.completions.create(
            model=self._model,
            messages=cast(Any, summary_messages),
            max_tokens=2048,
            stream=False,
        )
        return response.choices[0].message.content or ""

    @staticmethod
    def _format_for_summary(messages: list[dict[str, Any]]) -> str:
        """Format messages for summarization, stripping images and truncating."""
        lines: list[str] = []
        for msg in messages:
            role = msg.get("role", "?")
            content = msg.get("content")
            if isinstance(content, list):
                # Extract only text parts, skip images.
                text_parts = [
                    p.get("text", "")
                    for p in content
                    if isinstance(p, dict) and p.get("type") == "text"
                ]
                text = " ".join(text_parts)
            elif isinstance(content, str):
                text = content
            else:
                text = str(content) if content else ""
            # Truncate long entries.
            if len(text) > 500:
                text = text[:497] + "..."
            if text:
                lines.append(f"[{role}] {text}")
        return "\n".join(lines)
