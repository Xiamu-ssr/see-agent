"""CLI entry point for see-agent — built with Typer.

v3.1 commands (launchd-based):
- ``see-agent install``   Install all dependencies.
- ``see-agent start``     Start via launchd + open browser.
- ``see-agent stop``      Stop via launchd (kills all agent subprocesses).
- ``see-agent restart``   Restart the service.
- ``see-agent status``    Show service status.
- ``see-agent uninstall`` Remove launchd service + optionally data.
- ``see-agent version``   Show version.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

import typer

from see_agent.config import RUN_DIR, ensure_workspace, load_config

app = typer.Typer(
    name="see-agent",
    help="AI assistant that can see your screen and operate your Mac.",
    add_completion=False,
)

# ---------------------------------------------------------------------------
# launchd constants
# ---------------------------------------------------------------------------

PLIST_LABEL = "dev.see-agent.server"
PLIST_DIR = Path("~/Library/LaunchAgents").expanduser()
PLIST_PATH = PLIST_DIR / f"{PLIST_LABEL}.plist"
LOG_PATH = Path("~/.see-agent/logs/server.log").expanduser()


def _get_uid() -> int:
    return os.getuid()


def _is_running() -> bool:
    result = subprocess.run(
        ["launchctl", "print", f"gui/{_get_uid()}/{PLIST_LABEL}"],
        capture_output=True,
    )
    return result.returncode == 0


def _validate_api_key(config: dict) -> None:  # type: ignore[type-arg]
    api_key: str = config.get("llm", {}).get("api_key", "")
    if not api_key:
        typer.echo(
            "Error: API key is not configured.\n"
            "Open the Web UI Config page to set it, "
            "or export SEE_AGENT_API_KEY.",
            err=True,
        )
        raise typer.Exit(code=1)


def _generate_plist(port: int) -> str:
    python = sys.executable
    home = str(Path("~/.see-agent").expanduser())
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{python}</string>
        <string>-m</string>
        <string>uvicorn</string>
        <string>see_agent.server.app:app</string>
        <string>--host</string>
        <string>0.0.0.0</string>
        <string>--port</string>
        <string>{port}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>{home}</string>
    <key>RunAtLoad</key>
    <false/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>AbandonProcessGroup</key>
    <false/>
    <key>StandardOutPath</key>
    <string>{LOG_PATH}</string>
    <key>StandardErrorPath</key>
    <string>{LOG_PATH}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:{Path(python).parent}</string>
        <key>SEE_AGENT_HOME</key>
        <string>{home}</string>
    </dict>
</dict>
</plist>"""


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


@app.command()
def install(
    full: bool = typer.Option(
        False, "--full", help="Install all optional deps.",
    ),
    memory: bool = typer.Option(
        False, "--memory", help="Install memory (mem0ai) deps.",
    ),
    mcp: bool = typer.Option(
        False, "--mcp", help="Install MCP deps.",
    ),
    dev: bool = typer.Option(
        False, "--dev", help="Install dev deps.",
    ),
) -> None:
    """Install see-agent dependencies."""
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


@app.command()
def start(
    port: int = typer.Option(
        8000, "--port", "-p", help="Port to listen on.",
    ),
    no_browser: bool = typer.Option(
        False, "--no-browser", help="Don't open browser.",
    ),
    foreground: bool = typer.Option(
        False, "--foreground", "-f",
        help="Run in foreground (skip launchd).",
    ),
) -> None:
    """Start the see-agent server."""
    ensure_workspace()
    config = load_config()
    _validate_api_key(config)

    if foreground:
        _start_foreground(port, no_browser)
        return

    if _is_running():
        typer.echo("see-agent is already running")
        if not no_browser:
            import webbrowser

            webbrowser.open(f"http://localhost:{port}")
        return

    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    PLIST_DIR.mkdir(parents=True, exist_ok=True)
    PLIST_PATH.write_text(_generate_plist(port))

    subprocess.run(
        [
            "launchctl", "bootstrap",
            f"gui/{_get_uid()}", str(PLIST_PATH),
        ],
        check=True,
    )
    typer.echo("see-agent started (launchd)")
    typer.echo(f"Log: {LOG_PATH}")

    if not no_browser:
        import webbrowser

        time.sleep(2)
        webbrowser.open(f"http://localhost:{port}")


