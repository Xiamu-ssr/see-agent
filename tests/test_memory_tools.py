"""Unit tests for memory tools."""

import pytest

from see_agent.hand.tools.memory import MemorySearchTool, WriteMemoryTool
from see_agent.memory.markdown_backend import MarkdownMemoryBackend


@pytest.fixture
def backend(tmp_path):
    return MarkdownMemoryBackend(memory_dir=tmp_path)


class TestMemorySearchTool:
    """Tests for MemorySearchTool."""

    @pytest.mark.asyncio
    async def test_search_no_results(self, backend):
        tool = MemorySearchTool(backend)
        result = await tool.execute(query="nonexistent")
        assert "No relevant memories" in result.text

    @pytest.mark.asyncio
    async def test_search_with_results(self, backend):
        backend.write("MEMORY.md", "User prefers dark mode")
        tool = MemorySearchTool(backend)
        result = await tool.execute(query="dark mode")
        assert "dark mode" in result.text.lower()
        assert "MEMORY.md" in result.text

    @pytest.mark.asyncio
    async def test_tool_schema(self, backend):
        tool = MemorySearchTool(backend)
        assert tool.name == "memory_search"
        schema = tool.to_openai_schema()
        assert schema["function"]["name"] == "memory_search"
        assert "query" in schema["function"]["parameters"]["properties"]


class TestWriteMemoryTool:
    """Tests for WriteMemoryTool."""

    @pytest.mark.asyncio
    async def test_write_success(self, backend):
        tool = WriteMemoryTool(backend)
        result = await tool.execute(file="MEMORY.md", content="Test note")
        assert "Written to MEMORY.md" in result.text

    @pytest.mark.asyncio
    async def test_write_invalid_filename(self, backend):
        tool = WriteMemoryTool(backend)
        result = await tool.execute(file="hack.py", content="bad")
        assert "Error" in result.text

    @pytest.mark.asyncio
    async def test_write_daily_file(self, backend):
        tool = WriteMemoryTool(backend)
        result = await tool.execute(file="2024-01-15.md", content="Daily log")
        assert "Written to 2024-01-15.md" in result.text

    @pytest.mark.asyncio
    async def test_tool_schema(self, backend):
        tool = WriteMemoryTool(backend)
        assert tool.name == "memory_write"
        schema = tool.to_openai_schema()
        params = schema["function"]["parameters"]["properties"]
        assert "file" in params
        assert "content" in params
