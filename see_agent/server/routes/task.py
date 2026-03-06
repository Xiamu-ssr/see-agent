"""Task status retrieval route for the see-agent API."""

from __future__ import annotations

import logging

from fastapi import APIRouter, HTTPException, Request

from see_agent.server.models import TaskStatus

logger = logging.getLogger(__name__)

router = APIRouter()


@router.get("/api/task/{task_id}")
async def get_task(task_id: str, request: Request) -> TaskStatus:
    """Return the current status of a task.

    Parameters:
        task_id: The unique identifier of the task (hex string).
        request: The incoming FastAPI request (used to access app state).

    Returns:
        The :class:`TaskStatus` for the requested task.

    Raises:
        HTTPException: 404 if the task_id is not found.
    """
    tasks: dict[str, TaskStatus] = request.app.state.tasks

    if task_id not in tasks:
        logger.warning("Task not found: %s", task_id)
        raise HTTPException(status_code=404, detail=f"Task '{task_id}' not found")

    return tasks[task_id]
