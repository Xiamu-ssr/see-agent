"""JSON-RPC style message definitions for IPC over Unix Domain Sockets."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class RPCRequest:
    """A JSON-RPC style request from agent subprocess to main process."""

    id: int
    method: str
    params: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {"id": self.id, "method": self.method, "params": self.params}


@dataclass
class RPCResponse:
    """A JSON-RPC style response from main process to agent subprocess."""

    id: int
    result: dict[str, Any] | None = None
    error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {"id": self.id}
        if self.error is not None:
            d["error"] = self.error
        else:
            d["result"] = self.result or {}
        return d


# Method constants for type-safe dispatch.
# Bus
BUS_SEND = "bus.send"
BUS_DRAIN = "bus.drain"

# TaskBoard
BOARD_LIST = "board.list"
BOARD_CREATE = "board.create"
BOARD_CLAIM = "board.claim"
BOARD_COMPLETE = "board.complete"
BOARD_UPDATE = "board.update"
BOARD_ASSIGN = "board.assign"

# Screen
SCREEN_ACQUIRE = "screen.acquire"
SCREEN_RELEASE = "screen.release"
SCREEN_CAPTURE = "screen.capture"
SCREEN_CLICK = "screen.click"
SCREEN_TYPE_TEXT = "screen.type_text"
SCREEN_SCROLL = "screen.scroll"
SCREEN_DRAG = "screen.drag"
SCREEN_HOTKEY = "screen.hotkey"
