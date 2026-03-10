"""API routes — log file reader."""

from __future__ import annotations

import logging
import re
from typing import Any

from fastapi import APIRouter, Query

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["logs"])

_LOG_RE = re.compile(
    r"^(\d{2}:\d{2}:\d{2})\s+"
    r"(\S+)\s+"
    r"(\S+)\s+"
    r"(.*)$",
)


@router.get("/logs")
async def get_logs(
    date: str = Query(default="", description="Date in YYYY-MM-DD format"),
    level: str = Query(default="", description="Minimum log level filter"),
    limit: int = Query(default=100, ge=1, le=1000),
    offset: int = Query(default=0, ge=0),
) -> list[dict[str, Any]]:
    """Read and parse log entries from the daily log file."""
    from datetime import datetime

    from see_agent.config import LOGS_DIR

    if not date:
        date = datetime.now().strftime("%Y-%m-%d")

    log_file = LOGS_DIR / f"{date}.log"
    if not log_file.exists():
        return []

    level_upper = level.upper() if level else ""
    level_order = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]
    min_idx = 0
    if level_upper in level_order:
        min_idx = level_order.index(level_upper)

    entries: list[dict[str, Any]] = []
    for line in log_file.read_text(encoding="utf-8").splitlines():
        m = _LOG_RE.match(line)
        if not m:
            continue
        time_str, entry_level, logger_name, message = m.groups()
        if level_upper:
            entry_stripped = entry_level.strip()
            if entry_stripped in level_order:
                if level_order.index(entry_stripped) < min_idx:
                    continue
        entries.append({
            "time": time_str,
            "level": entry_level.strip(),
            "logger": logger_name,
            "message": message,
        })

    return entries[offset:offset + limit]
