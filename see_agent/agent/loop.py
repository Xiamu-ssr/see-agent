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
from see_agent.overlay.base import OverlayRenderer
from see_agent.session.store import Session, SessionStore

if TYPE_CHECKING:
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
        overlay: Optional screen overlay renderer for visual feedback.
        memory: Optional memory backend for cross-session knowledge.
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
        overlay: OverlayRenderer | None = None,
        memory: Any | None = None,
        mcp_manager: Any | None = None,
    ) -> None:
        self._brain = brain
        self._eye = eye
        self._registry = registry
        self._config = config
        self._on_step = on_step
        self._on_user_input = on_user_input
        self._overlay = overlay
        self._memory = memory
        self._mcp_manager = mcp_manager
        self._mcp_connected = False

        # Configurable knobs with sensible defaults.
        self._max_steps: int = int(config.get("max_steps", 50))
        self._max_images: int = int(config.get("max_images", 5))
        self._screenshot_interval_ms: int = int(
            config.get("screenshot_interval_ms", 800)
        )
        self._tool_delay_ms: int = int(config.get("tool_delay_ms", 200))
        self._scaling_enabled: bool = bool(config.get("scaling_enabled", True))
        self._scaling_match: str = str(config.get("scaling_match", "aspect_ratio"))

    # ------------------------------------------------------------------ #
    # Scaling helper
    # ------------------------------------------------------------------ #

    def _fail_result(
        self, session: Session, steps: int, t0: float, summary: str,
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

    def _save_memory(self, ctx: ConversationContext, session_id: str) -> None:
        """Persist conversation to memory backend (if configured)."""
        if self._memory is None:
            return
        try:
            messages = _strip_base64(ctx.get_messages())
            self._memory.add(messages, session_id)
        except Exception:
            logger.warning("Memory save failed", exc_info=True)

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
        final_step = 0

        # ── 0. Connect MCP servers (lazy, first run only) ────────────
        if self._mcp_manager is not None and not self._mcp_connected:
            try:
                await self._mcp_manager.connect_all()
                await self._mcp_manager.register_tools(self._registry)
                self._mcp_connected = True
            except Exception:
                logger.warning("MCP connection failed", exc_info=True)

        # ── 1. Create or load session ─────────────────────────────────
        if session_id:
            session = SessionStore.load(session_id)
            session.update_meta(status="running")
        else:
            session = SessionStore.create(task, self._config)

        task_dir = session.screenshots_dir
        task_dir.mkdir(parents=True, exist_ok=True)
        logger.info("Session %s — task dir: %s", session.id, task_dir)

        # ── 2. Initial screenshot ─────────────────────────────────────
        screenshot = await self._eye.capture()
        scaled = self._maybe_scale(screenshot)

        # Determine step numbering: continue from last step on resume.
        if session_id:
            next_step = session.next_step_number()
        else:
            next_step = 0

        initial_path = task_dir / f"step_{next_step:03d}.webp"
        scaled.save(initial_path)

        # ── 2b. Collect desktop environment info ──────────────────────
        from see_agent.agent.environment import collect_environment

        try:
            env_block = await collect_environment(
                screenshot.width, screenshot.height,
            )
        except Exception:
            logger.warning("Failed to collect environment info", exc_info=True)
            env_block = ""

        # ── 3. Build conversation context ─────────────────────────────
        from see_agent.brain.prompts import build_system_prompt
        from see_agent.skill.loader import load_skills

        # Load skills
        skills_dirs = self._config.get("skills_dirs", [])
        skills = load_skills(skills_dirs) if skills_dirs else []

        # Load memory (if available)
        memory_block = ""
        if self._memory is not None:
            try:
                memories = self._memory.search(task, limit=5)
                if memories:
                    memory_block = "\n".join(f"- {m}" for m in memories)
            except Exception:
                logger.warning("Memory search failed", exc_info=True)

        system_prompt = build_system_prompt(
            self._config,
            skills=skills or None,
            memory_block=memory_block,
        )

        if session_id:
            # Resume: restore full conversation history from JSONL + screenshots.
            ctx = session.restore_context(
                system_prompt,
                max_images=self._max_images,
                on_append=session.append_message,
            )
            # Append only the new user task (no duplicate system prompt).
            task_text = f"{env_block}\n\n{task}" if env_block else task
            ctx.add_user_task(
                task_text, scaled.base64, scaled.detail,
                mime_type=scaled.mime_type,
                screenshot_ref=f"step_{next_step:03d}.webp",
            )
        else:
            # New session: fresh context.
            ctx = ConversationContext(
                system_prompt,
                max_images=self._max_images,
                on_append=session.append_message,
            )
            task_text = f"{env_block}\n\n{task}" if env_block else task
            ctx.add_user_task(
                task_text, scaled.base64, scaled.detail,
                mime_type=scaled.mime_type,
                screenshot_ref=f"step_{next_step:03d}.webp",
            )

        # Step offset for screenshot naming in the main loop.
        step_offset = next_step

        # ── 4. Main loop ──────────────────────────────────────────────
        consecutive_errors = 0
        no_progress_count = 0
        last_screenshot_hash: str | None = None
        tools_schema = self._registry.get_openai_schemas()
        repeat_count = 0
        last_action_key: str | None = None
        steps_since_screenshot = 0

        for step in range(1, self._max_steps + 1):
            logger.info("=== Step %d / %d ===", step, self._max_steps)

            # ── 4a. Ask the LLM ──────────────────────────────────────
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
                    )
                continue

            thought = response.content or ""
            if thought:
                logger.info("Thought: %s", thought[:200])

            # ── 4b. No tool calls -- LLM wants to stop ──────────────
            if not response.tool_calls:
                logger.info("No tool calls returned -- ending loop")
                ctx.add_assistant(response.raw)
                break

            # Append the full assistant message (may include text + tool_calls).
            ctx.add_assistant(response.raw)

            # ── 4c. Execute all tool calls serially ──────────────────

            for tc in response.tool_calls:
                logger.info("Tool call: %s(%s)", tc.name, tc.arguments)

                # ── Handle "finished" ────────────────────────────────
                if tc.name == "finished":
                    if self._overlay:
                        _show_overlay(self._overlay, "finished", tc.arguments)
                    summary = tc.arguments.get("summary", "Task completed.")
                    ctx.add_tool_result(tc.id, summary)
                    logger.info("Task finished: %s", summary)
                    final_step = step
                    elapsed = time.monotonic() - t0
                    session.update_meta(
                        status="completed", total_steps=final_step,
                        elapsed_seconds=round(elapsed, 1), summary=summary,
                    )
                    self._save_memory(ctx, session.id)
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

                    if self._overlay:
                        _show_overlay(self._overlay, "call_user", tc.arguments)

                    if self._on_user_input is not None:
                        user_reply = await self._on_user_input(question)
                    else:
                        user_reply = "已处理，请继续"

                    ctx.add_tool_result(tc.id, f"User replied: {user_reply}")
                    ctx.add_user_reply(user_reply)
                    continue

                # ── Scale coordinates & show overlay, then execute ────
                exec_args = tc.arguments
                if scaled.screen_width is not None and scaled.screen_height is not None:
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
                            )
                        continue
                    if exec_args != tc.arguments:
                        logger.info(
                            "Scaled args: %s -> %s", tc.arguments, exec_args,
                        )

                if self._overlay:
                    _show_overlay(self._overlay, tc.name, exec_args)

                try:
                    result: ToolResult = await self._registry.execute(tc.name, exec_args)
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
                        )

                # Save tool-returned images to disk
                shot_path: str | None = None
                for img in result.images:
                    shot_num = step_offset + step
                    img_path = task_dir / f"step_{shot_num:03d}.webp"
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

            # ── 4d. No-screenshot warning ────────────────────────────
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


