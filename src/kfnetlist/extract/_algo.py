"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from rlayout._native import _kfnetlist_extract as _native
from typing import TYPE_CHECKING, Protocol
from kfnetlist import Netlist, Placement
from ._geometry import _BaseLike, _CrossSectionWrapperLike

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable, Mapping, Sequence
    from rlayout import db as kdb


class _FactoryLike(Protocol):
    lvs_equivalent_ports: list[list[str]] | None


class _SettingsLike(Protocol):
    def model_dump(self) -> dict[str, object]: ...


class _LibraryLike(Protocol):
    def name(self) -> str: ...


class _PortLike(Protocol):
    name: str
    port_type: str
    trans: kdb.Trans

    @property
    def layer_info(self) -> kdb.LayerInfo: ...

    @property
    def base(self) -> _BaseLike: ...

    @property
    def cross_section(self) -> _CrossSectionWrapperLike: ...


class _CellLike(Protocol):
    name: str

    @property
    def virtual(self) -> bool: ...

    @property
    def lvs_equivalent_ports(self) -> list[list[str]] | None: ...

    @property
    def factory_name(self) -> str: ...

    @property
    def settings(self) -> _SettingsLike: ...

    @property
    def library_cell(self) -> _CellLike: ...

    @property
    def kcl(self) -> _KCLLike: ...

    @property
    def ports(self) -> Iterable[_PortLike]: ...

    @property
    def insts(self) -> Iterable[_InstanceLike]: ...

    def has_factory_name(self) -> bool: ...

    def is_library_cell(self) -> bool: ...

    def library(self) -> _LibraryLike: ...

    def cell_index(self) -> int: ...


class _InstanceLike(Protocol):
    name: str
    na: int
    nb: int
    dcplx_trans: kdb.DCplxTrans
    purpose: str | None

    @property
    def instance(self) -> kdb.Instance: ...

    @property
    def cell(self) -> _CellLike: ...

    @property
    def ports(self) -> Iterable[_PortLike]: ...

    def is_named(self) -> bool: ...


class _KCLLike(Protocol):
    name: str
    layout: kdb.Layout

    @property
    def dbu(self) -> float: ...

    @property
    def connectivity(self) -> Sequence[Sequence[kdb.LayerInfo]]: ...

    @property
    def factories(self) -> Mapping[str, _FactoryLike]: ...

    @property
    def virtual_factories(self) -> Mapping[str, _FactoryLike]: ...

    def __getitem__(self, key: int | str, /) -> _CellLike: ...


class _RootCellLike(_CellLike, Protocol):
    def called_cells(self) -> Iterable[int]: ...


class _DBoxLike(Protocol):
    left: float
    bottom: float
    right: float
    top: float


class _DVectorLike(Protocol):
    x: float
    y: float


class _DCplxTransLike(Protocol):
    angle: float
    mirror: bool

    @property
    def disp(self) -> _DVectorLike: ...


class _InstanceShapeLike(Protocol):
    def dbbox(self) -> _DBoxLike: ...


class _PlaceableLike(Protocol):
    @property
    def instance(self) -> _InstanceShapeLike: ...

    @property
    def dcplx_trans(self) -> _DCplxTransLike: ...


def _placement_for(inst: _PlaceableLike) -> Placement:
    """Build a :class:`Placement` from a placed klayout instance.

    Reads the origin transform (displacement, rotation, mirror) in micrometres
    plus the transformed bounding box in the parent cell's coordinates. This is
    purely geometric; the placed cell name is captured separately onto
    :class:`~kfnetlist.PlacedInstance`.
    """
    return _native._placement_for(inst)


def _create_inst_entry(nl: Netlist, inst: _InstanceLike) -> None:
    return _native._create_inst_entry(nl, inst)


def extract(
    cell: _RootCellLike,
    *,
    wrap_kdb_instance: Callable[[kdb.Instance], _InstanceLike],
    port_types: Sequence[str] = ("optical",),
    mark_port_types: Iterable[str] = ("electrical", "RF", "DC"),
    connectivity: Sequence[Sequence[kdb.LayerInfo]] | None = None,
    equivalent_ports: dict[str, list[list[str]]] | None = None,
    ignore_unnamed: bool = False,
    exclude_purposes: list[str] | None = None,
    allow_width_mismatch: bool = False,
    include_placement: bool = False,
    flatten: bool | Sequence[str] = False,
) -> dict[str, Netlist]:
    """Extract a hierarchical netlist from a cell.

    Mirrors ``ProtoTKCell.netlist`` from kfactory: gathers LVS-equivalent ports
    from cell metadata or factories (unless supplied), runs electrical L2N
    extraction once, then for each cell walks optical-port geometry plus the
    electrical circuit to assemble a :class:`Netlist`.

    The ``wrap_kdb_instance`` callable is the only required kfactory-shaped
    hook: it converts a raw :class:`klayout.db.Instance` into an object with
    ``.name`` matching the names used elsewhere in the cell hierarchy. The
    kfactory shim passes ``lambda i: Instance(kcl=cell.kcl, instance=i)``.

    When ``include_placement`` is ``True``, each returned value is a
    :class:`~kfnetlist.PlacedNetlist` (a :class:`~kfnetlist.Netlist` subclass)
    whose instances additionally carry a :class:`~kfnetlist.Placement` — the
    cell name, origin transform (x, y, orientation, mirror), and bounding box —
    read from the layout. The default (``False``) returns plain
    :class:`~kfnetlist.Netlist` objects, identical to before.

    ``flatten`` inlines instances into their parent: each returned netlist has
    the selected instances replaced by the contents of their own cell's netlist
    (renamed ``"{instance}.{inner instance}"``), with the nets of both levels
    merged through the sub-cell's ports. Pass ``True`` to inline the whole
    hierarchy, or a sequence of cell names to inline only those — so a
    containerized subcircuit can be dissolved while an MZI that has its own
    model stays intact. Works with or without ``include_placement``; with it,
    each inlined placement is composed with the placement of the instance it
    came from.
    """
    return _native.extract(
        cell,
        wrap_kdb_instance=wrap_kdb_instance,
        port_types=port_types,
        mark_port_types=tuple(mark_port_types),
        connectivity=connectivity,
        equivalent_ports=equivalent_ports,
        ignore_unnamed=ignore_unnamed,
        exclude_purposes=exclude_purposes,
        allow_width_mismatch=allow_width_mismatch,
        include_placement=include_placement,
        flatten=flatten,
    )
