"""Coordinate scaling utilities for sending smaller screenshots to the LLM.

When a screenshot is captured at high logical resolution (e.g. 1728x1117),
sending it directly to the LLM wastes tokens and can reduce click precision
because the model must reason over a larger coordinate space.

This module provides:
- :func:`find_target_resolution` — pick a standard resolution that matches
  the screen's aspect ratio.
- :func:`scale_screenshot` — resize a :class:`Screenshot` for the LLM,
  preserving the original screen dimensions for reverse mapping.
- :func:`scale_coordinates` — map model-space coordinates back to screen space.
"""

from __future__ import annotations

import base64
import io
import logging
from typing import Any

from PIL import Image

from src.eye.base import Screenshot

logger = logging.getLogger(__name__)

# Standard target resolutions (width, height).
# Chosen to cover common monitor aspect ratios.
SCALING_TARGETS: list[tuple[int, int]] = [
    (1024, 768),   # XGA   — 4:3   (1.333)
    (1280, 800),   # WXGA  — 16:10 (1.600)
    (1366, 768),   # FWXGA — ~16:9 (1.779)
]

ASPECT_TOLERANCE = 0.05  # 5%


def find_target_resolution(
    width: int,
    height: int,
    match_mode: str = "aspect_ratio",
) -> tuple[int, int] | None:
    """Return the best standard target for the given screen dimensions.

    Parameters:
        width: Source image width.
        height: Source image height.
        match_mode: ``"aspect_ratio"`` (default) picks the target whose
            aspect ratio is closest within ``ASPECT_TOLERANCE``.
            ``"pixel_count"`` picks the target whose total pixel count
            is closest to the source (still only from targets smaller
            than the source).

    Returns:
        ``None`` if no target qualifies or the image is already small.
    """
    if width <= 0 or height <= 0:
        return None

    # Filter to targets strictly smaller than the source.
    candidates = [
        (tw, th) for tw, th in SCALING_TARGETS
        if not (tw >= width and th >= height)
    ]
    if not candidates:
        return None

    if match_mode == "pixel_count":
        src_pixels = width * height
        best: tuple[int, int] | None = None
        best_diff = float("inf")
        for tw, th in candidates:
            diff = abs(src_pixels - tw * th)
            if diff < best_diff:
                best = (tw, th)
                best_diff = diff
        return best

    # Default: aspect_ratio matching.
    aspect = width / height
    best_ar: tuple[int, int] | None = None
    best_ar_diff = float("inf")

    for tw, th in candidates:
        target_aspect = tw / th
        diff = abs(aspect - target_aspect) / aspect
        if diff < ASPECT_TOLERANCE and diff < best_ar_diff:
            best_ar = (tw, th)
            best_ar_diff = diff

    return best_ar


def scale_screenshot(screenshot: Screenshot, target: tuple[int, int]) -> Screenshot:
    """Resize *screenshot* to *target* resolution for the LLM.

    The returned :class:`Screenshot` carries the original screen dimensions in
    ``screen_width`` / ``screen_height`` so that coordinates from the LLM can
    be mapped back.

    When the source :class:`Screenshot` carries an ``image`` (PIL Image), we
    resize from that lossless source to avoid double WebP compression.
    """
    tw, th = target
    if screenshot.width == tw and screenshot.height == th:
        return screenshot

    # Prefer the retained PIL Image (lossless) over base64-decode (lossy).
    if screenshot.image is not None:
        img = screenshot.image.resize((tw, th), Image.Resampling.LANCZOS)
    else:
        img_data = base64.b64decode(screenshot.base64)
        img = Image.open(io.BytesIO(img_data))
        img = img.resize((tw, th), Image.Resampling.LANCZOS)

    buf = io.BytesIO()
    img.save(buf, format="WEBP", lossless=True)
    b64 = base64.b64encode(buf.getvalue()).decode("ascii")

    logger.info(
        "Scaled screenshot %dx%d -> %dx%d (b64_len %d -> %d)",
        screenshot.width, screenshot.height, tw, th,
        len(screenshot.base64), len(b64),
    )

    return Screenshot(
        base64=b64,
        width=tw,
        height=th,
        scale_factor=screenshot.scale_factor,
        mime_type="image/webp",
        screen_width=screenshot.width,
        screen_height=screenshot.height,
        image=img,
    )


def scale_coordinates(
    x: int | Any,
    y: int | Any,
    model_width: int,
    model_height: int,
    screen_width: int,
    screen_height: int,
) -> tuple[int, int]:
    """Map coordinates from model (scaled) space back to screen space.

    *x* and *y* are coerced to ``int`` so that string values returned by
    some LLMs (e.g. ``"640"``) are handled gracefully.  Raises
    :class:`ValueError` if conversion fails.
    """
    x = int(x)
    y = int(y)
    sx = round(x * screen_width / model_width)
    sy = round(y * screen_height / model_height)
    return sx, sy


def scale_tool_args(
    tool_name: str,
    args: dict[str, Any],
    model_width: int,
    model_height: int,
    screen_width: int,
    screen_height: int,
) -> dict[str, Any]:
    """Return a copy of *args* with coordinates scaled to screen space.

    Only tools that use screen coordinates (``click``, ``drag``, ``scroll``)
    are affected.  Other tools pass through unchanged.
    """
    if tool_name == "click":
        sx, sy = scale_coordinates(
            args["x"], args["y"],
            model_width, model_height, screen_width, screen_height,
        )
        return {**args, "x": sx, "y": sy}

    if tool_name == "drag":
        sx1, sy1 = scale_coordinates(
            args["start_x"], args["start_y"],
            model_width, model_height, screen_width, screen_height,
        )
        sx2, sy2 = scale_coordinates(
            args["end_x"], args["end_y"],
            model_width, model_height, screen_width, screen_height,
        )
        return {
            **args,
            "start_x": sx1, "start_y": sy1,
            "end_x": sx2, "end_y": sy2,
        }

    if tool_name == "scroll":
        sx, sy = scale_coordinates(
            args["x"], args["y"],
            model_width, model_height, screen_width, screen_height,
        )
        return {**args, "x": sx, "y": sy}

    return args
