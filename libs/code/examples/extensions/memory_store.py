"""Reference extension: shared data through a virtual storage route."""

from niki_agent.extensions import ExtensionAPI
from niki_agent.sdk.backends import StoreBackend


async def extension(d: ExtensionAPI) -> None:  # noqa: RUF029  # Public extension factories are async by contract.
    """Make LangGraph store data available under `/memories/`.

    Args:
        d: The Niki extension API.
    """
    d.register_backend_route(
        "/memories/",
        StoreBackend(namespace=lambda _runtime: ("filesystem",)),
    )
