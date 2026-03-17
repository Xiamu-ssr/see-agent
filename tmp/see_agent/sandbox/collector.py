"""Sandbox violation collector — reads macOS unified log for deny events."""

from __future__ import annotations

import asyncio
import json
import logging
import re

logger = logging.getLogger(__name__)

_PATH_RE = re.compile(r'path\s+"([^"]+)"', re.IGNORECASE)
_OP_RE = re.compile(r"deny\(1\)\s+([\w-]+)", re.IGNORECASE)


class SandboxViolationCollector:
    """Collect sandbox deny records from macOS unified log."""

    async def collect(
        self,
        agent_pid: int,
        since_minutes: int = 60,
    ) -> list[dict[str, str]]:
        """Extract sandbox violations for a given PID."""
        try:
            proc = await asyncio.create_subprocess_exec(
                "log", "show",
                "--predicate",
                f'processID == {agent_pid} AND category == "Sandbox"',
                "--last", f"{since_minutes}m",
                "--style", "json",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            stdout, _ = await proc.communicate()
            return self._parse_violations(stdout.decode())
        except FileNotFoundError:
            logger.warning("'log' command not found — cannot collect violations")
            return []
        except Exception:
            logger.exception("Failed to collect sandbox violations")
            return []

    def _parse_violations(self, log_output: str) -> list[dict[str, str]]:
        if not log_output.strip():
            return []
        try:
            entries = json.loads(log_output)
        except json.JSONDecodeError:
            return []

        violations: list[dict[str, str]] = []
        for entry in entries:
            msg = entry.get("eventMessage", "")
            if "deny" not in msg.lower():
                continue
            path = self._extract_path(msg)
            operation = self._extract_operation(msg)
            violations.append({
                "timestamp": entry.get("timestamp", ""),
                "operation": operation,
                "path": path,
            })
        return violations

    def _extract_path(self, msg: str) -> str:
        m = _PATH_RE.search(msg)
        return m.group(1) if m else ""

    def _extract_operation(self, msg: str) -> str:
        m = _OP_RE.search(msg)
        return m.group(1) if m else "unknown"
