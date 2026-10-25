"""Honba API and Strategy v2 namespace (Latest & Recommended)."""

from honba.client.client import Client as _Client
from honba.v2 import strategies


def Client(*args, **kwargs):
    kwargs.setdefault("api_version", "v2")
    return _Client(*args, **kwargs)


__all__ = ["Client", "strategies"]
