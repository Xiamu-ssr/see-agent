"""WebSocket route for real-time task step streaming."""

from __future__ import annotations

import asyncio
import json
import logging

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

logger = logging.getLogger(__name__)

router = APIRouter()


@router.websocket("/api/ws/{task_id}")
async def websocket_endpoint(websocket: WebSocket, task_id: str) -> None:
    """Stream agent step events and final status over a WebSocket connection.

    Each connected client receives :class:`StepMessage` objects as JSON as
    the agent progresses.  When the task finishes (completed or failed), a
    final status message is sent before the connection is closed.

    Parameters:
        websocket: The WebSocket connection.
        task_id: The task to subscribe to.
    """
    await websocket.accept()
    logger.info("WebSocket connected for task %s", task_id)

    # Get (or create) the subscriber queue list for this task.
    subscribers: dict[str, list[asyncio.Queue[dict | None]]] = (
        websocket.app.state.ws_subscribers
    )
    queue: asyncio.Queue[dict | None] = asyncio.Queue()

    if task_id not in subscribers:
        subscribers[task_id] = []
    subscribers[task_id].append(queue)

    try:
        while True:
            message = await queue.get()

            # A ``None`` sentinel signals the task is done — send final
            # status and break out of the loop.
            if message is None:
                # Send the final task status if available.
                tasks = websocket.app.state.tasks
                if task_id in tasks:
                    await websocket.send_text(tasks[task_id].model_dump_json())
                break

            await websocket.send_text(json.dumps(message))
    except WebSocketDisconnect:
        logger.info("WebSocket disconnected for task %s", task_id)
    except Exception:
        logger.exception("WebSocket error for task %s", task_id)
    finally:
        # Clean up: remove this queue from the subscriber list.
        if task_id in subscribers:
            try:
                subscribers[task_id].remove(queue)
            except ValueError:
                pass
            # Remove the task key entirely if no subscribers remain.
            if not subscribers[task_id]:
                del subscribers[task_id]
        logger.info("WebSocket cleaned up for task %s", task_id)
