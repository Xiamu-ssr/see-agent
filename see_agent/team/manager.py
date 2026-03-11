"""TeamManager — orchestrates multi-agent team execution.

v3.1: Each agent runs in an independent subprocess communicating with
the main process via Unix Domain Socket (UDS).  The main process runs
an AgentRouter that manages the TeamBus, TaskBoard, and ScreenManager.
"""

from __future__ import annotations

import asyncio
import json
import logging
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from see_agent.agent.definition import AgentDefinition
from see_agent.agent.loop import RunResult
from see_agent.config import TEAMS_DIR
from see_agent.ipc.router import AgentRouter
from see_agent.team.definition import TeamDefinition

logger = logging.getLogger(__name__)


@dataclass
class TeamRunResult:
    """Result from a full team run."""

    team_id: str
    agent_results: dict[str, RunResult] = field(default_factory=dict)
    success: bool = True
    summary: str = ""


class TeamManager:
    """Orchestrate a team of agents working on a shared task.

    v3.1 architecture: agents run as independent subprocesses connected
    to an AgentRouter via UDS.  The main process manages shared state
    (bus, board, screen) and delegates execution to worker processes.

    Parameters:
        team_def: The team definition.
        global_config: The global application config dict.
    """

    def __init__(
        self,
        team_def: TeamDefinition,
        global_config: dict[str, Any],
    ) -> None:
        self._team_def = team_def
        self._global_config = global_config
        self._team_dir = TEAMS_DIR / team_def.id
        self._team_dir.mkdir(parents=True, exist_ok=True)

        # Create team-level subdirectories.
        (self._team_dir / "shared").mkdir(exist_ok=True)

        self._router: AgentRouter | None = None
        self._processes: dict[str, subprocess.Popen[bytes]] = {}
        self._stopped = False

    # ------------------------------------------------------------------ #
    # Public API
    # ------------------------------------------------------------------ #

    async def run(self, task: str) -> TeamRunResult:
        """Execute *task* with the team.

        1. Start AgentRouter (UDS server with bus, board, screen).
        2. Spawn agent subprocesses.
        3. Wait for completion and collect results.
        4. Clean up.
        """
        logger.info("Team %s starting task: %s", self._team_def.id, task[:100])
        self._team_def.status = "running"
        self._team_def.save()

        # 1. Start AgentRouter.
        router = AgentRouter(self._team_def.id)
        self._router = router

        # Register owner on bus if configured.
        if self._team_def.owner:
            router.register_agent("owner")

        # Register all agents on the bus.
        for agent_id in self._team_def.members:
            router.register_agent(agent_id)

        # Create initial task on the board.
        router.board.create_task(
            title=task, description=task, created_by="system",
        )

        await router.start()

        try:
            # Collect environment info once.
            await self._cache_environment()

            leader_id = self._team_def.leader
            results: dict[str, RunResult] = {}

            # Phase 1: Run leader first (if any) to decompose tasks.
            if leader_id and leader_id in self._team_def.members:
                result = await self._run_agent_subprocess(
                    leader_id, task, router.sock_path,
                )
                results[leader_id] = result

            # Phase 2: Run workers concurrently.
            worker_ids = [
                aid for aid in self._team_def.members
                if aid != leader_id
            ]
            if worker_ids:
                worker_tasks = [
                    self._run_agent_subprocess(
                        aid, task, router.sock_path,
                    )
                    for aid in worker_ids
                ]
                worker_results = await asyncio.gather(
                    *worker_tasks, return_exceptions=True,
                )
                for aid, res in zip(worker_ids, worker_results):
                    if isinstance(res, BaseException):
                        logger.exception("Agent %s failed", aid)
                        results[aid] = RunResult(
                            summary=f"Agent {aid} crashed: {res}",
                            task_dir="",
                            total_steps=0,
                            elapsed_seconds=0,
                            success=False,
                        )
                    else:
                        results[aid] = res

            # Determine overall success.
            success = all(r.success for r in results.values())
            self._team_def.status = "completed" if success else "failed"
            self._team_def.save()
            logger.info(
                "Team %s finished: success=%s, agents=%d",
                self._team_def.id, success, len(results),
            )

            summaries = [
                f"{aid}: {r.summary}" for aid, r in results.items()
            ]
            return TeamRunResult(
                team_id=self._team_def.id,
                agent_results=results,
                success=success,
                summary="\n".join(summaries),
            )

        finally:
            await router.stop()
            self._router = None

    async def stop(self) -> None:
        """Stop all agent subprocesses and the router."""
        self._stopped = True
        for agent_id, proc in self._processes.items():
            logger.info("Terminating agent %s (pid %d)", agent_id, proc.pid)
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
        self._processes.clear()

        if self._router is not None:
            await self._router.stop()
            self._router = None

        self._team_def.status = "stopped"
        self._team_def.save()

    # ------------------------------------------------------------------ #
    # Internal — subprocess management
    # ------------------------------------------------------------------ #

    async def _run_agent_subprocess(
        self, agent_id: str, task: str, sock_path: Path,
    ) -> RunResult:
        """Spawn and wait for an agent subprocess."""
        config = self._build_agent_config(agent_id)

        # Write config to temp file.
        agent_base = self._team_dir / "agents" / agent_id
        for subdir in ("sessions", "workspace", "memory", "logs"):
            (agent_base / subdir).mkdir(parents=True, exist_ok=True)

        config_path = agent_base / "worker_config.json"
        result_path = agent_base / "worker_result.json"

        # Load agent definition.
        agent_def: AgentDefinition | None = None
        try:
            agent_def = AgentDefinition.load(agent_id)
        except FileNotFoundError:
            pass

        # Add subprocess-specific keys.
        config["_agent_id"] = agent_id
        config["_session_root"] = str(agent_base / "sessions")
        config["_memory_dir"] = str(agent_base / "memory")
        config["_result_path"] = str(result_path)
        config["_leader_id"] = self._team_def.leader

        # Determine screen_access from agent sandbox config.
        sandbox_cfg: dict[str, Any] = {}
        if agent_def:
            sandbox_cfg = agent_def.sandbox or {}
        config["_screen_access"] = sandbox_cfg.get("screen_access", True)

        owner_display: str | None = None
        if self._team_def.owner:
            owner_display = self._team_def.owner.get("display", "Owner")
        config["_owner_display"] = owner_display

        # Inject team context.
        config["_team_context"] = self._build_team_context(agent_id)

        # Collect denied tools.
        denied: list[str] = []
        if agent_def and agent_def.tools_config:
            denied = agent_def.tools_config.get("denied", [])
        config["_denied_tools"] = denied

        config_path.write_text(json.dumps(config, ensure_ascii=False))

        # Remove stale result file.
        result_path.unlink(missing_ok=True)

        # Spawn subprocess.
        cmd = [
            sys.executable, "-m", "see_agent.agent.worker",
            str(config_path), str(sock_path), task,
        ]

        # Wrap with sandbox-exec if enabled.
        if sandbox_cfg.get("enabled", False):
            profile_path = self._generate_sandbox_profile(
                agent_id, sandbox_cfg,
            )
            cmd = ["sandbox-exec", "-f", str(profile_path)] + cmd

        log_path = agent_base / "logs" / "worker.log"
        log_fh = open(log_path, "a")

        proc = subprocess.Popen(
            cmd,
            stdout=log_fh,
            stderr=subprocess.STDOUT,
        )
        self._processes[agent_id] = proc
        logger.info(
            "Spawned agent %s (pid %d)", agent_id, proc.pid,
        )

        # Wait for process in a thread so we don't block the event loop.
        loop = asyncio.get_running_loop()
        returncode = await loop.run_in_executor(None, proc.wait)
        log_fh.close()

        # Clean up.
        self._processes.pop(agent_id, None)

        # Collect result.
        if result_path.exists():
            try:
                data = json.loads(result_path.read_text())
                return RunResult(
                    summary=data.get("summary", ""),
                    task_dir=str(agent_base),
                    total_steps=data.get("total_steps", 0),
                    elapsed_seconds=data.get("elapsed_seconds", 0),
                    success=data.get("success", False),
                    session_id=data.get("session_id", ""),
                )
            except (json.JSONDecodeError, OSError) as exc:
                logger.warning(
                    "Failed to read result for %s: %s", agent_id, exc,
                )

        return RunResult(
            summary=f"Agent {agent_id} exited with code {returncode}",
            task_dir=str(agent_base),
            total_steps=0,
            elapsed_seconds=0,
            success=returncode == 0,
        )

    def _generate_sandbox_profile(
        self, agent_id: str, sandbox_cfg: dict[str, Any],
    ) -> Path:
        """Generate a sandbox-exec profile for an agent subprocess."""
        from see_agent.sandbox.manager import SandboxProfileGenerator

        # Check if agent uses node-based MCP servers.
        agent_def: AgentDefinition | None = None
        try:
            agent_def = AgentDefinition.load(agent_id)
        except FileNotFoundError:
            pass
        has_node_mcp = False
        if agent_def and agent_def.mcp_config:
            # Rough heuristic: any MCP server using npx/node.
            mcp_servers = self._global_config.get("mcp_servers", {})
            for cfg in mcp_servers.values():
                cmd = cfg.get("command", "")
                if "npx" in cmd or "node" in cmd:
                    has_node_mcp = True
                    break

        gen = SandboxProfileGenerator()
        return gen.generate(
            agent_id=agent_id,
            team_id=self._team_def.id,
            team_dir=self._team_dir,
            sandbox_cfg=sandbox_cfg,
            has_node_mcp=has_node_mcp,
        )

    def _build_agent_config(self, agent_id: str) -> dict[str, Any]:
        """Build the config dict for an agent subprocess."""
        from see_agent.config import _deep_merge

        agent_def: AgentDefinition | None = None
        try:
            agent_def = AgentDefinition.load(agent_id)
            config = agent_def.get_merged_config()
        except FileNotFoundError:
            config = self._global_config.copy()

        # Apply team-level overrides.
        if self._team_def.overrides:
            env_overrides = self._team_def.overrides.get("env", {})
            if env_overrides:
                config = _deep_merge(config, env_overrides)
            agent_overrides = self._team_def.overrides.get(agent_id, {})
            if agent_overrides:
                config = _deep_merge(config, agent_overrides)

        # Apply MCP filtering.
        if agent_def and agent_def.mcp_config:
            mcp_cfg = agent_def.mcp_config
            mcp_servers = config.get("mcp_servers", {})
            if mcp_cfg.get("enabled"):
                enabled = set(mcp_cfg["enabled"])
                mcp_servers = {
                    k: v for k, v in mcp_servers.items()
                    if k in enabled
                }
            elif mcp_cfg.get("disabled"):
                disabled = set(mcp_cfg["disabled"])
                mcp_servers = {
                    k: v for k, v in mcp_servers.items()
                    if k not in disabled
                }
            config = {**config, "mcp_servers": mcp_servers}

        return config

    async def _cache_environment(self) -> None:
        """Collect desktop environment info once and store in global config."""
        try:
            from see_agent.agent.environment import collect_environment
            from see_agent.eye.mac import MacEye

            eye = MacEye()
            screenshot = await eye.capture()
            env_block = await collect_environment(
                screenshot.width, screenshot.height,
            )
            self._global_config["_cached_env_block"] = env_block
        except Exception:
            logger.warning(
                "Failed to pre-collect environment info", exc_info=True,
            )

    def _build_team_context(self, agent_id: str) -> str:
        """Build the team context string injected into the system prompt."""
        members_info: list[str] = []
        for mid in self._team_def.members:
            try:
                defn = AgentDefinition.load(mid)
                members_info.append(f"- {defn.name} ({defn.role})")
            except FileNotFoundError:
                members_info.append(f"- {mid} (unknown)")

        leader_name = self._team_def.leader or "none"
        try:
            if self._team_def.leader:
                ldef = AgentDefinition.load(self._team_def.leader)
                leader_name = ldef.name
        except FileNotFoundError:
            pass

        # Current task board summary.
        board = self._router.board if self._router else None
        tasks = board.list_tasks() if board else []
        task_lines = []
        for t in tasks:
            assignee = t.assigned_to or "unassigned"
            task_lines.append(
                f"- [{t.id}] {t.title} ({t.status}, {assignee})"
            )
        board_summary = (
            "\n".join(task_lines) if task_lines else "(empty)"
        )

        is_leader = agent_id == self._team_def.leader
        role_rules = (
            "你是 Team Leader，负责分解任务、分配工作、协调进度。"
            if is_leader
            else "你是 Team Worker，专注执行分配给你的任务。"
        )

        # Owner info.
        owner_info = ""
        if self._team_def.owner:
            display = self._team_def.owner.get("display", "Owner")
            owner_info = f"- Owner: {display}\n"

        return (
            f"## Team Context\n"
            f"- Team: {self._team_def.name}\n"
            f"- Leader: {leader_name}\n"
            f"{owner_info}"
            f"- 队友:\n"
            + "\n".join(f"  {m}" for m in members_info)
            + "\n\n"
            f"## 当前任务列表\n{board_summary}\n\n"
            f"## 协作规则\n"
            f"{role_rules}\n"
            "- 用 send_message 工具和队友沟通\n"
            "- 用 claim_task 领取任务，complete_task 完成任务\n"
            "- 收到队友消息（[teammate xxx]: ...）时优先处理\n"
            "- 用 send_message(to='owner') 向项目负责人汇报\n"
        )