def _strip_base64(messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Return a copy of *messages* with base64 image data removed.

    Used before persisting to memory to avoid storing large blobs.
    """
    import copy
    import re

    stripped: list[dict[str, Any]] = []
    data_uri_re = re.compile(r"data:[^;]+;base64,[A-Za-z0-9+/=]+")

    for msg in messages:
        msg = copy.deepcopy(msg)
        content = msg.get("content")
        if isinstance(content, list):
            new_parts = []
            for part in content:
                if isinstance(part, dict) and part.get("type") == "image_url":
                    new_parts.append({"type": "text", "text": "[image]"})
                else:
                    new_parts.append(part)
            msg["content"] = new_parts
        elif isinstance(content, str):
            msg["content"] = data_uri_re.sub("[image]", content)
        stripped.append(msg)
    return stripped


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


def _show_overlay(overlay: OverlayRenderer, tool_name: str, args: dict[str, Any]) -> None:
    """Dispatch a visual overlay for the given tool call."""
    try:
        match tool_name:
            case "click":
                overlay.show_click(args["x"], args["y"], args.get("double", False))
            case "type_text":
                overlay.show_type(args["text"])
            case "drag":
                overlay.show_drag(
                    args["start_x"], args["start_y"],
                    args["end_x"], args["end_y"],
                )
            case "scroll":
                overlay.show_scroll(
                    args["x"], args["y"],
                    args["direction"], args.get("amount", 3),
                )
            case "hotkey":
                overlay.show_hotkey(args["keys"])
            case "shell":
                overlay.show_shell(args["command"])
            case "wait":
                overlay.show_wait(args.get("seconds", 2))
            case "screenshot":
                overlay.show_screenshot()
            case "call_user":
                overlay.show_call_user(args.get("question", ""))
            case "finished":
                overlay.show_finished(args.get("summary", ""))
    except Exception:
        logger.exception("Overlay error (non-fatal)")
