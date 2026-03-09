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
import importlib
import json
import select
import subprocess
import sys
import threading
from typing import Any

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

mcp_app = typer.Typer(
    name="mcp",
    help="Manage MCP server connections.",
    add_completion=False,
)
app.add_typer(mcp_app, name="mcp")

setup_app = typer.Typer(
    name="setup",
    help="Install optional dependencies and check environment.",
    add_completion=False,
)
app.add_typer(setup_app, name="setup")

agent_app = typer.Typer(
    name="agent",
    help="Manage agent definitions.",
    add_completion=False,
)
app.add_typer(agent_app, name="agent")

team_app = typer.Typer(
    name="team",
    help="Manage and run agent teams.",
    add_completion=False,
)
app.add_typer(team_app, name="team")

quick_app = typer.Typer(
    name="quick",
    help="Quick single-agent shortcuts.",
    add_completion=False,
)
app.add_typer(quick_app, name="quick")


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


def _build_components(  # noqa: ANN202
    config: dict,
    *,
    no_overlay: bool = False,
    no_scaling: bool = False,
    user_queue: "asyncio.Queue[str] | None" = None,
):
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

    if no_scaling:
        config = {**config, "scaling_enabled": False}

    # Build a scale function for the screenshot tool if scaling is enabled.
    scale_fn = None
    if config.get("scaling_enabled", True):
        from see_agent.eye.scaling import find_target_resolution, scale_screenshot

        scaling_match = config.get("scaling_match", "aspect_ratio")

        def _scale(screenshot):  # type: ignore[no-untyped-def]
            target = find_target_resolution(
                screenshot.width, screenshot.height, scaling_match,
            )
            if target is None:
                return screenshot
            return scale_screenshot(screenshot, target)

        scale_fn = _scale

    registry = create_registry(eye, scale_fn=scale_fn)

    overlay = None
    if not no_overlay and config.get("show_overlay", True):
        try:
            from see_agent.overlay.mac_overlay import MacOverlayRenderer

            overlay = MacOverlayRenderer()
        except Exception:
            logging.getLogger(__name__).warning(
                "Failed to initialize overlay, continuing without it"
            )

    # MCP servers (optional) — connected lazily inside AgentLoop.run()
    mcp_manager = None
    mcp_servers = config.get("mcp_servers", {})
    if mcp_servers:
        try:
            from see_agent.hand.mcp import MCPManager

            mcp_manager = MCPManager(mcp_servers, global_env=config.get("env", {}))
        except ImportError:
            typer.echo(
                "Warning: MCP configured but mcp not installed. "
                "Run: see-agent setup install --mcp"
            )
        except Exception:
            logging.getLogger(__name__).warning(
                "Failed to initialize MCP manager, continuing without it"
            )
            mcp_manager = None

    # Memory backend (optional)
    memory = None
    mem_cfg = config.get("memory", {})
    if mem_cfg.get("enabled", False):
        try:
            from see_agent.memory.mem0_backend import Mem0Memory

            memory = Mem0Memory(config=mem_cfg.get("mem0") or None)
        except ImportError:
            typer.echo(
                "Warning: Memory enabled but mem0ai not installed. "
                "Run: see-agent setup install --memory"
            )
        except Exception:
            logging.getLogger(__name__).warning(
                "Failed to initialize memory backend, continuing without it"
            )

    loop = AgentLoop(
        brain=brain,
        eye=eye,
        registry=registry,
        config=config,
        on_step=_on_step_async,
        on_user_input=_on_user_input_async,
        overlay=overlay,
        memory=memory,
        mcp_manager=mcp_manager,
        user_queue=user_queue,
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


def _stdin_reader_thread(
    queue: "asyncio.Queue[str]",
    stop: threading.Event,
) -> None:
    """Background thread: read stdin lines and push them into *queue*.

    Runs while the agent is executing a task.  The thread checks stdin
    every 300 ms and exits when *stop* is set.
    """
    while not stop.is_set():
        # Use select to avoid blocking forever on stdin.
        if select.select([sys.stdin], [], [], 0.3)[0]:
            try:
                line = sys.stdin.readline()
            except (EOFError, OSError):
                break
            text = line.strip()
            if text:
                queue.put_nowait(text)
                typer.echo(f"📨 Queued ({queue.qsize()} pending)")


def _print_startup_status(config: dict) -> None:
    """Print component status at startup."""
    # Memory status
    mem_cfg = config.get("memory", {})
    if mem_cfg.get("enabled", False):
        try:
            importlib.import_module("mem0")
            typer.echo("  Memory: active")
        except ImportError:
            typer.echo("  Memory: failed (mem0ai not installed)")
    else:
        typer.echo("  Memory: not configured")

    # MCP status
    mcp_servers = config.get("mcp_servers", {})
    if mcp_servers:
        typer.echo(f"  MCP: active ({len(mcp_servers)} servers)")
    else:
        typer.echo("  MCP: not configured")

    # Skills status
    from see_agent.skill.loader import gate_skills, load_skills

    skills_dirs = config.get("skills_dirs", [])
    skills = load_skills(skills_dirs) if skills_dirs else []
    if skills:
        skills = gate_skills(skills)
        blocked = [s for s in skills if s.blocked]
        active_count = len(skills) - len(blocked)
        msg = f"  Skills: {active_count} loaded"
        if blocked:
            msg += f", {len(blocked)} blocked"
            for s in blocked:
                msg += f"\n    - {s.name}: {s.block_reason}"
        typer.echo(msg)
    else:
        typer.echo("  Skills: none loaded")


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
    _print_startup_status(config)
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
# MCP sub-commands
# ---------------------------------------------------------------------------

@mcp_app.command("list")
def mcp_list(
    check: bool = typer.Option(False, "--check", help="Try connecting and report health."),
) -> None:
    """List configured MCP servers."""
    config = load_config()
    servers = config.get("mcp_servers", {})
    if not servers:
        typer.echo("No MCP servers configured.")
        return
    for name, cfg in servers.items():
        transport = cfg.get("type", "stdio")
        target = cfg.get("command", cfg.get("url", "?"))
        typer.echo(f"  {name} ({transport}): {target}")

    if check:
        import asyncio as _aio

        async def _check() -> None:
            from see_agent.hand.mcp import MCPManager

            manager = MCPManager(servers, global_env=config.get("env", {}))
            for sname, client in manager._clients.items():
                try:
                    await client.connect()
                    tools = await client.list_tools()
                    typer.echo(f"  {sname}: OK ({len(tools)} tools)")
                    await client.disconnect()
                except Exception as exc:
                    typer.echo(f"  {sname}: FAILED ({exc})")

        _aio.run(_check())


@mcp_app.command("add")
def mcp_add(
    name: str = typer.Argument(..., help="Server name."),
    command: str = typer.Argument(None, help="Command to start the server (for stdio)."),
    args: list[str] | None = typer.Option(None, "--arg", help="Arguments for the command."),
    transport_type: str = typer.Option(
        "stdio", "--type", "-t", help="Transport type: stdio or http.",
    ),
    url: str | None = typer.Option(None, "--url", help="URL for HTTP transport."),
    env: list[str] | None = typer.Option(None, "--env", "-e", help="Env vars in KEY=VALUE format."),
) -> None:
    """Add an MCP server to the configuration."""
    if transport_type == "http" and not url:
        typer.echo("Error: --url is required for HTTP transport.", err=True)
        raise typer.Exit(code=1)
    if transport_type == "stdio" and not command:
        typer.echo("Error: command argument is required for stdio transport.", err=True)
        raise typer.Exit(code=1)

    config = load_config()
    if "mcp_servers" not in config:
        config["mcp_servers"] = {}

    server_cfg: dict[str, Any] = {"type": transport_type}
    if transport_type == "stdio":
        server_cfg["command"] = command
        server_cfg["args"] = args or []
    else:
        server_cfg["url"] = url

    # Parse env vars
    if env:
        env_dict: dict[str, str] = {}
        for item in env:
            if "=" in item:
                k, v = item.split("=", 1)
                env_dict[k] = v
        if env_dict:
            server_cfg["env"] = env_dict

    config["mcp_servers"][name] = server_cfg
    save_config(config)
    typer.echo(f"Added MCP server: {name}")


@mcp_app.command("remove")
def mcp_remove(
    name: str = typer.Argument(..., help="Server name to remove."),
) -> None:
    """Remove an MCP server from the configuration."""
    config = load_config()
    servers = config.get("mcp_servers", {})
    if name not in servers:
        typer.echo(f"MCP server not found: {name}", err=True)
        raise typer.Exit(code=1)
    del servers[name]
    save_config(config)
    typer.echo(f"Removed MCP server: {name}")


# ---------------------------------------------------------------------------
# Setup sub-commands
# ---------------------------------------------------------------------------


@setup_app.command("install")
def setup_install(
    full: bool = typer.Option(False, "--full", help="Install all optional deps."),
    memory: bool = typer.Option(False, "--memory", help="Install memory (mem0ai) deps."),
    mcp: bool = typer.Option(False, "--mcp", help="Install MCP deps."),
    dev: bool = typer.Option(False, "--dev", help="Install dev deps."),
) -> None:
    """Install optional dependencies for see-agent."""
    extras: list[str] = []
    if full or not any([memory, mcp, dev]):
        extras.append("all")
    if memory and not full:
        extras.append("memory")
    if mcp and not full:
        extras.append("mcp")
    if dev:
        extras.append("dev")

    spec = ",".join(extras)
    cmd = [sys.executable, "-m", "pip", "install", "-e", f".[{spec}]"]

    typer.echo(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=False)
    raise typer.Exit(code=result.returncode)


@setup_app.command("check")
def setup_check() -> None:
    """Check which optional dependencies are installed."""
    config = load_config()

    # Check mem0ai
    try:
        importlib.import_module("mem0")
        mem0_ok = True
    except ImportError:
        mem0_ok = False

    mem_enabled = config.get("memory", {}).get("enabled", False)
    if mem0_ok:
        typer.echo("mem0ai   ... installed")
    else:
        hint = " (config: memory.enabled=true)" if mem_enabled else ""
        typer.echo(f"mem0ai   ... not installed{hint}")
        if mem_enabled:
            typer.echo("  Fix: see-agent setup install --memory")

    # Check mcp
    try:
        importlib.import_module("mcp")
        mcp_ok = True
    except ImportError:
        mcp_ok = False

    mcp_configured = bool(config.get("mcp_servers"))
    if mcp_ok:
        typer.echo("mcp      ... installed")
    else:
        hint = " (config: mcp_servers configured)" if mcp_configured else ""
        typer.echo(f"mcp      ... not installed{hint}")
        if mcp_configured:
            typer.echo("  Fix: see-agent setup install --mcp")


# ---------------------------------------------------------------------------
# Agent sub-commands
# ---------------------------------------------------------------------------

@agent_app.command("create")
def agent_create(
    agent_id: str = typer.Argument(..., help="Unique agent ID."),
    name: str = typer.Option("", "--name", "-n", help="Human-readable name."),
    role: str = typer.Option(
        "general assistant", "--role", "-r", help="Agent role description.",
    ),
    model: str | None = typer.Option(None, "--model", help="Override LLM model."),
    max_steps: int | None = typer.Option(None, "--max-steps", help="Override max steps."),
    deny_tools: str | None = typer.Option(
        None, "--deny-tools", help="Comma-separated tool names to deny.",
    ),
) -> None:
    """Create a new agent definition."""
    ensure_workspace()
    from see_agent.agent.definition import AgentDefinition

    display_name = name or agent_id
    config_overrides: dict[str, Any] = {}
    if model:
        config_overrides["llm"] = {"model": model}
    if max_steps is not None:
        config_overrides["max_steps"] = max_steps

    tools_config: dict[str, Any] = {}
    if deny_tools:
        tools_config["denied"] = [t.strip() for t in deny_tools.split(",") if t.strip()]

    AgentDefinition.create(
        agent_id, name=display_name, role=role,
        config_overrides=config_overrides,
        tools_config=tools_config,
    )
    typer.echo(f"Created agent: {agent_id}")


@agent_app.command("list")
def agent_list() -> None:
    """List all agent definitions."""
    ensure_workspace()
    from see_agent.agent.definition import AgentDefinition

    agents = AgentDefinition.list_all()
    if not agents:
        typer.echo("No agents defined.")
        return
    for a in agents:
        typer.echo(f"  {a.id}: {a.name} ({a.role})")


@agent_app.command("show")
def agent_show(
    agent_id: str = typer.Argument(..., help="Agent ID."),
) -> None:
    """Show agent definition details."""
    ensure_workspace()
    from see_agent.agent.definition import AgentDefinition

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        typer.echo(f"Agent not found: {agent_id}", err=True)
        raise typer.Exit(code=1)
    typer.echo(json.dumps({
        "id": defn.id,
        "name": defn.name,
        "role": defn.role,
        "config_overrides": defn.config_overrides,
        "tools_config": defn.tools_config,
        "skills_config": defn.skills_config,
    }, indent=2, ensure_ascii=False))


# ---------------------------------------------------------------------------
# Team sub-commands
# ---------------------------------------------------------------------------

@team_app.command("create")
def team_create(
    name: str = typer.Option(..., "--name", "-n", help="Team name."),
    members: str = typer.Option(
        ..., "--members", "-m", help="Comma-separated agent IDs.",
    ),
    leader: str | None = typer.Option(
        None, "--leader", "-l", help="Leader agent ID.",
    ),
) -> None:
    """Create a new team."""
    ensure_workspace()
    from see_agent.team.definition import TeamDefinition

    member_list = [m.strip() for m in members.split(",") if m.strip()]
    team = TeamDefinition.create(name, member_list, leader=leader)
    typer.echo(f"Created team: {team.id} ({team.name})")


@team_app.command("list")
def team_list() -> None:
    """List all teams."""
    ensure_workspace()
    from see_agent.team.definition import TeamDefinition

    teams = TeamDefinition.list_all()
    if not teams:
        typer.echo("No teams defined.")
        return
    for t in teams:
        typer.echo(
            f"  {t.id}: {t.name} "
            f"(members={','.join(t.members)}, status={t.status})"
        )


@team_app.command("status")
def team_status(
    team_id: str = typer.Argument(..., help="Team ID."),
) -> None:
    """Show team status and task board."""
    ensure_workspace()
    from see_agent.team.definition import TeamDefinition
    from see_agent.team.task_board import TaskBoard

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        typer.echo(f"Team not found: {team_id}", err=True)
        raise typer.Exit(code=1)

    typer.echo(f"Team: {team.name} (status={team.status})")
    typer.echo(f"Members: {', '.join(team.members)}")
    typer.echo(f"Leader: {team.leader or 'none'}")

    from see_agent.config import TEAMS_DIR

    board = TaskBoard(TEAMS_DIR / team_id)
    tasks = board.list_tasks()
    if tasks:
        typer.echo("\nTasks:")
        for t in tasks:
            assignee = t.assigned_to or "unassigned"
            typer.echo(
                f"  [{t.id}] {t.title} ({t.status}, {assignee})"
            )
    else:
        typer.echo("\nNo tasks.")


@team_app.command("run")
def team_run(
    team_id: str = typer.Argument(..., help="Team ID."),
    task: str = typer.Argument(..., help="Task description."),
) -> None:
    """Run a task with a team."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    from see_agent.team.definition import TeamDefinition
    from see_agent.team.manager import TeamManager

    try:
        team_def = TeamDefinition.load(team_id)
    except FileNotFoundError:
        typer.echo(f"Team not found: {team_id}", err=True)
        raise typer.Exit(code=1)

    typer.echo(f"Running team '{team_def.name}' on: {task}")
    manager = TeamManager(team_def, config)
    result = asyncio.run(manager.run(task))

    icon = "\u2705" if result.success else "\u274c"
    typer.echo(f"\n{icon} Team run complete:")
    typer.echo(result.summary)


@team_app.command("stop")
def team_stop(
    team_id: str = typer.Argument(..., help="Team ID."),
) -> None:
    """Stop a running team."""
    ensure_workspace()
    from see_agent.team.definition import TeamDefinition

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        typer.echo(f"Team not found: {team_id}", err=True)
        raise typer.Exit(code=1)

    team.status = "stopped"
    team.save()
    typer.echo(f"Stopped team: {team_id}")


# ---------------------------------------------------------------------------
# Quick sub-commands (single-agent sugar)
# ---------------------------------------------------------------------------

@quick_app.command("run")
def quick_run(
    task: str = typer.Argument(..., help="Task description."),
    no_overlay: bool = typer.Option(
        False, "--no-overlay", help="Disable visual overlay.",
    ),
    no_scaling: bool = typer.Option(
        False, "--no-scaling", help="Disable coordinate scaling.",
    ),
) -> None:
    """Quick single-task execution."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    loop = _build_components(
        config, no_overlay=no_overlay, no_scaling=no_scaling,
    )
    typer.echo(f"\U0001f680 Running task: {task}")
    _print_startup_status(config)
    typer.echo()

    try:
        result: RunResult = asyncio.run(loop.run(task))
    except KeyboardInterrupt:
        typer.echo("\n\nTask interrupted.")
        raise typer.Exit(code=130)

    _print_task_result(result)


