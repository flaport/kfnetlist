"""Geometric port-adjacency extraction → optical :class:`Net` list."""

from __future__ import annotations

from dataclasses import dataclass
from itertools import chain
from typing import TYPE_CHECKING, TypeVar

from kfnetlist import Net, NetlistPort, PortArrayRef, PortRef
from kfnetlist.port_check import (
    PortCheck,
    _CrossSectionLike,
    _KCLLike,
    check_connection,
)

from ._protocols import (
    BaseLike as _BaseLike,
    CellLike as _CellLike,
    InstanceLike as _InstanceLike,
    PortLike as _PortLike,
)

if TYPE_CHECKING:
    from collections.abc import Iterator, Sequence

    from klayout import db as kdb

_T = TypeVar("_T")


def _forward_neighbors(
    buckets: dict[tuple[int, int], dict[str, list[_T]]],
    position: tuple[int, int],
    layer: str,
) -> Iterator[_T]:
    """Yield the +x/+y buckets used to compare each adjacent pair once."""
    x, y = position
    yield from buckets.get((x + 1, y), {}).get(layer, ())
    yield from buckets.get((x, y + 1), {}).get(layer, ())


def _nearby(
    buckets: dict[tuple[int, int], dict[str, list[_T]]],
    position: tuple[int, int],
    layer: str,
) -> Iterator[_T]:
    """Yield candidates in the surrounding 3×3 snapped-position window."""
    x, y = position
    for nx in range(x - 1, x + 2):
        for ny in range(y - 1, y + 2):
            yield from buckets.get((nx, ny), {}).get(layer, ())


@dataclass(frozen=True, slots=True)
class _ResolvedPort:
    """A port resolved to its (possibly transformed) geometry + cross section.

    ``base`` carries the transform/layout geometry; ``cross_section`` is the
    non-optional cross section read from the port wrapper via
    ``port.cross_section.base``. Satisfies ``port_check.PortLike``.
    """

    base: _BaseLike
    cross_section: _CrossSectionLike
    name: str
    port_type: str

    @property
    def trans(self) -> kdb.Trans | None:
        return self.base.trans

    @property
    def dcplx_trans(self) -> kdb.DCplxTrans | None:
        return self.base.dcplx_trans

    @property
    def kcl(self) -> _KCLLike:
        return self.base.kcl


def _resolve(port: _PortLike, base: _BaseLike) -> _ResolvedPort:
    return _ResolvedPort(
        base=base,
        cross_section=port.cross_section.base,
        name=port.name,
        port_type=port.port_type,
    )


def _layer_key(port: _ResolvedPort) -> str:
    li = port.cross_section.main_layer
    return f"{li.layer}_{li.datatype}"


def _snapped_disp(base: _BaseLike) -> tuple[int, int]:
    """Resolve to an integer transform, snap angle mod 2, return (x, y)."""
    from klayout import db as kdb

    if base.trans is not None:
        t = base.trans
    else:
        assert base.dcplx_trans is not None, "port has neither trans nor dcplx_trans"
        t = kdb.ICplxTrans(trans=base.dcplx_trans, dbu=base.kcl.dbu).s_trans()
    t = t.dup()
    t.angle %= 2
    t.mirror = False
    v = t.disp
    return (v.x, v.y)


