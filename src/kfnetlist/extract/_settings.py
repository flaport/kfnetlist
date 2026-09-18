"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from rlayout._native import _kfnetlist_extract as _native
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass


def serialize_setting(setting: Any) -> Any:
    """Serialise a setting value to a JSON-friendly form.

    klayout shape types are encoded as ``"!#ClassName <str(value)>"``; dicts,
    lists, and tuples recurse. Other values pass through unchanged.
    """
    return _native.serialize_setting(setting)
