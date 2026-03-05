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
    "screenshot_interval_ms": 500,
    "show_overlay": True,
    "scaling_enabled": True,
    "soul_path": "~/.see-agent/SOUL.md",
}

WORKSPACE_DIR = Path.home() / ".see-agent"
CONFIG_PATH = WORKSPACE_DIR / "config.json"
SCREENSHOTS_DIR = WORKSPACE_DIR / "screenshots"
LOGS_DIR = WORKSPACE_DIR / "logs"

# Path to bundled workspace templates
_TEMPLATE_DIR = Path(__file__).parent.parent / "workspace"


def ensure_workspace() -> None:
    """Ensure ~/.see-agent/ exists with default files."""
    WORKSPACE_DIR.mkdir(parents=True, exist_ok=True)
    SCREENSHOTS_DIR.mkdir(exist_ok=True)
    LOGS_DIR.mkdir(exist_ok=True)

    if not CONFIG_PATH.exists():
        template_config = _TEMPLATE_DIR / "config.json"
        if template_config.exists():
            shutil.copy(template_config, CONFIG_PATH)
        else:
            CONFIG_PATH.write_text(json.dumps(DEFAULT_CONFIG, indent=4, ensure_ascii=False))

    soul_path = WORKSPACE_DIR / "SOUL.md"
    if not soul_path.exists():
        template_soul = _TEMPLATE_DIR / "SOUL.md"
        if template_soul.exists():
            shutil.copy(template_soul, soul_path)


def load_config() -> dict[str, Any]:
    """Load configuration from config.json, with env var overrides."""
    ensure_workspace()

    if CONFIG_PATH.exists():
        with open(CONFIG_PATH) as f:
            config = json.load(f)
    else:
        config = DEFAULT_CONFIG.copy()

    # Environment variable overrides
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

    # Apply defaults for missing keys
    for key, value in DEFAULT_CONFIG.items():
        if key not in config:
            config[key] = value

    return config


def save_config(config: dict[str, Any]) -> None:
    """Save configuration to config.json."""
    ensure_workspace()
    with open(CONFIG_PATH, "w") as f:
        json.dump(config, f, indent=4, ensure_ascii=False)


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
    logging.getLogger("httpx").setLevel(logging.INFO)
    logging.getLogger("openai").setLevel(logging.INFO)
    logging.getLogger("httpcore").setLevel(logging.INFO)
