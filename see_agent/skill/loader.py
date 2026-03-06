"""Load SKILL.md files from configured directories.

Each SKILL.md uses a simple YAML-like frontmatter (parsed without PyYAML)::

    ---
    name: open-browser
    description: Open a URL in Safari or Chrome.
    ---
    Step 1: ...
    Step 2: ...
"""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass
from pathlib import Path

logger = logging.getLogger(__name__)

_FRONTMATTER_RE = re.compile(r"^---\s*\n(.*?)\n---\s*\n", re.DOTALL)
_KV_RE = re.compile(r"^(\w+)\s*:\s*(.+)$", re.MULTILINE)


@dataclass
class SkillInfo:
    """Parsed representation of a single SKILL.md file."""

    name: str
    description: str
    body: str
    path: Path


def _parse_skill(path: Path) -> SkillInfo | None:
    """Parse a single SKILL.md file, returning ``None`` on failure."""
    try:
        text = path.read_text(encoding="utf-8")
    except Exception:
        logger.warning("Cannot read skill file: %s", path)
        return None

    match = _FRONTMATTER_RE.match(text)
    if not match:
        logger.warning("No frontmatter in skill file: %s", path)
        return None

    frontmatter = match.group(1)
    body = text[match.end():].strip()

    meta: dict[str, str] = {}
    for kv in _KV_RE.finditer(frontmatter):
        meta[kv.group(1).strip()] = kv.group(2).strip()

    name = meta.get("name")
    description = meta.get("description", "")

    if not name:
        logger.warning("Skill file missing 'name': %s", path)
        return None

    return SkillInfo(name=name, description=description, body=body, path=path)


def load_skills(dirs: list[str]) -> list[SkillInfo]:
    """Scan directories for SKILL.md files and parse them.

    Parameters:
        dirs: List of directory paths (may contain ``~``).  Non-existent
            directories are silently skipped.

    Returns:
        A list of successfully parsed :class:`SkillInfo` objects.
    """
    skills: list[SkillInfo] = []
    seen_names: set[str] = set()

    for d in dirs:
        dir_path = Path(d).expanduser()
        if not dir_path.is_dir():
            continue
        for skill_file in sorted(dir_path.glob("**/SKILL.md")):
            info = _parse_skill(skill_file)
            if info is None:
                continue
            if info.name in seen_names:
                logger.warning("Duplicate skill name '%s' in %s, skipping", info.name, skill_file)
                continue
            seen_names.add(info.name)
            skills.append(info)

    logger.info("Loaded %d skills from %d dirs", len(skills), len(dirs))
    return skills
