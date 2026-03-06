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


def create_registry(eye: BaseEye) -> ToolRegistry:
    """Create a ToolRegistry with all tools registered."""
    registry = ToolRegistry()
    registry.register(ClickTool())
    registry.register(TypeTextTool())
    registry.register(HotkeyTool())
    registry.register(ScrollTool())
    registry.register(DragTool())
    registry.register(ShellTool())
    registry.register(WaitTool())
    registry.register(ScreenshotTool(eye=eye))
    registry.register(FinishedTool())
    registry.register(CallUserTool())
    return registry
