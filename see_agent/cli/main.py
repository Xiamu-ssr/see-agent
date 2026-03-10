"""CLI entry point for see-agent — built with Typer.

Commands
--------
- ``see-agent start``         Start the server + open browser.
- ``see-agent stop``          Stop the running server.
- ``see-agent version``       Show version.
- ``see-agent config show``   Display current configuration.
- ``see-agent config init``   Interactive configuration wizard.
- ``see-agent setup install`` Install optional dependencies.
- ``see-agent setup check``   Check environment status.
"""

from __future__ import annotations

import importlib
import json
import signal
import subprocess
import sys

import typer

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

setup_app = typer.Typer(
    name="setup",
    help="Install optional dependencies and check environment.",
    add_completion=False,
)
app.add_typer(setup_app, name="setup")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _mask_api_key(key: str) -> str:
    """Return a masked version of *key*, showing only the last 4 characters."""
    if len(key) <= 4:
        return "****"
    return "*" * (len(key) - 4) + key[-4:]


def _validate_api_key(config: dict) -> None:  # type: ignore[type-arg]
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


_PID_FILE = "~/.see-agent/server.pid"


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

@app.command()
def start(
    port: int = typer.Option(8000, "--port", "-p", help="Port to listen on."),
    no_browser: bool = typer.Option(False, "--no-browser", help="Don't open browser."),
) -> None:
    """Start the see-agent server and open the browser."""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    import os
    from pathlib import Path

    pid_path = Path(os.path.expanduser(_PID_FILE))
    pid_path.write_text(str(os.getpid()))

    url = f"http://localhost:{port}"
    typer.echo(f"Starting see-agent on {url}")

    if not no_browser:
        import threading
        import webbrowser

        threading.Timer(1.5, lambda: webbrowser.open(url)).start()

    import uvicorn

    uvicorn.run("see_agent.server.app:app", host="0.0.0.0", port=port, reload=False)


@app.command()
def stop() -> None:
    """Stop a running see-agent server."""
    import os
    from pathlib import Path

    pid_path = Path(os.path.expanduser(_PID_FILE))
    if not pid_path.exists():
        typer.echo("No running server found (PID file missing).", err=True)
        raise typer.Exit(code=1)

    pid = int(pid_path.read_text().strip())
    try:
        os.kill(pid, signal.SIGTERM)
        typer.echo(f"Stopped see-agent server (PID {pid}).")
    except ProcessLookupError:
        typer.echo(f"Process {pid} not found (already stopped).")
    finally:
        pid_path.unlink(missing_ok=True)


@app.command()
def version() -> None:
    """Show see-agent version."""
    typer.echo("see-agent v3.0.0")


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
    typer.echo("\nConfiguration saved.")


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
# Allow ``python -m see_agent.cli.main``
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    app()
