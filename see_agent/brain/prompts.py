"""System prompt builder — agent-file-driven injection.

v4: Prompts are assembled from agent directory files (IDENTITY.md,
AGENTS.md, SOUL.md) plus memory/MEMORY.md, optional skills, and team
context sections.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from see_agent.skill.loader import SkillInfo

# Agent files to inject, in order.
_AGENT_FILES = [
    "IDENTITY.md",
    "AGENTS.md",
    "SOUL.md",
]

_DEFAULT_MAX_CHARS_PER_FILE = 20_000
_DEFAULT_TOTAL_MAX_CHARS = 100_000


def _inject_agent_files(
    agent_dir: Path,
    *,
    max_chars: int = _DEFAULT_MAX_CHARS_PER_FILE,
    total_max: int = _DEFAULT_TOTAL_MAX_CHARS,
) -> str:
    """Read agent files from *agent_dir* and return concatenated content.

    Reads IDENTITY.md, AGENTS.md, SOUL.md from agent_dir, plus
    memory/MEMORY.md. Each file is truncated to *max_chars* characters,
    and the total output is truncated to *total_max* characters.
    """
    if not agent_dir.is_dir():
        return ""

    # Build file paths: agent root md files + memory/MEMORY.md
    file_paths = [agent_dir / fname for fname in _AGENT_FILES]
    file_paths.append(agent_dir / "memory" / "MEMORY.md")

    parts: list[str] = []
    total = 0

    for fpath in file_paths:
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

    v4: The prompt is assembled from agent directory files instead of
    hardcoded rules. A minimal identity line is always present. Everything
    else comes from the agent directory itself.

    Parameters:
        config: Application configuration dict.
        skills: Optional loaded skill definitions.
        team_context: Optional team context block.
        agent_dir: Path to the agent directory.
            Falls back to config["_agent_dir"] for backward compatibility.
    """
    max_steps: int = config.get("agent", {}).get("max_steps", 50)

    parts: list[str] = []

    # ── 1. Agent file injection ─────────────────────────────────────
    resolved_dir: Path | None = None
    if agent_dir is not None:
        resolved_dir = Path(agent_dir)
    else:
        agent_dir_str: str | None = config.get("_agent_dir")
        if agent_dir_str:
            resolved_dir = Path(agent_dir_str)

    if resolved_dir is not None:
        agent_content = _inject_agent_files(resolved_dir)
        if agent_content:
            parts.append(agent_content)

    # ── 2. Constraints (always present) ──────────────────────────────
    parts.append(
        f"最多执行 {max_steps} 步。"
        "不要执行危险的 shell 命令。"
        "不要访问或泄露密码、密钥等敏感信息。"
    )

    # ── 3. Skills (optional) ─────────────────────────────────────────
    if skills:
        active = [s for s in skills if not s.blocked]
        if active:
            skill_lines = [f"- **{s.name}**: {s.description}" for s in active]
            parts.append(
                "<SKILLS>\n" + "\n".join(skill_lines) + "\n</SKILLS>"
            )

    # ── 4. Team context (optional) ───────────────────────────────────
    if team_context:
        parts.append(f"<TEAM_CONTEXT>\n{team_context}\n</TEAM_CONTEXT>")

    return "\n\n".join(parts)
