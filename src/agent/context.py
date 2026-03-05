"""Conversation context management with sliding-window screenshot pruning.

Maintains the full message history in OpenAI chat-completion format and
transparently applies a sliding window so that only the *N* most recent
screenshots are included when the context is handed to the LLM.  All text
content (assistant messages, tool results, system hints) is always preserved.
"""

from __future__ import annotations

import copy
import logging
from typing import Any

logger = logging.getLogger(__name__)


class ConversationContext:
    """Thread-safe (single-task) conversation buffer with screenshot pruning.

    Parameters:
        system_prompt: The system-level instruction prepended to every request.
        max_images: Maximum number of ``image_url`` entries to keep in the
            context window.  Older screenshots are replaced with a placeholder
            text message so the LLM still knows a screenshot *was* present.
    """

    def __init__(self, system_prompt: str, max_images: int = 5) -> None:
        self._messages: list[dict[str, Any]] = [
            {"role": "system", "content": system_prompt},
        ]
        self._max_images = max_images

    # ------------------------------------------------------------------ #
    # Public mutators
    # ------------------------------------------------------------------ #

    def add_user_task(
        self, text: str, screenshot_b64: str, detail: str
    ) -> None:
        """Append the initial user task message together with the first screenshot.

        Parameters:
            text: The natural-language task description.
            screenshot_b64: Base64-encoded PNG of the current screen.
            detail: OpenAI vision detail level (``"low"`` or ``"high"``).
        """
        self._messages.append(
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": text},
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": f"data:image/png;base64,{screenshot_b64}",
                            "detail": detail,
                        },
                    },
                ],
            }
        )
        logger.debug("Added user task with screenshot (detail=%s)", detail)

    def add_assistant(self, message: Any) -> None:
        """Append the raw assistant message returned by the LLM.

        Parameters:
            message: The raw API response object; ``message.model_dump()`` is
                called to convert it into a serialisable dict.
        """
        self._messages.append(message.model_dump())
        logger.debug("Added assistant message")

    def add_tool_result(
        self,
        tool_call_id: str,
        result: str,
        screenshot_b64: str | None = None,
        detail: str = "high",
    ) -> None:
        """Append a tool-result message and an optional follow-up screenshot.

        The screenshot (if provided) is appended as a *separate* user message
        so that the model sees the updated screen state immediately after the
        tool output.

        Parameters:
            tool_call_id: The ``id`` of the tool call this result corresponds to.
            result: Textual result returned by the tool.
            screenshot_b64: Optional base64-encoded PNG taken after execution.
            detail: OpenAI vision detail level for the screenshot.
        """
        self._messages.append(
            {
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": result,
            }
        )
        if screenshot_b64 is not None:
            self.add_screenshot(screenshot_b64, detail)
        logger.debug(
            "Added tool result (id=%s, has_screenshot=%s)",
            tool_call_id,
            screenshot_b64 is not None,
        )

    def add_screenshot(
        self, screenshot_b64: str, detail: str = "high"
    ) -> None:
        """Append a standalone screenshot as a user message.

        Parameters:
            screenshot_b64: Base64-encoded PNG image data.
            detail: OpenAI vision detail level (``"low"`` or ``"high"``).
        """
        self._messages.append(
            {
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": f"data:image/png;base64,{screenshot_b64}",
                            "detail": detail,
                        },
                    },
                ],
            }
        )
        logger.debug("Added standalone screenshot (detail=%s)", detail)

    def add_user_reply(self, text: str) -> None:
        """Append a plain-text user reply (e.g. after ``call_user``).

        Parameters:
            text: The user's response text.
        """
        self._messages.append({"role": "user", "content": text})
        logger.debug("Added user reply")

    def add_system_hint(self, text: str) -> None:
        """Inject a system-level hint into the conversation.

        The hint is delivered as a ``user`` message so it does not conflict
        with the single system message at the start of the conversation.

        Parameters:
            text: The hint text to inject.
        """
        self._messages.append({"role": "user", "content": text})
        logger.debug("Added system hint: %s", text[:80])

    # ------------------------------------------------------------------ #
    # Context retrieval with sliding window
    # ------------------------------------------------------------------ #

    def get_messages(self) -> list[dict[str, Any]]:
        """Return the full message list with the sliding-window applied.

        Only the most recent ``max_images`` screenshots are preserved.  Older
        screenshot entries are replaced with a placeholder text message so the
        LLM knows a screenshot existed at that position.  All non-image
        content is always included.

        Returns:
            A **new** list of message dicts (internal state is never mutated).
        """
        # 1. Locate every (message_index, content_part_index) that contains
        #    an image_url entry.  For messages whose ``content`` is a list of
        #    parts we record the part index; for messages where the whole
        #    content dict is an image we use ``None``.
        image_positions: list[tuple[int, int | None]] = []

        for msg_idx, msg in enumerate(self._messages):
            content = msg.get("content")
            if isinstance(content, list):
                for part_idx, part in enumerate(content):
                    if isinstance(part, dict) and part.get("type") == "image_url":
                        image_positions.append((msg_idx, part_idx))

        total_images = len(image_positions)
        if total_images <= self._max_images:
            # Nothing to prune -- return a shallow copy.
            return list(self._messages)

        # Indices of images to *drop* (the older ones).
        drop_count = total_images - self._max_images
        positions_to_drop = set(
            (msg_idx, part_idx)
            for msg_idx, part_idx in image_positions[:drop_count]
        )

        # Build the set of message indices that contain at least one dropped
        # image so we know which messages need rewriting.
        msgs_with_drops: dict[int, set[int | None]] = {}
        for msg_idx, part_idx in positions_to_drop:
            msgs_with_drops.setdefault(msg_idx, set()).add(part_idx)

        # 2. Build the output list, rewriting affected messages.
        output: list[dict[str, Any]] = []
        for msg_idx, msg in enumerate(self._messages):
            if msg_idx not in msgs_with_drops:
                output.append(msg)
                continue

            dropped_parts = msgs_with_drops[msg_idx]
            content = msg.get("content")

            if not isinstance(content, list):
                # Should not happen based on how we collected positions, but
                # be defensive.
                output.append(msg)
                continue

            # Filter out the dropped image parts.
            remaining_parts: list[dict[str, Any]] = []
            has_dropped = False
            for part_idx, part in enumerate(content):
                if part_idx in dropped_parts:
                    has_dropped = True
                else:
                    remaining_parts.append(part)

            if has_dropped:
                if remaining_parts:
                    # There are still text parts left -- keep the message but
                    # prepend a note about the omitted screenshot.
                    new_content = [
                        {"type": "text", "text": "[Screenshot omitted]"},
                    ] + remaining_parts
                    output.append(
                        {**msg, "content": copy.deepcopy(new_content)}
                    )
                else:
                    # The message consisted *only* of the image -- replace
                    # entirely with a placeholder.
                    output.append(
                        {
                            "role": msg["role"],
                            "content": [
                                {
                                    "type": "text",
                                    "text": "[Screenshot omitted]",
                                }
                            ],
                        }
                    )
            else:
                output.append(msg)

        return output
