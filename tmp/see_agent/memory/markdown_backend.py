"""Markdown-based memory backend with BM25 search.

Stores memories as ``*.md`` files in a directory.  Search uses BM25 scoring
over paragraphs, with character-bigram tokenisation for CJK text.
"""

from __future__ import annotations

import math
import re
from pathlib import Path

from see_agent.memory.base import BaseMemory

# Allowed filename patterns
_VALID_FILENAME_RE = re.compile(r"^(MEMORY\.md|\d{4}-\d{2}-\d{2}\.md)$")

# CJK Unified Ideographs range
_CJK_RE = re.compile(r"[\u4e00-\u9fff]")


class MarkdownMemoryBackend(BaseMemory):
    """Markdown file memory with BM25 paragraph search."""

    def __init__(self, memory_dir: Path) -> None:
        self._dir = memory_dir
        self._dir.mkdir(parents=True, exist_ok=True)

    # ------------------------------------------------------------------ #
    # BaseMemory interface
    # ------------------------------------------------------------------ #

    def search(self, query: str, limit: int = 5) -> list[dict[str, str]]:
        """BM25 search over paragraphs from all ``*.md`` files."""
        query_tokens = self._tokenize(query)
        if not query_tokens:
            return []

        # Collect all paragraphs from all md files.
        paragraphs: list[tuple[str, str]] = []  # (filename, text)
        for md_file in sorted(self._dir.glob("*.md")):
            text = md_file.read_text(encoding="utf-8")
            for para in self._split_paragraphs(text):
                paragraphs.append((md_file.name, para))

        if not paragraphs:
            return []

        # Tokenize all paragraphs.
        doc_tokens = [self._tokenize(p[1]) for p in paragraphs]
        n = len(paragraphs)
        avg_dl = sum(len(t) for t in doc_tokens) / n if n else 1

        # IDF for each query term.
        df: dict[str, int] = {}
        for qt in query_tokens:
            df[qt] = sum(1 for dt in doc_tokens if qt in dt)

        # BM25 scoring (k1=1.5, b=0.75).
        k1 = 1.5
        b = 0.75
        scores: list[tuple[float, int]] = []
        for i, dt in enumerate(doc_tokens):
            score = 0.0
            dl = len(dt)
            for qt in query_tokens:
                tf = dt.count(qt)
                if tf == 0:
                    continue
                idf = math.log((n - df[qt] + 0.5) / (df[qt] + 0.5) + 1)
                score += idf * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl / avg_dl))
            if score > 0:
                scores.append((score, i))

        scores.sort(key=lambda x: x[0], reverse=True)
        results: list[dict[str, str]] = []
        for _, idx in scores[:limit]:
            filename, snippet = paragraphs[idx]
            results.append({"file": filename, "snippet": snippet})
        return results

    def write(self, file: str, content: str) -> None:
        """Append *content* to a memory file.

        Only ``MEMORY.md`` and ``YYYY-MM-DD.md`` filenames are allowed.
        """
        if not _VALID_FILENAME_RE.match(file):
            raise ValueError(
                f"Invalid memory filename: {file!r}. "
                "Only MEMORY.md or YYYY-MM-DD.md are allowed."
            )
        path = self._dir / file
        # Append with a blank line separator.
        existing = ""
        if path.exists():
            existing = path.read_text(encoding="utf-8")
        separator = "\n\n" if existing and not existing.endswith("\n\n") else (
            "\n" if existing and not existing.endswith("\n") else ""
        )
        path.write_text(existing + separator + content + "\n", encoding="utf-8")

    # ------------------------------------------------------------------ #
    # Tokenization
    # ------------------------------------------------------------------ #

    @staticmethod
    def _tokenize(text: str) -> list[str]:
        """Tokenize text: whitespace split for ASCII, character bigrams for CJK."""
        tokens: list[str] = []
        text_lower = text.lower()

        # Split into segments: CJK runs vs non-CJK runs.
        segments = re.split(r"([\u4e00-\u9fff]+)", text_lower)
        for seg in segments:
            if not seg:
                continue
            if _CJK_RE.search(seg):
                # CJK: character bigrams
                for i in range(len(seg) - 1):
                    tokens.append(seg[i : i + 2])
                # Also add individual characters for single-char matches
                if len(seg) == 1:
                    tokens.append(seg)
            else:
                # Non-CJK: whitespace split, strip punctuation
                for word in seg.split():
                    word = re.sub(r"[^\w]", "", word)
                    if word:
                        tokens.append(word)
        return tokens

    @staticmethod
    def _split_paragraphs(text: str) -> list[str]:
        """Split markdown text into non-empty paragraphs."""
        paragraphs: list[str] = []
        for para in re.split(r"\n\s*\n", text):
            para = para.strip()
            if para:
                paragraphs.append(para)
        return paragraphs
