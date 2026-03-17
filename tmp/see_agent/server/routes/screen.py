"""Screen lease status API route."""

from __future__ import annotations

from fastapi import APIRouter

from see_agent.server.schemas import ScreenLeaseStatus

router = APIRouter(prefix="/api/screen", tags=["screen"])


@router.get("")
async def get_screen_status() -> ScreenLeaseStatus:
    """Get current screen lease status.

    Returns holder info and queue length.  The ScreenManager lives inside
    an AgentRouter which is owned by a running TeamManager, so we return
    a basic status summary here.
    """
    # The ScreenManager is per-router and only exists while a team is
    # running.  For now return a static "free" status; the real status
    # would be fetched from the active router in a future iteration.
    return ScreenLeaseStatus(
        holder=None,
        started_at=None,
        idle_seconds=0,
        queue_length=0,
    )
