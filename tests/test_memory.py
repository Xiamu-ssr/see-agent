"""Unit tests for memory system."""

from see_agent.brain.prompts import build_system_prompt
from see_agent.memory.base import BaseMemory


class FakeMemory(BaseMemory):
    """In-memory implementation for testing."""

    def __init__(self) -> None:
        self.written: list[tuple[str, str]] = []

    def search(self, query: str, limit: int = 5) -> list[dict[str, str]]:
        return []

    def write(self, file: str, content: str) -> None:
        self.written.append((file, content))


class TestBaseMemory:
    """Tests for memory interface via FakeMemory."""

    def test_search_returns_list(self):
        mem = FakeMemory()
        results = mem.search("browser")
        assert isinstance(results, list)

    def test_write_stores(self):
        mem = FakeMemory()
        mem.write("MEMORY.md", "hello")
        assert len(mem.written) == 1
        assert mem.written[0] == ("MEMORY.md", "hello")


class TestMemoryPrompt:
    """Tests for memory tool rule in system prompt."""

    def test_memory_rule_in_prompt_zh(self):
        config = {"language": "zh", "max_steps": 10}
        prompt = build_system_prompt(config)
        assert "memory_search" in prompt
        assert "memory_write" in prompt

    def test_memory_rule_in_prompt_en(self):
        config = {"language": "en", "max_steps": 10}
        prompt = build_system_prompt(config)
        assert "memory_search" in prompt
        assert "memory_write" in prompt

    def test_no_memory_block_section(self):
        """The old <MEMORY> section should no longer appear."""
        config = {"language": "en", "max_steps": 10}
        prompt = build_system_prompt(config)
        assert "<MEMORY>" not in prompt
