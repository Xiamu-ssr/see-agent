"""Unit tests for FileMemory backend."""

import pytest

from see_agent.memory.file_backend import FileMemory


@pytest.fixture
def mem(tmp_path):
    return FileMemory(memory_dir=tmp_path)


class TestFileMemory:
    """Tests for FileMemory JSONL backend."""

    def test_add_and_search(self, mem):
        mem.add([{"role": "user", "content": "I prefer dark mode"}], "s1")
        results = mem.search("dark mode")
        assert len(results) >= 1
        assert "dark mode" in results[0].lower()

    def test_search_empty(self, mem):
        results = mem.search("anything")
        assert results == []

    def test_search_limit(self, mem):
        for i in range(10):
            mem.add([{"role": "user", "content": f"memory entry {i} hello"}], f"s{i}")
        results = mem.search("hello", limit=3)
        assert len(results) == 3

    def test_keyword_scoring(self, mem):
        mem.add([{"role": "user", "content": "cat dog fish"}], "s1")
        mem.add([{"role": "user", "content": "cat dog bird"}], "s2")
        mem.add([{"role": "user", "content": "cat elephant"}], "s3")
        # "cat dog" has 2-word overlap with first two, 1 with third
        results = mem.search("cat dog")
        assert len(results) == 3
        # First results should have higher overlap
        assert "dog" in results[0]

    def test_agent_id_filtering(self, mem):
        mem.add([{"role": "user", "content": "hello from alice"}], "s1", agent_id="alice")
        mem.add([{"role": "user", "content": "hello from bob"}], "s2", agent_id="bob")
        results = mem.search("hello", agent_id="alice")
        assert len(results) == 1
        assert "alice" in results[0]

    def test_clear_all(self, mem):
        mem.add([{"role": "user", "content": "data"}], "s1")
        mem.clear()
        assert mem.search("data") == []

    def test_clear_by_agent_id(self, mem):
        mem.add([{"role": "user", "content": "alice data"}], "s1", agent_id="alice")
        mem.add([{"role": "user", "content": "bob data"}], "s2", agent_id="bob")
        mem.clear(agent_id="alice")
        results = mem.search("data")
        assert len(results) == 1
        assert "bob" in results[0]

    def test_empty_dir(self, tmp_path):
        """FileMemory works with a freshly created directory."""
        mem = FileMemory(memory_dir=tmp_path / "new_subdir")
        assert mem.search("test") == []
        mem.add([{"role": "user", "content": "first entry"}], "s1")
        assert len(mem.search("first")) == 1
