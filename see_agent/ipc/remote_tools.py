"""Remote tool proxies — used by agent subprocesses to call main process.

These classes replace the local TeamBus, TaskBoard, and screen tools
with UDS-based remote calls through the UDSClient.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from see_agent.hand.tool import Tool, ToolResult, ToolResultImage
from see_agent.ipc.protocol import (
    BOARD_ASSIGN,
    BOARD_CLAIM,
    BOARD_COMPLETE,
    BOARD_CREATE,
    BOARD_LIST,
    BOARD_UPDATE,
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

if TYPE_CHECKING:
    from see_agent.ipc.client import UDSClient


# -------------------------------------------------------------------- #
# RemoteBus — routes send_message through UDS to main process
# -------------------------------------------------------------------- #


class RemoteBus:
    """Bus proxy — routes bus.send through UDS to main process.

    v3.5: drain() returns empty (messages delivered via MessageRouter push).
    send/async_send still write to audit log via router's bus.send handler.
    """

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    def register(self, agent_id: str) -> None:
        """No-op."""

    def send(self, msg: Any) -> None:
        """Send a message through the router."""
        import asyncio
        loop = asyncio.get_event_loop()
        loop.run_until_complete(
            self._client.call(
                BUS_SEND,
                sender=msg.sender,
                recipient=msg.recipient,
                content=msg.content,
            ),
        )

    async def async_send(
        self, sender: str, recipient: str, content: str,
    ) -> None:
        """Async variant of send."""
        await self._client.call(
            BUS_SEND,
            sender=sender,
            recipient=recipient,
            content=content,
        )

    def drain(self, agent_id: str) -> list[Any]:
        """v3.5: Returns empty. Messages delivered via MessageRouter."""
        return []

    async def async_drain(self, agent_id: str) -> list[Any]:
        """v3.5: Returns empty. Messages delivered via MessageRouter."""
        return []

    def has_prior_message(self, from_: str, to: str) -> bool:
        """Always return True in subprocess mode.

        The full audit log is on the main process.  For permission checks
        we err on the permissive side to avoid blocking legitimate comms.
        """
        return True

    def get_queue(self, agent_id: str) -> Any:
        """Not supported in subprocess mode — raise for safety."""
        raise NotImplementedError(
            "get_queue not available in subprocess mode",
        )


# -------------------------------------------------------------------- #
# RemoteBoard — replaces TaskBoard in subprocess
# -------------------------------------------------------------------- #


class RemoteBoard:
    """TaskBoard proxy — routes board operations through UDS.

    Mirrors the TaskBoard public API used by team_tools.
    All public methods have ``async_*`` variants for use from async contexts.
    """

    def __init__(self, client: UDSClient) -> None:
        self._client = client

    # -- async variants (use these from async contexts) --------------------

    async def async_list_tasks(
        self, status: str | None = None,
    ) -> list[Any]:
        result = await self._client.call(
            BOARD_LIST, **({"status": status} if status else {}),
        )
        return _tasks_from_dicts(result.get("tasks", []))

    async def async_create_task(
        self, title: str, description: str = "", created_by: str = "",
    ) -> Any:
        result = await self._client.call(
            BOARD_CREATE,
            title=title, description=description, created_by=created_by,
        )
        return _task_stub(result)

    async def async_claim_task(
        self, task_id: str, agent_id: str,
    ) -> Any:
        result = await self._client.call(
            BOARD_CLAIM, task_id=task_id, agent_id=agent_id,
        )
        return _task_stub(result)

    async def async_complete_task(
        self, task_id: str, agent_id: str, result: str = "",
    ) -> Any:
        res = await self._client.call(
            BOARD_COMPLETE,
            task_id=task_id, agent_id=agent_id, result=result,
        )
        return _task_stub(res)

    async def async_update_task(
        self, task_id: str, **kwargs: Any,
    ) -> Any:
        result = await self._client.call(
            BOARD_UPDATE, task_id=task_id, **kwargs,
        )
        return _task_stub(result)

    async def async_assign_task(
        self, task_id: str, agent_id: str,
    ) -> Any:
        result = await self._client.call(
            BOARD_ASSIGN, task_id=task_id, agent_id=agent_id,
        )
        return _task_stub(result)

    # -- sync wrappers (kept for non-async callers) ------------------------

    def list_tasks(self, status: str | None = None) -> list[Any]:
        import asyncio
        loop = asyncio.get_event_loop()
        result = loop.run_until_complete(
            self._client.call(
                BOARD_LIST,
                **({"status": status} if status else {}),
            ),
        )
        return _tasks_from_dicts(result.get("tasks", []))

    def create_task(
        self, title: str, description: str = "", created_by: str = "",
    ) -> Any:
        import asyncio
        loop = asyncio.get_event_loop()
        result = loop.run_until_complete(
            self._client.call(
                BOARD_CREATE,
                title=title, description=description,
                created_by=created_by,
            ),
        )
        return _task_stub(result)

    def claim_task(self, task_id: str, agent_id: str) -> Any:
        import asyncio
        loop = asyncio.get_event_loop()
        result = loop.run_until_complete(
            self._client.call(
                BOARD_CLAIM, task_id=task_id, agent_id=agent_id,
            ),
        )
        return _task_stub(result)

    def complete_task(
        self, task_id: str, agent_id: str, result: str = "",
    ) -> Any:
        import asyncio
        loop = asyncio.get_event_loop()
        res = loop.run_until_complete(
            self._client.call(
                BOARD_COMPLETE,
                task_id=task_id, agent_id=agent_id, result=result,
            ),
        )
        return _task_stub(res)

    def update_task(self, task_id: str, **kwargs: Any) -> Any:
        import asyncio
        loop = asyncio.get_event_loop()
        result = loop.run_until_complete(
            self._client.call(BOARD_UPDATE, task_id=task_id, **kwargs),
        )
        return _task_stub(result)

    def assign_task(self, task_id: str, agent_id: str) -> Any:
        import asyncio
        loop = asyncio.get_event_loop()
        result = loop.run_until_complete(
            self._client.call(
                BOARD_ASSIGN, task_id=task_id, agent_id=agent_id,
            ),
        )
        return _task_stub(result)


def _tasks_from_dicts(dicts: list[dict[str, Any]]) -> list[Any]:
    """Convert dict list to TaskItem-like objects."""
    from see_agent.team.task_board import TaskItem
    return [
        TaskItem(
            id=d["id"],
            title=d.get("title", ""),
            description=d.get("description", ""),
            status=d.get("status", "pending"),
            assigned_to=d.get("assigned_to"),
        )
        for d in dicts
    ]


def _task_stub(d: dict[str, Any]) -> Any:
    """Convert a single response dict to a TaskItem."""
    from see_agent.team.task_board import TaskItem
    return TaskItem(
        id=d.get("id", ""),
        title=d.get("title", ""),
        status=d.get("status", "pending"),
        assigned_to=d.get("assigned_to"),
    )


# -------------------------------------------------------------------- #
# Remote screen tools — replace local pyautogui tools in subprocess
# -------------------------------------------------------------------- #


class RemoteScreenshotTool(Tool):
    """Take a screenshot via the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "screenshot"

    @property
    def description(self) -> str:
        return "截取当前屏幕截图"

    @property
    def parameters(self) -> dict[str, Any]:
        return {"type": "object", "properties": {}}

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_CAPTURE, agent_id=self._agent_id,
        )
        if result.get("status") == "busy":
            return ToolResult(text=result["message"])
        return ToolResult(
            text=f"截图已保存: {result.get('screenshot_path', '')}",
            images=[
                ToolResultImage(base64=result["screenshot_base64"]),
            ],
        )


