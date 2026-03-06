"""CLI command tests using Typer's CliRunner."""

from __future__ import annotations

import json
from unittest.mock import patch

from typer.testing import CliRunner

from see_agent.cli.main import app

runner = CliRunner()


# -------------------------------------------------------------------- #
# Helpers — patch all workspace dirs to tmp_path
# -------------------------------------------------------------------- #

def _workspace_patches(tmp_path):
    """Return a list of context managers that redirect workspace to tmp_path."""
    return [
        patch("see_agent.config.WORKSPACE_DIR", tmp_path),
        patch("see_agent.config.CONFIG_PATH", tmp_path / "config.json"),
        patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
        patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
        patch("see_agent.config.PROFILES_DIR", tmp_path / "profiles"),
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.MEMORY_DIR", tmp_path / "memory"),
    ]


def _apply_patches(patches):
    """Enter all patches and return a cleanup function."""
    for p in patches:
        p.start()

    def cleanup():
        for p in patches:
            p.stop()

    return cleanup


def _setup_workspace(tmp_path, config_data=None):
    """Create a minimal workspace with config.json."""
    for d in ["sessions", "logs", "profiles", "skills", "memory"]:
        (tmp_path / d).mkdir(parents=True, exist_ok=True)
    if config_data is None:
        config_data = {
            "llm": {
                "base_url": "https://test.example.com/v1",
                "api_key": "test-key",
                "model": "test-model",
            }
        }
    (tmp_path / "config.json").write_text(json.dumps(config_data))


# -------------------------------------------------------------------- #
# MCP commands
# -------------------------------------------------------------------- #


class TestMCPCommands:
    """Tests for mcp list/add/remove CLI commands."""

    def test_mcp_list_empty(self, tmp_path):
        """mcp list with no servers shows message."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["mcp", "list"])
            assert "No MCP servers" in result.output
        finally:
            cleanup()

    def test_mcp_list_shows_servers(self, tmp_path):
        """mcp list with configured servers shows them."""
        _setup_workspace(tmp_path, {
            "llm": {"api_key": "k"},
            "mcp_servers": {
                "tavily": {"command": "npx", "args": ["-y", "tavily-server"]},
            },
        })
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["mcp", "list"])
            assert "tavily" in result.output
        finally:
            cleanup()

    def test_mcp_add_stdio(self, tmp_path):
        """mcp add creates a stdio server entry in config."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(
                app, ["mcp", "add", "test-srv", "node", "--arg", "server.js"]
            )
            assert result.exit_code == 0
            assert "Added" in result.output
            # Verify written to config.json
            saved = json.loads((tmp_path / "config.json").read_text())
            assert "test-srv" in saved["mcp_servers"]
            assert saved["mcp_servers"]["test-srv"]["command"] == "node"
        finally:
            cleanup()

    def test_mcp_add_http(self, tmp_path):
        """mcp add --type http --url creates an HTTP server entry."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(
                app, [
                    "mcp", "add", "remote",
                    "--type", "http",
                    "--url", "https://mcp.example.com",
                ]
            )
            assert result.exit_code == 0
            saved = json.loads((tmp_path / "config.json").read_text())
            assert saved["mcp_servers"]["remote"]["type"] == "http"
            assert saved["mcp_servers"]["remote"]["url"] == "https://mcp.example.com"
        finally:
            cleanup()

    def test_mcp_add_http_requires_url(self, tmp_path):
        """mcp add --type http without --url should fail."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["mcp", "add", "bad", "--type", "http"])
            assert result.exit_code == 1
        finally:
            cleanup()

    def test_mcp_remove(self, tmp_path):
        """mcp remove deletes a server from config."""
        _setup_workspace(tmp_path, {
            "llm": {"api_key": "k"},
            "mcp_servers": {"to-remove": {"command": "echo"}},
        })
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["mcp", "remove", "to-remove"])
            assert result.exit_code == 0
            assert "Removed" in result.output
            saved = json.loads((tmp_path / "config.json").read_text())
            assert "to-remove" not in saved.get("mcp_servers", {})
        finally:
            cleanup()

    def test_mcp_remove_nonexistent(self, tmp_path):
        """mcp remove of unknown server exits with code 1."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["mcp", "remove", "ghost"])
            assert result.exit_code == 1
        finally:
            cleanup()


# -------------------------------------------------------------------- #
# Sessions commands
# -------------------------------------------------------------------- #


class TestSessionsCommands:
    """Tests for sessions list/show/clean CLI commands."""

    def test_sessions_list_empty(self, tmp_path):
        """sessions list with no sessions shows message."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with patch("see_agent.session.store.SESSIONS_DIR", tmp_path / "sessions"):
                result = runner.invoke(app, ["sessions", "list"])
            assert "No sessions found" in result.output
        finally:
            cleanup()

    def test_sessions_show_nonexistent(self, tmp_path):
        """sessions show of unknown session exits with code 1."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with patch("see_agent.session.store.SESSIONS_DIR", tmp_path / "sessions"):
                result = runner.invoke(app, ["sessions", "show", "nonexistent-id"])
            assert result.exit_code == 1
        finally:
            cleanup()

    def test_sessions_list_shows_sessions(self, tmp_path):
        """sessions list displays created sessions."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with patch("see_agent.session.store.SESSIONS_DIR", tmp_path / "sessions"):
                from see_agent.session import SessionStore

                s = SessionStore.create("test task", {"llm": {"model": "m"}})
                result = runner.invoke(app, ["sessions", "list"])
            assert s.id in result.output
        finally:
            cleanup()


# -------------------------------------------------------------------- #
# Config show with profile
# -------------------------------------------------------------------- #


class TestConfigCommands:
    """Tests for config show command."""

    def test_config_show_with_profile(self, tmp_path):
        """config show --profile loads profile overlay."""
        _setup_workspace(tmp_path)
        (tmp_path / "profiles" / "opus.json").write_text(
            json.dumps({"llm": {"model": "claude-opus"}})
        )
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["config", "show", "--profile", "opus"])
            assert "claude-opus" in result.output
        finally:
            cleanup()

    def test_config_show_nonexistent_profile(self, tmp_path):
        """config show --profile ghost exits with error."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["config", "show", "--profile", "ghost"])
            assert result.exit_code != 0
        finally:
            cleanup()


# -------------------------------------------------------------------- #
# Resume command
# -------------------------------------------------------------------- #


class TestResumeCommand:
    """Tests for the resume command."""

    def test_resume_nonexistent_session(self, tmp_path):
        """resume of nonexistent session exits with code 1."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with patch("see_agent.session.store.SESSIONS_DIR", tmp_path / "sessions"):
                result = runner.invoke(app, ["resume", "nonexistent-id"])
            assert result.exit_code == 1
        finally:
            cleanup()

    def test_resume_last_no_sessions(self, tmp_path):
        """resume --last with no sessions exits with code 1."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with patch("see_agent.session.store.SESSIONS_DIR", tmp_path / "sessions"):
                result = runner.invoke(app, ["resume", "--last"])
            assert result.exit_code == 1
        finally:
            cleanup()
