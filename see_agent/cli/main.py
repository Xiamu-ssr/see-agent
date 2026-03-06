"""CLI entry point for see-agent — built with Typer.

Commands
--------
- ``see-agent serve``        Start the FastAPI/WebSocket server.
- ``see-agent chat``         Interactive conversation mode.
- ``see-agent run "task"``   Execute a single task then exit.
- ``see-agent config show``  Display current configuration.
- ``see-agent config init``  Interactive configuration wizard.
"""

from __future__ import annotations

import asyncio
import json
import sys

import typer

from see_agent.agent.loop import RunResult, StepEvent
from see_agent.config import ensure_workspace, load_config, save_config, setup_logging

# ---------------------------------------------------------------------------
# Typer app hierarchy
# ---------------------------------------------------------------------------

app = typer.Typer(
    name="see-agent",
    help="AI assistant that can see your screen and operate your Mac.",
    add_completion=False,
)

config_app = typer.Typer(
    name="config",
    help="View or initialise see-agent configuration.",
    add_completion=False,
)
app.add_typer(config_app, name="config")

sessions_app = typer.Typer(
    name="sessions",
    help="List, inspect, and clean up sessions.",
    add_completion=False,
)
app.add_typer(sessions_app, name="sessions")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _mask_api_key(key: str) -> str:
    """Return a masked version of *key*, showing only the last 4 characters."""
    if len(key) <= 4:
        return "****"
    return "*" * (len(key) - 4) + key[-4:]


def _validate_api_key(config: dict) -> None:
    """Abort with a friendly message when the API key is missing."""
    api_key: str = config.get("llm", {}).get("api_key", "")
    if not api_key:
        typer.echo(
            "Error: API key is not configured.\n"
            "Run `see-agent config init` to set it, "
            "or export SEE_AGENT_API_KEY.",
            err=True,
        )
        raise typer.Exit(code=1)


def _build_components(config: dict, *, no_overlay: bool = False, no_scaling: bool = False):  # noqa: ANN202
    """Instantiate the Eye, Brain, ToolRegistry, and AgentLoop from *config*.

    Returns:
        An ``AgentLoop`` ready to call ``loop.run(task)``.
    """
    import logging

    from see_agent.agent.loop import AgentLoop
    from see_agent.brain.openai_client import OpenAIBrain
    from see_agent.eye.mac import MacEye
    from see_agent.hand.tools import create_registry

    llm_cfg = config["llm"]
    eye = MacEye()
    brain = OpenAIBrain(
        base_url=llm_cfg["base_url"],
        api_key=llm_cfg["api_key"],
        model=llm_cfg["model"],
    )
    registry = create_registry(eye)

    overlay = None
    if not no_overlay and config.get("show_overlay", True):
        try:
            from see_agent.overlay.mac_overlay import MacOverlayRenderer

            overlay = MacOverlayRenderer()
        except Exception:
            logging.getLogger(__name__).warning(
                "Failed to initialize overlay, continuing without it"
            )

    if no_scaling:
        config = {**config, "scaling_enabled": False}

    loop = AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        on_step=_on_step_async,
        on_user_input=_on_user_input_async,
        overlay=overlay,
    )
    return loop


def _format_tool_args(args: dict) -> str:
    """Return a compact single-line representation of tool arguments."""
    if not args:
        return ""
    parts: list[str] = []
    for key, value in args.items():
        if isinstance(value, str) and len(value) > 60:
            value = value[:57] + "..."
        parts.append(f"{key}={value!r}")
    return ", ".join(parts)


def _flush_stdin() -> None:
    """Discard any stale bytes left in stdin by pyautogui keyboard events.

    On macOS/Linux this uses ``termios.tcflush``; on other platforms it is
    a silent no-op.
    """
    try:
        import termios  # noqa: I001

        termios.tcflush(sys.stdin, termios.TCIFLUSH)
    except Exception:  # ImportError, termios.error, or others
        pass


def _safe_input(prompt: str) -> str:
    """Read a line from stdin, tolerating garbled bytes from pyautogui."""
    while True:
        try:
            _flush_stdin()
            return input(prompt).strip()
        except UnicodeDecodeError:
            typer.echo("(stdin contained garbled bytes — please re-enter)")


def _print_task_result(result: RunResult) -> None:
    """Print the post-task summary block matching PRD format."""
    icon = "\u2705" if result.success else "\u274c"
    label = "finished" if result.success else "aborted"
    typer.echo(
        f"\n{icon} [Step {result.total_steps}] {label}: {result.summary}"
    )
    # Count screenshots on disk.
    from pathlib import Path

    screenshots_dir = Path(result.task_dir) / "screenshots"
    if screenshots_dir.is_dir():
        n_screenshots = len(list(screenshots_dir.glob("*.webp")))
    else:
        task_path = Path(result.task_dir)
        n_screenshots = len(list(task_path.glob("*.webp"))) if task_path.is_dir() else 0
    typer.echo(f"\U0001f4c1 截图已保存: {result.task_dir} ({n_screenshots} 张)")
    if result.session_id:
        typer.echo(f"\U0001f4cb 会话 ID: {result.session_id}")
    typer.echo(f"\u23f1\ufe0f  总耗时: {result.elapsed_seconds:.0f}s\n")


