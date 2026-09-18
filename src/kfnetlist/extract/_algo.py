"""Thin exports from the shared Rust extraction extension."""
from rlayout._native import _kfnetlist_extract as _native

extract = _native.extract
_placement_for = _native._placement_for

from typing import Protocol

class _InstanceLike(Protocol):
    """Structural metadata input retained for private adapter callers."""
    name: str
    na: int
    nb: int
    @property
    def cell(self) -> object: ...
    def is_named(self) -> bool: ...

_create_inst_entry = _native._create_inst_entry
