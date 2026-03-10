"""API tests for logs endpoint."""

from __future__ import annotations

from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def workspace(tmp_path):
    logs_dir = tmp_path / "logs"
    logs_dir.mkdir()
    with (
        patch("see_agent.config.CONFIG_PATH", tmp_path / "config.json"),
        patch("see_agent.config.WORKSPACE_DIR", tmp_path),
        patch("see_agent.config.LOGS_DIR", logs_dir),
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.AGENTS_DIR", tmp_path / "agents"),
        patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
    ):
        for d in ("sessions", "skills", "agents", "teams"):
            (tmp_path / d).mkdir(exist_ok=True)
        yield tmp_path


@pytest.fixture()
def client(workspace):
    import json

    from see_agent.server.app import app

    config = {
        "llm": {"base_url": "http://test/v1", "api_key": "k", "model": "m"},
        "max_steps": 5,
    }
    (workspace / "config.json").write_text(json.dumps(config))
    with TestClient(app, raise_server_exceptions=True) as c:
        yield c


class TestLogsAPI:

    def test_no_log_file(self, client):
        resp = client.get("/api/logs?date=2000-01-01")
        assert resp.status_code == 200
        assert resp.json() == []

    def test_parse_entries(self, client, workspace):
        log_content = (
            "10:30:00  INFO      see_agent.server  Server started\n"
            "10:30:01  DEBUG     see_agent.brain  Thinking...\n"
            "10:30:02  ERROR     see_agent.hand  Failed to click\n"
        )
        logs_dir = workspace / "logs"
        (logs_dir / "2025-06-15.log").write_text(log_content)

        resp = client.get("/api/logs?date=2025-06-15")
        assert resp.status_code == 200
        entries = resp.json()
        assert len(entries) == 3
        assert entries[0]["level"] == "INFO"
        assert entries[2]["level"] == "ERROR"

    def test_level_filter(self, client, workspace):
        log_content = (
            "10:30:00  INFO      see_agent.server  msg1\n"
            "10:30:01  DEBUG     see_agent.brain  msg2\n"
            "10:30:02  ERROR     see_agent.hand  msg3\n"
        )
        logs_dir = workspace / "logs"
        (logs_dir / "2025-06-15.log").write_text(log_content)

        resp = client.get("/api/logs?date=2025-06-15&level=ERROR")
        entries = resp.json()
        assert len(entries) == 1
        assert entries[0]["level"] == "ERROR"
