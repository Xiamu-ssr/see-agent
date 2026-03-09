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
        # After stripping, all-text list is flattened to a plain string
        # so mem0 doesn't mistake it for vision content.
        content = stripped[0]["content"]
        assert isinstance(content, str)
        assert "Look at this" in content
        assert "[image]" in content

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


class TestBuildMem0Config:
    """Tests for _build_mem0_config helper."""

    def test_empty_config_returns_none(self):
        from see_agent.memory.mem0_backend import _build_mem0_config

        result = _build_mem0_config({})
        assert result is None

    def test_all_empty_strings_returns_none(self):
        from see_agent.memory.mem0_backend import _build_mem0_config

        result = _build_mem0_config({
            "llm_base_url": "",
            "llm_api_key": "",
            "llm_model": "",
            "embedding_model": "",
            "storage_path": "",
        })
        assert result is None

    def test_llm_model_sets_llm_section(self):
        from see_agent.memory.mem0_backend import _build_mem0_config

        result = _build_mem0_config({
            "llm_model": "gpt-4o",
            "llm_base_url": "https://api.example.com/v1",
            "llm_api_key": "sk-test",
        })
        assert result is not None
        assert result["llm"]["config"]["model"] == "gpt-4o"
        assert result["llm"]["config"]["api_key"] == "sk-test"

    def test_storage_path_expanded(self):
        from see_agent.memory.mem0_backend import _build_mem0_config

        result = _build_mem0_config({
            "storage_path": "~/test/qdrant",
        })
        assert result is not None
        assert "~" not in result["vector_store"]["config"]["path"]

    def test_embedding_model_set(self):
        from see_agent.memory.mem0_backend import _build_mem0_config

        result = _build_mem0_config({
            "embedding_model": "text-embedding-3-small",
            "llm_base_url": "https://api.example.com/v1",
        })
        assert result is not None
        assert result["embedder"]["config"]["model"] == "text-embedding-3-small"
