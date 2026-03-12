"""AgentSupervisor — manages agent subprocess lifecycle.

v3.5: Replaces TeamManager as the process manager.  Each agent runs in
its own subprocess connected via UDS.  The supervisor can start/stop
agents and send messages to them.
"""

from __future__ import annotations

import json
import logging
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

        # Write config to a temp file for the worker.
        from see_agent.agent.definition import AgentDefinition

        try:
            agent_def = AgentDefinition.load(agent_id)
            config = agent_def.get_merged_config()
        except FileNotFoundError:
            config = self._global_config.copy()

        agent_dir = AGENTS_DIR / agent_id
        agent_dir.mkdir(parents=True, exist_ok=True)
        config["_agent_id"] = agent_id
        config["_agent_dir"] = str(agent_dir)
        config["_session_dir"] = str(agent_dir / "session")
        config["_memory_dir"] = str(agent_dir / "memory")

        # Write runtime config to agent dir.
        config_path = agent_dir / "runtime_config.json"
        config_path.write_text(json.dumps(config, default=str))

        sock_path = Path(f"/tmp/see-agent-{agent_id}.sock")
        self._sock_paths[agent_id] = sock_path

        # Spawn the worker process.
        proc = subprocess.Popen(
            [
                sys.executable, "-m", "see_agent.agent.worker",
                str(config_path), str(sock_path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self._processes[agent_id] = proc
        logger.info("Started agent %s (pid=%d)", agent_id, proc.pid)

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

        If the agent is not running, it is started first.
        The message is written to the agent's UDS socket.

        Note: This is a placeholder for the full UDS push implementation.
        In the current architecture, messages are delivered via the
        AgentRouter's bus.send RPC, which the worker polls.
        """
        if not self.is_running(agent_id):
            self.start_agent(agent_id)

        # Write the message to the agent's inbox file for the worker to pick up.
        agent_dir = AGENTS_DIR / agent_id
        agent_dir.mkdir(parents=True, exist_ok=True)
        inbox = agent_dir / "inbox.jsonl"
        with open(inbox, "a", encoding="utf-8") as f:
            f.write(msg.to_json() + "\n")

        logger.debug("Message sent to %s: %s", agent_id, msg.format_prefix())

    @property
    def running_agents(self) -> list[str]:
        """List of currently running agent IDs."""
        return [
            aid for aid in self._processes
            if self.is_running(aid)
        ]
