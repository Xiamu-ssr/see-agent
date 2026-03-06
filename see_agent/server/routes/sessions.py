"""Sessions API routes — list, show, delete sessions and serve screenshots."""

from __future__ import annotations

import logging

from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse
from pydantic import BaseModel

from see_agent.session import SessionStore

logger = logging.getLogger(__name__)

router = APIRouter()


class SessionListResponse(BaseModel):
    sessions: list[dict]


class SessionDetailResponse(BaseModel):
    meta: dict
    message_count: int
    screenshot_count: int


class CleanResponse(BaseModel):
    deleted: int
    freed_bytes: int


@router.get("/api/sessions")
async def list_sessions(
    status: str | None = None,
    limit: int = 20,
) -> SessionListResponse:
    """List sessions, newest first."""
    summaries = SessionStore.list(status=status, limit=limit)
    return SessionListResponse(
        sessions=[
            {
                "id": s.id,
                "task": s.task,
                "status": s.status,
                "total_steps": s.total_steps,
                "elapsed_seconds": s.elapsed_seconds,
                "created_at": s.created_at,
                "updated_at": s.updated_at,
            }
            for s in summaries
        ]
    )


@router.get("/api/sessions/{session_id}")
async def get_session(session_id: str) -> SessionDetailResponse:
    """Get session details."""
    try:
        session = SessionStore.load(session_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Session not found")
    messages = session.read_messages()
    ss_dir = session.screenshots_dir
    screenshots = list(ss_dir.glob("*.webp")) if ss_dir.exists() else []
    return SessionDetailResponse(
        meta=session.meta,
        message_count=len(messages),
        screenshot_count=len(screenshots),
    )


@router.get("/api/sessions/{session_id}/screenshot/{step}")
async def get_screenshot(session_id: str, step: int) -> FileResponse:
    """Serve a session screenshot as a WebP file."""
    try:
        session = SessionStore.load(session_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Session not found")
    path = session.screenshot_path(step)
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"Screenshot step_{step:03d}.webp not found")
    return FileResponse(path, media_type="image/webp")


@router.delete("/api/sessions/{session_id}")
async def delete_session(session_id: str) -> dict:
    """Delete a session."""
    try:
        SessionStore.load(session_id)  # Verify it exists.
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Session not found")
    SessionStore.delete(session_id)
    return {"deleted": session_id}