def _net_ref(
    inst: _InstanceLike, port_name: str, ia: int, ib: int
) -> PortArrayRef | PortRef:
    if inst.na > 0 and inst.nb > 0:
        return PortArrayRef(instance=inst.name, port=port_name, ia=ia, ib=ib)
    return PortRef(instance=inst.name, port=port_name)


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
    from klayout import db as kdb

    cell_ports: dict[tuple[int, int], dict[str, list[tuple[int, _ResolvedPort]]]] = {}
    inst_ports: dict[
        tuple[int, int],
        dict[
            str,
            list[tuple[int, int, int, int, _InstanceLike, _ResolvedPort]],
        ],
    ] = {}

    portnames: set[str] = set()
    nets: list[Net] = []
    connected_cell_ports: set[str] = set()
    connected_inst_ports: set[tuple[str, str, int, int]] = set()

    for i, port in enumerate(cell.ports):
        if port.port_type not in port_types:
            continue
        if port.name in portnames:
            raise ValueError(
                "Netlist extraction is not possible with colliding port names."
                f" Duplicate name: {port.name}"
            )
        rp = _resolve(port, port.base)
        h = _snapped_disp(rp.base)
        layer = _layer_key(rp)
        cell_ports.setdefault(h, {}).setdefault(layer, []).append((i, rp))
        if port.name:
            portnames.add(port.name)

    for i, inst in enumerate(cell.insts):
        if inst.na > 1 or inst.nb > 1:
            for ia in range(inst.na):
                for ib in range(inst.nb):
                    st = kdb.InstElement(inst.instance, ia, ib).specific_trans()
                    for j, port in enumerate(inst.ports):
                        if port.port_type not in port_types:
                            continue
                        rp = _resolve(port, port.base.transformed(st))
                        h = _snapped_disp(rp.base)
                        layer = _layer_key(rp)
                        inst_ports.setdefault(h, {}).setdefault(layer, []).append(
                            (i, j, ia, ib, inst, rp)
                        )
        else:
            for j, port in enumerate(inst.ports):
                if port.port_type not in port_types:
                    continue
                rp = _resolve(port, port.base)
                h = _snapped_disp(rp.base)
                layer = _layer_key(rp)
                inst_ports.setdefault(h, {}).setdefault(layer, []).append(
                    (i, j, 0, 0, inst, rp)
                )

    base_check = PortCheck.position + PortCheck.layer + PortCheck.port_type
    if not allow_width_mismatch:
        base_check += PortCheck.width
    check_same = base_check + PortCheck.same
    check_opposite = base_check + PortCheck.opposite

    for h, cellport_layer_dict in cell_ports.items():
        for layer, cellports in cellport_layer_dict.items():
            additional_cellports = tuple(_forward_neighbors(cell_ports, h, layer))
            ports_near = tuple(_nearby(inst_ports, h, layer))

            for n, (_, cellport) in enumerate(cellports):
                for _, cellport2 in chain(cellports[n + 1 :], additional_cellports):
                    if (
                        check_connection(cellport, cellport2) & check_opposite
                    ) == check_opposite:
                        nets.append(
                            Net(
                                [
                                    NetlistPort(name=cellport.name),
                                    NetlistPort(name=cellport2.name),
                                ]
                            )
                        )
                        connected_cell_ports.add(cellport.name)
                        connected_cell_ports.add(cellport2.name)

                for _, _, ia2, ib2, inst2, port2 in ports_near:
                    if (
                        check_connection(cellport, port2, snapped=True) & check_same
                    ) == check_same:
                        nets.append(
                            Net(
                                [
                                    NetlistPort(name=cellport.name),
                                    _net_ref(inst2, port2.name, ia2, ib2),
                                ]
                            )
                        )
                        connected_cell_ports.add(cellport.name)
                        connected_inst_ports.add((inst2.name, port2.name, ia2, ib2))

    for h, inst_layer_dict in inst_ports.items():
        for layer, ports in inst_layer_dict.items():
            additional_ports = tuple(_forward_neighbors(inst_ports, h, layer))
            for n, (_, _, ia, ib, inst, port) in enumerate(ports):
                for _, _, ia2, ib2, inst2, port2 in chain(
                    ports[n + 1 :], additional_ports
                ):
                    if (
                        check_connection(port, port2) & check_opposite
                    ) == check_opposite:
                        nets.append(
                            Net(
                                [
                                    _net_ref(inst, port.name, ia, ib),
                                    _net_ref(inst2, port2.name, ia2, ib2),
                                ]
                            )
                        )
                        connected_inst_ports.add((inst.name, port.name, ia, ib))
                        connected_inst_ports.add((inst2.name, port2.name, ia2, ib2))

    for _layer_dict in cell_ports.values():
        for cellport_list in _layer_dict.values():
            for _, cellport in cellport_list:
                if cellport.name not in connected_cell_ports:
                    nets.append(Net([NetlistPort(name=cellport.name)]))

    for _layer_dict in inst_ports.values():
        for inst_port_list in _layer_dict.values():
            for _, _, ia, ib, inst, port in inst_port_list:
                if (inst.name, port.name, ia, ib) not in connected_inst_ports:
                    nets.append(Net([_net_ref(inst, port.name, ia, ib)]))

    return nets
