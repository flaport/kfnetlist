"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from . import _native
from typing import TYPE_CHECKING, TypeVar
from ._native import Netlist

if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence
NetlistT = TypeVar("NetlistT", bound=Netlist)


def flatten_netlists(
    netlists: Mapping[str, NetlistT],
    cells: Sequence[str] | None = None,
    *,
    exclude: Sequence[str] | None = None,
    instance_cell_maps: Mapping[str, Mapping[str, str]] | None = None,
    recursive: bool = True,
    allow_unconnected_ports: bool = False,
    warn_skipped: bool = False,
    separator: str = ".",
) -> dict[str, NetlistT]:
    """Flatten every netlist of a hierarchy against the hierarchy itself.

    ``netlists`` is a ``{cell name: netlist}`` mapping — what
    :func:`kfnetlist.extract.extract` returns. Each netlist gets the selected
    instances replaced by the contents of their own cell's netlist; see
    :meth:`kfnetlist.Netlist.flatten` for the per-netlist semantics and for
    every keyword argument.

    Every netlist is flattened against the *original* mapping, so the result
    does not depend on iteration order. Cells that were inlined keep their own
    entry in the returned mapping — flattening a parent does not invalidate the
    child's netlist.

    ``instance_cell_maps`` maps a cell name to that cell's
    ``{instance name -> cell name}``. It is only needed for plain
    :class:`~kfnetlist.Netlist` objects, which do not record the cell an
    instance refers to; a :class:`~kfnetlist.PlacedNetlist` already does.
    """
    return _native.flatten_netlists(
        netlists,
        cells,
        exclude=exclude,
        instance_cell_maps=instance_cell_maps,
        recursive=recursive,
        allow_unconnected_ports=allow_unconnected_ports,
        warn_skipped=warn_skipped,
        separator=separator,
    )
