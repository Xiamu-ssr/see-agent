"""Unit tests for brain prompts and base data types."""


from unittest.mock import AsyncMock, MagicMock

import pytest

from see_agent.brain.base import BrainResponse, ToolCallInfo
from see_agent.brain.prompts import build_system_prompt

# -------------------------------------------------------------------- #
# Tests for build_system_prompt
# -------------------------------------------------------------------- #


class TestBuildSystemPrompt:
    """Tests for the build_system_prompt function."""

    def test_build_system_prompt_zh(self):
        """Chinese prompt contains constraints."""
        config = {"web": {"language": "zh"}, "agent": {"max_steps": 30}}
        prompt = build_system_prompt(config)

        assert "30" in prompt
        assert "最多执行" in prompt

    def test_build_system_prompt_en(self):
        """Even with language=en, constraints are in Chinese (unified)."""
        config = {"web": {"language": "en"}, "agent": {"max_steps": 50}}
        prompt = build_system_prompt(config)

        assert "50" in prompt
        assert "最多执行" in prompt

    def test_build_system_prompt_with_agent_files(self, tmp_path):
        """When agent_dir has md files, their content is injected."""
        (tmp_path / "SOUL.md").write_text("I am friendly and patient.", encoding="utf-8")
        (tmp_path / "AGENTS.md").write_text("# Rules\nDo stuff.", encoding="utf-8")

        config = {"web": {"language": "en"}, "agent": {"max_steps": 50}}
        prompt = build_system_prompt(config, agent_dir=tmp_path)

        assert "friendly and patient" in prompt
        assert "Do stuff" in prompt

    def test_build_system_prompt_backward_compat_agent_dir(self, tmp_path):
        """config['_agent_dir'] still works for backward compat."""
        (tmp_path / "IDENTITY.md").write_text("# Bot\nI am a bot.", encoding="utf-8")

        config = {
            "web": {"language": "en"},
            "agent": {"max_steps": 50},
            "_agent_dir": str(tmp_path),
        }
        prompt = build_system_prompt(config)

        assert "I am a bot" in prompt

    def test_build_system_prompt_max_steps(self):
        """max_steps value appears in constraints."""
        config = {"web": {"language": "en"}, "agent": {"max_steps": 42}}
        prompt = build_system_prompt(config)

        assert "42" in prompt
        assert "最多执行 42 步" in prompt

    def test_build_system_prompt_max_steps_zh(self):
        """max_steps value appears in Chinese constraints."""
        config = {"web": {"language": "zh"}, "agent": {"max_steps": 99}}
        prompt = build_system_prompt(config)

        assert "99" in prompt

    def test_agent_file_truncation(self, tmp_path):
        """Large agent files are truncated."""
        # Write a file larger than per-file limit
        (tmp_path / "AGENTS.md").write_text("x" * 25_000, encoding="utf-8")

        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = build_system_prompt(config, agent_dir=tmp_path)

        assert "truncated" in prompt
        # Should not have full 25K chars
        assert len(prompt) < 25_000

    def test_no_workspace_no_crash(self):
        """When no agent_dir is provided, prompt still works."""
        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = build_system_prompt(config)
        assert "最多执行" in prompt


# -------------------------------------------------------------------- #
# Tests for BrainResponse and ToolCallInfo
# -------------------------------------------------------------------- #


class TestBrainResponse:
    """Tests for the BrainResponse data class."""

    def test_brain_response_no_tool_calls(self):
        """BrainResponse with empty tool_calls list."""
        resp = BrainResponse(content="I see a desktop.", tool_calls=[])

        assert resp.content == "I see a desktop."
        assert resp.tool_calls == []
        assert resp.raw is None

    def test_brain_response_with_tool_calls(self):
        """BrainResponse with a populated list of ToolCallInfo objects."""
        tc1 = ToolCallInfo(id="tc_001", name="click", arguments={"x": 100, "y": 200})
        tc2 = ToolCallInfo(id="tc_002", name="type", arguments={"text": "hello"})

        resp = BrainResponse(
            content="I will click and type.",
            tool_calls=[tc1, tc2],
            raw={"some": "raw_data"},
        )

        assert resp.content == "I will click and type."
        assert len(resp.tool_calls) == 2
        assert resp.tool_calls[0].name == "click"
        assert resp.tool_calls[0].id == "tc_001"
        assert resp.tool_calls[0].arguments == {"x": 100, "y": 200}
        assert resp.tool_calls[1].name == "type"
        assert resp.tool_calls[1].arguments == {"text": "hello"}
        assert resp.raw == {"some": "raw_data"}

    def test_brain_response_default_content_none(self):
        """BrainResponse content can be None (model returned only tool calls)."""
        tc = ToolCallInfo(id="tc_003", name="finished", arguments={"summary": "Done"})
        resp = BrainResponse(content=None, tool_calls=[tc])

        assert resp.content is None
        assert len(resp.tool_calls) == 1
        assert resp.tool_calls[0].name == "finished"


class TestOpenAIBrainSummarize:
    """Tests for OpenAIBrain.summarize()."""

    @pytest.mark.asyncio
    async def test_summarize_method(self):
        """summarize() should call the client and return a string."""
        from see_agent.brain.openai_client import OpenAIBrain

        brain = OpenAIBrain(
            base_url="https://api.example.com/v1",
            api_key="test-key",
            model="test-model",
        )

        # Mock the OpenAI client
        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]
        mock_response.choices[0].message.content = "Summary of the conversation."
        brain._client = AsyncMock()
        brain._client.chat.completions.create = AsyncMock(return_value=mock_response)

        messages = [
            {"role": "user", "content": "Open Safari"},
            {"role": "assistant", "content": "I will open Safari."},
        ]
        result = await brain.summarize(messages)
        assert result == "Summary of the conversation."
        brain._client.chat.completions.create.assert_called_once()
