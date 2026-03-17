"""UDSClient — agent subprocess connects to main process AgentRouter."""

from __future__ import annotations

import asyncio
import json
import logging
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class UDSClient:
    """Client that connects to the AgentRouter UDS server.

    Used by agent worker subprocesses to communicate with the main process.
    """

    def __init__(self, sock_path: Path) -> None:
        self._sock_path = sock_path
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._counter = 0
        self._lock = asyncio.Lock()

    async def connect(self) -> None:
        """Open connection to the UDS server."""
        self._reader, self._writer = await asyncio.open_unix_connection(
            str(self._sock_path),
        )
        logger.info("UDSClient connected to %s", self._sock_path)

    async def close(self) -> None:
        """Close the connection."""
        if self._writer is not None:
            self._writer.close()
            await self._writer.wait_closed()
            self._writer = None
            self._reader = None

    async def call(self, method: str, **params: Any) -> dict[str, Any]:
        """Send an RPC request and wait for the response.

        Raises ``RuntimeError`` if the server returns an error.
        """
        if self._writer is None or self._reader is None:
            raise RuntimeError("UDSClient not connected")

        async with self._lock:
            self._counter += 1
            request = {
                "id": self._counter,
                "method": method,
                "params": params,
            }
            self._writer.write(json.dumps(request).encode() + b"\n")
            await self._writer.drain()
            data = await self._reader.readline()

        if not data:
            raise ConnectionError("UDS connection closed by server")

        response = json.loads(data)
        if "error" in response:
            raise RuntimeError(response["error"])
        return response.get("result", {})
