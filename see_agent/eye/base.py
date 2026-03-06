"""Abstract base class for visual perception (eye module)."""

from __future__ import annotations

import base64
import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class Screenshot:
    """Immutable container for a captured screenshot.

    Attributes:
        base64: Image encoded as a base64 string (no data-URI prefix).
        width: Logical width in pixels (CSS / point resolution).
        height: Logical height in pixels (CSS / point resolution).
        scale_factor: Ratio of physical to logical pixels (e.g. 2.0 on Retina).
        mime_type: MIME type of the encoded image (e.g. ``"image/webp"``).
        screen_width: Original screen width before scaling for the LLM.
            ``None`` means no scaling was applied (width == screen width).
        screen_height: Original screen height before scaling for the LLM.
        image: Optional PIL Image object retained from capture, so that
            downstream scaling can resize from the lossless source instead
            of decoding the base64-encoded WebP (avoiding double compression).
    """

    base64: str
    width: int
    height: int
    scale_factor: float = field(default=1.0)
    mime_type: str = field(default="image/webp")
    screen_width: int | None = field(default=None)
    screen_height: int | None = field(default=None)
    image: Any = field(default=None, repr=False, compare=False)

    # --------------------------------------------------------------------- #
    # Derived helpers
    # --------------------------------------------------------------------- #

    @property
    def physical_width(self) -> int:
        """Physical (device) width in pixels."""
        return int(self.width * self.scale_factor)

    @property
    def physical_height(self) -> int:
        """Physical (device) height in pixels."""
        return int(self.height * self.scale_factor)

    @property
    def detail(self) -> Literal["low", "high"]:
        """Recommended OpenAI vision detail level.

        ``"low"`` when both dimensions fit within 1024 px; ``"high"`` otherwise.
        """
        if self.width <= 1024 and self.height <= 1024:
            return "low"
        return "high"

    @property
    def _extension(self) -> str:
        """File extension derived from :attr:`mime_type` (e.g. ``".webp"``)."""
        return "." + self.mime_type.split("/")[-1]

    # --------------------------------------------------------------------- #
    # Persistence
    # --------------------------------------------------------------------- #

    def save(self, path: str | Path) -> Path:
        """Decode the base64 payload and write the image file to *path*.

        If *path* ends with a different extension than :attr:`mime_type`
        implies, the extension is replaced automatically.  Parent directories
        are created as needed.

        Returns:
            The resolved :class:`Path` that was written.
        """
        dest = Path(path).expanduser().resolve()
        dest = dest.with_suffix(self._extension)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(base64.b64decode(self.base64))
        logger.info(
            "Screenshot saved to %s (%dx%d, scale=%.1f)",
            dest, self.width, self.height, self.scale_factor,
        )
        return dest


class BaseEye(ABC):
    """Abstract base for screen-capture backends.

    Every concrete implementation must provide :meth:`capture`, which returns a
    :class:`Screenshot` with the image encoded as base64.
    """

    @abstractmethod
    async def capture(self) -> Screenshot:
        """Capture the current screen and return a :class:`Screenshot`.

        Raises:
            RuntimeError: If the capture fails for any platform-specific reason.
        """
        ...