# ---------------------------------------------------------------------------
# Step / user-input callbacks used by chat & run
# ---------------------------------------------------------------------------

async def _on_step_async(event: StepEvent) -> None:
    """Print real-time progress for each agent step (PRD §8 format)."""
    # Thought
    if event.thought:
        typer.echo(f"\U0001f4ad {event.thought}")

    # Tool invocation
    if event.tool_name:
        formatted = _format_tool_args(event.tool_args or {})
        line = (
            f"\U0001f590\ufe0f  [Step {event.step}/{event.max_steps}] "
            f"{event.tool_name}: {formatted}"
        )
        # When coordinate scaling is active, show screen-space args too.
        if event.screen_tool_args is not None:
            screen_fmt = _format_tool_args(event.screen_tool_args)
            line += f"  \u2192 screen: {screen_fmt}"
        typer.echo(line)

    # Wait hint
    if event.wait_ms > 0:
        typer.echo(f"\u23f3 等待 {event.wait_ms}ms...")

    # Screenshot
    if event.screenshot_path:
        typer.echo(
            f"\U0001f4f8 [Step {event.step}] "
            f"截屏完成 \u2192 {event.screenshot_path}"
        )


async def _on_user_input_async(question: str) -> str:
    """Called when the agent invokes ``call_user`` and needs human input."""
    typer.echo(f"\n\u2753 {question}")
    _flush_stdin()
    return _safe_input("> ")


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

@app.command()
def serve(
    port: int = typer.Option(8000, "--port", "-p", help="Port to listen on."),
) -> None:
    """Start the see-agent API server (FastAPI + WebSocket)."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    typer.echo(f"Starting see-agent server on 0.0.0.0:{port} ...")

    import uvicorn

    uvicorn.run("see_agent.server.app:app", host="0.0.0.0", port=port, reload=False)


@app.command()
def chat(
    no_overlay: bool = typer.Option(False, "--no-overlay", help="Disable visual overlay."),
    no_scaling: bool = typer.Option(False, "--no-scaling", help="Disable coordinate scaling."),
) -> None:
    """Interactive conversation mode — keep asking, keep executing."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    from see_agent.session import SessionStore

    loop = _build_components(config, no_overlay=no_overlay, no_scaling=no_scaling)
    session = SessionStore.create("interactive-chat", config)

    typer.echo("\U0001f916 see-agent v0.1 已启动")
    typer.echo(f"\U0001f4cb 会话 ID: {session.id}")
    typer.echo("Enter a task description (Ctrl+C to exit).\n")

    try:
        while True:
            _flush_stdin()
            try:
                task = _safe_input("> ")
            except EOFError:
                break

            if not task:
                continue

            result: RunResult = asyncio.run(loop.run(task, session_id=session.id))
            _print_task_result(result)

    except KeyboardInterrupt:
        typer.echo("\n\nBye!")
        raise typer.Exit()


