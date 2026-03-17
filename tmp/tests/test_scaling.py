"""Tests for coordinate scaling utilities (src/eye/scaling.py)."""

from __future__ import annotations

import pytest
from PIL import Image

from see_agent.eye.base import Screenshot
from see_agent.eye.scaling import (
    find_target_resolution,
    scale_coordinates,
    scale_screenshot,
    scale_tool_args,
)

# Valid 2x2 red PNG that Pillow can decode, resize, and re-encode as WebP.
_VALID_PNG_B64 = (
    "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAA"
    "EElEQVR4nGP8zwACTGCSAQANHQEDgslx/wAAAABJRU5ErkJggg=="
)


# -------------------------------------------------------------------- #
# find_target_resolution
# -------------------------------------------------------------------- #


class TestFindTargetResolution:
    """Tests for target resolution selection."""

    def test_16_10_matches_wxga(self):
        """1728×1080 (~16:10) should match WXGA 1280×800."""
        target = find_target_resolution(1728, 1080)
        assert target == (1280, 800)

    def test_16_9_matches_fwxga(self):
        """1920×1080 (16:9) should match FWXGA 1366×768."""
        target = find_target_resolution(1920, 1080)
        assert target == (1366, 768)

    def test_4_3_matches_xga(self):
        """1600×1200 (4:3) should match XGA 1024×768."""
        target = find_target_resolution(1600, 1200)
        assert target == (1024, 768)

    def test_mac_m4_aspect_ratio_returns_wxga(self):
        """1728×1117 (ratio 1.547) — aspect_ratio mode matches 1280×800 (ratio 1.6, diff 3.4%)."""
        target = find_target_resolution(1728, 1117, match_mode="aspect_ratio")
        assert target == (1280, 800)

    def test_mac_m4_pixel_count_returns_closest(self):
        """1728×1117 — pixel_count mode picks the target with closest pixel count."""
        target = find_target_resolution(1728, 1117, match_mode="pixel_count")
        # 1728*1117 = 1,930,176 pixels
        # 1366*768 = 1,049,088 (diff 881,088)
        # 1280*800 = 1,024,000 (diff 906,176)
        # 1024*768 =   786,432 (diff 1,143,744)
        assert target == (1366, 768)

    def test_already_small_returns_none(self):
        """800×600 is smaller than any target → None."""
        assert find_target_resolution(800, 600) is None

    def test_exact_target_returns_none(self):
        """1280×800 exactly equals WXGA → None (no down-scale needed)."""
        assert find_target_resolution(1280, 800) is None

    def test_unusual_aspect_returns_none(self):
        """21:9 ultra-wide has no matching target."""
        assert find_target_resolution(2560, 1080) is None

    def test_zero_dimensions_returns_none(self):
        assert find_target_resolution(0, 0) is None

    def test_pixel_count_small_returns_none(self):
        """pixel_count mode also returns None when source is smaller."""
        assert find_target_resolution(800, 600, match_mode="pixel_count") is None


# -------------------------------------------------------------------- #
# scale_coordinates
# -------------------------------------------------------------------- #


class TestScaleCoordinates:
    """Tests for model→screen coordinate mapping."""

    def test_identity_same_dimensions(self):
        sx, sy = scale_coordinates(100, 200, 1280, 800, 1280, 800)
        assert (sx, sy) == (100, 200)

    def test_upscale_2x(self):
        sx, sy = scale_coordinates(100, 200, 640, 400, 1280, 800)
        assert (sx, sy) == (200, 400)

    def test_real_case(self):
        """1280×800 model → 1728×1080 screen."""
        sx, sy = scale_coordinates(640, 400, 1280, 800, 1728, 1080)
        assert sx == round(640 * 1728 / 1280)
        assert sy == round(400 * 1080 / 800)

    def test_string_coordinates_coerced(self):
        """LLM may return string coordinates; they should be coerced to int."""
        sx, sy = scale_coordinates("100", "200", 1280, 800, 1280, 800)  # type: ignore[arg-type]
        assert (sx, sy) == (100, 200)

    def test_invalid_coordinate_raises(self):
        """Non-numeric strings should raise ValueError."""
        with pytest.raises(ValueError):
            scale_coordinates("abc", "200", 1280, 800, 1280, 800)  # type: ignore[arg-type]


# -------------------------------------------------------------------- #
# scale_tool_args
# -------------------------------------------------------------------- #


