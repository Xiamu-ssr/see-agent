"""Abstract base class for visual perception (eye module)."""

from __future__ import annotations

import base64
import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class Screenshot:
    """Immutable container for a captured screenshot.

    Attributes:
        base64: PNG image encoded as a base64 string (no data-URI prefix).
        width: Logical width in pixels (CSS / point resolution).
        height: Logical height in pixels (CSS / point resolution).
        scale_factor: Ratio of physical to logical pixels (e.g. 2.0 on Retina).
    """

    base64: str
    width: int
    height: int
    scale_factor: float = field(default=1.0)

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

    # --------------------------------------------------------------------- #
    # Persistence
    # --------------------------------------------------------------------- #

    def save(self, path: str | Path) -> Path:
        """Decode the base64 payload and write the PNG file to *path*.

        Parent directories are created automatically.

        Returns:
            The resolved :class:`Path` that was written.
        """
        dest = Path(path).expanduser().resolve()
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
    :class:`Screenshot` encoded as a lossless PNG in base64.
    """

    @abstractmethod
    async def capture(self) -> Screenshot:
        """Capture the current screen and return a :class:`Screenshot`.

        Raises:
            RuntimeError: If the capture fails for any platform-specific reason.
        """
        ...
