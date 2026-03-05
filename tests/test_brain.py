"""Unit tests for brain prompts and base data types."""



from src.brain.base import BrainResponse, ToolCallInfo
from src.brain.prompts import build_system_prompt

# -------------------------------------------------------------------- #
# Tests for build_system_prompt
# -------------------------------------------------------------------- #


class TestBuildSystemPrompt:
    """Tests for the build_system_prompt function."""

    def test_build_system_prompt_zh(self):
        """Chinese prompt contains RULES and CONSTRAINTS XML sections."""
        config = {"language": "zh", "max_steps": 30}
        prompt = build_system_prompt(config)

        assert "<RULES>" in prompt
        assert "</RULES>" in prompt
        assert "<CONSTRAINTS>" in prompt
        assert "</CONSTRAINTS>" in prompt
        # Should mention Chinese thinking
        assert "中文" in prompt

    def test_build_system_prompt_en(self):
        """English prompt contains RULES and CONSTRAINTS, English instructions."""
        config = {"language": "en", "max_steps": 50}
        prompt = build_system_prompt(config)

        assert "<RULES>" in prompt
        assert "</RULES>" in prompt
        assert "<CONSTRAINTS>" in prompt
        assert "</CONSTRAINTS>" in prompt
        assert "English" in prompt
        # Should not contain Chinese identity text
        assert "中文" not in prompt

    def test_build_system_prompt_with_soul(self, tmp_path):
        """When soul_path points to a valid file, the PERSONALITY section is included."""
        soul_file = tmp_path / "SOUL.md"
        soul_file.write_text("I am a friendly and patient assistant.", encoding="utf-8")

        config = {
            "language": "en",
            "max_steps": 50,
            "soul_path": str(soul_file),
        }
        prompt = build_system_prompt(config)

        assert "<PERSONALITY>" in prompt
        assert "friendly and patient" in prompt
        assert "</PERSONALITY>" in prompt

    def test_build_system_prompt_max_steps(self):
        """max_steps value appears in the CONSTRAINTS section."""
        config = {"language": "en", "max_steps": 42}
        prompt = build_system_prompt(config)

        assert "42" in prompt
        assert "Maximum 42 steps" in prompt

    def test_build_system_prompt_max_steps_zh(self):
        """max_steps value appears in the Chinese CONSTRAINTS section."""
        config = {"language": "zh", "max_steps": 99}
        prompt = build_system_prompt(config)

        assert "99" in prompt


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
