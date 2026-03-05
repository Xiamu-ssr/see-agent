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
from datetime import datetime
from typing import TYPE_CHECKING, Any, Awaitable, Callable

from src.agent.context import ConversationContext
from src.brain.base import BaseBrain, BrainResponse
from src.config import SCREENSHOTS_DIR
from src.eye.base import BaseEye
from src.eye.scaling import find_target_resolution, scale_screenshot, scale_tool_args
from src.hand.tool import ToolRegistry
from src.overlay.base import OverlayRenderer

if TYPE_CHECKING:
    from src.eye.base import Screenshot

logger = logging.getLogger(__name__)

# -------------------------------------------------------------------- #
# Public data types
# -------------------------------------------------------------------- #

MAX_CONSECUTIVE_ERRORS = 3
NO_PROGRESS_LIMIT = 3
REPEAT_WARN_LIMIT = 3
REPEAT_ABORT_LIMIT = 5


@dataclass
class StepEvent:
    """Snapshot of a single agent step, emitted via the *on_step* callback."""

    step: int
    max_steps: int
    thought: str
    tool_name: str
    tool_args: dict[str, Any]
    tool_result: str
    screenshot_path: str
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
    ) -> None:
        self._brain = brain
        self._eye = eye
        self._registry = registry
        self._config = config
        self._on_step = on_step
        self._on_user_input = on_user_input
        self._overlay = overlay

        # Configurable knobs with sensible defaults.
        self._max_steps: int = int(config.get("max_steps", 50))
        self._max_images: int = int(config.get("max_images", 5))
        self._screenshot_interval_ms: int = int(
            config.get("screenshot_interval_ms", 800)
        )
        self._scaling_enabled: bool = bool(config.get("scaling_enabled", True))
        self._scaling_match: str = str(config.get("scaling_match", "aspect_ratio"))

    # ------------------------------------------------------------------ #
    # Scaling helper
    # ------------------------------------------------------------------ #

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

    # ------------------------------------------------------------------ #
    # Public entry point
    # ------------------------------------------------------------------ #

    async def run(self, task: str) -> RunResult:
        """Execute *task* and return a :class:`RunResult`.

        Parameters:
            task: The user-facing task description.

        Returns:
            A :class:`RunResult` containing the summary, task directory,
            total steps executed, and elapsed wall-clock time.
        """
        t0 = time.monotonic()
        final_step = 0

        # ── 1. Create task-specific screenshot directory ──────────────
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        task_dir = SCREENSHOTS_DIR / f"task_{timestamp}"
        task_dir.mkdir(parents=True, exist_ok=True)
        logger.info("Task dir: %s", task_dir)

        # ── 2. Initial screenshot ─────────────────────────────────────
        screenshot = await self._eye.capture()
        scaled = self._maybe_scale(screenshot)
        initial_path = task_dir / "step_000.webp"
        scaled.save(initial_path)

        # ── 2b. Collect desktop environment info ──────────────────────
        from src.agent.environment import collect_environment

        try:
            env_block = await collect_environment(
                screenshot.width, screenshot.height,
            )
        except Exception:
            logger.warning("Failed to collect environment info", exc_info=True)
            env_block = ""

        # ── 3. Build conversation context ─────────────────────────────
        from src.brain.prompts import build_system_prompt

        system_prompt = build_system_prompt(self._config)
        ctx = ConversationContext(system_prompt, max_images=self._max_images)

        # Prepend environment block to the user task text.
        task_text = f"{env_block}\n\n{task}" if env_block else task
        ctx.add_user_task(
            task_text, scaled.base64, scaled.detail,
            mime_type=scaled.mime_type,
        )

        # ── 4. Main loop ──────────────────────────────────────────────
        consecutive_errors = 0
        no_progress_count = 0
        last_screenshot_hash: str | None = None
        tools_schema = self._registry.get_openai_schemas()
        repeat_count = 0
        last_action_key: str | None = None

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
                    return RunResult(
                        summary=(
                            "Error: max consecutive LLM errors reached."
                            f" (last: {exc})"
                        ),
                        task_dir=str(task_dir),
                        total_steps=final_step,
                        elapsed_seconds=time.monotonic() - t0,
                        success=False,
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

            # Take only the first tool call (GUI ops must be serial).
            tc = response.tool_calls[0]
            logger.info(
                "Tool call: %s(%s)", tc.name, tc.arguments
            )

            # Append the full assistant message (may include text + tool_calls).
            ctx.add_assistant(response.raw)

            # ── 4c. Handle "finished" ────────────────────────────────
            if tc.name == "finished":
                if self._overlay:
                    _show_overlay(self._overlay, "finished", tc.arguments)
                summary = tc.arguments.get("summary", "Task completed.")
                ctx.add_tool_result(tc.id, summary)
                logger.info("Task finished: %s", summary)
                final_step = step
                return RunResult(
                    summary=summary,
                    task_dir=str(task_dir),
                    total_steps=final_step,
                    elapsed_seconds=time.monotonic() - t0,
                )

            # ── 4d. Handle "call_user" ───────────────────────────────
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

                # Take a fresh screenshot after user interaction.
                # (Overlay uses setSharingType_(0) so it won't appear in screenshots.)
                screenshot = await self._eye.capture()
                scaled = self._maybe_scale(screenshot)
                shot_path = task_dir / f"step_{step:03d}.webp"
                scaled.save(shot_path)
                ctx.add_screenshot(
                    scaled.base64, scaled.detail,
                    mime_type=scaled.mime_type,
                )
                continue

            # ── 4e. Scale coordinates & show overlay, then execute ────
            # If scaling is active, map model coordinates to screen
            # coordinates for pyautogui and the overlay.
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
                    result = f"Error: invalid coordinate — {exc}"
                    logger.warning(
                        "Coordinate scaling failed (%d/%d): %s",
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS,
                        exc,
                    )
                    ctx.add_tool_result(tc.id, result)
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS:
                        return RunResult(
                            summary=(
                                "Error: max consecutive errors reached."
                                f" (last: {exc})"
                            ),
                            task_dir=str(task_dir),
                            total_steps=final_step,
                            elapsed_seconds=time.monotonic() - t0,
                            success=False,
                        )
                    continue
                if exec_args != tc.arguments:
                    logger.info(
                        "Scaled args: %s -> %s", tc.arguments, exec_args,
                    )

            if self._overlay:
                _show_overlay(self._overlay, tc.name, exec_args)

            try:
                result = await self._registry.execute(tc.name, exec_args)
                consecutive_errors = 0
            except Exception as exc:
                consecutive_errors += 1
                result = f"Error: {exc}"
                logger.exception(
                    "Tool execution error (%d/%d)",
                    consecutive_errors,
                    MAX_CONSECUTIVE_ERRORS,
                )
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS:
                    logger.error(
                        "Max consecutive tool errors -- aborting task"
                    )
                    return RunResult(
                        summary=(
                            "Error: max consecutive tool errors reached."
                            f" (last: {exc})"
                        ),
                        task_dir=str(task_dir),
                        total_steps=final_step,
                        elapsed_seconds=time.monotonic() - t0,
                        success=False,
                    )

            # ── 4f. Wait for UI to settle ──────────────────────────────
            # Overlay stays visible (setSharingType_(0) keeps it out of
            # screenshots).  It will be replaced by the next show_overlay call.
            await asyncio.sleep(self._screenshot_interval_ms / 1000.0)

            # ── 4g. Take new screenshot, save to disk ─────────────────
            if self._overlay:
                self._overlay.show_screenshot()
            screenshot = await self._eye.capture()
            scaled = self._maybe_scale(screenshot)
            shot_path = task_dir / f"step_{step:03d}.webp"
            scaled.save(shot_path)

            # ── 4h. Add to context ────────────────────────────────────
            ctx.add_tool_result(
                tc.id, result, scaled.base64, scaled.detail,
                mime_type=scaled.mime_type,
            )

            # ── 4i. No-progress detection (screenshot hash) ──────────
            current_hash = _screenshot_hash(screenshot.base64)
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

            # ── 4j. Repeated-action detection ───────────────────────────
            action_key = _action_key(tc.name, tc.arguments)
            if action_key == last_action_key:
                repeat_count += 1
                logger.warning(
                    "Repeated action (%d): %s", repeat_count, action_key
                )
                if repeat_count >= REPEAT_ABORT_LIMIT:
                    logger.error("Aborting: agent stuck in repeated action loop")
                    return RunResult(
                        summary="Aborted: agent stuck in repeated action loop.",
                        task_dir=str(task_dir),
                        total_steps=step,
                        elapsed_seconds=time.monotonic() - t0,
                        success=False,
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

            # ── 4k. Fire step callback ────────────────────────────────
            final_step = step
            if self._on_step is not None:
                event = StepEvent(
                    step=step,
                    max_steps=self._max_steps,
                    thought=thought,
                    tool_name=tc.name,
                    tool_args=tc.arguments,
                    tool_result=result,
                    screenshot_path=str(shot_path),
                    wait_ms=self._screenshot_interval_ms,
                    screen_tool_args=exec_args if exec_args != tc.arguments else None,
                )
                try:
                    await self._on_step(event)
                except Exception:
                    logger.exception("on_step callback error (non-fatal)")

        # ── 5. Budget exhausted ───────────────────────────────────────
        logger.warning("Max steps (%d) reached", self._max_steps)
        return RunResult(
            summary=f"Max steps ({self._max_steps}) reached. Task may be incomplete.",
            task_dir=str(task_dir),
            total_steps=final_step,
            elapsed_seconds=time.monotonic() - t0,
            success=False,
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
