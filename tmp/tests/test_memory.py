"""Unit tests for memory system — real MarkdownMemoryBackend."""

from __future__ import annotations

from pathlib import Path

import pytest

from see_agent.memory.markdown_backend import MarkdownMemoryBackend


@pytest.fixture
def mem(tmp_path: Path) -> MarkdownMemoryBackend:
    return MarkdownMemoryBackend(tmp_path / "memory")


class TestMarkdownMemoryBackend:
    """Tests for the actual memory backend."""

    def test_write_creates_file(self, mem: MarkdownMemoryBackend, tmp_path: Path):
        mem.write("MEMORY.md", "# Long-term\n- lesson one")
        path = tmp_path / "memory" / "MEMORY.md"
        assert path.exists()
        assert "lesson one" in path.read_text()

    def test_write_and_search(self, mem: MarkdownMemoryBackend):
        mem.write("MEMORY.md", "今天修了 plist 权限问题，macOS Sequoia 签名检查")
        results = mem.search("plist 权限")
        assert len(results) >= 1
        assert any("plist" in r.get("snippet", "") for r in results)

    def test_search_empty(self, mem: MarkdownMemoryBackend):
        results = mem.search("nonexistent topic xyz123")
        assert isinstance(results, list)
        assert len(results) == 0

    def test_write_overwrite(self, mem: MarkdownMemoryBackend):
        mem.write("MEMORY.md", "version 1")
        mem.write("MEMORY.md", "version 2 updated")
        results = mem.search("version 2")
        assert len(results) >= 1
        assert any("version 2" in r.get("snippet", "") for r in results)

    def test_search_across_memory_and_daily(self, mem: MarkdownMemoryBackend):
        mem.write("MEMORY.md", "apple banana cherry")
        mem.write("2026-03-11.md", "dog elephant frog")
        results_banana = mem.search("banana")
        assert len(results_banana) >= 1
        assert any("banana" in r.get("snippet", "") for r in results_banana)
        results_elephant = mem.search("elephant")
        assert len(results_elephant) >= 1

    def test_search_limit(self, mem: MarkdownMemoryBackend):
        # Write enough content to get multiple results.
        lines = [f"## Section {i}\nkeyword important data item {i}\n" for i in range(10)]
        mem.write("MEMORY.md", "\n".join(lines))
        results = mem.search("keyword", limit=3)
        assert len(results) <= 3

    def test_invalid_filename_rejected(self, mem: MarkdownMemoryBackend):
        with pytest.raises(ValueError, match="Invalid memory filename"):
            mem.write("random.md", "should fail")

    def test_daily_file_valid(self, mem: MarkdownMemoryBackend):
        mem.write("2026-03-11.md", "today's log")
        results = mem.search("today's log")
        assert len(results) >= 1

    def test_search_returns_file_and_snippet(self, mem: MarkdownMemoryBackend):
        mem.write("MEMORY.md", "unique_token_abc123 in context")
        results = mem.search("unique_token_abc123")
        assert len(results) >= 1
        r = results[0]
        assert "file" in r
        assert "snippet" in r
        assert r["file"] == "MEMORY.md"


class TestMemoryPromptIntegration:
    """Memory-related prompt injection via workspace files."""

    def test_memory_rule_from_agent_files(self, tmp_path: Path):
        from see_agent.brain.prompts import build_system_prompt

        (tmp_path / "AGENTS.md").write_text(
            "# Rules\n- Use memory_search to find info\n- Use write_memory to save",
            encoding="utf-8",
        )
        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = build_system_prompt(config, agent_dir=tmp_path)
        assert "memory_search" in prompt

    def test_no_legacy_memory_section(self):
        from see_agent.brain.prompts import build_system_prompt

        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = build_system_prompt(config)
        assert "<MEMORY>" not in prompt
