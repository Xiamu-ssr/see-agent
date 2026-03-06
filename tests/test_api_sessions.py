"""API route tests for sessions endpoints."""

from __future__ import annotations

from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient

from see_agent.session import SessionStore


@pytest.fixture()
def sessions_dir(tmp_path):
    """Create a temporary sessions directory and patch it."""
    d = tmp_path / "sessions"
    d.mkdir()
    with patch("see_agent.session.store.SESSIONS_DIR", d):
        yield d


@pytest.fixture()
def client(sessions_dir):
    """Create a FastAPI TestClient with mocked config."""
    from see_agent.server.app import app

    app.state.config = {
        "llm": {"base_url": "http://test/v1", "api_key": "k", "model": "m"},
        "max_steps": 5,
    }
    app.state.tasks = {}
    app.state.ws_subscribers = {}
    with TestClient(app, raise_server_exceptions=False) as c:
        yield c


class TestSessionsAPI:
    """Tests for /api/sessions endpoints."""

    def test_list_sessions_empty(self, client):
        """GET /api/sessions with no sessions returns empty list."""
        resp = client.get("/api/sessions")
        assert resp.status_code == 200
        assert resp.json()["sessions"] == []

    def test_list_sessions(self, client, sessions_dir):
        """GET /api/sessions returns created sessions."""
        s = SessionStore.create("task1", {"llm": {"model": "m"}})
        resp = client.get("/api/sessions")
        assert resp.status_code == 200
        ids = [item["id"] for item in resp.json()["sessions"]]
        assert s.id in ids

    def test_list_sessions_status_filter(self, client, sessions_dir):
        """GET /api/sessions?status=completed filters correctly."""
        s = SessionStore.create("task1", {"llm": {"model": "m"}})
        s.update_meta(status="completed")

        s2 = SessionStore.create("task2", {"llm": {"model": "m"}})
        s2.update_meta(status="running")

        resp = client.get("/api/sessions?status=completed")
        assert resp.status_code == 200
        sessions = resp.json()["sessions"]
        assert all(item["status"] == "completed" for item in sessions)

    def test_get_session_detail(self, client, sessions_dir):
        """GET /api/sessions/<id> returns session detail."""
        s = SessionStore.create("task1", {"llm": {"model": "m"}})
        resp = client.get(f"/api/sessions/{s.id}")
        assert resp.status_code == 200
        data = resp.json()
        assert "meta" in data
        assert data["message_count"] == 0

    def test_get_session_404(self, client):
        """GET /api/sessions/<bad_id> returns 404."""
        resp = client.get("/api/sessions/nonexistent")
        assert resp.status_code == 404

    def test_get_screenshot_404_no_session(self, client):
        """GET /api/sessions/<bad_id>/screenshot/0 returns 404."""
        resp = client.get("/api/sessions/nonexistent/screenshot/0")
        assert resp.status_code == 404

    def test_get_screenshot_404_no_step(self, client, sessions_dir):
        """GET /api/sessions/<id>/screenshot/999 returns 404."""
        s = SessionStore.create("task1", {"llm": {"model": "m"}})
        resp = client.get(f"/api/sessions/{s.id}/screenshot/999")
        assert resp.status_code == 404

    def test_get_screenshot_success(self, client, sessions_dir):
        """GET /api/sessions/<id>/screenshot/<step> returns webp file."""
        s = SessionStore.create("task1", {"llm": {"model": "m"}})
        ss_dir = s.screenshots_dir
        ss_dir.mkdir(parents=True, exist_ok=True)
        (ss_dir / "step_000.webp").write_bytes(b"RIFF....WEBP")
        resp = client.get(f"/api/sessions/{s.id}/screenshot/0")
        assert resp.status_code == 200

    def test_delete_session(self, client, sessions_dir):
        """DELETE /api/sessions/<id> deletes the session."""
        s = SessionStore.create("task1", {"llm": {"model": "m"}})
        resp = client.delete(f"/api/sessions/{s.id}")
        assert resp.status_code == 200
        assert resp.json()["deleted"] == s.id

    def test_delete_session_404(self, client):
        """DELETE /api/sessions/<bad_id> returns 404."""
        resp = client.delete("/api/sessions/nonexistent")
        assert resp.status_code == 404
