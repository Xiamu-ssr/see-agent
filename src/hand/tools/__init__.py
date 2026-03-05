from src.eye.base import BaseEye
from src.hand.tool import ToolRegistry
from src.hand.tools.call_user import CallUserTool
from src.hand.tools.click import ClickTool
from src.hand.tools.drag import DragTool
from src.hand.tools.finished import FinishedTool
from src.hand.tools.hotkey import HotkeyTool
from src.hand.tools.screenshot import ScreenshotTool
from src.hand.tools.scroll import ScrollTool
from src.hand.tools.shell import ShellTool
from src.hand.tools.type_text import TypeTextTool
from src.hand.tools.wait import WaitTool


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
