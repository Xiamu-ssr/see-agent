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

from src.agent.loop import RunResult, StepEvent
from src.config import ensure_workspace, load_config, save_config, setup_logging

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


def _build_components(config: dict):  # noqa: ANN202
    """Instantiate the Eye, Brain, ToolRegistry, and AgentLoop from *config*.

    Returns:
        An ``AgentLoop`` ready to call ``loop.run(task)``.
    """
    from src.agent.loop import AgentLoop
    from src.brain.openai_client import OpenAIBrain
    from src.eye.mac import MacEye
    from src.hand.tools import create_registry

    llm_cfg = config["llm"]
    eye = MacEye()
    brain = OpenAIBrain(
        base_url=llm_cfg["base_url"],
        api_key=llm_cfg["api_key"],
        model=llm_cfg["model"],
    )
    registry = create_registry(eye)

    loop = AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        on_step=_on_step_async,
        on_user_input=_on_user_input_async,
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

    task_path = Path(result.task_dir)
    n_screenshots = len(list(task_path.glob("*.webp"))) if task_path.is_dir() else 0
    typer.echo(f"\U0001f4c1 截图已保存: {result.task_dir} ({n_screenshots} 张)")
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
        typer.echo(
            f"\U0001f590\ufe0f  [Step {event.step}/{event.max_steps}] "
            f"{event.tool_name}: {formatted}"
        )

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

    uvicorn.run("src.server.app:app", host="0.0.0.0", port=port, reload=False)


@app.command()
def chat() -> None:
    """Interactive conversation mode — keep asking, keep executing."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    loop = _build_components(config)

    typer.echo("\U0001f916 see-agent v0.1 已启动")
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

            result: RunResult = asyncio.run(loop.run(task))
            _print_task_result(result)

    except KeyboardInterrupt:
        typer.echo("\n\nBye!")
        raise typer.Exit()


@app.command()
def run(
    task: str = typer.Argument(..., help="Task description to execute."),
) -> None:
    """Execute a single task and exit."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    loop = _build_components(config)

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
# Allow ``python -m src.cli.main``
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    app()
