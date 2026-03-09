"""JSONL-based memory backend — zero external dependencies.

Each memory entry is a JSON line in ``{memory_dir}/memories.jsonl``::

    {"text": "...", "session_id": "...", "agent_id": "...", "ts": "..."}

Search uses simple keyword-overlap scoring.
"""

from __future__ import annotations

import json
import logging
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from see_agent.memory.base import BaseMemory

logger = logging.getLogger(__name__)


class FileMemory(BaseMemory):
    """JSONL-based memory.  Keyword-overlap search, zero external deps."""

    def __init__(self, memory_dir: Path | None = None) -> None:
        if memory_dir is None:
            from see_agent.config import WORKSPACE_DIR

            memory_dir = WORKSPACE_DIR / "memory"
        self._dir = memory_dir
        self._dir.mkdir(parents=True, exist_ok=True)

    @property
    def _jsonl_path(self) -> Path:
        return self._dir / "memories.jsonl"

    # ------------------------------------------------------------------ #
    # BaseMemory interface
    # ------------------------------------------------------------------ #

    def search(self, query: str, limit: int = 5, agent_id: str | None = None) -> list[str]:
        """Keyword-overlap search across stored memories.

        Note: uses whitespace-based tokenization (``str.split()``), which
        works for English but is ineffective for CJK languages (Chinese,
        Japanese, Korean) where words are not space-separated.  A future
        version should integrate a proper tokenizer (e.g. jieba) for
        Chinese support.
        """
        entries = self._read_entries()
        if agent_id is not None:
            entries = [e for e in entries if e.get("agent_id") == agent_id]

        query_words = set(query.lower().split())
        if not query_words:
            return []

        scored: list[tuple[float, str]] = []
        for entry in entries:
            text = entry.get("text", "")
            mem_words = set(text.lower().split())
            overlap = len(query_words & mem_words)
            if overlap > 0:
                scored.append((overlap, text))

        scored.sort(key=lambda x: x[0], reverse=True)
        return [text for _, text in scored[:limit]]

    def add(
        self, messages: list[dict[str, Any]], session_id: str, agent_id: str | None = None,
    ) -> None:
        """Extract text from *messages* and persist as memory entries."""
        texts: list[str] = []
        for msg in messages:
            content = msg.get("content")
            if isinstance(content, str) and content.strip():
                texts.append(content.strip())
            elif isinstance(content, list):
                for part in content:
                    if isinstance(part, dict) and part.get("type") == "text":
                        t = part.get("text", "").strip()
                        if t:
                            texts.append(t)

        ts = datetime.now(timezone.utc).isoformat()
        with open(self._jsonl_path, "a", encoding="utf-8") as fh:
            for text in texts:
                entry = {"text": text, "session_id": session_id, "ts": ts}
                if agent_id:
                    entry["agent_id"] = agent_id
                fh.write(json.dumps(entry, ensure_ascii=False) + "\n")

    def clear(self, agent_id: str | None = None) -> None:
        """Clear memories.  If *agent_id* given, only clear that agent's memories."""
        if agent_id is None:
            if self._jsonl_path.exists():
                self._jsonl_path.unlink()
            return

        entries = [e for e in self._read_entries() if e.get("agent_id") != agent_id]
        self._write_entries(entries)

    # ------------------------------------------------------------------ #
    # Internal helpers
    # ------------------------------------------------------------------ #

    def _read_entries(self) -> list[dict[str, Any]]:
        if not self._jsonl_path.exists():
            return []
        entries: list[dict[str, Any]] = []
        for line in self._jsonl_path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line:
                try:
                    entries.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
        return entries

    def _write_entries(self, entries: list[dict[str, Any]]) -> None:
        with open(self._jsonl_path, "w", encoding="utf-8") as fh:
            for entry in entries:
                fh.write(json.dumps(entry, ensure_ascii=False) + "\n")
