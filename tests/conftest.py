"""Global test fixtures — workspace isolation."""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import patch

import pytest


@pytest.fixture(autouse=True)
def isolate_workspace(tmp_path: Path) -> None:  # noqa: PT004
    """Redirect all WORKSPACE_DIR-derived paths to a temp directory.

    Prevents tests from touching the real ``~/.see-agent/`` directory.
    """
    ws = tmp_path / ".see-agent"
    ws.mkdir()

    # Minimal config so load_config() doesn't crash.
    (ws / "config.json").write_text(
        json.dumps({"llm": {"api_key": "test", "model": "test"}}),
    )

    patches = {
        "see_agent.config.WORKSPACE_DIR": ws,
        "see_agent.config.CONFIG_PATH": ws / "config.json",
        "see_agent.config.LOGS_DIR": ws / "logs",
        "see_agent.config.SKILLS_DIR": ws / "skills",
        "see_agent.config.AGENTS_DIR": ws / "agents",
        "see_agent.config.TEAMS_DIR": ws / "teams",
        "see_agent.config.RUN_DIR": ws / "run",
    }

    with patch.dict("os.environ", {"SEE_AGENT_HOME": str(ws)}):
        stack = [patch(target, value) for target, value in patches.items()]
        for p in stack:
            p.start()
        yield  # type: ignore[misc]
        for p in reversed(stack):
            p.stop()
