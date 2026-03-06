"""Chat route — accepts a task and runs the agent loop in the background."""

from __future__ import annotations

import asyncio
import logging
import uuid
from typing import Any

from fastapi import APIRouter, Request

from see_agent.agent.loop import AgentLoop, StepEvent
from see_agent.brain.openai_client import OpenAIBrain
from see_agent.eye.mac import MacEye
from see_agent.hand.tools import create_registry
from see_agent.server.models import ChatRequest, ChatResponse, StepMessage, TaskStatus

logger = logging.getLogger(__name__)

router = APIRouter()


async def _broadcast(
    subscribers: dict[str, list[asyncio.Queue[dict | None]]],
    task_id: str,
    message: dict | None,
) -> None:
    """Put *message* into every subscriber queue for *task_id*.

    A ``None`` message acts as a sentinel indicating the task is finished.

    Parameters:
        subscribers: The app-level ``ws_subscribers`` mapping.
        task_id: The task whose subscribers should receive the message.
        message: The JSON-serialisable dict to broadcast, or ``None`` to
            signal completion.
    """
    queues = subscribers.get(task_id, [])
    for queue in queues:
        await queue.put(message)


async def _run_agent(
    task_id: str,
    task: str,
    config: dict[str, Any],
    tasks: dict[str, TaskStatus],
    subscribers: dict[str, list[asyncio.Queue[dict | None]]],
    session_id: str | None = None,
) -> None:
    """Create and run the :class:`AgentLoop`, updating shared state as it progresses.

    This coroutine is spawned as a background ``asyncio.Task`` so that the
    ``POST /api/chat`` handler can return immediately.

    Parameters:
        task_id: Unique identifier for this run.
        task: The user's natural-language task description.
        config: Application configuration dict (from :func:`load_config`).
        tasks: The app-level dict that tracks ``TaskStatus`` objects.
        subscribers: The app-level WebSocket subscriber mapping.
    """
    llm_cfg = config.get("llm", {})

    eye = MacEye()
    brain = OpenAIBrain(
        base_url=llm_cfg.get("base_url", "https://api.openai.com/v1"),
        api_key=llm_cfg.get("api_key", ""),
        model=llm_cfg.get("model", "gpt-4o"),
    )
    registry = create_registry(eye)

    step_count = 0

    async def on_step(event: StepEvent) -> None:
        """Callback invoked by the agent loop after each step."""
        nonlocal step_count
        step_count += 1

        step_msg = StepMessage(
            step=event.step,
            max_steps=event.max_steps,
            thought=event.thought,
            tool_name=event.tool_name,
            tool_args=event.tool_args,
            tool_result=event.tool_result,
            screenshot_path=event.screenshot_path,
        )

        # Update shared task state.
        tasks[task_id] = TaskStatus(
            task_id=task_id,
            status="running",
            steps=step_count,
        )

        # Broadcast step to WebSocket subscribers.
        await _broadcast(subscribers, task_id, step_msg.model_dump())

        logger.debug(
            "Task %s step %d/%d — tool=%s",
            task_id,
            event.step,
            event.max_steps,
            event.tool_name,
        )

    try:
        logger.info("Starting agent loop for task %s: %s", task_id, task)

        agent = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config=config,
            on_step=on_step,
        )
        run_result = await agent.run(task, session_id=session_id)

        tasks[task_id] = TaskStatus(
            task_id=task_id,
            status="completed" if run_result.success else "failed",
            summary=run_result.summary,
            steps=step_count,
        )
        logger.info("Task %s %s: %s", task_id,
                     "completed" if run_result.success else "failed",
                     run_result.summary)

    except Exception as exc:
        logger.exception("Task %s failed", task_id)
        tasks[task_id] = TaskStatus(
            task_id=task_id,
            status="failed",
            steps=step_count,
            error=str(exc),
        )
    finally:
        # Notify all WebSocket subscribers that the task is done.
        await _broadcast(subscribers, task_id, None)


@router.post("/api/chat")
async def chat(body: ChatRequest, request: Request) -> ChatResponse:
    """Accept a task and run the agent loop in the background.

    Parameters:
        body: The chat request containing the task description.
        request: The incoming FastAPI request (used to access app state).

    Returns:
        A :class:`ChatResponse` with the generated ``task_id`` and initial
        ``"running"`` status.
    """
    task_id: str = uuid.uuid4().hex[:8]
    config: dict[str, Any] = request.app.state.config
    tasks: dict[str, TaskStatus] = request.app.state.tasks
    subscribers: dict[str, list[asyncio.Queue[dict | None]]] = (
        request.app.state.ws_subscribers
    )

    # Initialise task state immediately so GET /api/task/{id} works right away.
    tasks[task_id] = TaskStatus(
        task_id=task_id,
        status="running",
        steps=0,
    )

    # Spawn the agent loop as a background asyncio task.
    asyncio.create_task(
        _run_agent(task_id, body.task, config, tasks, subscribers, session_id=body.session_id),
        name=f"agent-{task_id}",
    )

    logger.info("Created task %s for: %s", task_id, body.task)
    return ChatResponse(task_id=task_id, status="running")
