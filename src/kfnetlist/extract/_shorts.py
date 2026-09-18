"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from rlayout._native import _kfnetlist_extract as _native
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence
    from rlayout import db as kdb

ShortResult = _native.ShortResult


def detect_shorts(
    l2n: kdb.LayoutToNetlist,
    *,
    short_layers: Sequence[kdb.LayerInfo] | None = None,
    circuit_name: str | None = None,
) -> list[ShortResult]:
    """Detect geometric shorts between nets via polygon overlap.

    For each layer (or only the layers in *short_layers*), collects the
    shapes of every net and checks all pairs for non-empty intersection.

    Parameters
    ----------
    l2n:
        A klayout ``LayoutToNetlist`` whose extraction is complete.
    short_layers:
        Restrict detection to these layers.  ``None`` checks every layer
        registered in the L2N.
    circuit_name:
        Circuit to inspect.  Defaults to the top cell.

    Returns
    -------
    list[ShortResult]
        One entry per (net_a, net_b, layer) triple that has a non-empty
        overlap region.
    """
    return _native.detect_shorts(
        l2n, short_layers=short_layers, circuit_name=circuit_name
    )
