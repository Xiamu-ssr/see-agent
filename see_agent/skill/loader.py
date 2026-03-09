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

import json as _json
import logging
import os
import re
import shutil
from dataclasses import dataclass, field
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
    requires_bins: list[str] = field(default_factory=list)
    requires_env: list[str] = field(default_factory=list)
    requires_any_bins: list[str] = field(default_factory=list)
    blocked: bool = False
    block_reason: str = ""


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

    # Parse optional metadata JSON for requirements.
    requires_bins: list[str] = []
    requires_env: list[str] = []
    requires_any_bins: list[str] = []
    raw_meta = meta.get("metadata", "")
    if raw_meta:
        try:
            md = _json.loads(raw_meta)
            if isinstance(md, dict):
                # Direct format: {"requires_bins": [...], ...}
                requires_bins = md.get("requires_bins", [])
                requires_env = md.get("requires_env", [])
                requires_any_bins = md.get("requires_any_bins", [])
                # OpenClaw nested format
                oc = md.get("openclaw", {})
                if isinstance(oc, dict):
                    req = oc.get("requires", {})
                    if isinstance(req, dict):
                        requires_bins = requires_bins or req.get("bins", [])
                        requires_env = requires_env or req.get("env", [])
                        requires_any_bins = requires_any_bins or req.get("anyBins", [])
        except (_json.JSONDecodeError, TypeError):
            logger.debug("Invalid metadata JSON in skill %s, ignoring", path)

    return SkillInfo(
        name=name, description=description, body=body, path=path,
        requires_bins=requires_bins, requires_env=requires_env,
        requires_any_bins=requires_any_bins,
    )


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


def filter_skills(
    skills: list[SkillInfo],
    disabled: list[str] | None = None,
) -> list[SkillInfo]:
    """Return skills excluding those in *disabled* list."""
    if not disabled:
        return skills
    disabled_set = set(disabled)
    return [s for s in skills if s.name not in disabled_set]


def gate_skills(skills: list[SkillInfo]) -> list[SkillInfo]:
    """Check requirements for each skill and mark unavailable ones as blocked.

    Mutates the *skills* list in-place and returns it.
    """
    for skill in skills:
        reasons: list[str] = []

        # Check required binaries (all must be present).
        for b in skill.requires_bins:
            if not shutil.which(b):
                reasons.append(f"missing binary: {b}")

        # Check required env vars (all must be set).
        for e in skill.requires_env:
            if not os.environ.get(e):
                reasons.append(f"missing env: {e}")

        # Check any-of binaries (at least one must be present).
        if skill.requires_any_bins:
            if not any(shutil.which(b) for b in skill.requires_any_bins):
                reasons.append(f"none of binaries available: {skill.requires_any_bins}")

        if reasons:
            skill.blocked = True
            skill.block_reason = "; ".join(reasons)
            logger.info("Skill '%s' blocked: %s", skill.name, skill.block_reason)

    return skills
