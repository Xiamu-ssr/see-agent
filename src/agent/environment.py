"""Collect macOS desktop environment information at task start.

The gathered info (running apps, frontmost app, installed apps, screen
resolution) is injected into the first user message so the LLM can
make informed decisions about app activation and desktop navigation.
"""

from __future__ import annotations

import asyncio
import logging

logger = logging.getLogger(__name__)


async def _run(cmd: str) -> str:
    """Run a shell command and return stripped stdout, or '' on failure."""
    try:
        proc = await asyncio.create_subprocess_shell(
            cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, _ = await asyncio.wait_for(proc.communicate(), timeout=5)
        return stdout.decode("utf-8", errors="replace").strip()
    except Exception:
        logger.debug("Environment command failed: %s", cmd)
        return ""


async def collect_environment(screen_width: int, screen_height: int) -> str:
    """Return an ``<ENVIRONMENT>`` XML block with desktop context.

    Runs several ``osascript`` / ``ls`` commands concurrently, then
    formats the results into a compact block suitable for injection
    into the conversation context.
    """
    running_task = _run(
        "osascript -e "
        "'tell app \"System Events\" to get name of every process "
        "whose background only is false'"
    )
    frontmost_task = _run(
        "osascript -e "
        "'tell app \"System Events\" to get name of first process "
        "whose frontmost is true'"
    )
    installed_task = _run("ls /Applications/ | sed 's/\\.app$//'")

    running_raw, frontmost, installed_raw = await asyncio.gather(
        running_task, frontmost_task, installed_task,
    )

    lines: list[str] = ["<ENVIRONMENT>"]

    if running_raw:
        lines.append(f"当前运行的应用: {running_raw}")
    if frontmost:
        lines.append(f"最前面的应用: {frontmost}")
    if installed_raw:
        apps = ", ".join(installed_raw.splitlines()[:40])
        lines.append(f"已安装的应用: {apps}")

    lines.append(f"屏幕分辨率: {screen_width}×{screen_height} (逻辑像素)")
    lines.append("</ENVIRONMENT>")

    text = "\n".join(lines)
    logger.info("Environment info collected:\n%s", text)
    return text
