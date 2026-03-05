"""ShellTool -- execute a shell command asynchronously."""

import asyncio
import logging
from typing import Any

from src.hand.tool import Tool

logger = logging.getLogger(__name__)

_DEFAULT_TIMEOUT: float = 30.0


class ShellTool(Tool):
    """Execute a shell command and return combined stdout/stderr."""

    @property
    def name(self) -> str:
        return "shell"

    @property
    def description(self) -> str:
        return (
            "执行终端命令。打开应用优先用 shell('open -a AppName')，"
            "比视觉找图标更快更准。"
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 shell 命令",
                },
            },
            "required": ["command"],
        }

    async def execute(self, **kwargs: Any) -> str:
        command: str = kwargs["command"]
        logger.info("shell: %s", command)

        try:
            process = await asyncio.create_subprocess_shell(
                command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            stdout, stderr = await asyncio.wait_for(
                process.communicate(),
                timeout=_DEFAULT_TIMEOUT,
            )
        except asyncio.TimeoutError:
            logger.warning("Shell command timed out after %ss: %s", _DEFAULT_TIMEOUT, command)
            # Attempt to kill the runaway process.
            try:
                process.kill()  # type: ignore[union-attr]
            except ProcessLookupError:
                pass
            return f"命令超时（{_DEFAULT_TIMEOUT}s）: {command}"

        stdout_text = stdout.decode("utf-8", errors="replace").strip()
        stderr_text = stderr.decode("utf-8", errors="replace").strip()

        parts: list[str] = []
        if stdout_text:
            parts.append(f"stdout:\n{stdout_text}")
        if stderr_text:
            parts.append(f"stderr:\n{stderr_text}")

        exit_code = process.returncode
        if exit_code != 0:
            parts.append(f"exit_code: {exit_code}")

        if not parts:
            return "命令已执行（无输出）"

        return "\n".join(parts)