@app.command()
def run(
    task: str = typer.Argument(..., help="Task description to execute."),
    no_overlay: bool = typer.Option(False, "--no-overlay", help="Disable visual overlay."),
    no_scaling: bool = typer.Option(False, "--no-scaling", help="Disable coordinate scaling."),
) -> None:
    """Execute a single task and exit."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    loop = _build_components(config, no_overlay=no_overlay, no_scaling=no_scaling)

    typer.echo(f"\U0001f680 Running task: {task}\n")

    try:
        result: RunResult = asyncio.run(loop.run(task))
    except KeyboardInterrupt:
        typer.echo("\n\nTask interrupted.")
        raise typer.Exit(code=130)

    _print_task_result(result)


# ---------------------------------------------------------------------------
# Config sub-commands
# ---------------------------------------------------------------------------

@config_app.command("show")
def config_show() -> None:
    """Pretty-print the current configuration (API key masked)."""
    config = load_config()

    # Mask the API key for display
    display = json.loads(json.dumps(config))  # deep copy
    if "llm" in display and "api_key" in display["llm"]:
        display["llm"]["api_key"] = _mask_api_key(display["llm"]["api_key"])

    typer.echo(json.dumps(display, indent=4, ensure_ascii=False))


@config_app.command("init")
def config_init() -> None:
    """Interactive wizard to initialise see-agent configuration."""
    ensure_workspace()
    config = load_config()

    llm = config.get("llm", {})

    typer.echo("see-agent configuration wizard")
    typer.echo("=" * 40)
    typer.echo("Press Enter to keep the current value shown in [brackets].\n")

    # --- base_url ---
    current_base_url: str = llm.get("base_url", "https://api.openai.com/v1")
    base_url = input(f"LLM base URL [{current_base_url}]: ").strip()
    if not base_url:
        base_url = current_base_url

    # --- api_key ---
    current_key: str = llm.get("api_key", "")
    masked = _mask_api_key(current_key) if current_key else "(not set)"
    api_key = input(f"API key [{masked}]: ").strip()
    if not api_key:
        api_key = current_key

    # --- model ---
    current_model: str = llm.get("model", "gpt-4o")
    model = input(f"Model [{current_model}]: ").strip()
    if not model:
        model = current_model

    # Apply
    config["llm"] = {
        "base_url": base_url,
        "api_key": api_key,
        "model": model,
    }

    save_config(config)
    typer.echo("\n\u2705 Configuration saved.")


# ---------------------------------------------------------------------------
# Sessions sub-commands
# ---------------------------------------------------------------------------

@sessions_app.command("list")
def sessions_list(
    status: str | None = typer.Option(None, "--status", "-s", help="Filter by status."),
    limit: int = typer.Option(20, "--limit", "-n", help="Max sessions to show."),
) -> None:
    """List recent sessions."""
    from see_agent.session import SessionStore

    sessions = SessionStore.list(status=status, limit=limit)
    if not sessions:
        typer.echo("No sessions found.")
        return

    # Header
    typer.echo(
        f"{'ID':<28} {'TASK':<24} {'STATUS':<12} {'STEPS':>5} {'TIME':>8} {'DATE':<16}"
    )
    typer.echo("-" * 95)
    for s in sessions:
        task_display = s.task[:22] + ".." if len(s.task) > 24 else s.task
        elapsed = _format_elapsed(s.elapsed_seconds)
        date = s.created_at[:16].replace("T", " ") if s.created_at else ""
        typer.echo(
            f"{s.id:<28} {task_display:<24} {s.status:<12}"
            f" {s.total_steps:>5} {elapsed:>8} {date:<16}"
        )


@sessions_app.command("show")
def sessions_show(
    session_id: str = typer.Argument(..., help="Session ID to inspect."),
) -> None:
    """Show details of a session."""
    from see_agent.session import SessionStore

    try:
        session = SessionStore.load(session_id)
    except FileNotFoundError:
        typer.echo(f"Session not found: {session_id}", err=True)
        raise typer.Exit(code=1)

    typer.echo(json.dumps(session.meta, indent=2, ensure_ascii=False))

    messages = session.read_messages()
    typer.echo(f"\nMessages: {len(messages)}")
    ss_dir = session.screenshots_dir
    screenshots = list(ss_dir.glob("*.webp")) if ss_dir.exists() else []
    typer.echo(f"Screenshots: {len(screenshots)}")


@sessions_app.command("clean")
def sessions_clean(
    keep: str = typer.Option("7d", "--keep", help="Keep sessions newer than this (e.g. 3d, 0)."),
    empty: bool = typer.Option(
        False, "--empty", help="Only clean empty sessions (no screenshots).",
    ),
) -> None:
    """Clean up old or empty sessions."""
    from see_agent.session import SessionStore

    if keep == "0":
        keep_days = 0
    elif keep.endswith("d"):
        keep_days = int(keep[:-1])
    else:
        keep_days = int(keep)

    deleted, freed = SessionStore.clean(keep_days=keep_days, empty_only=empty)
    freed_mb = freed / (1024 * 1024)
    typer.echo(f"\U0001f9f9 Cleaned {deleted} sessions ({freed_mb:.1f}MB freed)")


# ---------------------------------------------------------------------------
# Resume command
# ---------------------------------------------------------------------------

@app.command()
def resume(
    session_id: str = typer.Argument(None, help="Session ID to resume (omit for latest)."),
    last: bool = typer.Option(False, "--last", help="Resume the most recent session."),
    no_overlay: bool = typer.Option(False, "--no-overlay", help="Disable visual overlay."),
    no_scaling: bool = typer.Option(False, "--no-scaling", help="Disable coordinate scaling."),
) -> None:
    """Resume a previous session."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    from see_agent.session import SessionStore

    if last or session_id is None:
        sessions = SessionStore.list(limit=1)
        if not sessions:
            typer.echo("No sessions found.", err=True)
            raise typer.Exit(code=1)
        session_id = sessions[0].id

    try:
        session = SessionStore.load(session_id)
    except FileNotFoundError:
        typer.echo(f"Session not found: {session_id}", err=True)
        raise typer.Exit(code=1)

    loop = _build_components(config, no_overlay=no_overlay, no_scaling=no_scaling)

    typer.echo(f"\U0001f504 Resuming session: {session.id}")
    typer.echo(f"\U0001f4cb Task: {session.task}")
    typer.echo("Enter a follow-up task (Ctrl+C to exit).\n")

    try:
        while True:
            _flush_stdin()
            try:
                task = _safe_input("> ")
            except EOFError:
                break
            if not task:
                continue
            result: RunResult = asyncio.run(loop.run(task, session_id=session.id))
            _print_task_result(result)
    except KeyboardInterrupt:
        typer.echo("\n\nBye!")
        raise typer.Exit()


def _format_elapsed(seconds: float) -> str:
    """Format seconds as Xm Ys."""
    m = int(seconds) // 60
    s = int(seconds) % 60
    return f"{m}m{s:02d}s"


# ---------------------------------------------------------------------------
# Allow ``python -m src.cli.main``
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    app()
