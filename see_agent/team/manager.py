"""TeamManager — orchestrates multi-agent team execution."""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass, field
from typing import Any

from see_agent.agent.definition import AgentDefinition
from see_agent.agent.loop import AgentLoop, RunResult
from see_agent.config import TEAMS_DIR
from see_agent.hand.tool import ToolRegistry
from see_agent.team.bus import TeamBus
from see_agent.team.definition import TeamDefinition
from see_agent.team.task_board import TaskBoard

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

        self._bus = TeamBus(self._team_dir)
        self._board = TaskBoard(self._team_dir)
        self._screen_lock = asyncio.Lock()
        self._stopped = False

    # ------------------------------------------------------------------ #
    # Public API
    # ------------------------------------------------------------------ #

    async def run(self, task: str) -> TeamRunResult:
        """Execute *task* with the team.

        Creates the initial task on the board, then runs all agent loops
        concurrently.  Each agent's session is scoped under the team
        directory.
        """
        self._team_def.status = "running"
        self._team_def.save()

        # Create initial task on the board.
        self._board.create_task(
            title=task, description=task, created_by="system",
        )

        # Build and run agent loops.
        loops: dict[str, AgentLoop] = {}
        for agent_id in self._team_def.members:
            self._bus.register(agent_id)
            loops[agent_id] = self._build_agent_loop(agent_id)

        results: dict[str, RunResult] = {}

        async def _run_agent(aid: str, loop: AgentLoop) -> None:
            try:
                # Leader gets the full task; workers get a scoped prompt.
                agent_task = self._build_agent_task(aid, task)
                results[aid] = await loop.run(agent_task)
            except Exception:
                logger.exception("Agent %s failed", aid)
                results[aid] = RunResult(
                    summary=f"Agent {aid} crashed",
                    task_dir="",
                    total_steps=0,
                    elapsed_seconds=0,
                    success=False,
                )

        # Run agents concurrently.
        tasks = [
            asyncio.create_task(_run_agent(aid, loop))
            for aid, loop in loops.items()
        ]
        await asyncio.gather(*tasks, return_exceptions=True)

        # Determine overall success.
        success = all(r.success for r in results.values())
        self._team_def.status = "completed" if success else "failed"
        self._team_def.save()

        summaries = [
            f"{aid}: {r.summary}" for aid, r in results.items()
        ]
        return TeamRunResult(
            team_id=self._team_def.id,
            agent_results=results,
            success=success,
            summary="\n".join(summaries),
        )

    async def stop(self) -> None:
        """Signal all agents to stop."""
        self._stopped = True
        self._team_def.status = "stopped"
        self._team_def.save()

    # ------------------------------------------------------------------ #
    # Internal builders
    # ------------------------------------------------------------------ #

    def _build_agent_loop(self, agent_id: str) -> AgentLoop:
        """Build an :class:`AgentLoop` for a single team member."""
        from see_agent.brain.openai_client import OpenAIBrain
        from see_agent.eye.mac import MacEye
        from see_agent.hand.tools import create_registry

        # Load agent-specific config or fall back to global.
        try:
            agent_def = AgentDefinition.load(agent_id)
            config = agent_def.get_merged_config()
        except FileNotFoundError:
            config = self._global_config.copy()

        llm_cfg = config["llm"]
        eye = MacEye()
        brain = OpenAIBrain(
            base_url=llm_cfg["base_url"],
            api_key=llm_cfg["api_key"],
            model=llm_cfg["model"],
        )
        registry = create_registry(eye)
        self._register_team_tools(registry, agent_id)

        # Agent-scoped sessions directory.
        session_root = (
            self._team_dir / "agents" / agent_id / "sessions"
        )
        session_root.mkdir(parents=True, exist_ok=True)

        # User queue for bus message injection.
        user_queue: asyncio.Queue[str] = asyncio.Queue()

        loop = AgentLoop(
            brain=brain,
            eye=eye,
            registry=registry,
            config=config,
            agent_id=agent_id,
            session_root=session_root,
            user_queue=user_queue,
        )
        return loop

    def _register_team_tools(
        self, registry: ToolRegistry, agent_id: str,
    ) -> None:
        """Register team collaboration tools."""
        from see_agent.hand.tools.team_tools import (
            AssignTaskTool,
            ClaimTaskTool,
            CompleteTaskTool,
            CreateTaskTool,
            ListTasksTool,
            SendMessageTool,
            UpdateTaskTool,
        )

        tools = [
            SendMessageTool(self._bus, agent_id),
            ListTasksTool(self._board),
            CreateTaskTool(self._board, agent_id),
            ClaimTaskTool(self._board, agent_id),
            CompleteTaskTool(self._board, agent_id),
            UpdateTaskTool(self._board),
            AssignTaskTool(self._board),
        ]
        for tool in tools:
            registry.register(tool, source="team")

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

        # Current task board summary
        tasks = self._board.list_tasks()
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

        return (
            f"## Team Context\n"
            f"- Team: {self._team_def.name}\n"
            f"- Leader: {leader_name}\n"
            f"- 队友:\n"
            + "\n".join(f"  {m}" for m in members_info)
            + "\n\n"
            f"## 当前任务列表\n{board_summary}\n\n"
            f"## 协作规则\n"
            f"{role_rules}\n"
            "- 用 send_message 工具和队友沟通\n"
            "- 用 claim_task 领取任务，complete_task 完成任务\n"
            "- 收到队友消息（[teammate xxx]: ...）时优先处理\n"
        )

    def _build_agent_task(self, agent_id: str, task: str) -> str:
        """Build the task string for a single agent, with team context."""
        team_context = self._build_team_context(agent_id)
        return f"{team_context}\n\n## 任务\n{task}"
