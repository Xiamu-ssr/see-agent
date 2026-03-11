"""API tests for owner communication endpoints."""

from __future__ import annotations

import json
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def workspace(tmp_path):
    teams_dir = tmp_path / "teams"
    teams_dir.mkdir()
    with (
        patch("see_agent.team.definition.TEAMS_DIR", teams_dir),
        patch("see_agent.config.TEAMS_DIR", teams_dir),
    ):
        yield teams_dir


@pytest.fixture()
def client(workspace):
    from see_agent.server.app import app

    app.state.config = {
        "llm": {"base_url": "http://test/v1", "api_key": "k", "model": "m"},
        "max_steps": 5,
    }
    from see_agent.server.supervisor import AgentSupervisor
    app.state.supervisor = AgentSupervisor({})
    from see_agent.server.message_router import MessageRouter
    app.state.message_router = MessageRouter(app.state.supervisor)
    with TestClient(app, raise_server_exceptions=True) as c:
        yield c


def _create_team(client: TestClient) -> str:
    resp = client.post(
        "/api/teams",
        json={"name": "T", "members": ["alice"], "leader": "alice"},
    )
    return resp.json()["id"]


class TestOwnerAPIs:

    def test_send_message(self, client, workspace):
        team_id = _create_team(client)
        resp = client.post(
            f"/api/teams/{team_id}/message",
            json={"to": "alice", "content": "hello"},
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "sent"
        # Verify persisted to messages.jsonl.
        log_path = workspace / team_id / "messages.jsonl"
        assert log_path.exists()
        entry = json.loads(log_path.read_text().strip())
        assert entry["sender"] == "owner"
        assert entry["content"] == "hello"

    def test_get_messages(self, client, workspace):
        team_id = _create_team(client)
        # Send two messages.
        client.post(
            f"/api/teams/{team_id}/message",
            json={"to": "alice", "content": "msg1"},
        )
        client.post(
            f"/api/teams/{team_id}/message",
            json={"to": "alice", "content": "msg2"},
        )
        resp = client.get(f"/api/teams/{team_id}/messages")
        assert resp.status_code == 200
        msgs = resp.json()
        assert len(msgs) == 2
        assert msgs[0]["content"] == "msg1"

    def test_unread_count(self, client, workspace):
        team_id = _create_team(client)
        # Write a message TO owner.
        td = workspace / team_id
        td.mkdir(parents=True, exist_ok=True)
        log_path = td / "messages.jsonl"
        log_path.write_text(
            json.dumps({
                "sender": "alice",
                "recipient": "owner",
                "content": "hi",
                "ts": "2025-01-01T00:00:00+00:00",
            })
            + "\n"
        )
        resp = client.get(f"/api/teams/{team_id}/unread")
        assert resp.status_code == 200
        assert resp.json()["unread"] == 1

    def test_mark_read(self, client, workspace):
        team_id = _create_team(client)
        # Write a message TO owner.
        td = workspace / team_id
        td.mkdir(parents=True, exist_ok=True)
        log_path = td / "messages.jsonl"
        log_path.write_text(
            json.dumps({
                "sender": "alice",
                "recipient": "owner",
                "content": "hi",
                "ts": "2025-01-01T00:00:00+00:00",
            })
            + "\n"
        )
        # Mark read.
        resp = client.post(f"/api/teams/{team_id}/mark_read")
        assert resp.status_code == 200
        assert "last_read_ts" in resp.json()
        # Now unread should be 0.
        resp = client.get(f"/api/teams/{team_id}/unread")
        assert resp.json()["unread"] == 0

    def test_send_message_team_not_found(self, client):
        resp = client.post(
            "/api/teams/nonexistent/message",
            json={"to": "alice", "content": "hi"},
        )
        assert resp.status_code == 404


class TestUpdateTeam:

    def test_update_name(self, client, workspace):
        team_id = _create_team(client)
        resp = client.put(
            f"/api/teams/{team_id}",
            json={"name": "Updated"},
        )
        assert resp.status_code == 200
        assert resp.json()["name"] == "Updated"

    def test_update_not_found(self, client):
        resp = client.put(
            "/api/teams/nonexistent",
            json={"name": "X"},
        )
        assert resp.status_code == 404
