"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from ._native import PortCheck
from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from rlayout import db as kdb


class _CrossSectionLike(Protocol):
    @property
    def width(self) -> int: ...

    @property
    def main_layer(self) -> kdb.LayerInfo: ...


class _KCLLike(Protocol):
    @property
    def dbu(self) -> float: ...


class PortLike(Protocol):
    """Duck-typed shape consumed by :func:`check_connection`.

    Exactly one of ``trans`` / ``dcplx_trans`` must be set.
    """

    @property
    def trans(self) -> kdb.Trans | None: ...

    @property
    def dcplx_trans(self) -> kdb.DCplxTrans | None: ...

    @property
    def cross_section(self) -> _CrossSectionLike: ...

    @property
    def port_type(self) -> str: ...

    @property
    def kcl(self) -> _KCLLike: ...


def check_connection(
    p1: PortLike,
    p2: PortLike,
    *,
    tolerance: float = 0.1,
    angle_tolerance: float = 0.01,
    snapped: bool = False,
) -> int:
    """Compare two ports, returning a :class:`PortCheck` bitmask.

    Integer transforms are used when both ports expose ``trans`` (or when
    ``snapped=True``); otherwise the complex transforms are used with the
    supplied tolerances. ``cross_section`` implies ``layer`` and ``width``.
    """
    from rlayout._native import _kfnetlist_extract as _native

    flags = _native.check_connection(
        p1, p2, tolerance=tolerance, angle_tolerance=angle_tolerance, snapped=snapped
    )
    return PortCheck(flags) if flags else 0
