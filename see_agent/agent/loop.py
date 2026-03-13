"""Main agent loop -- orchestrates eye, brain, and hand modules.

The :class:`AgentLoop` drives a single task from start to finish:

1. Capture an initial screenshot.
2. Build a :class:`ConversationContext` with the system prompt and screenshot.
3. Repeatedly ask the LLM for the next action, execute it via the
   :class:`ToolRegistry`, capture a fresh screenshot, and feed everything
   back into the context.
4. Stop when the LLM calls ``finished``, the step budget is exhausted, or
   consecutive errors exceed the safety limit.
"""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Awaitable, Callable

from see_agent.agent.context import ConversationContext
from see_agent.brain.base import BaseBrain, BrainResponse
from see_agent.eye.base import BaseEye
from see_agent.eye.scaling import find_target_resolution, scale_screenshot, scale_tool_args
from see_agent.hand.tool import ToolRegistry, ToolResult
from see_agent.session.store import Session, SessionStore

if TYPE_CHECKING:
    from pathlib import Path

    from see_agent.eye.base import Screenshot

logger = logging.getLogger(__name__)

# -------------------------------------------------------------------- #
# Public data types
# -------------------------------------------------------------------- #

MAX_CONSECUTIVE_ERRORS = 3
NO_PROGRESS_LIMIT = 3
REPEAT_WARN_LIMIT = 3
REPEAT_ABORT_LIMIT = 5
MAX_STEPS_WITHOUT_SCREENSHOT = 5


@dataclass
class StepEvent:
    """Snapshot of a single agent step, emitted via the *on_step* callback."""

    step: int
    max_steps: int
    thought: str
    tool_name: str
    tool_args: dict[str, Any]
    tool_result: str
    screenshot_path: str | None
    wait_ms: int = 0
    screen_tool_args: dict[str, Any] | None = None
    """When coordinate scaling is active, this holds the screen-space
    coordinates actually sent to pyautogui.  ``tool_args`` contains the
    original model-space coordinates from the LLM."""


@dataclass
class RunResult:
    """Result returned by :meth:`AgentLoop.run`."""

    summary: str
    task_dir: str
    total_steps: int
    elapsed_seconds: float
    success: bool = True
    session_id: str = ""


StepCallback = Callable[[StepEvent], Awaitable[None]]
"""Async callback invoked after each successful tool-execution step."""

UserInputCallback = Callable[[str], Awaitable[str]]
"""Async callback invoked when the agent calls ``call_user``.

Receives the question string and must return the user's reply.
"""


# -------------------------------------------------------------------- #
# Agent loop
# -------------------------------------------------------------------- #


