"""Configuration loading: ~/.see-agent/config.json + environment variables."""

import json
import logging
import logging.handlers
import os
import shutil
from datetime import datetime
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

DEFAULT_CONFIG: dict[str, Any] = {
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o",
    },
    "language": "zh",
    "max_steps": 50,
    "max_images": 5,
    "screenshot_interval_ms": 800,
    "tool_delay_ms": 200,
    "show_overlay": True,
    "scaling_enabled": True,
    "scaling_match": "aspect_ratio",
    "soul_path": None,
    "skills_dirs": ["~/.see-agent/skills"],
    "context_engine": "legacy",
    "memory": {
        "enabled": False,
        "provider": "file",
        "mem0": {
            "llm_base_url": "",
            "llm_api_key": "",
            "llm_model": "",
            "embedding_model": "",
            "storage_path": "~/.see-agent/memory/qdrant",
        },
    },
    "compact": {
        "enabled": False,
        "context_window": 128000,
        "target_ratio": 0.75,
        "keep_recent": 8,
        "summary_model": "",
    },
    "env": {},
    "mcp_servers": {},
}

WORKSPACE_DIR = Path.home() / ".see-agent"
CONFIG_PATH = WORKSPACE_DIR / "config.json"
SESSIONS_DIR = WORKSPACE_DIR / "sessions"
LOGS_DIR = WORKSPACE_DIR / "logs"
SKILLS_DIR = WORKSPACE_DIR / "skills"
AGENTS_DIR = WORKSPACE_DIR / "agents"
TEAMS_DIR = WORKSPACE_DIR / "teams"

# Path to bundled workspace templates
_TEMPLATE_DIR = Path(__file__).parent.parent / "workspace"


def ensure_workspace() -> None:
    """Ensure ~/.see-agent/ exists with default files."""
    WORKSPACE_DIR.mkdir(parents=True, exist_ok=True)
    SESSIONS_DIR.mkdir(exist_ok=True)
    LOGS_DIR.mkdir(exist_ok=True)
    SKILLS_DIR.mkdir(exist_ok=True)
    AGENTS_DIR.mkdir(exist_ok=True)
    TEAMS_DIR.mkdir(exist_ok=True)

    if not CONFIG_PATH.exists():
        CONFIG_PATH.write_text(json.dumps(DEFAULT_CONFIG, indent=4, ensure_ascii=False))

    soul_path = WORKSPACE_DIR / "SOUL.md"
    if not soul_path.exists():
        template_soul = _TEMPLATE_DIR / "SOUL.md"
        if template_soul.exists():
            shutil.copy(template_soul, soul_path)


def _deep_merge(base: dict[str, Any], overlay: dict[str, Any]) -> dict[str, Any]:
    """Recursively merge *overlay* into *base*, returning a new dict.

    Nested dicts are merged recursively; all other values in *overlay*
    overwrite those in *base*.
    """
    result = base.copy()
    for key, value in overlay.items():
        if (
            key in result
            and isinstance(result[key], dict)
            and isinstance(value, dict)
        ):
            result[key] = _deep_merge(result[key], value)
        else:
            result[key] = value
    return result


def load_config() -> dict[str, Any]:
    """Load configuration with priority: DEFAULT → config.json → env vars."""
    ensure_workspace()

    if CONFIG_PATH.exists():
        with open(CONFIG_PATH) as f:
            config = json.load(f)
    else:
        config = DEFAULT_CONFIG.copy()

    # Apply defaults for missing keys (deep merge with DEFAULT_CONFIG as base).
    config = _deep_merge(DEFAULT_CONFIG, config)

    # Environment variable overrides (highest priority)
    env_base_url = os.environ.get("SEE_AGENT_BASE_URL")
    env_api_key = os.environ.get("SEE_AGENT_API_KEY")
    env_model = os.environ.get("SEE_AGENT_MODEL")

    if "llm" not in config:
        config["llm"] = {}

    if env_base_url:
        config["llm"]["base_url"] = env_base_url
    if env_api_key:
        config["llm"]["api_key"] = env_api_key
    if env_model:
        config["llm"]["model"] = env_model

    return config


def save_config(config: dict[str, Any]) -> None:
    """Save configuration to config.json."""
    ensure_workspace()
    with open(CONFIG_PATH, "w") as f:
        json.dump(config, f, indent=4, ensure_ascii=False)


def load_agent_config(agent_id: str) -> dict[str, Any]:
    """Load merged config for *agent_id*: global → agent.json → env vars.

    Returns the final merged dict.  Raises ``FileNotFoundError`` when the
    agent directory or ``agent.json`` does not exist.
    """
    global_config = load_config()
    agent_dir = AGENTS_DIR / agent_id
    agent_json = agent_dir / "agent.json"
    if not agent_json.exists():
        raise FileNotFoundError(f"Agent config not found: {agent_json}")

    with open(agent_json) as f:
        agent_data = json.load(f)

    overrides = agent_data.get("config_overrides", {})
    return _deep_merge(global_config, overrides)


_logging_configured = False


def setup_logging() -> None:
    """Configure the root logger to write to ``~/.see-agent/logs/YYYY-MM-DD.log``.

    Uses :class:`RotatingFileHandler` (10 MB per file, 5 backups) so logs
    don't grow unbounded.  ``httpx`` and ``openai`` loggers are pinned to
    INFO to prevent base64 request bodies from bloating the log.

    Safe to call multiple times — only the first invocation takes effect.
    """
    global _logging_configured  # noqa: PLW0603
    if _logging_configured:
        return
    _logging_configured = True

    ensure_workspace()
    log_file = LOGS_DIR / f"{datetime.now().strftime('%Y-%m-%d')}.log"

    file_handler = logging.handlers.RotatingFileHandler(
        log_file,
        maxBytes=10 * 1024 * 1024,  # 10 MB
        backupCount=5,
        encoding="utf-8",
    )
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(
        logging.Formatter(
            "%(asctime)s  %(levelname)-8s  %(name)s  %(message)s",
            datefmt="%H:%M:%S",
        )
    )

    root = logging.getLogger()
    root.setLevel(logging.DEBUG)
    root.addHandler(file_handler)

    # Suppress verbose DEBUG logs from HTTP clients that dump full request
    # bodies (including base64 screenshot payloads).
    logging.getLogger("httpx").setLevel(logging.WARNING)
    logging.getLogger("openai").setLevel(logging.INFO)
    logging.getLogger("httpcore").setLevel(logging.INFO)
    logging.getLogger("PIL").setLevel(logging.WARNING)

    # Session-related loggers: WARNING on global file, DEBUG detail goes to
    # per-session log files via Session.setup_logging().
    for _name in (
        "see_agent.agent", "see_agent.brain",
        "see_agent.eye", "see_agent.hand",
    ):
        logging.getLogger(_name).setLevel(logging.WARNING)
