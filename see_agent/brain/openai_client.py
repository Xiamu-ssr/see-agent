"""OpenAI-protocol LLM backend (compatible with all OpenAI-API providers)."""

import json
import logging
from typing import Any, cast

from openai import AsyncOpenAI
from openai.types.chat import ChatCompletionMessageToolCall

from see_agent.brain.base import BaseBrain, BrainResponse, ToolCallInfo

logger = logging.getLogger(__name__)

_TEXT_TRUNCATE = 500


def _summarise_messages(messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Return a lightweight summary of *messages* for logging.

    - Text content is truncated to ``_TEXT_TRUNCATE`` characters.
    - ``image_url`` entries are replaced with a size placeholder so
      base64 blobs never appear in the log.
    """
    summaries: list[dict[str, Any]] = []
    for msg in messages:
        role = msg.get("role", "?")
        content = msg.get("content")

        # Tool result messages — preserve tool_call_id.
        if "tool_call_id" in msg:
            entry: dict[str, Any] = {"role": role, "tool_call_id": msg["tool_call_id"]}
            if content is not None:
                entry["content"] = str(content)[:_TEXT_TRUNCATE]
            summaries.append(entry)
        elif isinstance(content, str):
            text = content[:_TEXT_TRUNCATE] + ("…" if len(content) > _TEXT_TRUNCATE else "")
            summaries.append({"role": role, "content": text})
        elif isinstance(content, list):
            parts: list[Any] = []
            for part in content:
                if isinstance(part, dict) and part.get("type") == "image_url":
                    img = part.get("image_url", {})
                    url: str = img.get("url", "")
                    parts.append({
                        "type": "image",
                        "detail": img.get("detail", "auto"),
                        "size": f"{len(url)} chars",
                    })
                elif isinstance(part, dict) and part.get("type") == "text":
                    text = (part.get("text") or "")[:_TEXT_TRUNCATE]
                    parts.append({"type": "text", "text": text})
                else:
                    parts.append(part)
            summaries.append({"role": role, "content": parts})
        else:
            # Other message types (e.g. assistant with no content).
            summaries.append({"role": role})
    return summaries


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
        logger.debug(
            "LLM request messages: %s",
            json.dumps(_summarise_messages(messages), ensure_ascii=False),
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
