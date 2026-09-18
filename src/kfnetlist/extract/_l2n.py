"""Thin exports from the shared Rust extraction extension."""
from rlayout._native import _kfnetlist_extract as _native

l2n_elec = _native.l2n_elec
