# pyright: reportAttributeAccessIssue=false, reportPossiblyUnboundVariable=false
"""macOS overlay renderer using PyObjC (AppKit + Quartz).

A daemon **subprocess** owns a full-screen, transparent, mouse-passthrough
:class:`NSWindow`.  The subprocess's *main thread* satisfies the macOS
requirement that all AppKit UI must be created on the main thread.

Draw commands are dispatched via :class:`multiprocessing.Queue` and the
subprocess pumps ``NSRunLoop`` at ~60 fps.
"""

from __future__ import annotations

import logging
import math
import multiprocessing
import multiprocessing.synchronize
import queue
import time
from dataclasses import dataclass, field
from typing import Any

from see_agent.overlay.base import OverlayRenderer

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# PyObjC imports (guarded so the module can be imported safely on non-macOS)
# ---------------------------------------------------------------------------

try:
    import objc  # noqa: F401 — needed for ObjC runtime integration
    from AppKit import (
        NSApplication,
        NSBackingStoreBuffered,
        NSBezierPath,
        NSColor,
        NSFont,
        NSFontAttributeName,
        NSForegroundColorAttributeName,
        NSMakePoint,
        NSMakeRect,
        NSScreen,
        NSView,
        NSWindow,
        NSWindowStyleMaskBorderless,
    )
    from Foundation import (
        NSAutoreleasePool,
        NSDate,
        NSRunLoop,
        NSString,
    )

    _PYOBJC_OK = True
except ImportError:
    _PYOBJC_OK = False

# ---------------------------------------------------------------------------
# Internal data types
# ---------------------------------------------------------------------------

_FADE_DURATION = 0.3  # seconds for the fade-out tail


def _truncate(text: str, max_len: int = 10) -> str:
    return text if len(text) <= max_len else text[:max_len] + "\u2026"


@dataclass
class _DrawCmd:
    """Instruction for the overlay subprocess (created inside the child)."""

    kind: str
    params: dict[str, Any]
    duration: float
    created_at: float = field(default_factory=time.monotonic)

    @property
    def expired(self) -> bool:
        return time.monotonic() - self.created_at > self.duration


# ---------------------------------------------------------------------------
# Custom NSView (only defined when PyObjC is available)
# ---------------------------------------------------------------------------

