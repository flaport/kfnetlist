"""Thin exports from the shared Rust extraction extension."""
from rlayout._native import _kfnetlist_extract as _native

extract = _native.extract
_placement_for = _native._placement_for
