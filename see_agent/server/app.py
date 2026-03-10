"""FastAPI application factory for the see-agent server."""

from __future__ import annotations

import logging
from contextlib import asynccontextmanager
from pathlib import Path
from typing import AsyncIterator

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles

from see_agent.config import load_config
from see_agent.server.routes import (
    agents,
    config_routes,
    dashboard,
    health,
    logs,
    mcp,
    screen,
    skills,
    team,
    tools,
)

logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    """Application lifespan handler: runs setup on startup and teardown on shutdown.

    On startup:
        - Load configuration from ``~/.see-agent/config.json`` (with env overrides).
        - Initialise shared state containers for tasks and WebSocket subscribers.
        - Log a startup message.

    On shutdown:
        - Log a shutdown message.
    """
    # ── Startup ────────────────────────────────────────────────────────
    config = load_config()
    app.state.config = config

    # Shared mutable state accessible from route handlers via ``request.app.state``.
    app.state.team_managers = {}

    logger.info(
        "see-agent server started  model=%s  max_steps=%s",
        config.get("llm", {}).get("model", "?"),
        config.get("max_steps", "?"),
    )

    yield

    # ── Shutdown ───────────────────────────────────────────────────────
    logger.info("see-agent server shutting down")

    # Stop all running team managers and their agent subprocesses.
    managers: dict = getattr(app.state, "team_managers", {})
    for team_id, mgr in list(managers.items()):
        try:
            await mgr.stop()
            logger.info("Stopped team manager: %s", team_id)
        except Exception:
            logger.exception("Error stopping team %s", team_id)
    managers.clear()

    # Clean up stale UDS socket files.
    from see_agent.config import RUN_DIR

    if RUN_DIR.exists():
        for sock in RUN_DIR.glob("*.sock"):
            sock.unlink(missing_ok=True)


app = FastAPI(
    title="see-agent",
    version="3.0.0",
    lifespan=lifespan,
)

# ── CORS (dev server) ────────────────────────────────────────────────
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:5173"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ── Register route routers ─────────────────────────────────────────────
app.include_router(health.router)
app.include_router(team.router)
app.include_router(agents.router)
app.include_router(tools.router)
app.include_router(skills.router)
app.include_router(config_routes.router)
app.include_router(dashboard.router)
app.include_router(logs.router)
app.include_router(mcp.router)
app.include_router(screen.router)

# ── Serve frontend build (production) ─────────────────────────────────
_frontend_dir = Path(__file__).parent.parent.parent / "web" / "dist"
if _frontend_dir.is_dir():
    app.mount("/", StaticFiles(directory=str(_frontend_dir), html=True), name="frontend")
