"""AgentRouter — main-process UDS server handling agent subprocess requests.

The router is the central hub: it owns the TeamBus, TaskBoard,
ScreenManager, and MacEye instances, and exposes them over a Unix
Domain Socket using a JSON-RPC style protocol.
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
from pathlib import Path
from typing import Any

from see_agent.config import RUN_DIR, TEAMS_DIR
from see_agent.ipc.protocol import (
    BOARD_ASSIGN,
    BOARD_CLAIM,
    BOARD_COMPLETE,
    BOARD_CREATE,
    BOARD_LIST,
    BOARD_UPDATE,
    BUS_DRAIN,
    BUS_SEND,
    SCREEN_ACQUIRE,
    SCREEN_CAPTURE,
    SCREEN_CLICK,
    SCREEN_DRAG,
    SCREEN_HOTKEY,
    SCREEN_RELEASE,
    SCREEN_SCROLL,
    SCREEN_TYPE_TEXT,
)
from see_agent.screen.manager import ScreenManager
from see_agent.team.bus import BusMessage, TeamBus
from see_agent.team.task_board import TaskBoard

logger = logging.getLogger(__name__)


class AgentRouter:
    """Main-process RPC server over UDS, handling all agent subprocess requests."""

    def __init__(self, team_id: str) -> None:
        self._team_id = team_id
        self._team_dir = TEAMS_DIR / team_id
        self._team_dir.mkdir(parents=True, exist_ok=True)

        self._bus = TeamBus(self._team_dir)
        self._board = TaskBoard(self._team_dir)
        self._screen = ScreenManager()
        self._eye: Any = None  # Lazy-loaded MacEye

        RUN_DIR.mkdir(parents=True, exist_ok=True)
        self._sock_path = RUN_DIR / f"{team_id}.sock"
        self._server: asyncio.AbstractServer | None = None

    @property
    def sock_path(self) -> Path:
        return self._sock_path

    @property
    def bus(self) -> TeamBus:
        return self._bus

    @property
    def board(self) -> TaskBoard:
        return self._board

    @property
    def screen(self) -> ScreenManager:
        return self._screen

    def register_agent(self, agent_id: str) -> None:
        """Register an agent on the bus (must be called before start)."""
        self._bus.register(agent_id)

    async def start(self) -> None:
        """Start the UDS server and screen manager."""
        # Remove stale socket file.
        if self._sock_path.exists():
            self._sock_path.unlink()

        self._server = await asyncio.start_unix_server(
            self._handle_connection, path=str(self._sock_path),
        )
        await self._screen.start()
        logger.info("AgentRouter started on %s", self._sock_path)

    async def stop(self) -> None:
        """Stop the UDS server and clean up."""
        await self._screen.stop()
        if self._server is not None:
            self._server.close()
            await self._server.wait_closed()
            self._server = None
        if self._sock_path.exists():
            self._sock_path.unlink(missing_ok=True)
        logger.info("AgentRouter stopped")

    # ------------------------------------------------------------------ #
    # Connection handler
    # ------------------------------------------------------------------ #

    async def _handle_connection(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> None:
        """Handle a single agent connection."""
        peer = writer.get_extra_info("peername", "unknown")
        logger.debug("Agent connected: %s", peer)
        try:
            while True:
                data = await reader.readline()
                if not data:
                    break
                try:
                    request = json.loads(data)
                except json.JSONDecodeError:
                    logger.warning("Invalid JSON from agent: %r", data[:200])
                    continue
                result = await self._dispatch(request)
                writer.write(json.dumps(result).encode() + b"\n")
                await writer.drain()
        except ConnectionError:
            logger.debug("Agent disconnected: %s", peer)
        finally:
            writer.close()
            await writer.wait_closed()

    # ------------------------------------------------------------------ #
    # Dispatch
    # ------------------------------------------------------------------ #

    async def _dispatch(self, request: dict[str, Any]) -> dict[str, Any]:
        """Route a request to the appropriate handler."""
        req_id = request.get("id", 0)
        method = request.get("method", "")
        params = request.get("params", {})

        handlers: dict[str, Any] = {
            # Bus
            BUS_SEND: self._bus_send,
            BUS_DRAIN: self._bus_drain,
            # Board
            BOARD_LIST: self._board_list,
            BOARD_CREATE: self._board_create,
            BOARD_CLAIM: self._board_claim,
            BOARD_COMPLETE: self._board_complete,
            BOARD_UPDATE: self._board_update,
            BOARD_ASSIGN: self._board_assign,
            # Screen
            SCREEN_ACQUIRE: self._screen_acquire,
            SCREEN_RELEASE: self._screen_release,
            SCREEN_CAPTURE: self._screen_capture,
            SCREEN_CLICK: self._screen_click,
            SCREEN_TYPE_TEXT: self._screen_type,
            SCREEN_SCROLL: self._screen_scroll,
            SCREEN_DRAG: self._screen_drag,
            SCREEN_HOTKEY: self._screen_hotkey,
        }

        handler = handlers.get(method)
        if handler is None:
            return {"id": req_id, "error": f"Unknown method: {method}"}

        try:
            result = await handler(**params)
            return {"id": req_id, "result": result}
        except Exception as exc:
            logger.exception("Handler error for %s", method)
            return {"id": req_id, "error": str(exc)}

    # ------------------------------------------------------------------ #
    # Bus handlers
    # ------------------------------------------------------------------ #

    async def _bus_send(
        self, sender: str, recipient: str, content: str, **_: Any,
    ) -> dict[str, Any]:
        self._bus.send(
            BusMessage(sender=sender, recipient=recipient, content=content),
        )
        return {"status": "ok"}

    async def _bus_drain(
        self, agent_id: str, **_: Any,
    ) -> dict[str, Any]:
        messages = self._bus.drain(agent_id)
        return {
            "messages": [
                {
                    "sender": m.sender,
                    "recipient": m.recipient,
                    "content": m.content,
                    "ts": m.ts,
                }
                for m in messages
            ],
        }

    # ------------------------------------------------------------------ #
    # Board handlers
    # ------------------------------------------------------------------ #

    async def _board_list(
        self, status: str | None = None, **_: Any,
    ) -> dict[str, Any]:
        tasks = self._board.list_tasks(status=status)
        return {
            "tasks": [
                {
                    "id": t.id,
                    "title": t.title,
                    "description": t.description,
                    "status": t.status,
                    "assigned_to": t.assigned_to,
                }
                for t in tasks
            ],
        }

    async def _board_create(
        self, title: str, description: str = "",
        created_by: str = "", **_: Any,
    ) -> dict[str, Any]:
        t = self._board.create_task(
            title=title, description=description, created_by=created_by,
        )
        return {"id": t.id, "title": t.title}

    async def _board_claim(
        self, task_id: str, agent_id: str, **_: Any,
    ) -> dict[str, Any]:
        t = self._board.claim_task(task_id, agent_id)
        return {"id": t.id, "title": t.title, "status": t.status}

    async def _board_complete(
        self, task_id: str, agent_id: str,
        result: str = "", **_: Any,
    ) -> dict[str, Any]:
        t = self._board.complete_task(task_id, agent_id, result=result)
        return {"id": t.id, "title": t.title, "status": t.status}

    async def _board_update(
        self, task_id: str, **kwargs: Any,
    ) -> dict[str, Any]:
        # Only pass recognised fields to update.
        allowed = {"status", "title", "description", "assigned_to", "result"}
        update_kw = {k: v for k, v in kwargs.items() if k in allowed}
        t = self._board.update_task(task_id, **update_kw)
        return {"id": t.id, "title": t.title, "status": t.status}

    async def _board_assign(
        self, task_id: str, agent_id: str, **_: Any,
    ) -> dict[str, Any]:
        t = self._board.assign_task(task_id, agent_id)
        return {"id": t.id, "assigned_to": t.assigned_to}

    # ------------------------------------------------------------------ #
    # Screen handlers
    # ------------------------------------------------------------------ #

    def _ensure_eye(self) -> Any:
        """Lazy-import MacEye so the module can be imported without pyautogui."""
        if self._eye is None:
            from see_agent.eye.mac import MacEye
            self._eye = MacEye()
        return self._eye

    def _holder_id(self, agent_id: str) -> str:
        return f"{self._team_id}:{agent_id}"

    async def _check_lease(self, agent_id: str) -> dict[str, Any] | None:
        """Check/acquire lease. Returns error dict if agent cannot use screen."""
        holder_id = self._holder_id(agent_id)
        if self._screen.is_holder(holder_id):
            self._screen.touch(holder_id)
            return None
        granted = await self._screen.acquire(holder_id)
        if not granted:
            return {
                "status": "busy",
                "message": (
                    "屏幕当前被其他 agent 占用，你的申请已排队。"
                    "请先处理不需要屏幕的工作。"
                ),
            }
        self._screen.touch(holder_id)
        return None

    async def _screen_acquire(
        self, agent_id: str, duration: int = 600, **_: Any,
    ) -> dict[str, Any]:
        holder_id = self._holder_id(agent_id)
        granted = await self._screen.acquire(holder_id, duration)
        return {"granted": granted}

    async def _screen_release(
        self, agent_id: str, **_: Any,
    ) -> dict[str, Any]:
        holder_id = self._holder_id(agent_id)
        await self._screen.release(holder_id)
        return {"status": "ok"}

    async def _screen_capture(
        self, agent_id: str, **_: Any,
    ) -> dict[str, Any]:
        busy = await self._check_lease(agent_id)
        if busy:
            return busy

        eye = self._ensure_eye()
        screenshot = await eye.capture()

        # Save screenshot to agent directory.
        agent_dir = self._team_dir / "agents" / agent_id / "screenshots"
        agent_dir.mkdir(parents=True, exist_ok=True)

        import time
        ts = int(time.time() * 1000)
        path = agent_dir / f"{ts}.webp"

        # Decode and save.
        img_data = base64.b64decode(screenshot.base64)
        path.write_bytes(img_data)

        return {
            "status": "ok",
            "screenshot_base64": screenshot.base64,
            "screenshot_path": str(path),
            "width": screenshot.width,
            "height": screenshot.height,
        }

    async def _screen_click(
        self, agent_id: str, x: int, y: int,
        button: str = "left", double: bool = False, **_: Any,
    ) -> dict[str, Any]:
        busy = await self._check_lease(agent_id)
        if busy:
            return busy

        import pyautogui
        clicks = 2 if double else 1
        pyautogui.click(x, y, button=button, clicks=clicks)
        return {"status": "ok"}

    async def _screen_type(
        self, agent_id: str, text: str, **_: Any,
    ) -> dict[str, Any]:
        busy = await self._check_lease(agent_id)
        if busy:
            return busy

        import subprocess as sp

        import pyautogui
        press_enter = text.endswith("\n")
        paste_text = text.rstrip("\n") if press_enter else text
        proc = sp.Popen(["pbcopy"], stdin=sp.PIPE)
        proc.communicate(paste_text.encode("utf-8"))
        pyautogui.hotkey("command", "v")
        if press_enter:
            pyautogui.press("enter")
        return {"status": "ok"}

    async def _screen_scroll(
        self, agent_id: str, x: int, y: int,
        direction: str, amount: int = 3, **_: Any,
    ) -> dict[str, Any]:
        busy = await self._check_lease(agent_id)
        if busy:
            return busy

        import pyautogui
        pyautogui.moveTo(x, y)
        if direction in ("up", "down"):
            clicks = amount if direction == "up" else -amount
            pyautogui.scroll(clicks)
        elif direction in ("left", "right"):
            clicks = amount if direction == "right" else -amount
            if hasattr(pyautogui, "hscroll"):
                pyautogui.hscroll(clicks)
        return {"status": "ok"}

    async def _screen_drag(
        self, agent_id: str,
        start_x: int, start_y: int,
        end_x: int, end_y: int, **_: Any,
    ) -> dict[str, Any]:
        busy = await self._check_lease(agent_id)
        if busy:
            return busy

        import pyautogui
        pyautogui.moveTo(start_x, start_y)
        pyautogui.mouseDown(button="left")
        pyautogui.moveTo(end_x, end_y, duration=0.5)
        pyautogui.mouseUp(button="left")
        return {"status": "ok"}

    async def _screen_hotkey(
        self, agent_id: str, keys: list[str], **_: Any,
    ) -> dict[str, Any]:
        busy = await self._check_lease(agent_id)
        if busy:
            return busy

        import pyautogui
        key_aliases = {
            "control": "ctrl", "cmd": "command",
            "option": "alt", "enter": "return", "esc": "escape",
        }
        normalised = [key_aliases.get(k.lower(), k) for k in keys]
        pyautogui.hotkey(*normalised)
        return {"status": "ok"}
