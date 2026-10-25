"""Honba API and Strategy v1 namespace (Maintained for existing software)."""

from honba.client.client import Client as _Client
from honba.v1 import strategies


def Client(*args, **kwargs):
    kwargs.setdefault("api_version", "v1")
    return _Client(*args, **kwargs)


__all__ = ["Client", "strategies"]