class RemoteClickTool(Tool):
    """Click at screen coordinates via the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "click"

    @property
    def description(self) -> str:
        return "点击屏幕上的指定坐标。坐标是逻辑像素，左上角为 (0,0)。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "x": {"type": "integer", "description": "横坐标（逻辑像素）"},
                "y": {"type": "integer", "description": "纵坐标（逻辑像素）"},
                "button": {
                    "type": "string",
                    "enum": ["left", "right", "middle"],
                    "default": "left",
                },
                "double": {"type": "boolean", "default": False},
            },
            "required": ["x", "y"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_CLICK,
            agent_id=self._agent_id,
            x=kwargs["x"], y=kwargs["y"],
            button=kwargs.get("button", "left"),
            double=kwargs.get("double", False),
        )
        if result.get("status") == "busy":
            return ToolResult(text=result["message"])
        x, y = kwargs["x"], kwargs["y"]
        action = "双击" if kwargs.get("double") else "点击"
        return ToolResult(text=f"已{action} ({x}, {y})")


class RemoteTypeTextTool(Tool):
    """Type text via the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "type_text"

    @property
    def description(self) -> str:
        return (
            "在当前焦点位置输入文字。中文通过剪贴板粘贴实现。"
            "如需按回车提交，在 text 末尾加 \\n。"
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "要输入的文字"},
            },
            "required": ["text"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_TYPE_TEXT,
            agent_id=self._agent_id,
            text=kwargs["text"],
        )
        if result.get("status") == "busy":
            return ToolResult(text=result["message"])
        return ToolResult(text=f"已输入文字: {kwargs['text']}")


