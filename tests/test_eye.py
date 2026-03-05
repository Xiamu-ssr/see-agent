"""Unit tests for the Screenshot dataclass (src/eye/base.py)."""

import base64

import pytest

from src.eye.base import Screenshot

# -------------------------------------------------------------------- #
# Helpers
# -------------------------------------------------------------------- #

# A minimal 1x1 red PNG encoded to base64 (valid PNG bytes).
_TINY_PNG_BYTES = (
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01"
    b"\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00"
    b"\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00"
    b"\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82"
)
TINY_PNG_B64 = base64.b64encode(_TINY_PNG_BYTES).decode("ascii")


# -------------------------------------------------------------------- #
# Tests
# -------------------------------------------------------------------- #


class TestScreenshot:
    """Tests for the Screenshot dataclass."""

    def test_screenshot_detail_low(self):
        """width=800, height=600 (both <= 1024) -> detail is 'low'."""
        shot = Screenshot(base64=TINY_PNG_B64, width=800, height=600)
        assert shot.detail == "low"

    def test_screenshot_detail_high(self):
        """width=1920, height=1080 (width > 1024) -> detail is 'high'."""
        shot = Screenshot(base64=TINY_PNG_B64, width=1920, height=1080)
        assert shot.detail == "high"

    def test_screenshot_detail_high_height_only(self):
        """height > 1024 alone also triggers 'high'."""
        shot = Screenshot(base64=TINY_PNG_B64, width=800, height=1200)
        assert shot.detail == "high"

    def test_screenshot_detail_edge_case(self):
        """Exactly 1024x1024 -> detail is 'low'."""
        shot = Screenshot(base64=TINY_PNG_B64, width=1024, height=1024)
        assert shot.detail == "low"

    def test_screenshot_save(self, tmp_path):
        """save() writes a file whose content matches the decoded base64 data."""
        shot = Screenshot(
            base64=TINY_PNG_B64, width=100, height=100, mime_type="image/png"
        )
        dest = tmp_path / "test_shot.png"

        returned_path = shot.save(dest)

        assert returned_path.exists()
        assert returned_path == dest.resolve()
        assert returned_path.read_bytes() == _TINY_PNG_BYTES

    def test_screenshot_save_creates_parents(self, tmp_path):
        """save() creates parent directories automatically."""
        shot = Screenshot(
            base64=TINY_PNG_B64, width=100, height=100, mime_type="image/png"
        )
        dest = tmp_path / "a" / "b" / "c" / "nested_shot.png"

        returned_path = shot.save(dest)
        assert returned_path.exists()

    def test_screenshot_save_webp_extension(self, tmp_path):
        """save() replaces extension based on mime_type."""
        shot = Screenshot(base64=TINY_PNG_B64, width=100, height=100)
        dest = tmp_path / "shot.png"

        returned_path = shot.save(dest)

        assert returned_path.suffix == ".webp"
        assert returned_path.name == "shot.webp"

    def test_screenshot_physical_dimensions(self):
        """scale_factor=2.0 doubles the physical dimensions."""
        shot = Screenshot(
            base64=TINY_PNG_B64, width=800, height=600, scale_factor=2.0
        )
        assert shot.physical_width == 1600
        assert shot.physical_height == 1200

    def test_screenshot_physical_dimensions_default_scale(self):
        """Default scale_factor=1.0 means physical == logical dimensions."""
        shot = Screenshot(base64=TINY_PNG_B64, width=1920, height=1080)
        assert shot.scale_factor == 1.0
        assert shot.physical_width == 1920
        assert shot.physical_height == 1080

    def test_screenshot_frozen(self):
        """Screenshot is a frozen dataclass -- attributes cannot be reassigned."""
        shot = Screenshot(base64=TINY_PNG_B64, width=100, height=100)
        with pytest.raises(AttributeError):
            shot.width = 200  # type: ignore[misc]
