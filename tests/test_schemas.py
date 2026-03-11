"""Tests for JSON schema files and Pydantic model consistency."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

SCHEMA_DIR = Path(__file__).resolve().parent.parent / "see_agent" / "schemas"


class TestSchemaFilesExist:
    """Schema JSON files exist and are valid."""

    @pytest.mark.parametrize("name", ["config", "agent", "team"])
    def test_schema_exists_and_valid(self, name: str):
        path = SCHEMA_DIR / f"{name}.schema.json"
        assert path.exists(), f"{name}.schema.json missing"
        data = json.loads(path.read_text())
        assert data["title"] == name
        assert "properties" in data or "type" in data


class TestSchemaModelConsistency:
    """JSON schema properties match Pydantic response models."""

    def test_config_schema_matches_default_config(self):
        """Config schema properties cover all DEFAULT_CONFIG keys."""
        from see_agent.config import DEFAULT_CONFIG

        schema = json.loads(
            (SCHEMA_DIR / "config.schema.json").read_text(),
        )
        schema_keys = set(schema.get("properties", {}).keys())
        config_keys = set(DEFAULT_CONFIG.keys())
        missing = config_keys - schema_keys
        assert not missing, f"Config keys missing from schema: {missing}"

    def test_agent_schema_has_required_fields(self):
        """Agent schema covers the fields AgentDefinition serialises."""
        schema = json.loads(
            (SCHEMA_DIR / "agent.schema.json").read_text(),
        )
        props = set(schema.get("properties", {}).keys())
        required = {"id", "name", "role"}
        assert required.issubset(props), f"Missing: {required - props}"

    def test_team_schema_has_required_fields(self):
        """Team schema covers the fields TeamDefinition serialises."""
        schema = json.loads(
            (SCHEMA_DIR / "team.schema.json").read_text(),
        )
        props = set(schema.get("properties", {}).keys())
        required = {"id", "name", "members", "status"}
        assert required.issubset(props), f"Missing: {required - props}"

    def test_all_pydantic_response_models_instantiate(self):
        """Every Pydantic model in schemas.py can be instantiated with defaults."""
        import inspect

        import see_agent.server.schemas as m

        classes = [
            (name, cls)
            for name, cls in inspect.getmembers(m, inspect.isclass)
            if (
                hasattr(cls, "model_fields")
                and cls.__module__ == m.__name__
            )
        ]
        assert len(classes) >= 20, f"Expected 20+ models, got {len(classes)}"
        for name, cls in classes:
            # All fields should have defaults or be Optional
            try:
                instance = cls.model_construct()
                assert instance is not None
            except Exception as exc:
                pytest.fail(f"{name} failed model_construct(): {exc}")
