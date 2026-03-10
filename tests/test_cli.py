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
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.AGENTS_DIR", tmp_path / "agents"),
        patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
        patch("see_agent.config.RUN_DIR", tmp_path / "run"),
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
    for d in ["sessions", "logs", "skills", "agents", "teams", "run"]:
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
# Version command
# -------------------------------------------------------------------- #


class TestVersionCommand:
    """Tests for the version command."""

    def test_version(self):
        result = runner.invoke(app, ["version"])
        assert result.exit_code == 0
        assert "v3.1.0" in result.output


# -------------------------------------------------------------------- #
# Status command
# -------------------------------------------------------------------- #


class TestStatusCommand:
    """Tests for the status command."""

    def test_status_not_running(self):
        """status when launchd service is not running."""
        with patch("see_agent.cli.main._is_running", return_value=False):
            result = runner.invoke(app, ["status"])
        assert result.exit_code == 0
        assert "not running" in result.output

    def test_status_running(self):
        """status when launchd service is running."""
        with (
            patch("see_agent.cli.main._is_running", return_value=True),
            patch("subprocess.run") as mock_run,
        ):
            mock_run.return_value.stdout = "  pid = 12345\n  state = running\n"
            mock_run.return_value.returncode = 0
            result = runner.invoke(app, ["status"])
        assert result.exit_code == 0
        assert "running" in result.output


# -------------------------------------------------------------------- #
# Stop command
# -------------------------------------------------------------------- #


class TestStopCommand:
    """Tests for the stop command."""

    def test_stop_not_running(self):
        """stop when service is not running."""
        with patch("see_agent.cli.main._is_running", return_value=False):
            result = runner.invoke(app, ["stop"])
        assert result.exit_code == 0
        assert "not running" in result.output

    def test_stop_running(self, tmp_path):
        """stop sends bootout to launchd."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with (
                patch("see_agent.cli.main._is_running", return_value=True),
                patch("subprocess.run") as mock_run,
            ):
                mock_run.return_value.returncode = 0
                result = runner.invoke(app, ["stop"])
            assert result.exit_code == 0
            assert "stopped" in result.output
        finally:
            cleanup()


# -------------------------------------------------------------------- #
# Start command (foreground not tested — would block)
# -------------------------------------------------------------------- #


class TestStartCommand:
    """Tests for the start command."""

    def test_start_already_running(self, tmp_path):
        """start when already running shows message."""
        _setup_workspace(tmp_path)
        patches = _workspace_patches(tmp_path)
        cleanup = _apply_patches(patches)
        try:
            with (
                patch("see_agent.cli.main._is_running", return_value=True),
                patch("webbrowser.open"),
            ):
                result = runner.invoke(
                    app, ["start", "--no-browser"],
                )
            assert result.exit_code == 0
            assert "already running" in result.output
        finally:
            cleanup()
