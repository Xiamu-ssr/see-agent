"""Tests for JSON schema files."""

from __future__ import annotations

import json
from pathlib import Path

SCHEMA_DIR = Path(__file__).resolve().parent.parent / "see_agent" / "schemas"


class TestSchemas:

    def test_config_schema(self):
        path = SCHEMA_DIR / "config.schema.json"
        assert path.exists()
        data = json.loads(path.read_text())
        assert data["title"] == "config"

    def test_agent_schema(self):
        path = SCHEMA_DIR / "agent.schema.json"
        assert path.exists()
        data = json.loads(path.read_text())
        assert data["title"] == "agent"

    def test_team_schema(self):
        path = SCHEMA_DIR / "team.schema.json"
        assert path.exists()
        data = json.loads(path.read_text())
        assert data["title"] == "team"
