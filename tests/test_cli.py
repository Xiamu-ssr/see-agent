"""CLI command tests using Typer's CliRunner."""

from __future__ import annotations

import json
import os
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
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.AGENTS_DIR", tmp_path / "agents"),
        patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
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
    for d in ["sessions", "logs", "skills", "agents", "teams"]:
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
# Config commands
# -------------------------------------------------------------------- #


class TestConfigCommands:
    """Tests for config show command."""

    def test_config_show(self, tmp_path):
        """config show displays configuration."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            result = runner.invoke(app, ["config", "show"])
            assert result.exit_code == 0
            assert "llm" in result.output
        finally:
            cleanup()


# -------------------------------------------------------------------- #
# Setup commands
# -------------------------------------------------------------------- #


class TestSetupCommands:
    """Tests for setup check CLI command."""

    def test_setup_check_all_installed(self, tmp_path):
        """setup check with all deps installed shows 'installed'."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with (
                patch("see_agent.cli.main.importlib") as mock_importlib,
            ):
                mock_importlib.import_module.return_value = True
                result = runner.invoke(app, ["setup", "check"])
            assert "installed" in result.output
        finally:
            cleanup()

    def test_setup_check_missing(self, tmp_path):
        """setup check with missing mem0ai shows 'not installed'."""
        _setup_workspace(tmp_path, {
            "llm": {"api_key": "k"},
            "memory": {"enabled": True},
        })
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            def _import_side_effect(name):
                if name == "mem0":
                    raise ImportError("no mem0")
                return True

            with patch("see_agent.cli.main.importlib") as mock_importlib:
                mock_importlib.import_module.side_effect = _import_side_effect
                result = runner.invoke(app, ["setup", "check"])
            assert "not installed" in result.output
            assert "see-agent setup install --memory" in result.output
        finally:
            cleanup()


# -------------------------------------------------------------------- #
# Version command
# -------------------------------------------------------------------- #


class TestVersionCommand:
    """Tests for the version command."""

    def test_version(self):
        result = runner.invoke(app, ["version"])
        assert result.exit_code == 0
        assert "v3.0.0" in result.output


# -------------------------------------------------------------------- #
# Stop command
# -------------------------------------------------------------------- #


class TestStopCommand:
    """Tests for the stop command."""

    def test_stop_no_pid_file(self, tmp_path):
        """stop with no PID file exits with code 1."""
        pid_file = tmp_path / "server.pid"
        with patch("see_agent.cli.main._PID_FILE", str(pid_file)):
            result = runner.invoke(app, ["stop"])
        assert result.exit_code == 1
        assert "No running server" in result.output

    def test_stop_stale_pid(self, tmp_path):
        """stop with a stale PID (process not found) cleans up."""
        pid_file = tmp_path / "server.pid"
        pid_file.write_text("999999999")  # unlikely to be a real PID
        with patch("see_agent.cli.main._PID_FILE", str(pid_file)):
            result = runner.invoke(app, ["stop"])
        assert result.exit_code == 0
        assert "not found" in result.output
        assert not pid_file.exists()

    def test_stop_running_server(self, tmp_path):
        """stop sends SIGTERM to the running process."""
        pid_file = tmp_path / "server.pid"
        pid_file.write_text(str(os.getpid()))  # use our own PID for testing
        with (
            patch("see_agent.cli.main._PID_FILE", str(pid_file)),
            patch("os.kill") as mock_kill,
        ):
            result = runner.invoke(app, ["stop"])
        assert result.exit_code == 0
        assert "Stopped" in result.output
        mock_kill.assert_called_once()
