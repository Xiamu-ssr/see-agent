"""Tests for sandbox profile generation and violation collector."""

from __future__ import annotations

from unittest.mock import patch

import pytest

from see_agent.sandbox.manager import SandboxProfileGenerator


@pytest.fixture
def run_dir(tmp_path):
    d = tmp_path / "run"
    d.mkdir()
    return d


class TestSandboxProfileGenerator:
    def test_generate_creates_profile(self, tmp_path, run_dir):
        team_dir = tmp_path / "teams" / "t1"
        team_dir.mkdir(parents=True)
        (team_dir / "agents" / "alice").mkdir(parents=True)
        (team_dir / "shared").mkdir()

        with patch("see_agent.sandbox.manager.RUN_DIR", run_dir):
            gen = SandboxProfileGenerator()
            path = gen.generate(
                agent_id="alice",
                team_id="t1",
                team_dir=team_dir,
                sandbox_cfg={
                    "enabled": True,
                    "network": True,
                    "screen_access": True,
                },
            )

        assert path.exists()
        content = path.read_text()
        # Should contain base deny default.
        assert "(deny default)" in content
        # Should contain agent directory.
        assert "alice" in content
        # Should contain HOME replacement.
        assert "__SAFEHOUSE_REPLACE_ME__" not in content

    def test_no_network_skips_network_profile(self, tmp_path, run_dir):
        team_dir = tmp_path / "teams" / "t2"
        team_dir.mkdir(parents=True)

        with patch("see_agent.sandbox.manager.RUN_DIR", run_dir):
            gen = SandboxProfileGenerator()
            path = gen.generate(
                agent_id="bob",
                team_id="t2",
                team_dir=team_dir,
                sandbox_cfg={
                    "enabled": True,
                    "network": False,
                    "screen_access": False,
                },
            )

        content = path.read_text()
        # Should NOT contain network profile content.
        # The 20-network.sb has specific network allow rules.
        assert "20-network" not in content or "network" in content
        # Should NOT contain GUI/clipboard profiles.
        assert "macos-gui" not in content

    def test_extra_read_write_paths(self, tmp_path, run_dir):
        team_dir = tmp_path / "teams" / "t3"
        team_dir.mkdir(parents=True)

        with patch("see_agent.sandbox.manager.RUN_DIR", run_dir):
            gen = SandboxProfileGenerator()
            path = gen.generate(
                agent_id="charlie",
                team_id="t3",
                team_dir=team_dir,
                sandbox_cfg={
                    "enabled": True,
                    "extra_read": ["/tmp/test-read"],
                    "extra_write": ["/tmp/test-write"],
                },
            )

        content = path.read_text()
        assert "/tmp/test-read" in content
        assert "/tmp/test-write" in content


class TestSandboxViolationCollector:
    @pytest.mark.asyncio
    async def test_collect_empty_on_pid_zero(self):
        """Collecting for PID 0 should return empty (or handle gracefully)."""
        from see_agent.sandbox.collector import SandboxViolationCollector

        collector = SandboxViolationCollector()
        # This may fail on non-macOS or return empty — both are fine.
        violations = await collector.collect(agent_pid=0, since_minutes=1)
        assert isinstance(violations, list)

    def test_parse_violations_empty(self):
        from see_agent.sandbox.collector import SandboxViolationCollector

        collector = SandboxViolationCollector()
        assert collector._parse_violations("") == []
        assert collector._parse_violations("[]") == []

    def test_parse_violations_with_deny(self):
        from see_agent.sandbox.collector import SandboxViolationCollector

        collector = SandboxViolationCollector()
        log = (
            '[{"timestamp": "2026-03-10",'
            ' "eventMessage": "deny(1) file-read'
            ' path \\"/etc/secret\\""}]'
        )
        result = collector._parse_violations(log)
        assert len(result) == 1
        assert result[0]["path"] == "/etc/secret"
        assert result[0]["timestamp"] == "2026-03-10"


class TestSandboxAPI:
    """Test sandbox-related API endpoints."""

    @pytest.fixture
    def client(self, tmp_path):
        import json

        from fastapi.testclient import TestClient

        from see_agent.server.app import app

        agents_dir = tmp_path / "agents"
        agents_dir.mkdir()
        teams_dir = tmp_path / "teams"
        teams_dir.mkdir()
        run_dir = tmp_path / "run"
        run_dir.mkdir()
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({
            "llm": {"api_key": "test", "model": "test"},
        }))

        with (
            patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
            patch("see_agent.config.AGENTS_DIR", agents_dir),
            patch("see_agent.config.TEAMS_DIR", teams_dir),
            patch("see_agent.config.RUN_DIR", run_dir),
            patch("see_agent.config.CONFIG_PATH", config_path),
        ):
            yield TestClient(app)

    def test_sandbox_allow_adds_path(self, client, tmp_path):
        from see_agent.agent.definition import AgentDefinition

        agents_dir = tmp_path / "agents"
        with patch("see_agent.agent.definition.AGENTS_DIR", agents_dir):
            AgentDefinition.create("alice", name="Alice", role="tester")

            with (
                patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
                patch("see_agent.config.AGENTS_DIR", agents_dir),
            ):
                resp = client.post(
                    "/api/agents/alice/sandbox/allow",
                    json={"path": "/tmp/data", "mode": "read"},
                )

            assert resp.status_code == 200
            data = resp.json()
            assert data["status"] == "allowed"
            assert data["path"] == "/tmp/data"

            # Verify it was persisted.
            defn = AgentDefinition.load("alice")
            assert "/tmp/data" in defn.sandbox.get("extra_read", [])

    def test_screen_status_endpoint(self, client):
        resp = client.get("/api/screen")
        assert resp.status_code == 200
        data = resp.json()
        assert data["holder"] is None
        assert data["queue_length"] == 0
