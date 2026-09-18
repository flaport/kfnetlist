"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from rlayout._native import _kfnetlist_extract as _native
from typing import TYPE_CHECKING, Protocol
from kfnetlist import Net
from kfnetlist.port_check import _CrossSectionLike, _KCLLike

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence
    from rlayout import db as kdb


class _BaseLike(Protocol):
    """Subset of ``kfactory.port.BasePort`` consumed here."""

    trans: kdb.Trans | None
    dcplx_trans: kdb.DCplxTrans | None
    port_type: str
    name: str

    @property
    def kcl(self) -> _KCLLike: ...

    def transformed(
        self,
        trans: kdb.Trans | kdb.DCplxTrans,
        post_trans: kdb.Trans | kdb.DCplxTrans = ...,
    ) -> _BaseLike: ...


class _CrossSectionWrapperLike(Protocol):
    @property
    def base(self) -> _CrossSectionLike: ...


class _PortLike(Protocol):
    name: str
    port_type: str

    @property
    def base(self) -> _BaseLike: ...

    @property
    def cross_section(self) -> _CrossSectionWrapperLike: ...


class _InstanceLike(Protocol):
    name: str
    na: int
    nb: int

    @property
    def instance(self) -> kdb.Instance: ...

    @property
    def ports(self) -> Iterable[_PortLike]: ...


class _CellLike(Protocol):
    @property
    def ports(self) -> Iterable[_PortLike]: ...

    @property
    def insts(self) -> Iterable[_InstanceLike]: ...


def get_optical_nets(
    cell: _CellLike,
    port_types: Sequence[str] = ("optical",),
    *,
    allow_width_mismatch: bool = False,
) -> list[Net]:
    """Extract optical-type nets from a cell's geometric port adjacency.

    Cell ports and instance ports are bucketed by snapped ``(x, y)`` /
    layer-key. Cell-to-cell pairings use the ``opposite`` connection mode;
    cell-to-instance pairings use ``same`` (snapped); instance-to-instance
    pairings use ``opposite``. The bitmask source of truth is
    :class:`kfnetlist.port_check.PortCheck`.
    """
    return _native.get_optical_nets(
        cell, port_types, allow_width_mismatch=allow_width_mismatch
    )