if _PYOBJC_OK:

    class _OverlayView(NSView):  # type: ignore[misc]
        """Draws the current overlay annotation."""

        def initWithFrame_(self, frame):  # noqa: N802
            self = objc.super(_OverlayView, self).initWithFrame_(frame)
            if self is not None:
                self._cmd: _DrawCmd | None = None
            return self

        # Use top-left origin (same as screen / PyAutoGUI coordinates).
        def isFlipped(self):  # noqa: N802
            return True

        # ---- main draw entry ----

        def drawRect_(self, dirty):  # noqa: N802
            NSColor.clearColor().set()
            NSBezierPath.fillRect_(self.bounds())

            cmd = self._cmd
            if cmd is None:
                return

            kind = cmd.kind
            p = cmd.params

            if kind == "click":
                self._draw_click(p["x"], p["y"], p.get("double", False))
            elif kind == "type_text":
                self._draw_center_top_label(
                    f'\u2328 "{_truncate(p["text"], 10)}"'
                )
            elif kind == "drag":
                self._draw_drag(p["x1"], p["y1"], p["x2"], p["y2"])
            elif kind == "scroll":
                self._draw_scroll(
                    p["x"], p["y"], p["direction"], p["amount"]
                )
            elif kind == "hotkey":
                self._draw_center_top_label(f"\u2328 {p['keys']}")
            elif kind == "shell":
                self._draw_center_top_label(
                    f"$ {_truncate(p['command'], 30)}"
                )
            elif kind == "wait":
                self._draw_center_top_label(
                    f"\u23f3 \u7b49\u5f85 {p['seconds']}s\u2026"
                )
            elif kind == "screenshot":
                self._draw_screen_border()
            elif kind == "call_user":
                self._draw_center_label(
                    f'\U0001f64b "{_truncate(p["question"], 20)}"'
                )
            elif kind == "finished":
                self._draw_center_label(
                    f"\u2705 {_truncate(p['summary'], 20)}", green=True
                )

        # ---- drawing primitives ----

        def _draw_circle(self, cx, cy, radius, color):
            rect = NSMakeRect(cx - radius, cy - radius, 2 * radius, 2 * radius)
            path = NSBezierPath.bezierPathWithOvalInRect_(rect)
            color.set()
            path.setLineWidth_(3.0)
            path.stroke()

        def _draw_click(self, x, y, double):
            red = NSColor.redColor()
            self._draw_circle(x, y, 20, red)
            if double:
                self._draw_circle(x, y, 28, red)
            label = f"\u53cc\u51fb ({x}, {y})" if double else f"({x}, {y})"
            self._draw_text_label(label, x + 25, y - 10)

        def _draw_drag(self, x1, y1, x2, y2):
            red = NSColor.redColor()
            self._draw_circle(x1, y1, 15, red)
            self._draw_circle(x2, y2, 15, red)

            # Arrow line
            path = NSBezierPath.bezierPath()
            path.moveToPoint_(NSMakePoint(x1, y1))
            path.lineToPoint_(NSMakePoint(x2, y2))
            red.set()
            path.setLineWidth_(3.0)
            path.stroke()

            # Arrowhead
            angle = math.atan2(y2 - y1, x2 - x1)
            alen = 15
            for da in (-0.4, 0.4):
                ah = NSBezierPath.bezierPath()
                ah.moveToPoint_(NSMakePoint(x2, y2))
                ah.lineToPoint_(
                    NSMakePoint(
                        x2 - alen * math.cos(angle + da),
                        y2 - alen * math.sin(angle + da),
                    )
                )
                ah.setLineWidth_(3.0)
                ah.stroke()

        def _draw_scroll(self, x, y, direction, amount):
            arrows = {"up": "\u2191", "down": "\u2193", "left": "\u2190", "right": "\u2192"}
            arrow = arrows.get(direction, "\u2193")
            self._draw_text_label(f"{arrow} scroll \u00d7{amount}", x + 10, y - 10)

        def _draw_screen_border(self):
            sw = self.bounds().size.width
            sh = self.bounds().size.height
            rect = NSMakeRect(3, 3, sw - 6, sh - 6)
            red = NSColor.colorWithCalibratedRed_green_blue_alpha_(1.0, 0.0, 0.0, 0.7)
            red.set()
            path = NSBezierPath.bezierPathWithRect_(rect)
            path.setLineWidth_(6.0)
            path.stroke()

        # ---- text helpers ----

        def _draw_text_label(self, text, x, y, bg=None, fg=None):
            if bg is None:
                bg = NSColor.colorWithCalibratedRed_green_blue_alpha_(
                    0.9, 0.1, 0.1, 0.85
                )
            if fg is None:
                fg = NSColor.whiteColor()
            attrs = {
                NSFontAttributeName: NSFont.boldSystemFontOfSize_(14),
                NSForegroundColorAttributeName: fg,
            }
            ns_str = NSString.stringWithString_(text)
            size = ns_str.sizeWithAttributes_(attrs)
            pad = 6
            bg_rect = NSMakeRect(
                x - pad, y - pad,
                size.width + 2 * pad, size.height + 2 * pad,
            )
            bg.set()
            NSBezierPath.bezierPathWithRoundedRect_xRadius_yRadius_(
                bg_rect, 4, 4
            ).fill()
            ns_str.drawAtPoint_withAttributes_(NSMakePoint(x, y), attrs)

        def _draw_center_top_label(self, text):
            sw = self.bounds().size.width
            attrs = {
                NSFontAttributeName: NSFont.boldSystemFontOfSize_(16),
                NSForegroundColorAttributeName: NSColor.whiteColor(),
            }
            size = NSString.stringWithString_(text).sizeWithAttributes_(attrs)
            x = (sw - size.width) / 2
            self._draw_text_label(text, x, 60)

        def _draw_center_label(self, text, green=False):
            sw = self.bounds().size.width
            sh = self.bounds().size.height
            attrs = {
                NSFontAttributeName: NSFont.boldSystemFontOfSize_(18),
                NSForegroundColorAttributeName: NSColor.whiteColor(),
            }
            size = NSString.stringWithString_(text).sizeWithAttributes_(attrs)
            x = (sw - size.width) / 2
            y = (sh - size.height) / 2
            bg = (
                NSColor.colorWithCalibratedRed_green_blue_alpha_(0.1, 0.7, 0.2, 0.85)
                if green
                else NSColor.colorWithCalibratedRed_green_blue_alpha_(0.9, 0.1, 0.1, 0.85)
            )
            self._draw_text_label(text, x, y, bg=bg)