@quick_app.command("chat")
def quick_chat(
    no_overlay: bool = typer.Option(
        False, "--no-overlay", help="Disable visual overlay.",
    ),
    no_scaling: bool = typer.Option(
        False, "--no-scaling", help="Disable coordinate scaling.",
    ),
) -> None:
    """Quick interactive chat mode."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    from see_agent.session import SessionStore

    session = SessionStore.create("interactive-chat", config)

    typer.echo("\U0001f916 see-agent quick chat")
    _print_startup_status(config)
    typer.echo(f"\U0001f4cb Session: {session.id}")
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

            user_queue: asyncio.Queue[str] = asyncio.Queue()
            loop = _build_components(
                config, no_overlay=no_overlay, no_scaling=no_scaling,
                user_queue=user_queue,
            )
            stop_reader = threading.Event()
            reader_thread = threading.Thread(
                target=_stdin_reader_thread,
                args=(user_queue, stop_reader),
                daemon=True,
            )
            reader_thread.start()
            try:
                result = asyncio.run(
                    loop.run(task, session_id=session.id),
                )
            finally:
                stop_reader.set()
                reader_thread.join(timeout=1.0)
            _print_task_result(result)
    except KeyboardInterrupt:
        typer.echo("\n\nBye!")
        raise typer.Exit()


# ---------------------------------------------------------------------------
# Allow ``python -m src.cli.main``
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    app()
