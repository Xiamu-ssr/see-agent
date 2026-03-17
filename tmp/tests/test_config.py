"""Unit tests for configuration loading."""

import json
from unittest.mock import patch

import pytest

from see_agent.config import _deep_merge, load_agent_config, load_config


def _config_patches(tmp_path):
    """Return a list of patches redirecting workspace dirs to *tmp_path*."""
    return [
        patch("see_agent.config.CONFIG_PATH", tmp_path / "config.json"),
        patch("see_agent.config.WORKSPACE_DIR", tmp_path),
        patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.AGENTS_DIR", tmp_path / "agents"),
        patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
    ]


def _apply_patches(patches):
    """Start all patches and return a cleanup function."""
    for p in patches:
        p.start()

    def cleanup():
        for p in patches:
            p.stop()

    return cleanup


class TestDeepMerge:
    """Tests for the _deep_merge helper."""

    def test_flat_merge(self):
        base = {"a": 1, "b": 2}
        overlay = {"b": 3, "c": 4}
        result = _deep_merge(base, overlay)
        assert result == {"a": 1, "b": 3, "c": 4}

    def test_nested_merge(self):
        base = {"llm": {"model": "gpt-4o", "api_key": "old"}, "x": 1}
        overlay = {"llm": {"model": "claude-3"}}
        result = _deep_merge(base, overlay)
        assert result["llm"]["model"] == "claude-3"
        assert result["llm"]["api_key"] == "old"  # preserved
        assert result["x"] == 1

    def test_overlay_replaces_non_dict(self):
        base = {"a": {"nested": True}}
        overlay = {"a": "flat_now"}
        result = _deep_merge(base, overlay)
        assert result["a"] == "flat_now"

    def test_base_not_mutated(self):
        base = {"a": 1}
        overlay = {"a": 2}
        _deep_merge(base, overlay)
        assert base["a"] == 1


class TestLoadConfig:
    """Tests for load_config."""

    def test_env_overrides_config(self, tmp_path):
        """Environment variables take precedence over config.json."""
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({
            "llm": {"api_key": "k", "model": "base"},
        }))

        patches = _config_patches(tmp_path)
        patches.append(patch.dict("os.environ", {"SEE_AGENT_MODEL": "env-model"}))
        cleanup = _apply_patches(patches)
        try:
            config = load_config()
        finally:
            cleanup()

        assert config["llm"]["model"] == "env-model"

    def test_defaults_applied(self, tmp_path):
        """Missing keys are filled from DEFAULT_CONFIG."""
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({"llm": {"api_key": "k"}}))

        cleanup = _apply_patches(_config_patches(tmp_path))
        try:
            config = load_config()
        finally:
            cleanup()

        assert "agent" in config
        assert config["agent"]["max_steps"] == 50


class TestWorkspaceDirs:
    """Tests for AGENTS_DIR and TEAMS_DIR."""

    def test_agents_teams_dirs_created(self, tmp_path):
        """ensure_workspace creates agents/ and teams/ directories."""
        from see_agent.config import ensure_workspace

        cleanup = _apply_patches(_config_patches(tmp_path))
        try:
            ensure_workspace()
        finally:
            cleanup()

        assert (tmp_path / "agents").is_dir()
        assert (tmp_path / "teams").is_dir()


class TestLoadAgentConfig:
    """Tests for load_agent_config."""

    def test_agent_config_inheritance(self, tmp_path):
        """Agent-level overrides merge on top of global config."""
        agents_dir = tmp_path / "agents"
        agent_dir = agents_dir / "test-agent"
        agent_dir.mkdir(parents=True)
        (agent_dir / "agent.json").write_text(json.dumps({
            "id": "test-agent",
            "agent": {"max_steps": 99},
        }))

        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({
            "llm": {"api_key": "k", "model": "base"},
        }))

        patches = _config_patches(tmp_path)
        patches.append(patch("see_agent.config.AGENTS_DIR", agents_dir))
        cleanup = _apply_patches(patches)
        try:
            config = load_agent_config("test-agent")
        finally:
            cleanup()

        assert config["agent"]["max_steps"] == 99
        assert config["llm"]["model"] == "base"

    def test_agent_not_found(self, tmp_path):
        agents_dir = tmp_path / "agents"
        agents_dir.mkdir()

        with patch("see_agent.config.AGENTS_DIR", agents_dir):
            with pytest.raises(FileNotFoundError):
                load_agent_config("nonexistent")


class TestSkillsDirs:
    """Verify skills_dirs default."""

    def test_no_openclaw_in_skills_dirs(self):
        from see_agent.config import DEFAULT_CONFIG

        dirs = DEFAULT_CONFIG["skills"]["dirs"]
        for d in dirs:
            assert "openclaw" not in d
