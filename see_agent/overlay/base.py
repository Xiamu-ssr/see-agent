"""Abstract base class for screen overlay renderers."""

from __future__ import annotations

from abc import ABC, abstractmethod


class OverlayRenderer(ABC):
    """Renders ephemeral visual annotations on top of the screen.

    Every method is non-blocking — the actual drawing happens on a
    background thread / process managed by the concrete implementation.
    """

    @abstractmethod
    def show_click(self, x: int, y: int, double: bool = False) -> None: ...

    @abstractmethod
    def show_type(self, text: str) -> None: ...

    @abstractmethod
    def show_drag(self, x1: int, y1: int, x2: int, y2: int) -> None: ...

    @abstractmethod
    def show_scroll(self, x: int, y: int, direction: str, amount: int) -> None: ...

    @abstractmethod
    def show_hotkey(self, keys: list[str]) -> None: ...

    @abstractmethod
    def show_shell(self, command: str) -> None: ...

    @abstractmethod
    def show_wait(self, seconds: float) -> None: ...

    @abstractmethod
    def show_screenshot(self) -> None: ...

    @abstractmethod
    def show_call_user(self, question: str) -> None: ...

    @abstractmethod
    def show_finished(self, summary: str) -> None: ...

    @abstractmethod
    def dismiss(self) -> None:
        """Immediately clear all overlay content.

        Blocks until the screen is guaranteed to be clean (important so
        that screenshots taken afterwards do not contain overlay artefacts).
        """
        ...