class TestScaleToolArgs:
    """Tests for per-tool coordinate scaling."""

    def test_click_scaled(self):
        args = {"x": 640, "y": 400, "button": "left"}
        result = scale_tool_args("click", args, 1280, 800, 1920, 1200)
        assert result["x"] == round(640 * 1920 / 1280)
        assert result["y"] == round(400 * 1200 / 800)
        assert result["button"] == "left"

    def test_click_string_coords(self):
        """String coordinates from LLM should be handled without error."""
        args = {"x": "640", "y": "400"}
        result = scale_tool_args("click", args, 1280, 800, 1920, 1200)
        assert result["x"] == round(640 * 1920 / 1280)
        assert result["y"] == round(400 * 1200 / 800)

    def test_drag_scaled(self):
        args = {"start_x": 100, "start_y": 200, "end_x": 300, "end_y": 400}
        result = scale_tool_args("drag", args, 1280, 800, 1920, 1200)
        assert result["start_x"] == round(100 * 1920 / 1280)
        assert result["start_y"] == round(200 * 1200 / 800)
        assert result["end_x"] == round(300 * 1920 / 1280)
        assert result["end_y"] == round(400 * 1200 / 800)

    def test_scroll_scaled(self):
        args = {"x": 500, "y": 300, "direction": "down", "amount": 3}
        result = scale_tool_args("scroll", args, 1280, 800, 1920, 1200)
        assert result["x"] == round(500 * 1920 / 1280)
        assert result["y"] == round(300 * 1200 / 800)
        assert result["direction"] == "down"
        assert result["amount"] == 3

    def test_non_coordinate_tool_passthrough(self):
        """type_text has no coordinates — args returned unchanged."""
        args = {"text": "hello"}
        result = scale_tool_args("type_text", args, 1280, 800, 1920, 1200)
        assert result == args

    def test_hotkey_passthrough(self):
        args = {"keys": ["command", "c"]}
        result = scale_tool_args("hotkey", args, 1280, 800, 1920, 1200)
        assert result == args


# -------------------------------------------------------------------- #
# scale_screenshot
# -------------------------------------------------------------------- #


class TestScaleScreenshot:
    """Tests for screenshot resize."""

    def test_scale_sets_screen_dimensions(self):
        shot = Screenshot(base64=_VALID_PNG_B64, width=1728, height=1080, mime_type="image/png")
        scaled = scale_screenshot(shot, (1280, 800))
        assert scaled.width == 1280
        assert scaled.height == 800
        assert scaled.screen_width == 1728
        assert scaled.screen_height == 1080

    def test_scale_noop_when_same_size(self):
        shot = Screenshot(base64=_VALID_PNG_B64, width=1280, height=800, mime_type="image/png")
        scaled = scale_screenshot(shot, (1280, 800))
        assert scaled is shot  # identical object, no resize

    def test_scale_preserves_scale_factor(self):
        shot = Screenshot(
            base64=_VALID_PNG_B64, width=1728, height=1080,
            scale_factor=2.0, mime_type="image/png",
        )
        scaled = scale_screenshot(shot, (1280, 800))
        assert scaled.scale_factor == 2.0

    def test_scale_prefers_pil_image(self):
        """When Screenshot carries a PIL Image, scale_screenshot should use it
        (avoiding double WebP compression) and the result should also carry image."""
        pil_img = Image.new("RGB", (1728, 1080), color=(128, 64, 32))
        shot = Screenshot(
            base64=_VALID_PNG_B64, width=1728, height=1080,
            mime_type="image/png", image=pil_img,
        )
        scaled = scale_screenshot(shot, (1280, 800))
        assert scaled.width == 1280
        assert scaled.height == 800
        assert scaled.image is not None
        assert scaled.image.size == (1280, 800)

    def test_scale_output_is_webp(self):
        """Scaled screenshots should always be WebP regardless of source format."""
        shot = Screenshot(base64=_VALID_PNG_B64, width=1728, height=1080, mime_type="image/png")
        scaled = scale_screenshot(shot, (1280, 800))
        assert scaled.mime_type == "image/webp"


# -------------------------------------------------------------------- #
# Screenshot.screen_width / screen_height defaults
# -------------------------------------------------------------------- #


class TestScreenshotScreenDimensions:
    """Verify the new screen_width/screen_height fields."""

    def test_defaults_to_none(self):
        shot = Screenshot(base64=_VALID_PNG_B64, width=800, height=600)
        assert shot.screen_width is None
        assert shot.screen_height is None

    def test_explicit_values(self):
        shot = Screenshot(
            base64=_VALID_PNG_B64, width=1280, height=800,
            screen_width=1728, screen_height=1080,
        )
        assert shot.screen_width == 1728
        assert shot.screen_height == 1080

    def test_image_field_default_none(self):
        shot = Screenshot(base64=_VALID_PNG_B64, width=800, height=600)
        assert shot.image is None

    def test_image_field_set(self):
        pil_img = Image.new("RGB", (800, 600))
        shot = Screenshot(base64=_VALID_PNG_B64, width=800, height=600, image=pil_img)
        assert shot.image is pil_img
