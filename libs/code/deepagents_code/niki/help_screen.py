"""The in-app help modal (K8).

`?` used to type a literal question mark: nothing bound it, so the "help open"
snapshot was a picture of the composer. Upstream's `/help` prints a
hand-maintained block; this modal shows the same generated keymap the CLI's
`--help` uses, built from the live bindings of the surface underneath.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, ClassVar

from textual.binding import Binding, BindingType
from textual.containers import VerticalScroll
from textual.screen import ModalScreen
from textual.widgets import Static

if TYPE_CHECKING:
    from textual.app import App, ComposeResult


class NikiHelpScreen(ModalScreen[None]):
    """A scrollable view of the keymap for the current surface."""

    BINDINGS: ClassVar[list[BindingType]] = [
        Binding("escape", "dismiss_help", "Close", show=False, priority=True),
        Binding("question_mark", "dismiss_help", "Close", show=False),
    ]

    DEFAULT_CSS = """
    NikiHelpScreen {
        align: center middle;
    }

    NikiHelpScreen > VerticalScroll {
        width: 90%;
        max-width: 110;
        height: 85%;
        background: $surface;
        border: solid $primary;
        padding: 1 2;
    }

    NikiHelpScreen #niki-help-body {
        width: 100%;
        height: auto;
    }
    """

    def __init__(self, body: str) -> None:
        """Initialize the help modal.

        Args:
            body: The generated keymap text, already composed for this surface.
        """
        super().__init__()
        self._body = body

    def compose(self) -> ComposeResult:
        """Compose the scrollable body.

        Yields:
            The panel and its text.
        """
        with VerticalScroll():
            yield Static(self._body, id="niki-help-body")

    def action_dismiss_help(self) -> None:
        """Close the modal; the app restores composer focus."""
        self.dismiss(None)


def help_body(app: App) -> str:
    """Render the keymap for `app`'s current surface.

    Args:
        app: The mounted app, so the body lists what this screen can do.

    Returns:
        The help text, without colour markup.
    """
    from deepagents_code.niki.help import render_help_for

    return render_help_for(app)


__all__ = ["NikiHelpScreen", "help_body"]