# ---------------------------------------------------------------------------
# Subprocess entry point
# ---------------------------------------------------------------------------


def _overlay_process_entry(
    cmd_queue: multiprocessing.Queue,  # type: ignore[type-arg]
    ready_event: multiprocessing.synchronize.Event,
    dismiss_event: multiprocessing.synchronize.Event,
) -> None:
    """Run in a subprocess — the child's **main thread** creates NSWindow.

    This satisfies macOS's requirement that all AppKit UI operations
    happen on the main thread.
    """
    try:
        pool = NSAutoreleasePool.alloc().init()  # noqa: F841

        app = NSApplication.sharedApplication()
        app.setActivationPolicy_(2)  # NSApplicationActivationPolicyProhibited

        screen = NSScreen.mainScreen()
        frame = screen.frame()

        window = NSWindow.alloc().initWithContentRect_styleMask_backing_defer_(
            frame,
            NSWindowStyleMaskBorderless,
            NSBackingStoreBuffered,
            False,
        )
        window.setLevel_(1000)  # kCGScreenSaverWindowLevel
        window.setBackgroundColor_(NSColor.clearColor())
        window.setOpaque_(False)
        window.setIgnoresMouseEvents_(True)
        window.setHasShadow_(False)
        # Stay on all Spaces / desktops.
        window.setCollectionBehavior_(1 | (1 << 4))
        # NSWindowSharingNone — screencapture/CGWindowListCreateImage
        # will NOT capture this window, so overlay never appears in
        # screenshots sent to the LLM.
        window.setSharingType_(0)

        view = _OverlayView.alloc().initWithFrame_(frame)
        window.setContentView_(view)
        window.orderFrontRegardless()

        ready_event.set()

        _overlay_loop(window, view, cmd_queue, dismiss_event)

    except Exception:
        # Re-import logger in child process.
        import logging as _logging

        _logging.getLogger(__name__).exception("Overlay process crashed")
        ready_event.set()  # unblock parent even on failure


def _overlay_loop(
    window: Any,
    view: Any,
    cmd_queue: multiprocessing.Queue,  # type: ignore[type-arg]
    dismiss_event: multiprocessing.synchronize.Event,
) -> None:
    """Pump the command queue and NSRunLoop forever."""
    current: _DrawCmd | None = None

    while True:
        # 1. Drain queue.
        try:
            msg = cmd_queue.get(timeout=0.016)
            cmd_type = msg[0]

            if cmd_type == "dismiss":
                current = None
                view._cmd = None
                window.setAlphaValue_(1.0)
                view.setNeedsDisplay_(True)
                window.display()  # force immediate
                dismiss_event.set()

            elif cmd_type == "draw":
                _, kind, params, duration = msg
                current = _DrawCmd(kind=kind, params=params, duration=duration)
                view._cmd = current
                window.setAlphaValue_(1.0)
                view.setNeedsDisplay_(True)

            elif cmd_type == "quit":
                break

        except queue.Empty:
            pass

        # 2. Auto-expire / fade.
        if current is not None:
            elapsed = time.monotonic() - current.created_at
            remaining = current.duration - elapsed
            if remaining <= 0:
                current = None
                view._cmd = None
                window.setAlphaValue_(1.0)
                view.setNeedsDisplay_(True)
            elif current.duration > _FADE_DURATION and remaining <= _FADE_DURATION:
                window.setAlphaValue_(remaining / _FADE_DURATION)

        # 3. Pump NSRunLoop.
        NSRunLoop.currentRunLoop().runMode_beforeDate_(
            "NSDefaultRunLoopMode",
            NSDate.dateWithTimeIntervalSinceNow_(0.016),
        )