def _start_foreground(port: int, no_browser: bool) -> None:
    """Run uvicorn in the current process (dev mode)."""
    from see_agent.config import setup_logging

    setup_logging()

    url = f"http://localhost:{port}"
    typer.echo(f"Starting see-agent on {url} (foreground)")

    if not no_browser:
        import threading
        import webbrowser

        threading.Timer(1.5, lambda: webbrowser.open(url)).start()

    import uvicorn

    uvicorn.run(
        "see_agent.server.app:app",
        host="0.0.0.0",
        port=port,
        reload=False,
    )


@app.command()
def stop() -> None:
    """Stop the see-agent server (and all agent subprocesses)."""
    if not _is_running():
        typer.echo("see-agent is not running")
        return

    subprocess.run(
        [
            "launchctl", "bootout",
            f"gui/{_get_uid()}/{PLIST_LABEL}",
        ],
        capture_output=True,
    )
    typer.echo("see-agent stopped")

    PLIST_PATH.unlink(missing_ok=True)

    # Clean up UDS sockets.
    if RUN_DIR.exists():
        for sock in RUN_DIR.glob("*.sock"):
            sock.unlink(missing_ok=True)


@app.command()
def restart(
    port: int = typer.Option(
        8000, "--port", "-p", help="Port to listen on.",
    ),
) -> None:
    """Restart the see-agent server."""
    if _is_running():
        subprocess.run(
            [
                "launchctl", "bootout",
                f"gui/{_get_uid()}/{PLIST_LABEL}",
            ],
            capture_output=True,
        )
        time.sleep(1)

    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    PLIST_DIR.mkdir(parents=True, exist_ok=True)
    PLIST_PATH.write_text(_generate_plist(port))

    subprocess.run(
        [
            "launchctl", "bootstrap",
            f"gui/{_get_uid()}", str(PLIST_PATH),
        ],
        check=True,
    )
    typer.echo("see-agent restarted")

    import webbrowser

    time.sleep(2)
    webbrowser.open(f"http://localhost:{port}")


@app.command()
def status() -> None:
    """Show see-agent service status."""
    if _is_running():
        result = subprocess.run(
            [
                "launchctl", "print",
                f"gui/{_get_uid()}/{PLIST_LABEL}",
            ],
            capture_output=True,
            text=True,
        )
        for line in result.stdout.splitlines():
            low = line.lower()
            if "pid" in low or "state" in low:
                typer.echo(f"  {line.strip()}")
        typer.echo("see-agent is running")
    else:
        typer.echo("see-agent is not running")


@app.command()
def uninstall(
    delete_data: bool = typer.Option(
        False, "--delete-data",
        help="Also delete ~/.see-agent data.",
    ),
) -> None:
    """Uninstall the see-agent launchd service."""
    if _is_running():
        subprocess.run(
            [
                "launchctl", "bootout",
                f"gui/{_get_uid()}/{PLIST_LABEL}",
            ],
            capture_output=True,
        )
        typer.echo("Service stopped")

    PLIST_PATH.unlink(missing_ok=True)
    typer.echo("Plist removed")

    if delete_data:
        import shutil

        data_dir = Path("~/.see-agent").expanduser()
        if data_dir.exists():
            shutil.rmtree(data_dir)
            typer.echo(f"Data deleted: {data_dir}")

    typer.echo("see-agent uninstalled")


@app.command()
def version() -> None:
    """Show see-agent version."""
    typer.echo("see-agent v3.1.0")


# ---------------------------------------------------------------------------
# Allow ``python -m see_agent.cli.main``
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    app()
