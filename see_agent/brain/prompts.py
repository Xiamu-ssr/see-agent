"""System prompt builder — workspace-driven file injection.

v3.5: Hardcoded RULES/CONSTRAINTS/PERSONALITY removed. Prompts are now
assembled from workspace files (IDENTITY.md, AGENTS.md, SOUL.md, etc.)
plus optional skills and team context sections.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from see_agent.skill.loader import SkillInfo

# Workspace files to inject, in order.
_WORKSPACE_FILES = [
    "IDENTITY.md",
    "AGENTS.md",
    "SOUL.md",
    "TOOLS.md",
    "USER.md",
    "MEMORY.md",
]

_DEFAULT_MAX_CHARS_PER_FILE = 20_000
_DEFAULT_TOTAL_MAX_CHARS = 100_000


def _inject_workspace(
    agent_dir: Path,
    *,
    max_chars: int = _DEFAULT_MAX_CHARS_PER_FILE,
    total_max: int = _DEFAULT_TOTAL_MAX_CHARS,
) -> str:
    """Read workspace files from *agent_dir*/workspace and return concatenated content.

    Each file is truncated to *max_chars* characters, and the total output
    is truncated to *total_max* characters.
    """
    workspace = agent_dir / "workspace"
    if not workspace.is_dir():
        return ""

    parts: list[str] = []
    total = 0

    for fname in _WORKSPACE_FILES:
        fpath = workspace / fname
        if not fpath.exists():
            continue
        content = fpath.read_text(encoding="utf-8").strip()
        if not content:
            continue
        if len(content) > max_chars:
            content = content[:max_chars] + "\n... (truncated)"
        remaining = total_max - total
        if remaining <= 0:
            break
        if len(content) > remaining:
            content = content[:remaining] + "\n... (truncated)"
        parts.append(content)
        total += len(content)

    return "\n\n".join(parts)


def build_system_prompt(
    config: dict,
    *,
    skills: list[SkillInfo] | None = None,
    team_context: str = "",
    agent_dir: Path | str | None = None,
) -> str:
    """Build the full system prompt.

    v3.5: The prompt is assembled from workspace files instead of hardcoded
    rules.  A minimal identity line is always present.  Everything else
    comes from the agent's workspace/ directory.

    Parameters:
        config: Application configuration dict.
        skills: Optional loaded skill definitions.
        team_context: Optional team context block.
        agent_dir: Path to the agent directory (contains workspace/).
            Falls back to config["_agent_dir"] for backward compatibility.
    """
    lang: str = config.get("language", "zh")
    max_steps: int = config.get("max_steps", 50)

    parts: list[str] = []

    # ── 1. Minimal identity ──────────────────────────────────────────
    if lang == "zh":
        parts.append(
            "你是一个能操作 Mac 电脑的 AI 助手。"
            "你可以看到屏幕截图，并通过工具操作鼠标、键盘和终端。"
        )
    else:
        parts.append(
            "You are an AI assistant that can operate a Mac computer. "
            "You can see screenshots and operate the mouse, keyboard, "
            "and terminal through tools."
        )

    # ── 2. Workspace file injection ──────────────────────────────────
    resolved_dir: Path | None = None
    if agent_dir is not None:
        resolved_dir = Path(agent_dir)
    else:
        # Backward compat: check config["_agent_dir"]
        agent_dir_str: str | None = config.get("_agent_dir")
        if agent_dir_str:
            resolved_dir = Path(agent_dir_str)

    if resolved_dir is not None:
        workspace_content = _inject_workspace(resolved_dir)
        if workspace_content:
            parts.append(workspace_content)

    # ── 3. Constraints (always present, minimal) ─────────────────────
    if lang == "zh":
        parts.append(
            f"最多执行 {max_steps} 步。"
            "不要执行危险的 shell 命令。"
            "不要访问或泄露密码、密钥等敏感信息。"
        )
    else:
        parts.append(
            f"Maximum {max_steps} steps per task. "
            "Do NOT execute dangerous shell commands. "
            "Do NOT access or leak passwords, secrets, or sensitive information."
        )

    # ── 4. Skills (optional) ─────────────────────────────────────────
    if skills:
        active = [s for s in skills if not s.blocked]
        if active:
            skill_lines = [f"- **{s.name}**: {s.description}" for s in active]
            parts.append(
                "<SKILLS>\n" + "\n".join(skill_lines) + "\n</SKILLS>"
            )

    # ── 5. Team context (optional) ───────────────────────────────────
    if team_context:
        parts.append(f"<TEAM_CONTEXT>\n{team_context}\n</TEAM_CONTEXT>")

    return "\n\n".join(parts)
