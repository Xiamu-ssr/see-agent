"""Tests for src/agent/environment.py — desktop environment collection."""

from __future__ import annotations

from unittest.mock import patch

import pytest

from src.agent.environment import collect_environment


@pytest.mark.asyncio
async def test_collect_environment_full():
    """All three osascript/ls commands return data → block contains all sections."""

    async def fake_run(cmd: str) -> str:
        if "background only" in cmd:
            return "Finder, Safari, Terminal"
        if "frontmost" in cmd:
            return "Safari"
        if "ls /Applications" in cmd:
            return "Safari\nTerminal\nNotes"
        return ""

    with patch("src.agent.environment._run", side_effect=fake_run):
        result = await collect_environment(1920, 1080)

    assert "<ENVIRONMENT>" in result
    assert "</ENVIRONMENT>" in result
    assert "Finder, Safari, Terminal" in result
    assert "Safari" in result
    assert "Safari, Terminal, Notes" in result
    assert "1920×1080" in result


@pytest.mark.asyncio
async def test_collect_environment_partial():
    """When some commands fail (return ''), those sections are omitted."""

    async def fake_run(cmd: str) -> str:
        if "frontmost" in cmd:
            return "Finder"
        return ""

    with patch("src.agent.environment._run", side_effect=fake_run):
        result = await collect_environment(1440, 900)

    assert "<ENVIRONMENT>" in result
    assert "最前面的应用: Finder" in result
    # Running apps section should be absent since _run returned "".
    assert "当前运行的应用" not in result
    # Resolution is always present.
    assert "1440×900" in result


@pytest.mark.asyncio
async def test_collect_environment_all_fail():
    """When all commands fail, still returns a valid block with resolution."""

    async def fake_run(cmd: str) -> str:
        return ""

    with patch("src.agent.environment._run", side_effect=fake_run):
        result = await collect_environment(800, 600)

    assert "<ENVIRONMENT>" in result
    assert "</ENVIRONMENT>" in result
    assert "800×600" in result
    # No app lines.
    assert "当前运行的应用" not in result
    assert "最前面的应用" not in result
    assert "已安装的应用" not in result


@pytest.mark.asyncio
async def test_installed_apps_truncated():
    """Only the first 40 installed apps should be listed."""
    app_names = [f"App{i}" for i in range(60)]

    async def fake_run(cmd: str) -> str:
        if "ls /Applications" in cmd:
            return "\n".join(app_names)
        return ""

    with patch("src.agent.environment._run", side_effect=fake_run):
        result = await collect_environment(1920, 1080)

    # App39 should be present (index 39, the 40th app).
    assert "App39" in result
    # App40 should NOT be present (index 40, the 41st app).
    assert "App40" not in result
