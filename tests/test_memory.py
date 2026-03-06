"""Unit tests for memory system."""

from see_agent.agent.loop import _strip_base64
from see_agent.brain.prompts import build_system_prompt
from see_agent.memory.base import BaseMemory


class FakeMemory(BaseMemory):
    """In-memory implementation for testing."""

    def __init__(self) -> None:
        self.stored: list[tuple[list[dict], str]] = []
        self.memories: list[str] = []

    def search(self, query: str, limit: int = 5) -> list[str]:
        return self.memories[:limit]

    def add(self, messages: list[dict], session_id: str) -> None:
        self.stored.append((messages, session_id))


class TestBaseMemory:
    """Tests for memory interface via FakeMemory."""

    def test_search_returns_list(self):
        mem = FakeMemory()
        mem.memories = ["User prefers dark mode", "Safari is the default browser"]
        results = mem.search("browser")
        assert len(results) == 2

    def test_add_stores_messages(self):
        mem = FakeMemory()
        messages = [{"role": "user", "content": "hello"}]
        mem.add(messages, "session-1")
        assert len(mem.stored) == 1
        assert mem.stored[0][1] == "session-1"


class TestStripBase64:
    """Tests for _strip_base64 helper."""

    def test_strips_image_url_parts(self):
        messages = [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Look at this"},
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/webp;base64,AAAA",
                            "detail": "high",
                        },
                    },
                ],
            }
        ]
        stripped = _strip_base64(messages)
        assert len(stripped) == 1
        parts = stripped[0]["content"]
        assert len(parts) == 2
        assert parts[0]["text"] == "Look at this"
        assert parts[1] == {"type": "text", "text": "[image]"}

    def test_preserves_plain_text(self):
        messages = [{"role": "user", "content": "hello world"}]
        stripped = _strip_base64(messages)
        assert stripped[0]["content"] == "hello world"

    def test_does_not_mutate_original(self):
        messages = [
            {
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {"url": "data:image/png;base64,ABC"},
                    },
                ],
            }
        ]
        _strip_base64(messages)
        # Original should be untouched
        assert messages[0]["content"][0]["type"] == "image_url"


class TestMemoryPromptInjection:
    """Tests for memory injection into system prompt."""

    def test_memory_in_prompt(self):
        config = {"language": "en", "max_steps": 10}
        prompt = build_system_prompt(config, memory_block="User prefers dark mode")
        assert "<MEMORY>" in prompt
        assert "User prefers dark mode" in prompt
        assert "</MEMORY>" in prompt

    def test_no_memory_no_section(self):
        config = {"language": "en", "max_steps": 10}
        prompt = build_system_prompt(config, memory_block="")
        assert "<MEMORY>" not in prompt
