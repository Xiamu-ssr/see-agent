"""macOS screen-capture implementation using PyAutoGUI + Pillow."""

from __future__ import annotations

import asyncio
import base64
import io
import logging
import subprocess

import pyautogui
from PIL import Image

from src.eye.base import BaseEye, Screenshot

logger = logging.getLogger(__name__)


def _detect_retina_scale() -> float:
    """Query macOS for the display's backing-scale factor.

    Uses ``system_profiler`` to inspect the main display.  Falls back to
    comparing the PyAutoGUI-reported size against a quick screenshot to
    infer the factor.  Returns ``1.0`` when detection fails.
    """
    # --- Attempt 1: system_profiler -------------------------------------------------
    try:
        output = subprocess.check_output(
            ["system_profiler", "SPDisplaysDataType"],
            text=True,
            timeout=5,
        )
        # Look for "Retina" in the output as a quick heuristic.
        if "Retina" in output:
            logger.debug("Retina display detected via system_profiler.")
            return 2.0
    except Exception:
        logger.debug("system_profiler detection failed, trying fallback.")

    # --- Attempt 2: compare screenshot vs. reported screen size ----------------------
    try:
        screen_w, screen_h = pyautogui.size()  # logical size
        img: Image.Image = pyautogui.screenshot()  # type: ignore[assignment]
        phys_w, phys_h = img.size
        factor = round(phys_w / screen_w, 1)
        if factor >= 1.5:
            logger.debug("Retina scale inferred as %.1f from screenshot comparison.", factor)
            return factor
    except Exception:
        logger.debug("Screenshot-based Retina detection failed.")

    logger.debug("Assuming non-Retina (scale_factor=1.0).")
    return 1.0


class MacEye(BaseEye):
    """Screen-capture backend for macOS.

    On first call to :meth:`capture`, the Retina scale factor is detected
    once and cached for the lifetime of the instance.

    The captured image is always returned at *logical* resolution so that
    pixel coordinates match what PyAutoGUI uses for mouse/keyboard input.
    On Retina displays the raw screenshot from PyAutoGUI already comes at
    physical (2x) resolution, so it is down-scaled with a high-quality
    Lanczos filter before encoding.
    """

    def __init__(self) -> None:
        self._scale_factor: float | None = None
        # Disable PyAutoGUI fail-safe (move to corner to abort) — we
        # only capture, we never move the mouse here.
        pyautogui.FAILSAFE = False

    # ------------------------------------------------------------------ #
    # Internal helpers
    # ------------------------------------------------------------------ #

    def _ensure_scale_factor(self) -> float:
        """Lazily detect and cache the display scale factor."""
        if self._scale_factor is None:
            self._scale_factor = _detect_retina_scale()
            logger.info("Display scale factor: %.1f", self._scale_factor)
        return self._scale_factor

    @staticmethod
    def _image_to_base64(img: Image.Image) -> str:
        """Encode a Pillow Image as a base64 WebP string (no data-URI prefix)."""
        buf = io.BytesIO()
        img.save(buf, format="WEBP", lossless=True)
        return base64.b64encode(buf.getvalue()).decode("ascii")

    # ------------------------------------------------------------------ #
    # Public API
    # ------------------------------------------------------------------ #

    async def capture(self) -> Screenshot:
        """Capture the macOS screen and return a :class:`Screenshot`.

        Steps:
            1. Take a screenshot via PyAutoGUI (runs in a thread to avoid
               blocking the event loop).
            2. Detect the Retina scaling factor (cached after first call).
            3. If Retina (scale > 1), resize down to logical resolution
               using Lanczos resampling so coordinates stay consistent.
            4. Encode as WebP (quality=100) and wrap in a :class:`Screenshot`.

        Raises:
            RuntimeError: If PyAutoGUI fails to capture the screen.
        """
        scale = self._ensure_scale_factor()

        # PyAutoGUI.screenshot() is synchronous and involves IO — offload
        # to the default executor so we don't block the async loop.
        loop = asyncio.get_running_loop()
        try:
            raw_img: Image.Image = await loop.run_in_executor(None, pyautogui.screenshot)  # type: ignore[arg-type]
        except Exception as exc:
            logger.error("PyAutoGUI screenshot failed: %s", exc)
            raise RuntimeError(f"Screen capture failed: {exc}") from exc

        physical_w, physical_h = raw_img.size
        logger.debug("Raw screenshot size: %dx%d", physical_w, physical_h)

        # On Retina displays PyAutoGUI returns an image at physical (2x)
        # resolution.  Resize to logical resolution so that all coordinates
        # used elsewhere (mouse clicks, bounding boxes, etc.) are consistent.
        if scale > 1.0:
            logical_w = int(physical_w / scale)
            logical_h = int(physical_h / scale)
            img = raw_img.resize((logical_w, logical_h), Image.Resampling.LANCZOS)
            logger.debug(
                "Resized from %dx%d to %dx%d (scale=%.1f)",
                physical_w,
                physical_h,
                logical_w,
                logical_h,
                scale,
            )
        else:
            img = raw_img
            logical_w, logical_h = physical_w, physical_h

        b64 = self._image_to_base64(img)

        screenshot = Screenshot(
            base64=b64,
            width=logical_w,
            height=logical_h,
            scale_factor=scale,
            image=img,
        )

        logger.info(
            "Captured screenshot: %dx%d (detail=%s, scale=%.1f, b64_len=%d)",
            screenshot.width,
            screenshot.height,
            screenshot.detail,
            screenshot.scale_factor,
            len(b64),
        )

        return screenshot
