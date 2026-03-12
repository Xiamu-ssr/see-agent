"""Unit tests for context engine interface."""

from see_agent.agent.context_engine import BaseContextEngine, LegacyContextEngine


class TestLegacyContextEngine:
    """Tests for LegacyContextEngine."""

    def test_builds_prompt(self):
        engine = LegacyContextEngine()
        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = engine.build_prompt(config)
        assert "最多执行" in prompt
        assert "10" in prompt

    def test_agent_file_injection(self, tmp_path):
        """Agent files are injected via agent_dir parameter."""
        (tmp_path / "AGENTS.md").write_text("Use memory_search to find info.", encoding="utf-8")
        engine = LegacyContextEngine()
        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = engine.build_prompt(config, agent_dir=tmp_path)
        assert "memory_search" in prompt

    def test_team_context_injected(self):
        engine = LegacyContextEngine()
        config = {"web": {"language": "en"}, "agent": {"max_steps": 10}}
        prompt = engine.build_prompt(config, team_context="Team info here")
        assert "<TEAM_CONTEXT>" in prompt
        assert "Team info here" in prompt

    def test_owns_compaction_false(self):
        engine = LegacyContextEngine()
        assert engine.owns_compaction is False

    def test_is_abstract(self):
        """BaseContextEngine cannot be instantiated directly."""
        import pytest

        with pytest.raises(TypeError):
            BaseContextEngine()  # type: ignore[abstract]
