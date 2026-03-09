"""TaskBoard — shared task list for team collaboration."""

from __future__ import annotations

import json
import logging
import secrets
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class TaskItem:
    """A single task on the board."""

    id: str
    title: str
    description: str = ""
    status: str = "pending"  # pending → claimed → in_progress → done / failed
    assigned_to: str | None = None
    depends_on: list[str] = field(default_factory=list)
    result: str | None = None
    created_by: str = ""
    created_at: str = ""
    updated_at: str = ""


class TaskBoard:
    """File-backed task board stored as ``tasks.json``."""

    def __init__(self, team_dir: Path) -> None:
        self._team_dir = team_dir
        self._path = team_dir / "tasks.json"
        team_dir.mkdir(parents=True, exist_ok=True)

    # ------------------------------------------------------------------ #
    # Public API
    # ------------------------------------------------------------------ #

    def list_tasks(
        self, status: str | None = None,
    ) -> list[TaskItem]:
        """Return all tasks, optionally filtered by *status*."""
        tasks = self._read()
        if status is not None:
            tasks = [t for t in tasks if t.status == status]
        return tasks

    def create_task(
        self,
        title: str,
        description: str = "",
        created_by: str = "",
    ) -> TaskItem:
        """Create a new task and persist it."""
        now = datetime.now(timezone.utc).isoformat()
        task = TaskItem(
            id=secrets.token_hex(4),
            title=title,
            description=description,
            created_by=created_by,
            created_at=now,
            updated_at=now,
        )
        tasks = self._read()
        tasks.append(task)
        self._write(tasks)
        return task

    def assign_task(
        self, task_id: str, agent_id: str,
    ) -> TaskItem:
        """Assign *task_id* to *agent_id*."""
        return self._update(task_id, assigned_to=agent_id)

    def claim_task(
        self, task_id: str, agent_id: str,
    ) -> TaskItem:
        """Claim *task_id* for *agent_id* (set status=claimed)."""
        return self._update(
            task_id, assigned_to=agent_id, status="claimed",
        )

    def update_task(self, task_id: str, **kwargs: Any) -> TaskItem:
        """Update arbitrary fields on *task_id*."""
        return self._update(task_id, **kwargs)

    def complete_task(
        self,
        task_id: str,
        agent_id: str,
        result: str = "",
    ) -> TaskItem:
        """Mark *task_id* as done."""
        return self._update(
            task_id,
            status="done",
            assigned_to=agent_id,
            result=result,
        )

    # ------------------------------------------------------------------ #
    # Persistence
    # ------------------------------------------------------------------ #

    def _read(self) -> list[TaskItem]:
        if not self._path.exists():
            return []
        try:
            data = json.loads(self._path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            return []
        return [self._from_dict(d) for d in data]

    def _write(self, tasks: list[TaskItem]) -> None:
        data = [self._to_dict(t) for t in tasks]
        self._path.write_text(
            json.dumps(data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    def _update(self, task_id: str, **kwargs: Any) -> TaskItem:
        tasks = self._read()
        for task in tasks:
            if task.id == task_id:
                for k, v in kwargs.items():
                    setattr(task, k, v)
                task.updated_at = datetime.now(timezone.utc).isoformat()
                self._write(tasks)
                return task
        raise KeyError(f"Task not found: {task_id}")

    @staticmethod
    def _to_dict(task: TaskItem) -> dict[str, Any]:
        return {
            "id": task.id,
            "title": task.title,
            "description": task.description,
            "status": task.status,
            "assigned_to": task.assigned_to,
            "depends_on": task.depends_on,
            "result": task.result,
            "created_by": task.created_by,
            "created_at": task.created_at,
            "updated_at": task.updated_at,
        }

    @staticmethod
    def _from_dict(d: dict[str, Any]) -> TaskItem:
        return TaskItem(
            id=d["id"],
            title=d.get("title", ""),
            description=d.get("description", ""),
            status=d.get("status", "pending"),
            assigned_to=d.get("assigned_to"),
            depends_on=d.get("depends_on", []),
            result=d.get("result"),
            created_by=d.get("created_by", ""),
            created_at=d.get("created_at", ""),
            updated_at=d.get("updated_at", ""),
        )
