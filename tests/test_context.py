"""Unit tests for ConversationContext (src/agent/context.py)."""


from see_agent.agent.context import ConversationContext
from see_agent.hand.tool import ToolResult, ToolResultImage

# -------------------------------------------------------------------- #
# Helpers
# -------------------------------------------------------------------- #

SYSTEM_PROMPT = "You are a test assistant."
FAKE_B64 = "aVZCT1J3MEtHZ29BQUFBTlNV"  # short dummy base64 string


def _count_image_parts(messages: list[dict]) -> int:
    """Return the total number of image_url content parts across all messages."""
    count = 0
    for msg in messages:
        content = msg.get("content")
        if isinstance(content, list):
            for part in content:
                if isinstance(part, dict) and part.get("type") == "image_url":
                    count += 1
    return count


def _count_omitted_placeholders(messages: list[dict]) -> int:
    """Return the number of '[Screenshot omitted]' text parts."""
    count = 0
    for msg in messages:
        content = msg.get("content")
        if isinstance(content, list):
            for part in content:
                if (
                    isinstance(part, dict)
                    and part.get("type") == "text"
                    and part.get("text") == "[Screenshot omitted]"
                ):
                    count += 1
    return count


# -------------------------------------------------------------------- #
# Tests
# -------------------------------------------------------------------- #


class TestConversationContext:
    """Tests for ConversationContext."""

    def test_initial_messages(self):
        """After init, get_messages() contains exactly one system message."""
        ctx = ConversationContext(SYSTEM_PROMPT)
        msgs = ctx.get_messages()
        assert len(msgs) == 1
        assert msgs[0]["role"] == "system"
        assert msgs[0]["content"] == SYSTEM_PROMPT

    def test_add_user_task(self):
        """add_user_task appends a user message with text and image content."""
        ctx = ConversationContext(SYSTEM_PROMPT)
        ctx.add_user_task("Open Safari", FAKE_B64, "low")

        msgs = ctx.get_messages()
        assert len(msgs) == 2
        user_msg = msgs[1]
        assert user_msg["role"] == "user"

        content = user_msg["content"]
        assert isinstance(content, list)
        assert len(content) == 2

        # First part is text
        assert content[0]["type"] == "text"
        assert content[0]["text"] == "Open Safari"

        # Second part is image
        assert content[1]["type"] == "image_url"
        assert FAKE_B64 in content[1]["image_url"]["url"]
        assert content[1]["image_url"]["detail"] == "low"

    def test_add_assistant(self):
        """add_assistant appends the assistant message via model_dump()."""

        class FakeRaw:
            def model_dump(self):
                return {"role": "assistant", "content": "I will click OK."}

        ctx = ConversationContext(SYSTEM_PROMPT)
        ctx.add_assistant(FakeRaw())

        msgs = ctx.get_messages()
        assert len(msgs) == 2
        assert msgs[1]["role"] == "assistant"
        assert msgs[1]["content"] == "I will click OK."

    def test_add_tool_result(self):
        """add_tool_result adds a tool message and optional screenshot."""
        ctx = ConversationContext(SYSTEM_PROMPT)

        # Without screenshot
        ctx.add_tool_result("tc_1", "Clicked OK")
        msgs = ctx.get_messages()
        assert len(msgs) == 2
        assert msgs[1]["role"] == "tool"
        assert msgs[1]["tool_call_id"] == "tc_1"
        assert msgs[1]["content"] == "Clicked OK"

        # With screenshot
        ctx.add_tool_result("tc_2", "Typed hello", FAKE_B64, "high")
        msgs = ctx.get_messages()
        assert len(msgs) == 4  # system + tool_1 + tool_2 + screenshot
        assert msgs[2]["role"] == "tool"
        assert msgs[2]["tool_call_id"] == "tc_2"
        # The screenshot is added as a separate user message
        assert msgs[3]["role"] == "user"
        assert isinstance(msgs[3]["content"], list)
        assert msgs[3]["content"][0]["type"] == "image_url"

    def test_add_user_reply(self):
        """add_user_reply adds a plain text user message."""
        ctx = ConversationContext(SYSTEM_PROMPT)
        ctx.add_user_reply("Yes, please continue.")

        msgs = ctx.get_messages()
        assert len(msgs) == 2
        assert msgs[1]["role"] == "user"
        assert msgs[1]["content"] == "Yes, please continue."

    def test_add_system_hint(self):
        """add_system_hint adds a hint as a user-role message."""
        ctx = ConversationContext(SYSTEM_PROMPT)
        ctx.add_system_hint("Warning: no progress detected.")

        msgs = ctx.get_messages()
        assert len(msgs) == 2
        assert msgs[1]["role"] == "user"
        assert msgs[1]["content"] == "Warning: no progress detected."

    def test_sliding_window(self):
        """With max_images=3, adding 6 screenshots keeps only the 3 most recent."""
        ctx = ConversationContext(SYSTEM_PROMPT, max_images=3)

        # Add 6 screenshots as standalone user messages
        for i in range(6):
            ctx.add_screenshot(f"screenshot_{i}", "high")

        msgs = ctx.get_messages()

        # Should have exactly 3 image_url parts remaining
        assert _count_image_parts(msgs) == 3

        # The 3 older screenshots should be replaced with placeholders
        assert _count_omitted_placeholders(msgs) == 3

    def test_sliding_window_preserves_text(self):
        """Text-only messages are never removed by the sliding window."""
        ctx = ConversationContext(SYSTEM_PROMPT, max_images=2)

        # Interleave text and screenshot messages
        ctx.add_user_reply("First text")
        ctx.add_screenshot(FAKE_B64, "high")
        ctx.add_user_reply("Second text")
        ctx.add_screenshot(FAKE_B64, "high")
        ctx.add_user_reply("Third text")
        ctx.add_screenshot(FAKE_B64, "high")
        ctx.add_user_reply("Fourth text")
        ctx.add_screenshot(FAKE_B64, "high")

        msgs = ctx.get_messages()

        # All text messages must be preserved
        text_contents = [
            m["content"]
            for m in msgs
            if m.get("role") == "user" and isinstance(m.get("content"), str)
        ]
        assert "First text" in text_contents
        assert "Second text" in text_contents
        assert "Third text" in text_contents
        assert "Fourth text" in text_contents

        # Only 2 images kept
        assert _count_image_parts(msgs) == 2

        # 2 images were dropped -> 2 placeholders
        assert _count_omitted_placeholders(msgs) == 2

    def test_add_tool_result_with_tool_result_object(self):
        """add_tool_result accepts a ToolResult and injects its images."""
        ctx = ConversationContext(SYSTEM_PROMPT, max_images=10)

        tr = ToolResult(
            text="截图完成 (800x600)",
            images=[ToolResultImage(base64=FAKE_B64, mime_type="image/webp", detail="high")],
        )
        ctx.add_tool_result("tc_1", tr)

        msgs = ctx.get_messages()
        # system + tool_result + screenshot_from_images
        assert len(msgs) == 3
        assert msgs[1]["role"] == "tool"
        assert msgs[1]["content"] == "截图完成 (800x600)"
        # Image injected as user message
        assert msgs[2]["role"] == "user"
        assert _count_image_parts(msgs) == 1

    def test_add_tool_result_str_backward_compat(self):
        """add_tool_result still works with a plain str."""
        ctx = ConversationContext(SYSTEM_PROMPT)
        ctx.add_tool_result("tc_1", "Clicked OK")

        msgs = ctx.get_messages()
        assert len(msgs) == 2
        assert msgs[1]["role"] == "tool"
        assert msgs[1]["content"] == "Clicked OK"
