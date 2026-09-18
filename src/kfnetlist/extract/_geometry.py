"""Thin exports from the shared Rust extraction extension."""
from rlayout._native import _kfnetlist_extract as _native

get_optical_nets = _native.get_optical_nets