# ---------------------------------------------------------------------------
# Public renderer
# ---------------------------------------------------------------------------


class MacOverlayRenderer(OverlayRenderer):
    """Concrete :class:`OverlayRenderer` for macOS using PyObjC.

    Creates a full-screen, transparent, mouse-passthrough window in a
    daemon **subprocess** whose main thread satisfies the macOS requirement
    for AppKit UI.  All ``show_*`` methods are non-blocking; :meth:`dismiss`
    blocks until the overlay is confirmed cleared.
    """

    def __init__(self) -> None:
        if not _PYOBJC_OK:
            raise RuntimeError(
                "PyObjC (AppKit) is required for MacOverlayRenderer"
            )
        self._cmd_queue: multiprocessing.Queue[tuple[Any, ...]] = multiprocessing.Queue()
        self._ready = multiprocessing.Event()
        self._dismiss_done = multiprocessing.Event()

        self._process = multiprocessing.Process(
            target=_overlay_process_entry,
            args=(self._cmd_queue, self._ready, self._dismiss_done),
            daemon=True,
            name="see-agent-overlay",
        )
        self._process.start()

        if not self._ready.wait(timeout=5.0):
            self._process.terminate()
            raise RuntimeError(
                "Overlay process did not become ready in time — "
                "check the log for details"
            )

    # ------------------------------------------------------------------ #
    # Overlay interface
    # ------------------------------------------------------------------ #

    def _send(self, kind: str, params: dict[str, Any], duration: float) -> None:
        if not self._process.is_alive():
            return
        self._cmd_queue.put(("draw", kind, params, duration))

    def show_click(self, x: int, y: int, double: bool = False) -> None:
        self._send("click", {"x": x, "y": y, "double": double}, 1.0)

    def show_type(self, text: str) -> None:
        self._send("type_text", {"text": text}, 1.0)

    def show_drag(self, x1: int, y1: int, x2: int, y2: int) -> None:
        self._send("drag", {"x1": x1, "y1": y1, "x2": x2, "y2": y2}, 1.5)

    def show_scroll(self, x: int, y: int, direction: str, amount: int) -> None:
        self._send(
            "scroll",
            {"x": x, "y": y, "direction": direction, "amount": amount},
            1.0,
        )

    def show_hotkey(self, keys: list[str]) -> None:
        self._send("hotkey", {"keys": "+".join(keys)}, 1.0)

    def show_shell(self, command: str) -> None:
        self._send("shell", {"command": command}, 1.5)

    def show_wait(self, seconds: float) -> None:
        self._send("wait", {"seconds": seconds}, seconds + 0.5)

    def show_screenshot(self) -> None:
        self._send("screenshot", {}, 0.3)

    def show_call_user(self, question: str) -> None:
        self._send("call_user", {"question": question}, 60.0)

    def show_finished(self, summary: str) -> None:
        self._send("finished", {"summary": summary}, 2.0)

    def dismiss(self) -> None:
        if not self._process.is_alive():
            return
        self._dismiss_done.clear()
        self._cmd_queue.put(("dismiss",))
        self._dismiss_done.wait(timeout=1.0)
