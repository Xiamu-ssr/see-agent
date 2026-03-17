"""Unit tests for MarkdownMemoryBackend."""

import pytest

from see_agent.memory.markdown_backend import MarkdownMemoryBackend


@pytest.fixture
def mem(tmp_path):
    return MarkdownMemoryBackend(memory_dir=tmp_path)


class TestMarkdownMemory:
    """Tests for MarkdownMemoryBackend."""

    def test_write_and_search(self, mem):
        mem.write("MEMORY.md", "User prefers dark mode")
        results = mem.search("dark mode")
        assert len(results) >= 1
        assert "dark mode" in results[0]["snippet"].lower()
        assert results[0]["file"] == "MEMORY.md"

    def test_search_empty_dir(self, tmp_path):
        mem = MarkdownMemoryBackend(memory_dir=tmp_path / "empty")
        assert mem.search("anything") == []

    def test_search_limit(self, mem):
        lines = [f"Entry number {i} about cats" for i in range(10)]
        mem.write("MEMORY.md", "\n\n".join(lines))
        results = mem.search("cats", limit=3)
        assert len(results) <= 3

    def test_cjk_bigram_search(self, mem):
        mem.write("MEMORY.md", "用户喜欢深色模式")
        results = mem.search("深色模式")
        assert len(results) >= 1
        assert "深色" in results[0]["snippet"]

    def test_daily_file(self, mem):
        mem.write("2024-01-15.md", "Today I learned about testing")
        results = mem.search("testing")
        assert len(results) >= 1
        assert results[0]["file"] == "2024-01-15.md"

    def test_invalid_filename_rejected(self, mem):
        with pytest.raises(ValueError, match="Invalid memory filename"):
            mem.write("hack.py", "evil code")

    def test_invalid_filename_path_traversal(self, mem):
        with pytest.raises(ValueError, match="Invalid memory filename"):
            mem.write("../etc/passwd", "bad")

    def test_bm25_ranking(self, mem):
        mem.write(
            "MEMORY.md",
            "cats are great pets\n\ndogs are loyal animals\n\ncats and dogs together",
        )
        results = mem.search("cats")
        # "cats" appears twice in first and third paragraphs
        assert len(results) >= 2
        # First result should contain "cats"
        assert "cats" in results[0]["snippet"].lower()

    def test_append_preserves_existing(self, mem):
        mem.write("MEMORY.md", "First entry")
        mem.write("MEMORY.md", "Second entry")
        results = mem.search("First")
        assert len(results) >= 1
        results2 = mem.search("Second")
        assert len(results2) >= 1

    def test_empty_query_returns_empty(self, mem):
        mem.write("MEMORY.md", "some content")
        assert mem.search("") == []
        assert mem.search("   ") == []
