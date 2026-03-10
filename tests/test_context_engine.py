"""Unit tests for context engine interface."""

from see_agent.agent.context_engine import BaseContextEngine, LegacyContextEngine


class TestLegacyContextEngine:
    """Tests for LegacyContextEngine."""

    def test_builds_prompt(self):
        engine = LegacyContextEngine()
        config = {"language": "en", "max_steps": 10}
        prompt = engine.build_prompt(config)
        assert "AI assistant" in prompt
        assert "10 steps" in prompt

    def test_memory_rule_in_prompt(self):
        engine = LegacyContextEngine()
        config = {"language": "en", "max_steps": 10}
        prompt = engine.build_prompt(config)
        assert "memory_search" in prompt
        assert "memory_write" in prompt

    def test_team_context_injected(self):
        engine = LegacyContextEngine()
        config = {"language": "en", "max_steps": 10}
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