class AgentLoop:
    """Drives a single task to completion by coordinating eye, brain, and hand.

    Parameters:
        brain: LLM backend used for reasoning.
        eye: Screen-capture backend.
        registry: Registry of available tools (already populated).
        config: Application configuration dict (see ``src/config.py``).
        on_step: Optional async callback fired after each tool-execution step.
        on_user_input: Optional async callback for ``call_user`` interactions.
        mcp_manager: Optional MCP manager for external tool servers.
    """

    def __init__(
        self,
        brain: BaseBrain,
        eye: BaseEye,
        registry: ToolRegistry,
        config: dict[str, Any],
        on_step: StepCallback | None = None,
        on_user_input: UserInputCallback | None = None,
        mcp_manager: Any | None = None,
        agent_id: str | None = None,
        session_dir: "Path | None" = None,
        owner_display: str | None = None,
        task_board: Any | None = None,
    ) -> None:
        self._brain = brain
        self._eye = eye
        self._registry = registry
        self._config = config
        self._on_step = on_step
        self._on_user_input = on_user_input
        self._mcp_manager = mcp_manager
        self._mcp_connected = False
        self._agent_id = agent_id
        self._session_dir = session_dir
        self._owner_display = owner_display
        self._task_board = task_board
        self._active_ctx: ConversationContext | None = None
        self._inject_queue: list[Any] = []
        self._compact_warned: bool = False

        # Configurable knobs with sensible defaults.
        agent_cfg = config.get("agent", {})
        screen_cfg = config.get("screen", {})
        self._max_steps: int = int(agent_cfg.get("max_steps", 50))
        self._max_images: int = int(screen_cfg.get("max_images", 5))
        self._screenshot_interval_ms: int = int(
            screen_cfg.get("screenshot_interval_ms", 800)
        )
        self._tool_delay_ms: int = int(screen_cfg.get("tool_delay_ms", 200))
        self._scaling_enabled: bool = bool(screen_cfg.get("scaling_enabled", True))
        self._scaling_match: str = str(screen_cfg.get("scaling_match", "aspect_ratio"))

    # ------------------------------------------------------------------ #
    # Scaling helper
    # ------------------------------------------------------------------ #

    def _fail_result(
        self, session: Session, steps: int, t0: float, summary: str,
        ctx: ConversationContext | None = None,
    ) -> RunResult:
        """Build a failed :class:`RunResult` and update session meta."""
        elapsed = time.monotonic() - t0
        session.update_meta(
            status="failed", total_steps=steps,
            elapsed_seconds=round(elapsed, 1), summary=summary,
        )
        return RunResult(
            summary=summary,
            task_dir=str(session.dir),
            total_steps=steps,
            elapsed_seconds=elapsed,
            success=False,
            session_id=session.id,
        )

    def _maybe_scale(self, screenshot: Screenshot) -> Screenshot:
        """Resize *screenshot* for the LLM if scaling is enabled."""
        if not self._scaling_enabled:
            return screenshot

        target = find_target_resolution(
            screenshot.width, screenshot.height, self._scaling_match,
        )
        if target is None:
            return screenshot
        return scale_screenshot(screenshot, target)

    # Tools that interact with the screen and need exclusive access.
    _SCREEN_TOOLS = frozenset({
        "screenshot", "click", "type_text", "scroll", "drag", "hotkey",
    })

    async def _execute_with_lock(
        self, name: str, args: dict[str, Any],
    ) -> ToolResult:
        """Execute a tool."""
        return await self._registry.execute(name, args)

    @staticmethod
    def _estimate_tokens(messages: list[dict[str, Any]]) -> int:
        """Rough token estimate: chars/4 for text, 765 per image."""
        total = 0
        for msg in messages:
            content = msg.get("content")
            if isinstance(content, str):
                total += len(content) // 4
            elif isinstance(content, list):
                for part in content:
                    if isinstance(part, dict):
                        if part.get("type") == "text":
                            total += len(part.get("text", "")) // 4
                        elif part.get("type") == "image_url":
                            total += 765
            # tool_calls arguments
            for tc in msg.get("tool_calls", []):
                func = tc.get("function", {})
                total += len(func.get("arguments", "")) // 4
        return total

    async def _maybe_compact(
        self, ctx: ConversationContext, session: Session,
    ) -> None:
        """Compact conversation if over threshold (always on)."""
        compact_cfg = self._config.get("agent", {}).get("compact", {})
        context_window = compact_cfg.get("context_window", 200000)
        target_ratio = compact_cfg.get("target_ratio", 0.75)
        threshold = int(context_window * target_ratio)

        messages = ctx.get_messages()
        estimated = self._estimate_tokens(messages)
        if estimated < threshold:
            return

        keep_recent = compact_cfg.get("keep_recent", 8)
        if len(messages) <= keep_recent + 2:
            return  # Not enough to compact.

        # First hit: warn the agent so it can save important info.
        if not self._compact_warned:
            self._compact_warned = True
            ctx.add_system_hint(
                "[系统提示] 上下文即将达到窗口上限，"
                "请立即用 write_memory 保存重要信息，"
                "下一轮将执行上下文压缩。"
            )
            logger.info(
                "Compact warning issued: ~%d tokens (threshold %d)",
                estimated, threshold,
            )
            return

        # Second hit: actually compact.
        self._compact_warned = False
        logger.info(
            "Context compaction triggered: ~%d tokens (threshold %d)",
            estimated, threshold,
        )

        old_messages = messages[1:-keep_recent]  # skip system, keep recent
        try:
            summary = await self._brain.summarize(old_messages)
        except NotImplementedError:
            logger.warning("Brain does not support summarize(), skipping compaction")
            return
        except Exception:
            logger.warning("Summarization failed, skipping compaction", exc_info=True)
            return

        ctx.apply_compaction(summary, keep_recent=keep_recent)

        # Determine first_kept_msg_id: the msg_counter at the time of
        # compaction minus the number of kept messages gives the cutoff.
        first_kept_msg_id = max(session._msg_counter - keep_recent, 0)

        # Persist compact marker to JSONL.
        session.append_message({
            "type": "compact",
            "summary": summary,
            "first_kept_msg_id": first_kept_msg_id,
        })
        logger.info("Compaction complete: summary length=%d", len(summary))

    async def _auto_complete_tasks(self, summary: str) -> None:
        """Auto-mark claimed/in_progress tasks for this agent as done."""
        if self._task_board is None or self._agent_id is None:
            return
        try:
            if hasattr(self._task_board, "async_list_tasks"):
                tasks = await self._task_board.async_list_tasks()
            else:
                tasks = self._task_board.list_tasks()
            for t in tasks:
                if (
                    t.assigned_to == self._agent_id
                    and t.status in ("claimed", "in_progress")
                ):
                    if hasattr(self._task_board, "async_complete_task"):
                        await self._task_board.async_complete_task(
                            t.id, self._agent_id, result=summary,
                        )
                    else:
                        self._task_board.complete_task(
                            t.id, self._agent_id, result=summary,
                        )
        except Exception:
            logger.warning("Auto-complete tasks failed", exc_info=True)

    # ------------------------------------------------------------------ #
    # Public entry point
    # ------------------------------------------------------------------ #

    async def run(self, task: str, session_id: str | None = None) -> RunResult:
        """Execute *task* and return a :class:`RunResult`.

        Parameters:
            task: The user-facing task description.
            session_id: Optional existing session to resume.  When ``None``
                a new session is created automatically.

        Returns:
            A :class:`RunResult` containing the summary, task directory,
            total steps executed, and elapsed wall-clock time.
        """
        t0 = time.monotonic()

        # ── 0. Connect MCP servers (lazy, first run only) ────────────
        if self._mcp_manager is not None and not self._mcp_connected:
            try:
                await self._mcp_manager.connect_all()
                await self._mcp_manager.register_tools(self._registry)
                self._mcp_connected = True
            except Exception:
                logger.warning("MCP connection failed", exc_info=True)

        # ── 1. Create or load session ─────────────────────────────────
        if self._session_dir is None:
            raise ValueError("session_dir is required to run AgentLoop")

        if session_id:
            session = SessionStore.load(self._session_dir)
            session.update_meta(status="running")
        else:
            session = SessionStore.create(
                task, self._config, session_dir=self._session_dir,
            )

        task_dir = session.screenshots_dir
        task_dir.mkdir(parents=True, exist_ok=True)
        logger.info("Session %s — task dir: %s", session.id, task_dir)

        # ── 2. Initial screenshot (skipped for text-only agents) ──────
        has_screen_tools = bool(
            self._SCREEN_TOOLS & set(self._registry._tools)
        )

        if has_screen_tools:
            screenshot = await self._eye.capture()
            scaled = self._maybe_scale(screenshot)
        else:
            screenshot = None
            scaled = None

        # Determine step numbering: continue from last step on resume.
        if session_id:
            next_step = session.next_step_number()
        else:
            next_step = 0

        if scaled is not None:
            initial_path = task_dir / f"step_{next_step:03d}.webp"
            scaled.save(initial_path)

        # ── 2b. Collect desktop environment info (cached if available) ─
        env_block = self._config.get("_cached_env_block", "")
        if not env_block and has_screen_tools:
            from see_agent.agent.environment import collect_environment

            try:
                env_block = await collect_environment(
                    screenshot.width, screenshot.height,  # type: ignore[union-attr]
                )
            except Exception:
                logger.warning("Failed to collect environment info", exc_info=True)
                env_block = ""

        # ── 3. Build conversation context ─────────────────────────────
        from see_agent.brain.prompts import build_system_prompt
        from see_agent.skill.loader import gate_skills, load_skills

        # Load skills
        skills_dirs = self._config.get("skills", {}).get("dirs", [])
        skills = load_skills(skills_dirs) if skills_dirs else []
        if skills:
            skills = gate_skills(skills)

        # Derive agent_dir for workspace file injection.
        agent_dir = None
        if self._agent_id:
            from see_agent.config import AGENTS_DIR

            candidate = AGENTS_DIR / self._agent_id
            if candidate.is_dir():
                agent_dir = candidate

        system_prompt = build_system_prompt(
            self._config,
            skills=skills or None,
            team_context=self._config.get("_team_context", ""),
            agent_dir=agent_dir,
        )
        session.log_system_prompt(system_prompt)
        session.setup_logging()

        if session_id:
            # Resume: restore full conversation history from JSONL + screenshots.
            ctx = session.restore_context(
                system_prompt,
                max_images=self._max_images,
                on_append=session.append_message,
            )
            task_text = f"{env_block}\n\n{task}" if env_block else task
            if scaled is not None:
                ctx.add_user_task(
                    task_text, scaled.base64, scaled.detail,
                    mime_type=scaled.mime_type,
                    screenshot_ref=f"step_{next_step:03d}.webp",
                )
            else:
                ctx.add_user_task_text_only(task_text)
        else:
            # New session: fresh context.
            ctx = ConversationContext(
                system_prompt,
                max_images=self._max_images,
                on_append=session.append_message,
            )
            task_text = f"{env_block}\n\n{task}" if env_block else task
            if scaled is not None:
                ctx.add_user_task(
                    task_text, scaled.base64, scaled.detail,
                    mime_type=scaled.mime_type,
                    screenshot_ref=f"step_{next_step:03d}.webp",
                )
            else:
                ctx.add_user_task_text_only(task_text)

        # Step offset for screenshot naming in the main loop.
        step_offset = next_step

        # ── 4. Main loop ──────────────────────────────────────────────
        try:
            return await self._run_loop(
                session, ctx, scaled, step_offset, t0,
            )
        except Exception:
            raise
        finally:
            session.teardown_logging()

    # ── Persistent session support ────────────────────────────────────

    _active_session: "Session | None" = None

    async def _ensure_session(self) -> None:
        """Ensure session directory exists. Only creates files on first call."""
        if self._active_session is not None:
            return

        if self._session_dir is None:
            raise ValueError("session_dir is required")

        # MCP (lazy, first run only)
        if self._mcp_manager is not None and not self._mcp_connected:
            try:
                await self._mcp_manager.connect_all()
                await self._mcp_manager.register_tools(self._registry)
                self._mcp_connected = True
            except Exception:
                logger.warning("MCP connection failed", exc_info=True)

        # Create or reuse session directory.
        meta_path = self._session_dir / "meta.json"
        if meta_path.exists():
            session = SessionStore.load(self._session_dir)
            session.update_meta(status="running")
            logger.info("Resumed session at %s", self._session_dir)
        else:
            session = SessionStore.create(
                "conversation", self._config, session_dir=self._session_dir,
            )
            logger.info("Created session at %s", self._session_dir)

        session.setup_logging()
        self._active_session = session

    def _build_system_prompt(self) -> str:
        """Build system prompt fresh (hot-reload every turn)."""
        from see_agent.brain.prompts import build_system_prompt
        from see_agent.config import AGENTS_DIR
        from see_agent.skill.loader import gate_skills, load_skills

        skills_dirs = self._config.get("skills", {}).get("dirs", [])
        skills = load_skills(skills_dirs) if skills_dirs else []
        if skills:
            skills = gate_skills(skills)

        agent_dir = None
        if self._agent_id:
            candidate = AGENTS_DIR / self._agent_id
            if candidate.is_dir():
                agent_dir = candidate

        return build_system_prompt(
            self._config,
            skills=skills or None,
            team_context=self._config.get("_team_context", ""),
            agent_dir=agent_dir,
        )

    async def run_one_turn(
        self,
        messages: list[Any] | None = None,
        inject_queue: list[Any] | None = None,
        drain_interrupts: Any | None = None,
    ) -> None:
        """Process incoming messages with a full ReAct loop.

        Args:
            messages: Batch of messages to process.
            inject_queue: (legacy) List ref for steer injection.
            drain_interrupts: Callable() -> list[Message] that polls inbox
                        for new steer messages. Called before each LLM step.

        1. Ensure session exists.
        2. Rebuild system prompt (hot-reload).
        3. Append user messages to context.
        4. ReAct loop: LLM -> tool -> LLM -> ... until no more tool_calls.
        """
        self._inject_queue = inject_queue or []
        self._drain_interrupts = drain_interrupts
        logger.debug(
            "run_one_turn: drain_interrupts=%s",
            "SET" if drain_interrupts else "NONE",
        )

        if not messages:
            return

        # 1. Session
        await self._ensure_session()
        session = self._active_session
        assert session is not None

        # 2. System prompt (hot-reload every turn)
        system_prompt = self._build_system_prompt()

        # 3. Build/update context
        # Append each message individually to context (not merged).
        if self._active_ctx is None:
            self._active_ctx = ConversationContext(
                system_prompt,
                max_images=self._max_images,
                on_append=session.append_message,
            )
            for msg in messages:
                if hasattr(msg, "format_prefix"):
                    text = f"{msg.format_prefix()}: {msg.content}"
                    self._active_ctx.add_user_task_text_only(
                        text, sender=msg.sender, priority=msg.priority,
                    )
                else:
                    self._active_ctx.add_user_task_text_only(str(msg))
        else:
            self._active_ctx.update_system_prompt(system_prompt)
            for msg in messages:
                if hasattr(msg, "format_prefix"):
                    text = f"{msg.format_prefix()}: {msg.content}"
                    self._active_ctx.add_user_reply(
                        text, sender=msg.sender, priority=msg.priority,
                    )
                else:
                    self._active_ctx.add_user_reply(str(msg))

        # 4. ReAct loop
        tools_schema = self._registry.get_openai_schemas()

        for step in range(self._max_steps):
            logger.info("=== ReAct step %d / %d ===", step + 1, self._max_steps)

            # Poll for interrupt (steer) messages before each LLM call.
            if self._drain_interrupts:
                steer_msgs = self._drain_interrupts()
                logger.debug(
                    "drain_interrupts returned %d message(s)", len(steer_msgs),
                )
                for inj in steer_msgs:
                    content = inj.content if hasattr(inj, "content") else str(inj)
                    prefix = inj.format_prefix() if hasattr(inj, "format_prefix") else ""
                    text = f"{prefix}: {content}" if prefix else content
                    self._active_ctx.add_user_reply(
                        text,
                        sender=getattr(inj, "sender", "user"),
                        priority=getattr(inj, "priority", "steer"),
                    )
                    logger.info("Steer message consumed: %s", prefix)

            # Legacy inject queue (for tests / backward compat).
            while self._inject_queue:
                inj = self._inject_queue.pop(0)
                content = inj.content if hasattr(inj, "content") else str(inj)
                prefix = inj.format_prefix() if hasattr(inj, "format_prefix") else ""
                self._active_ctx.add_user_reply(f"{prefix}: {content}" if prefix else content)

            # Call LLM.
            try:
                response = await self._brain.chat(
                    self._active_ctx.get_messages(), tools_schema,
                )
            except Exception:
                logger.exception("LLM call failed at step %d", step + 1)
                break  # Break ReAct loop, back to idle. Do NOT exit process.

            # Add assistant response to context (on_append writes to JSONL).
            self._active_ctx.add_assistant(response.raw)

            # If no tool calls, LLM is done — but check for new steer
            # messages that arrived during the LLM call.
            if not response.tool_calls:
                has_steer = bool(self._inject_queue)
                if not has_steer and self._drain_interrupts:
                    # Peek: call drain_interrupts to check for new steer.
                    # If found, they'll be consumed at the top of next step.
                    peeked = self._drain_interrupts()
                    if peeked:
                        # Put them into inject queue so next step picks them up.
                        self._inject_queue.extend(peeked)
                        has_steer = True
                if has_steer:
                    logger.info(
                        "LLM finished but steer message(s) pending"
                        " — continuing loop.",
                    )
                    continue
                logger.info("LLM finished (no tool calls). Back to idle.")
                break

            # Execute tools.
            for tc in response.tool_calls:
                logger.info("Tool call: %s", tc.name)
                try:
                    result = await self._registry.execute(tc.name, tc.arguments)
                    result_str = str(result)
                except Exception as exc:
                    logger.exception("Tool %s failed", tc.name)
                    result_str = f"Error: {exc}"

                self._active_ctx.add_tool_result(tc.id, result_str)

                # Check if finished tool was called.
                if tc.name == "finished":
                    logger.info("Finished tool called. Back to idle.")
                    return
        else:
            logger.warning("Max ReAct steps reached (%d). Back to idle.", self._max_steps)

    async def _run_loop(
        self,
        session: Session,
        ctx: ConversationContext,
        scaled: "Screenshot | None",
        step_offset: int,
        t0: float,
    ) -> RunResult:
        """Inner loop extracted for try/finally teardown in :meth:`run`."""
        final_step = 0
        consecutive_errors = 0
        no_progress_count = 0
        last_screenshot_hash: str | None = None
        tools_schema = self._registry.get_openai_schemas()
        repeat_count = 0
        last_action_key: str | None = None
        steps_since_screenshot = 0

        for step in range(1, self._max_steps + 1):
            logger.info("=== Step %d / %d ===", step, self._max_steps)

            # ── 4a. Maybe compact context ────────────────────────────
            await self._maybe_compact(ctx, session)

            # ── 4a2. Drain inject queue (v3.5 steer messages) ────────
            inject_queue = getattr(self, "_inject_queue", None)
            if inject_queue:
                while inject_queue:
                    imsg = inject_queue.pop(0)
                    if hasattr(imsg, "format_prefix"):
                        ctx.add_user_reply(
                            f"{imsg.format_prefix()}: {imsg.content}",
                        )
                    else:
                        ctx.add_user_reply(str(imsg))

            # ── 4b. Ask the LLM ──────────────────────────────────────
            try:
                response: BrainResponse = await self._brain.chat(
                    ctx.get_messages(), tools_schema
                )
                consecutive_errors = 0
            except Exception as exc:
                consecutive_errors += 1
                logger.exception(
                    "Brain error (%d/%d)",
                    consecutive_errors,
                    MAX_CONSECUTIVE_ERRORS,
                )
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS:
                    logger.error(
                        "Max consecutive errors reached -- aborting task"
                    )
                    return self._fail_result(
                        session, final_step, t0,
                        f"Error: max consecutive LLM errors reached. (last: {exc})",
                        ctx=ctx,
                    )
                continue

            thought = response.content or ""
            if thought:
                logger.info("Thought: %s", thought[:200])

            # ── 4c. No tool calls -- LLM wants to stop ──────────────
            if not response.tool_calls:
                logger.info("No tool calls returned -- ending loop")
                ctx.add_assistant(response.raw)
                summary = thought or "Task completed (no tool calls)."
                final_step = step
                elapsed = time.monotonic() - t0
                session.update_meta(
                    status="completed", total_steps=final_step,
                    elapsed_seconds=round(elapsed, 1), summary=summary,
                )

                return RunResult(
                    summary=summary,
                    task_dir=str(session.dir),
                    total_steps=final_step,
                    elapsed_seconds=elapsed,
                    session_id=session.id,
                )

            # Append the full assistant message (may include text + tool_calls).
            ctx.add_assistant(response.raw)

            # ── 4d. Execute all tool calls serially ──────────────────

            for tc in response.tool_calls:
                logger.info("Tool call: %s(%s)", tc.name, tc.arguments)

                # ── Handle "finished" ────────────────────────────────
                if tc.name == "finished":
                    summary = tc.arguments.get("summary", "Task completed.")
                    ctx.add_tool_result(tc.id, summary)
                    logger.info("Task finished: %s", summary)
                    await self._auto_complete_tasks(summary)
                    final_step = step
                    elapsed = time.monotonic() - t0
                    session.update_meta(
                        status="completed", total_steps=final_step,
                        elapsed_seconds=round(elapsed, 1), summary=summary,
                    )

                    return RunResult(
                        summary=summary,
                        task_dir=str(session.dir),
                        total_steps=final_step,
                        elapsed_seconds=elapsed,
                        session_id=session.id,
                    )

                # ── Handle "call_user" ───────────────────────────────
                if tc.name == "call_user":
                    question = tc.arguments.get("question", "")
                    logger.info("call_user: %s", question)

                    if self._on_user_input is not None:
                        user_reply = await self._on_user_input(question)
                    else:
                        user_reply = "已处理，请继续"

                    ctx.add_tool_result(tc.id, f"User replied: {user_reply}")
                    ctx.add_user_reply(user_reply)
                    continue

                # ── Scale coordinates & show overlay, then execute ────
                exec_args = tc.arguments
                has_scale_info = (
                    scaled is not None
                    and scaled.screen_width is not None
                    and scaled.screen_height is not None
                )
                if has_scale_info:
                    assert scaled is not None  # guarded above
                    assert scaled.screen_width is not None
                    assert scaled.screen_height is not None
                    try:
                        exec_args = scale_tool_args(
                            tc.name, tc.arguments,
                            scaled.width, scaled.height,
                            scaled.screen_width, scaled.screen_height,
                        )
                    except (ValueError, TypeError) as exc:
                        consecutive_errors += 1
                        err_result = f"Error: invalid coordinate — {exc}"
                        logger.warning(
                            "Coordinate scaling failed (%d/%d): %s",
                            consecutive_errors,
                            MAX_CONSECUTIVE_ERRORS,
                            exc,
                        )
                        ctx.add_tool_result(tc.id, err_result)
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS:
                            return self._fail_result(
                                session, final_step, t0,
                                f"Error: max consecutive errors reached. (last: {exc})",
                                ctx=ctx,
                            )
                        continue
                    if exec_args != tc.arguments:
                        logger.info(
                            "Scaled args: %s -> %s", tc.arguments, exec_args,
                        )

                try:
                    result: ToolResult = await self._execute_with_lock(
                        tc.name, exec_args,
                    )
                    consecutive_errors = 0
                except Exception as exc:
                    consecutive_errors += 1
                    result = ToolResult(text=f"Error: {exc}")
                    logger.exception(
                        "Tool execution error (%d/%d)",
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS,
                    )
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS:
                        logger.error(
                            "Max consecutive tool errors -- aborting task"
                        )
                        return self._fail_result(
                            session, final_step, t0,
                            f"Error: max consecutive tool errors reached. (last: {exc})",
                            ctx=ctx,
                        )

                # Save tool-returned images to disk
                shot_path: str | None = None
                for img in result.images:
                    shot_num = step_offset + step
                    img_path = session.screenshots_dir / f"step_{shot_num:03d}.webp"
                    import base64 as b64mod
                    img_path.parent.mkdir(parents=True, exist_ok=True)
                    img_path.write_bytes(b64mod.b64decode(img.base64))
                    shot_path = str(img_path)

                # Add tool result to context (images auto-injected via ToolResult)
                ctx.add_tool_result(
                    tc.id, result,
                    screenshot_ref=f"step_{step_offset + step:03d}.webp" if shot_path else None,
                )

                # Track screenshot presence for no-screenshot warning
                if result.images:
                    steps_since_screenshot = 0
                    # No-progress detection based on screenshot hash
                    current_hash = _screenshot_hash(result.images[0].base64)
                    if current_hash == last_screenshot_hash:
                        no_progress_count += 1
                        logger.warning(
                            "No progress detected (%d/%d)",
                            no_progress_count,
                            NO_PROGRESS_LIMIT,
                        )
                        if no_progress_count >= NO_PROGRESS_LIMIT:
                            ctx.add_system_hint(
                                "Warning: the screen has not changed for "
                                f"{no_progress_count} consecutive steps. "
                                "Please re-analyse the current state and try a "
                                "completely different strategy."
                            )
                            no_progress_count = 0
                    else:
                        no_progress_count = 0
                    last_screenshot_hash = current_hash
                else:
                    steps_since_screenshot += 1

                # Repeated-action detection
                action_key = _action_key(tc.name, tc.arguments)
                if action_key == last_action_key:
                    repeat_count += 1
                    logger.warning(
                        "Repeated action (%d): %s", repeat_count, action_key
                    )
                    if repeat_count >= REPEAT_ABORT_LIMIT:
                        logger.error("Aborting: agent stuck in repeated action loop")
                        return self._fail_result(
                            session, step, t0,
                            "Aborted: agent stuck in repeated action loop.",
                            ctx=ctx,
                        )
                    if repeat_count >= REPEAT_WARN_LIMIT:
                        ctx.add_system_hint(
                            f"Warning: you have repeated the same action "
                            f"{repeat_count} times with no visible change. "
                            "Try a different approach or use call_user to ask "
                            "the user for help."
                        )
                else:
                    repeat_count = 1
                last_action_key = action_key

                # Fire step callback
                final_step = step
                if self._on_step is not None:
                    event = StepEvent(
                        step=step,
                        max_steps=self._max_steps,
                        thought=thought,
                        tool_name=tc.name,
                        tool_args=tc.arguments,
                        tool_result=result.text,
                        screenshot_path=shot_path,
                        wait_ms=self._tool_delay_ms,
                        screen_tool_args=exec_args if exec_args != tc.arguments else None,
                    )
                    try:
                        await self._on_step(event)
                    except Exception:
                        logger.exception("on_step callback error (non-fatal)")

                # Inter-tool delay
                if self._tool_delay_ms > 0:
                    await asyncio.sleep(self._tool_delay_ms / 1000.0)

            # ── 4e. No-screenshot warning ────────────────────────────
            if steps_since_screenshot >= MAX_STEPS_WITHOUT_SCREENSHOT:
                ctx.add_system_hint(
                    f"Hint: you have not taken a screenshot for "
                    f"{steps_since_screenshot} steps. Consider calling "
                    "the screenshot tool to check the current screen state."
                )
                steps_since_screenshot = 0

        # ── 5. Budget exhausted ───────────────────────────────────────
        logger.warning("Max steps (%d) reached", self._max_steps)
        return self._fail_result(
            session, final_step, t0,
            f"Max steps ({self._max_steps}) reached. Task may be incomplete.",
            ctx=ctx,
        )


# -------------------------------------------------------------------- #
# Helpers
# -------------------------------------------------------------------- #


def _screenshot_hash(b64: str) -> str:
    """Return a fast, non-cryptographic hash of a screenshot's base64 data.

    Only the first 1000 characters are hashed to keep the comparison cheap
    while still being sensitive to any visible change on screen.
    """
    return str(hash(b64[:1000]))



def _action_key(tool_name: str, args: dict[str, Any]) -> str:
    """Return a normalised key for an action, treating nearby coordinates as equal.

    Coordinate values (``x``, ``y``) are rounded to the nearest 10 so that
    clicks at (822, 91) and (818, 95) are considered the same action.
    """
    normalised: dict[str, Any] = {}
    for k, v in sorted(args.items()):
        if k in ("x", "y") and isinstance(v, (int, float)):
            v = round(v / 10) * 10
        normalised[k] = v
    return f"{tool_name}:{normalised}"

