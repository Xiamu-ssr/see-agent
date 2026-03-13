"""AgentSupervisor — manages agent subprocess lifecycle.

v3.5: Replaces TeamManager as the process manager.  Each agent runs in
its own subprocess connected via UDS.  The supervisor can start/stop
agents and send messages to them.
"""

from __future__ import annotations

import logging
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

from see_agent.config import AGENTS_DIR
from see_agent.ipc.message import Message

logger = logging.getLogger(__name__)


class AgentSupervisor:
    """Manage agent subprocesses.

    Parameters:
        global_config: The application config dict (LLM settings etc.).
    """

    def __init__(self, global_config: dict[str, Any]) -> None:
        self._global_config = global_config
        self._processes: dict[str, subprocess.Popen[bytes]] = {}
        self._sock_paths: dict[str, Path] = {}

    # ── Lifecycle ─────────────────────────────────────────────────────

    def start_agent(self, agent_id: str) -> Path:
        """Spawn an agent subprocess and return the UDS socket path.

        If the agent is already running, return its existing socket path.
        """
        if agent_id in self._processes:
            proc = self._processes[agent_id]
            if proc.poll() is None:
                return self._sock_paths[agent_id]
            # Process exited — clean up and restart.
            self._processes.pop(agent_id)
            self._sock_paths.pop(agent_id, None)

        agent_dir = AGENTS_DIR / agent_id
        agent_dir.mkdir(parents=True, exist_ok=True)

        sock_path = Path(f"/tmp/see-agent-{agent_id}.sock")
        self._sock_paths[agent_id] = sock_path

        # Use the venv python — resolve to absolute path.
        project_root = Path(__file__).resolve().parent.parent.parent
        venv_python = project_root / ".venv" / "bin" / "python"
        python_exe = str(venv_python) if venv_python.exists() else sys.executable

        # Worker stderr goes to a log file for debugging.
        stderr_log = agent_dir / "worker_stderr.log"
        stderr_fh = open(stderr_log, "a")

        logger.info(
            "Spawning worker: agent=%s python=%s cwd=%s",
            agent_id, python_exe, project_root,
        )

        # Build a clean environment for the agent subprocess.
        # Remove conda/virtualenv vars that might pollute the child,
        # and ensure .venv/bin is at the front of PATH.
        clean_env = os.environ.copy()
        for key in (
            "CONDA_PREFIX", "CONDA_DEFAULT_ENV", "CONDA_PYTHON_EXE",
            "CONDA_SHLVL", "CONDA_PROMPT_MODIFIER",
            "VIRTUAL_ENV", "PYTHONHOME",
        ):
            clean_env.pop(key, None)
        venv_bin = str(Path(python_exe).parent)
        path_parts = clean_env.get("PATH", "").split(os.pathsep)
        # Remove any existing conda/venv paths, put our venv first.
        path_parts = [
            p for p in path_parts
            if "conda" not in p.lower() and "envs" not in p.lower()
        ]
        if venv_bin not in path_parts:
            path_parts.insert(0, venv_bin)
        clean_env["PATH"] = os.pathsep.join(path_parts)

        # Spawn the agent process — detach from parent session to prevent
        # uvicorn/asyncio from reaping it.
        proc = subprocess.Popen(
            [
                python_exe, "-m", "see_agent.agent.worker",
                agent_id, str(sock_path),
            ],
            stdout=subprocess.DEVNULL,
            stderr=stderr_fh,
            cwd=str(project_root),
            start_new_session=True,
            env=clean_env,
        )
        self._processes[agent_id] = proc
        logger.info("Started agent %s (pid=%d)", agent_id, proc.pid)

        # Quick sanity check: is the process still alive?
        import time
        time.sleep(0.5)
        rc = proc.poll()
        if rc is not None:
            logger.error(
                "Agent %s (pid=%d) died immediately! exit_code=%s",
                agent_id, proc.pid, rc,
            )
            stderr_log = agent_dir / "worker_stderr.log"
            if stderr_log.exists():
                tail = stderr_log.read_text()[-1000:]
                logger.error("Agent %s stderr: %s", agent_id, tail)

        return sock_path

    def stop_agent(self, agent_id: str) -> None:
        """Stop a running agent subprocess."""
        proc = self._processes.pop(agent_id, None)
        self._sock_paths.pop(agent_id, None)
        if proc is None:
            return
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=2)
        logger.info("Stopped agent %s", agent_id)

    def stop_all(self) -> None:
        """Stop all running agent subprocesses."""
        for agent_id in list(self._processes):
            self.stop_agent(agent_id)

    def start_all(self) -> None:
        """Auto-start all configured agents (called on server boot)."""
        from see_agent.config import AGENTS_DIR

        if not AGENTS_DIR.is_dir():
            return
        for agent_dir in sorted(AGENTS_DIR.iterdir()):
            if not agent_dir.is_dir():
                continue
            agent_json = agent_dir / "agent.json"
            if not agent_json.exists():
                continue
            agent_id = agent_dir.name
            # TODO: respect a "frozen" flag in agent.json to skip startup
            try:
                self.start_agent(agent_id)
                logger.info("Auto-started agent %s on server boot", agent_id)
            except Exception:
                logger.exception("Failed to auto-start agent %s", agent_id)

    def is_running(self, agent_id: str) -> bool:
        """Check if an agent subprocess is currently running."""
        proc = self._processes.get(agent_id)
        if proc is None:
            return False
        if proc.poll() is not None:
            # Process exited — clean up.
            self._processes.pop(agent_id, None)
            self._sock_paths.pop(agent_id, None)
            return False
        return True

    def send_to(self, agent_id: str, msg: Message) -> None:
        """Send a message to an agent.

        1. Write to inbox.jsonl with auto-incrementing msg_id (persistence).
        2. Send SIGUSR1 to wake the agent process (notification).
        """
        import json
        import signal as _signal

        logger.info("send_to called: agent=%s running=%s", agent_id, self.is_running(agent_id))

        if not self.is_running(agent_id):
            try:
                self.start_agent(agent_id)
            except Exception:
                logger.exception("start_agent FAILED: agent=%s", agent_id)

        # Write to inbox with msg_id.
        agent_dir = AGENTS_DIR / agent_id
        agent_dir.mkdir(parents=True, exist_ok=True)
        inbox = agent_dir / "inbox.jsonl"

        # Determine next msg_id.
        msg_id = 1
        if inbox.exists():
            for line in inbox.read_text().splitlines():
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                    mid = data.get("msg_id", 0)
                    if mid >= msg_id:
                        msg_id = mid + 1
                except json.JSONDecodeError:
                    continue

        # Build the JSONL entry (msg.to_json() fields + msg_id).
        entry = json.loads(msg.to_json())
        entry["msg_id"] = msg_id

        with open(inbox, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

        # Wake the worker via SIGUSR1.
        proc = self._processes.get(agent_id)
        if proc and proc.poll() is None:
            try:
                os.kill(proc.pid, _signal.SIGUSR1)
            except OSError:
                logger.warning("Failed to signal worker %s", agent_id)
        elif proc:
            logger.error(
                "Agent %s (pid=%d) died before SIGUSR1! exit_code=%s",
                agent_id, proc.pid, proc.returncode,
            )
            # Read stderr log for clues.
            stderr_log = agent_dir / "worker_stderr.log"
            if stderr_log.exists():
                tail = stderr_log.read_text()[-500:]
                logger.error("Agent %s stderr tail: %s", agent_id, tail)

        logger.debug("Message sent to %s (msg_id=%d): %s", agent_id, msg_id, msg.format_prefix())

    @property
    def running_agents(self) -> list[str]:
        """List of currently running agent IDs."""
        return [
            aid for aid in self._processes
            if self.is_running(aid)
        ]
