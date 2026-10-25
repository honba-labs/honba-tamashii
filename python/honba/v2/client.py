"""Honba API v2 REST Client."""

from __future__ import annotations

from typing import Any
from honba.client.client import Client as _BaseClient


class Client(_BaseClient):
    """Client pinned to REST API v2 (Latest & Recommended)."""

    def __init__(self, transport: Any, **kwargs: Any) -> None:
        kwargs.setdefault("api_version", "v2")
        super().__init__(transport, **kwargs)


__all__ = ["Client"]
