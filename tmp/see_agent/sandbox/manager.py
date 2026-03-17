"""Sandbox profile generator — combines Safehouse base profiles with
see-agent fixed layer and per-agent dynamic rules.

Usage::

    gen = SandboxProfileGenerator()
    profile_path = gen.generate(
        agent_id="alice",
        team_id="team-abc",
        team_dir=Path("~/.see-agent/teams/team-abc"),
        sandbox_cfg={"enabled": True, "network": True, "screen_access": True},
    )
    # Then: sandbox-exec -f <profile_path> python -m see_agent.agent.worker ...
"""

from __future__ import annotations

import logging
import tempfile
from pathlib import Path

logger = logging.getLogger(__name__)

PROFILES_DIR = Path(__file__).parent / "profiles"

# Profiles always included.
_ALWAYS = [
    "00-base.sb",
    "10-system-runtime.sb",
    "30-toolchains/python.sb",
    "30-toolchains/runtime-managers.sb",
    "40-shared/agent-common.sb",
    "50-integrations-core/git.sb",
    "50-integrations-core/scm-clis.sb",
    "55-integrations-optional/shell-init.sb",
    "see-agent-base.sb",
]

# Conditional profiles.
_NETWORK = "20-network.sb"
_GUI = "55-integrations-optional/macos-gui.sb"
_CLIPBOARD = "55-integrations-optional/clipboard.sb"
_NODE = "30-toolchains/node.sb"


class SandboxProfileGenerator:
    """Generate a combined sandbox-exec profile for an agent subprocess."""

    def generate(
        self,
        agent_id: str,
        team_id: str,
        team_dir: Path,
        sandbox_cfg: dict[str, object],
        *,
        has_node_mcp: bool = False,
    ) -> Path:
        """Build and write a combined .sb profile, return its path."""
        parts: list[str] = []

        # Always-included profiles.
        for name in _ALWAYS:
            content = self._read_profile(name)
            if content:
                parts.append(content)

        # Conditional profiles.
        if sandbox_cfg.get("network", True):
            content = self._read_profile(_NETWORK)
            if content:
                parts.append(content)

        if sandbox_cfg.get("screen_access", True):
            for name in (_GUI, _CLIPBOARD):
                content = self._read_profile(name)
                if content:
                    parts.append(content)

        if has_node_mcp:
            content = self._read_profile(_NODE)
            if content:
                parts.append(content)

        # Dynamic layer: agent + team directories.
        agent_dir = team_dir / "agents" / agent_id
        shared_dir = team_dir / "shared"
        extra_read = list(sandbox_cfg.get("extra_read", []))  # type: ignore[arg-type]
        extra_write = list(sandbox_cfg.get("extra_write", []))  # type: ignore[arg-type]

        parts.append(self._dynamic_rules(
            agent_id, team_id, agent_dir, shared_dir,
            extra_read, extra_write,
        ))

        # Replace HOME_DIR placeholder with actual home.
        combined = "\n\n".join(parts)
        home = str(Path.home())
        combined = combined.replace(
            "__SAFEHOUSE_REPLACE_ME_WITH_ABSOLUTE_HOME_DIR__", home,
        )

        # Write to tmp dir.
        profile_path = Path(tempfile.gettempdir()) / f"see-agent-{team_id}-{agent_id}.sb"
        profile_path.write_text(combined)
        logger.info("Sandbox profile written: %s", profile_path)
        return profile_path

    # ------------------------------------------------------------------ #
    # Helpers
    # ------------------------------------------------------------------ #

    def _read_profile(self, name: str) -> str | None:
        path = PROFILES_DIR / name
        if not path.exists():
            logger.warning("Sandbox profile not found: %s", path)
            return None
        return path.read_text()

    def _dynamic_rules(
        self,
        agent_id: str,
        team_id: str,
        agent_dir: Path,
        shared_dir: Path,
        extra_read: list[str],
        extra_write: list[str],
    ) -> str:
        lines = [
            f";; Dynamic layer — agent: {agent_id}, team: {team_id}",
            "",
            ";; Agent own directory (read-write)",
            "(allow file-read* file-write*",
            f'    (subpath "{agent_dir}"))',
            "",
            ";; Team shared workspace (read-write)",
            "(allow file-read* file-write*",
            f'    (subpath "{shared_dir}"))',
        ]

        for path in extra_read:
            expanded = str(Path(path).expanduser())
            lines.append(f'(allow file-read* (subpath "{expanded}"))')

        for path in extra_write:
            expanded = str(Path(path).expanduser())
            lines.append(
                f'(allow file-read* file-write* (subpath "{expanded}"))',
            )

        return "\n".join(lines)
