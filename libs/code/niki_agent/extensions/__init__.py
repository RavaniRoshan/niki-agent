"""Public contract for dcode Python extensions."""

from niki_agent.extensions.api import ExtensionAPI, ExtensionMode
from niki_agent.extensions.runtime import load_extensions

__all__ = ["ExtensionAPI", "ExtensionMode", "load_extensions"]
