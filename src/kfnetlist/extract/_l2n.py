"""Build a :class:`klayout.db.LayoutToNetlist` from electrical port markers."""

from __future__ import annotations

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
    from rlayout import db as kdb

    connectivity = connectivity or cell.kcl.connectivity
    ly_elec = cell.kcl.layout.dup()
    port_mapping = port_mapping or {}

    for ci in [cell.cell_index(), *cell.called_cells()]:
        c_ = cell.kcl[ci]
        c = ly_elec.cell(c_.name)
        assert c_.name == c.name
        c.locked = False
        mapping = port_mapping.get(
            c_.name,
            port_mapping.get(c_.factory_name, {}) if c_.has_factory_name() else {},
        )
        # For each equivalence group (keyed by its canonical name), pick the
        # first port whose ``port_type`` is markable — that's the one we
        # stamp into the layout, labelled with the canonical name. Without
        # this step a group whose declared canonical has a non-markable
        # ``port_type`` (e.g. a pad cell where ``"pad"`` is the canonical
        # but ``port_type='pad'``) would emit no label at all, leaving the
        # extracted net unnamed.
        preferred_for_canonical: dict[str, str] = {}
        for port in c_.ports:
            if port.port_type not in mark_port_types:
                continue
            canonical = mapping.get(port.name, port.name)
            preferred_for_canonical.setdefault(canonical, port.name)
        for port in c_.ports:
            canonical = mapping.get(port.name, port.name)
            if preferred_for_canonical.get(canonical) != port.name:
                continue
            c.shapes(port.layer_info).insert(
                kdb.Text(string=canonical, trans=port.trans)
            )

    resolved_connectivity: list[list[tuple[kdb.LayerId, kdb.LayerInfo]]] = []
    for layer_set in connectivity:
        resolved_layer_set = []
        for info in layer_set:
            layer_id = ly_elec.layer(info)
            resolved_layer_set.append((layer_id, ly_elec.get_info(layer_id)))
        resolved_connectivity.append(resolved_layer_set)
    l2n = kdb.LayoutToNetlist.from_cell(ly_elec.cell(cell.name))
    layers: dict[int, kdb.Region] = {}
    for layer_set in resolved_connectivity:
        for layer_id, info in layer_set:
            if layer_id.index in layers:
                continue
            layer = l2n.make_layer(layer_id, info.name)
            layers[layer_id.index] = layer
            l2n.connect(layer)
    for conn in resolved_connectivity:
        old_layer = layers[conn[0][0].index]
        for layer_id, _ in conn[1:]:
            li = layers[layer_id.index]
            l2n.connect(old_layer, li)
            old_layer = li
    l2n.extract_netlist()
    l2n.check_extraction_errors()
    return l2n
