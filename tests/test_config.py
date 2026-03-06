"""Unit tests for configuration loading and profile system."""

import json
from unittest.mock import patch

import pytest

from see_agent.config import _deep_merge, load_config


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


class TestLoadConfigWithProfile:
    """Tests for profile-based configuration loading."""

    def test_load_config_with_profile(self, tmp_path):
        """Profile overlay merges on top of base config."""
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({
            "llm": {"base_url": "https://base.example.com/v1", "api_key": "k", "model": "base"},
            "language": "en",
        }))

        profiles_dir = tmp_path / "profiles"
        profiles_dir.mkdir()
        profile_path = profiles_dir / "opus.json"
        profile_path.write_text(json.dumps({
            "llm": {"model": "claude-opus"},
            "language": "zh",
        }))

        with (
            patch("see_agent.config.CONFIG_PATH", config_path),
            patch("see_agent.config.PROFILES_DIR", profiles_dir),
            patch("see_agent.config.WORKSPACE_DIR", tmp_path),
            patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
            patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
            patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
            patch("see_agent.config.MEMORY_DIR", tmp_path / "memory"),
        ):
            config = load_config(profile="opus")

        assert config["llm"]["model"] == "claude-opus"
        assert config["llm"]["base_url"] == "https://base.example.com/v1"
        assert config["language"] == "zh"

    def test_profile_not_found(self, tmp_path):
        """Missing profile raises FileNotFoundError."""
        profiles_dir = tmp_path / "profiles"
        profiles_dir.mkdir()

        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({"llm": {"api_key": "k"}}))

        with (
            patch("see_agent.config.CONFIG_PATH", config_path),
            patch("see_agent.config.PROFILES_DIR", profiles_dir),
            patch("see_agent.config.WORKSPACE_DIR", tmp_path),
            patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
            patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
            patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
            patch("see_agent.config.MEMORY_DIR", tmp_path / "memory"),
        ):
            with pytest.raises(FileNotFoundError, match="Profile not found"):
                load_config(profile="nonexistent")

    def test_profile_from_config_default(self, tmp_path):
        """When profile=None but config has 'profile' key, that is used."""
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({
            "llm": {"api_key": "k", "model": "base"},
            "profile": "fast",
        }))

        profiles_dir = tmp_path / "profiles"
        profiles_dir.mkdir()
        (profiles_dir / "fast.json").write_text(json.dumps({
            "llm": {"model": "fast-model"},
        }))

        with (
            patch("see_agent.config.CONFIG_PATH", config_path),
            patch("see_agent.config.PROFILES_DIR", profiles_dir),
            patch("see_agent.config.WORKSPACE_DIR", tmp_path),
            patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
            patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
            patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
            patch("see_agent.config.MEMORY_DIR", tmp_path / "memory"),
        ):
            config = load_config()

        assert config["llm"]["model"] == "fast-model"

    def test_env_overrides_profile(self, tmp_path):
        """Environment variables take precedence over profile values."""
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({"llm": {"api_key": "k", "model": "base"}}))

        profiles_dir = tmp_path / "profiles"
        profiles_dir.mkdir()
        (profiles_dir / "p.json").write_text(json.dumps({
            "llm": {"model": "profile-model"},
        }))

        with (
            patch("see_agent.config.CONFIG_PATH", config_path),
            patch("see_agent.config.PROFILES_DIR", profiles_dir),
            patch("see_agent.config.WORKSPACE_DIR", tmp_path),
            patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
            patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
            patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
            patch("see_agent.config.MEMORY_DIR", tmp_path / "memory"),
            patch.dict("os.environ", {"SEE_AGENT_MODEL": "env-model"}),
        ):
            config = load_config(profile="p")

        assert config["llm"]["model"] == "env-model"
