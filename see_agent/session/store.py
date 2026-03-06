"""File-based session store under ``~/.see-agent/sessions/``.

Each session is a directory containing:
- ``meta.json``  — session metadata (id, task, status, timestamps, …)
- ``messages.jsonl`` — one JSON object per line (no base64, screenshot file refs)
- ``screenshots/`` — WebP files named ``step_NNN.webp``
"""

from __future__ import annotations

import base64
import json
import logging
import shutil
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable

from see_agent.config import SESSIONS_DIR

if TYPE_CHECKING:
    from see_agent.agent.context import ConversationContext

logger = logging.getLogger(__name__)


# ------------------------------------------------------------------ #
# Data models
# ------------------------------------------------------------------ #

@dataclass
class SessionSummary:
    """Lightweight view returned by :meth:`SessionStore.list`."""

    id: str
    task: str
    status: str
    total_steps: int
    elapsed_seconds: float
    created_at: str
    updated_at: str


@dataclass
class Session:
    """Single session instance wrapping a directory on disk.

    ``on_append`` is a convenience callback: when set, every call to
    :meth:`append_message` also fires it with the JSONL-ready dict.
    """

    id: str
    task: str
    status: str
    dir: Path
    meta: dict[str, Any] = field(default_factory=dict)

    # ---- internal bookkeeping ----
    _jsonl_path: Path = field(init=False, repr=False)
    _meta_path: Path = field(init=False, repr=False)
    _screenshots_dir: Path = field(init=False, repr=False)

    def __post_init__(self) -> None:
        self._jsonl_path = self.dir / "messages.jsonl"
        self._meta_path = self.dir / "meta.json"
        self._screenshots_dir = self.dir / "screenshots"

    @property
    def screenshots_dir(self) -> Path:
        return self._screenshots_dir

    # ---- message persistence ----

    def append_message(self, msg: dict[str, Any]) -> None:
        """Append *msg* as one JSON line to ``messages.jsonl``."""
        msg_with_ts = {"ts": _now_iso(), **msg}
        with open(self._jsonl_path, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(msg_with_ts, ensure_ascii=False) + "\n")

    def read_messages(self) -> list[dict[str, Any]]:
        """Read all persisted JSONL messages."""
        if not self._jsonl_path.exists():
            return []
        lines: list[dict[str, Any]] = []
        for raw in self._jsonl_path.read_text(encoding="utf-8").splitlines():
            raw = raw.strip()
            if raw:
                lines.append(json.loads(raw))
        return lines

    # ---- screenshot helpers ----

    def screenshot_path(self, step: int) -> Path:
        """Return the path for a given step's screenshot (does not create it)."""
        return self._screenshots_dir / f"step_{step:03d}.webp"

    # ---- meta persistence ----

    def next_step_number(self) -> int:
        """Return the next available step number based on existing screenshots."""
        if not self._screenshots_dir.exists():
            return 0
        existing = list(self._screenshots_dir.glob("step_*.webp"))
        if not existing:
            return 0
        max_num = max(int(f.stem.split("_")[1]) for f in existing)
        return max_num + 1

    def restore_context(
        self,
        system_prompt: str,
        max_images: int = 5,
        on_append: Callable[[dict[str, Any]], None] | None = None,
    ) -> "ConversationContext":
        """Rebuild a :class:`ConversationContext` from JSONL + screenshot files.

        Reads ``messages.jsonl``, converts each line back into the OpenAI
        message format, loading screenshot base64 from disk where referenced.
        The ``on_append`` callback is only activated *after* replay so that
        old messages are not written back to the JSONL.

        Parameters:
            system_prompt: Current system prompt (used for the initial message).
            max_images: Sliding-window limit forwarded to the context.
            on_append: Callback activated after replay completes.

        Returns:
            A fully populated :class:`ConversationContext`.
        """
        from see_agent.agent.context import ConversationContext

        # Build context WITHOUT on_append during replay.
        ctx = ConversationContext(system_prompt, max_images=max_images)

        messages = self.read_messages()
        for msg in messages:
            msg_type = msg.get("type")
            if msg_type == "system":
                # Already added by ConversationContext.__init__; skip.
                continue
            elif msg_type == "user_task":
                b64 = self._load_screenshot_b64(msg.get("screenshot"))
                detail = msg.get("detail", "high")
                ctx.add_user_task(
                    msg.get("text", ""), b64, detail,
                )
            elif msg_type == "assistant":
                ctx._messages.append(
                    self._rebuild_assistant_message(msg)
                )
            elif msg_type == "tool_result":
                ctx._messages.append({
                    "role": "tool",
                    "tool_call_id": msg.get("tool_call_id", ""),
                    "content": msg.get("result", ""),
                })
                # The screenshot for tool_result is stored as a
                # separate "screenshot" JSONL entry — handled below.
            elif msg_type == "screenshot":
                b64 = self._load_screenshot_b64(msg.get("screenshot"))
                detail = msg.get("detail", "high")
                if b64:
                    ctx._messages.append({
                        "role": "user",
                        "content": [{
                            "type": "image_url",
                            "image_url": {
                                "url": f"data:image/webp;base64,{b64}",
                                "detail": detail,
                            },
                        }],
                    })
                else:
                    ctx._messages.append({
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "[Screenshot omitted]"},
                        ],
                    })
            elif msg_type == "user_reply":
                ctx._messages.append({
                    "role": "user",
                    "content": msg.get("text", ""),
                })
            elif msg_type == "system_hint":
                ctx._messages.append({
                    "role": "user",
                    "content": msg.get("text", ""),
                })

        # NOW activate the on_append callback for future messages.
        ctx._on_append = on_append

        logger.info(
            "Restored context: %d messages from session %s",
            len(ctx._messages), self.id,
        )
        return ctx

    def _load_screenshot_b64(self, ref: str | None) -> str:
        """Load a screenshot file and return its base64 encoding.

        Returns an empty string if *ref* is ``None`` or the file is missing.
        """
        if not ref:
            return ""
        path = self._screenshots_dir / ref
        if not path.exists():
            logger.warning("Screenshot file missing: %s", path)
            return ""
        return base64.b64encode(path.read_bytes()).decode("ascii")

    @staticmethod
    def _rebuild_assistant_message(msg: dict[str, Any]) -> dict[str, Any]:
        """Convert a JSONL assistant entry back to OpenAI format."""
        result: dict[str, Any] = {"role": "assistant"}
        if msg.get("content"):
            result["content"] = msg["content"]
        else:
            result["content"] = None
        if msg.get("tool_calls"):
            result["tool_calls"] = [
                {
                    "id": tc["id"],
                    "type": "function",
                    "function": {
                        "name": tc["name"],
                        "arguments": tc.get("args", ""),
                    },
                }
                for tc in msg["tool_calls"]
            ]
        return result

    def update_meta(self, **kwargs: Any) -> None:
        """Merge *kwargs* into meta.json and flush to disk."""
        self.meta.update(kwargs)
        self.meta["updated_at"] = _now_iso()
        if "status" in kwargs:
            self.status = kwargs["status"]
        self._meta_path.write_text(
            json.dumps(self.meta, indent=2, ensure_ascii=False), encoding="utf-8",
        )


