"""Geometric short detection using native Region operations.

Given a :class:`rlayout.db.LayoutToNetlist` (from :func:`l2n_elec`),
detects unexpected polygon overlaps between different nets on the same
layer.  Overlap regions are computed via ``Region.__and__`` (boolean
intersection) and returned as structured results.
"""

from __future__ import annotations
import dataclasses
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence
    from rlayout import db as kdb

@dataclasses.dataclass
class ShortResult:
    """A geometric short between two nets on a single layer."""

    net_a: str
    net_b: str
    layer: str
    overlap: kdb.Region

def detect_shorts(
    l2n: kdb.LayoutToNetlist,
    *,
    short_layers: Sequence[kdb.LayerInfo] | None = None,
    circuit_name: str | None = None,
) -> list[ShortResult]: ...