class RemoteScrollTool(Tool):
    """Scroll via the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "scroll"

    @property
    def description(self) -> str:
        return "在指定位置滚动。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "x": {"type": "integer", "description": "滚动位置横坐标"},
                "y": {"type": "integer", "description": "纵坐标"},
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                },
                "amount": {
                    "type": "integer", "default": 3,
                    "description": "滚动格数",
                },
            },
            "required": ["x", "y", "direction"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_SCROLL,
            agent_id=self._agent_id,
            x=kwargs["x"], y=kwargs["y"],
            direction=kwargs["direction"],
            amount=kwargs.get("amount", 3),
        )
        if result.get("status") == "busy":
            return ToolResult(text=result["message"])
        return ToolResult(text=f"已在 ({kwargs['x']}, {kwargs['y']}) 向{kwargs['direction']}滚动")


class RemoteDragTool(Tool):
    """Drag via the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "drag"

    @property
    def description(self) -> str:
        return "从一个坐标拖拽到另一个坐标。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "start_x": {"type": "integer"},
                "start_y": {"type": "integer"},
                "end_x": {"type": "integer"},
                "end_y": {"type": "integer"},
            },
            "required": ["start_x", "start_y", "end_x", "end_y"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_DRAG,
            agent_id=self._agent_id,
            start_x=kwargs["start_x"], start_y=kwargs["start_y"],
            end_x=kwargs["end_x"], end_y=kwargs["end_y"],
        )
        if result.get("status") == "busy":
            return ToolResult(text=result["message"])
        return ToolResult(
            text=f"已从 ({kwargs['start_x']}, {kwargs['start_y']}) "
            f"拖拽到 ({kwargs['end_x']}, {kwargs['end_y']})"
        )


class RemoteHotkeyTool(Tool):
    """Press keyboard shortcut via the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "hotkey"

    @property
    def description(self) -> str:
        return (
            "按下快捷键组合。例如 ['command','c'] 表示 Cmd+C。"
            "键名请用: command, ctrl, alt, shift, return, escape, "
            "tab, space, delete, fn, up, down, left, right。"
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "keys": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "按键列表",
                },
            },
            "required": ["keys"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_HOTKEY,
            agent_id=self._agent_id,
            keys=kwargs["keys"],
        )
        if result.get("status") == "busy":
            return ToolResult(text=result["message"])
        combo = "+".join(kwargs["keys"])
        return ToolResult(text=f"已按下快捷键: {combo}")


class RemoteScreenAcquireTool(Tool):
    """Request screen lease from the main process."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "screen_acquire"

    @property
    def description(self) -> str:
        return "申请屏幕使用权。获得租约后才能使用截屏、点击等屏幕工具。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "duration": {
                    "type": "integer", "default": 600,
                    "description": "租约时长（秒），默认 600",
                },
            },
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        result = await self._client.call(
            SCREEN_ACQUIRE,
            agent_id=self._agent_id,
            duration=kwargs.get("duration", 600),
        )
        if result.get("granted"):
            return ToolResult(text="屏幕租约已获得，可以开始使用屏幕工具。")
        return ToolResult(text="屏幕当前被占用，已加入等待队列。请先做其他工作。")


class RemoteScreenReleaseTool(Tool):
    """Release screen lease."""

    def __init__(self, client: UDSClient, agent_id: str) -> None:
        self._client = client
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "screen_release"

    @property
    def description(self) -> str:
        return "主动释放屏幕使用权，让其他 agent 可以使用屏幕。"

    @property
    def parameters(self) -> dict[str, Any]:
        return {"type": "object", "properties": {}}

    async def execute(self, **kwargs: Any) -> ToolResult:
        await self._client.call(
            SCREEN_RELEASE, agent_id=self._agent_id,
        )
        return ToolResult(text="屏幕租约已释放。")
