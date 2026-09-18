"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from rlayout._native import _kfnetlist_extract as _native
from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from collections.abc import Iterable, Mapping, Sequence
    from rlayout import db as kdb


class _PortLike(Protocol):
    name: str
    port_type: str
    trans: kdb.Trans

    @property
    def layer_info(self) -> kdb.LayerInfo: ...


class _CellLike(Protocol):
    name: str

    @property
    def factory_name(self) -> str: ...

    @property
    def ports(self) -> Iterable[_PortLike]: ...

    def has_factory_name(self) -> bool: ...

    def cell_index(self) -> int: ...


class _KCLLike(Protocol):
    layout: kdb.Layout

    @property
    def connectivity(self) -> Sequence[Sequence[kdb.LayerInfo]]: ...

    def __getitem__(self, key: int, /) -> _CellLike: ...


class _RootCellLike(_CellLike, Protocol):
    @property
    def kcl(self) -> _KCLLike: ...

    def called_cells(self) -> Iterable[int]: ...


def l2n_elec(
    cell: _RootCellLike,
    mark_port_types: Iterable[str] = ("electrical", "RF", "DC"),
    connectivity: Sequence[Sequence[kdb.LayerInfo]] | None = None,
    port_mapping: Mapping[str, Mapping[str, str]] | None = None,
) -> kdb.LayoutToNetlist:
    """Build a klayout LayoutToNetlist driven by electrical port markers.

    Each cell port whose type is in ``mark_port_types`` is materialised as a
    :class:`kdb.Text` marker on its layer in a fresh layout copy, then klayout
    runs its own connectivity extraction across ``connectivity``.
    """
    return _native.l2n_elec(cell, tuple(mark_port_types), connectivity, port_mapping)