# ------------------------------------------------------------------ #
# Session store (static helpers)
# ------------------------------------------------------------------ #

class SessionStore:
    """Pure-file session store rooted at ``~/.see-agent/sessions/``."""

    @staticmethod
    def create(task: str, config: dict[str, Any]) -> Session:
        """Create a new session directory and return the :class:`Session`."""
        session_id = datetime.now().strftime("%Y%m%d_%H%M%S") + "_" + _short_id()
        session_dir = SESSIONS_DIR / session_id
        session_dir.mkdir(parents=True, exist_ok=True)
        (session_dir / "screenshots").mkdir(exist_ok=True)

        meta: dict[str, Any] = {
            "id": session_id,
            "task": task,
            "status": "running",
            "created_at": _now_iso(),
            "updated_at": _now_iso(),
            "total_steps": 0,
            "elapsed_seconds": 0.0,
            "summary": "",
            "config_snapshot": {
                "model": config.get("llm", {}).get("model", ""),
                "max_steps": config.get("max_steps", 50),
                "scaling_enabled": config.get("scaling_enabled", True),
            },
        }
        (session_dir / "meta.json").write_text(
            json.dumps(meta, indent=2, ensure_ascii=False), encoding="utf-8",
        )
        # Create empty JSONL file.
        (session_dir / "messages.jsonl").touch()

        logger.info("Created session %s at %s", session_id, session_dir)
        return Session(id=session_id, task=task, status="running", dir=session_dir, meta=meta)

    @staticmethod
    def load(session_id: str) -> Session:
        """Load an existing session from disk.

        Raises:
            FileNotFoundError: If the session directory or meta.json is missing.
        """
        session_dir = SESSIONS_DIR / session_id
        meta_path = session_dir / "meta.json"
        if not meta_path.exists():
            raise FileNotFoundError(f"Session not found: {session_id}")
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        return Session(
            id=meta["id"],
            task=meta.get("task", ""),
            status=meta.get("status", "unknown"),
            dir=session_dir,
            meta=meta,
        )

    @staticmethod
    def list(*, status: str | None = None, limit: int = 20) -> list[SessionSummary]:
        """List sessions sorted by creation time (newest first)."""
        if not SESSIONS_DIR.exists():
            return []
        summaries: list[SessionSummary] = []
        for d in sorted(SESSIONS_DIR.iterdir(), reverse=True):
            meta_path = d / "meta.json"
            if not meta_path.exists():
                continue
            try:
                meta = json.loads(meta_path.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, OSError):
                continue
            if status and meta.get("status") != status:
                continue
            summaries.append(
                SessionSummary(
                    id=meta.get("id", d.name),
                    task=meta.get("task", ""),
                    status=meta.get("status", "unknown"),
                    total_steps=meta.get("total_steps", 0),
                    elapsed_seconds=meta.get("elapsed_seconds", 0.0),
                    created_at=meta.get("created_at", ""),
                    updated_at=meta.get("updated_at", ""),
                )
            )
            if len(summaries) >= limit:
                break
        return summaries

    @staticmethod
    def delete(session_id: str) -> None:
        """Delete a session directory entirely."""
        session_dir = SESSIONS_DIR / session_id
        if session_dir.exists():
            shutil.rmtree(session_dir)
            logger.info("Deleted session %s", session_id)

    @staticmethod
    def clean(*, keep_days: int = 7, empty_only: bool = False) -> tuple[int, int]:
        """Clean old or empty sessions.

        Returns:
            ``(deleted_count, freed_bytes)``
        """
        if not SESSIONS_DIR.exists():
            return 0, 0
        cutoff = time.time() - keep_days * 86400
        deleted = 0
        freed = 0
        for d in list(SESSIONS_DIR.iterdir()):
            meta_path = d / "meta.json"
            if not meta_path.exists():
                # Orphan directory — remove.
                size = _dir_size(d)
                shutil.rmtree(d, ignore_errors=True)
                deleted += 1
                freed += size
                continue
            if empty_only:
                ss_dir = d / "screenshots"
                screenshots = (
                    list(ss_dir.glob("*.webp")) if ss_dir.exists() else []
                )
                if screenshots:
                    continue
                size = _dir_size(d)
                shutil.rmtree(d, ignore_errors=True)
                deleted += 1
                freed += size
                continue
            # Age-based cleanup.
            try:
                meta = json.loads(meta_path.read_text(encoding="utf-8"))
                created = meta.get("created_at", "")
                ts = datetime.fromisoformat(created).timestamp()
            except Exception:
                ts = d.stat().st_mtime
            if ts < cutoff:
                size = _dir_size(d)
                shutil.rmtree(d, ignore_errors=True)
                deleted += 1
                freed += size
        return deleted, freed


# ------------------------------------------------------------------ #
# Internal helpers
# ------------------------------------------------------------------ #

def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def _short_id() -> str:
    """Return a 6-char hex suffix for session IDs."""
    import secrets
    return secrets.token_hex(3)


def _dir_size(path: Path) -> int:
    """Return total size in bytes of all files under *path*."""
    total = 0
    for f in path.rglob("*"):
        if f.is_file():
            total += f.stat().st_size
    return total
