from __future__ import annotations

from typing import TYPE_CHECKING, Callable

from see_agent.eye.base import BaseEye
from see_agent.hand.tool import ToolRegistry
from see_agent.hand.tools.call_user import CallUserTool
from see_agent.hand.tools.click import ClickTool
from see_agent.hand.tools.drag import DragTool
from see_agent.hand.tools.finished import FinishedTool
from see_agent.hand.tools.hotkey import HotkeyTool
from see_agent.hand.tools.screenshot import ScreenshotTool
from see_agent.hand.tools.scroll import ScrollTool
from see_agent.hand.tools.shell import ShellTool
from see_agent.hand.tools.type_text import TypeTextTool
from see_agent.hand.tools.wait import WaitTool

if TYPE_CHECKING:
    from see_agent.eye.base import Screenshot


def create_registry(
    eye: BaseEye | None = None,
    scale_fn: Callable[[Screenshot], Screenshot] | None = None,
) -> ToolRegistry:
    """Create a ToolRegistry with all tools registered.

    Parameters:
        eye: Screen-capture backend for the ScreenshotTool.  When *None*,
             the screenshot tool is omitted (useful for listing tools).
        scale_fn: Optional function to resize screenshots for the LLM.
    """
    registry = ToolRegistry()
    registry.register(ClickTool())
    registry.register(TypeTextTool())
    registry.register(HotkeyTool())
    registry.register(ScrollTool())
    registry.register(DragTool())
    registry.register(ShellTool())
    registry.register(WaitTool())
    if eye is not None:
        registry.register(ScreenshotTool(eye=eye, scale_fn=scale_fn))
    registry.register(FinishedTool())
    registry.register(CallUserTool())
    return registry
