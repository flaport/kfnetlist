"""Thin exports from the shared Rust extraction extension."""
from rlayout._native import _kfnetlist_extract as _native

ShortResult = _native.ShortResult
detect_shorts = _native.detect_shorts
