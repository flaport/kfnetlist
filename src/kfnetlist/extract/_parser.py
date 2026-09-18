"""Typed Python boundary; all domain operations execute in Rust."""

from __future__ import annotations
from rlayout._native import _kfnetlist_extract as _native
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from collections.abc import Sequence
    from rlayout import db as kdb


def _layer_display_name(info: kdb.LayerInfo) -> str:
    """Human-readable name for a LayerInfo (falls back to ``layer/datatype``)."""
    return _native._layer_display_name(info)


def _discover_layer_regions(
    l2n: kdb.LayoutToNetlist,
) -> dict[kdb.LayerInfo, kdb.Region]:
    """Map each registered LayerInfo to its L2N region handle."""
    return _native._discover_layer_regions(l2n)


def _net_shapes_by_layer(
    net: kdb.Net,
    l2n: kdb.LayoutToNetlist,
    layer_regions: dict[kdb.LayerInfo, kdb.Region],
) -> dict[kdb.LayerInfo, kdb.Region]:
    """Return ``{LayerInfo: shapes}`` for layers where *net* has shapes."""
    return _native._net_shapes_by_layer(net, l2n, layer_regions)


def _serialize_net(
    net: kdb.Net,
    l2n: kdb.LayoutToNetlist | None,
    layer_regions: dict[kdb.LayerInfo, kdb.Region],
    subc_id_filter: set[int] | None,
    include_layers: set[kdb.LayerInfo] | None,
    exclude_layers: set[kdb.LayerInfo] | None,
) -> dict[str, Any] | None:
    """Serialize one net.  Returns ``None`` when filtered out."""
    return _native._serialize_net(
        net, l2n, layer_regions, subc_id_filter, include_layers, exclude_layers
    )


def _serialize_circuit(
    circuit: kdb.Circuit,
    l2n: kdb.LayoutToNetlist | None,
    layer_regions: dict[kdb.LayerInfo, kdb.Region],
    include_instances: set[str] | None,
    exclude_instances: set[str] | None,
    include_layers: set[kdb.LayerInfo] | None,
    exclude_layers: set[kdb.LayerInfo] | None,
) -> dict[str, Any]:
    """Serialize one circuit: pins, subcircuits, and nets."""
    return _native._serialize_circuit(
        circuit,
        l2n,
        layer_regions,
        include_instances,
        exclude_instances,
        include_layers,
        exclude_layers,
    )


def parse_l2n(
    l2n: kdb.LayoutToNetlist,
    *,
    flatten: bool = False,
    include_layers: Sequence[kdb.LayerInfo] | None = None,
    exclude_layers: Sequence[kdb.LayerInfo] | None = None,
    include_instances: Sequence[str] | None = None,
    exclude_instances: Sequence[str] | None = None,
) -> dict[str, Any]:
    """Convert a :class:`kdb.LayoutToNetlist` to a JSON-serializable dict.

    Parameters
    ----------
    l2n:
        A klayout ``LayoutToNetlist`` whose extraction is complete.
    flatten:
        Collapse the full circuit hierarchy into the top-level circuit.
        Per-net layer annotation is unavailable in flat mode because the
        flattened netlist is a copy detached from the L2N shape data.
    include_layers:
        Keep only nets that touch at least one of these layers.
        Ignored when ``flatten=True``.
    exclude_layers:
        Drop nets whose shapes lie entirely on excluded layers.
        Ignored when ``flatten=True``.
    include_instances:
        Keep only subcircuits whose ``circuit_ref`` name is in this set.
        Ignored when ``flatten=True``.
    exclude_instances:
        Remove subcircuits whose ``circuit_ref`` name is in this set.
        Ignored when ``flatten=True``.

    Returns
    -------
    dict
        ``{"top_circuit": str, "layers": list[{"name": str, "layer": int, "datatype": int}],
        "circuits": {name: {...}, ...}}``.
    """
    return _native.parse_l2n(
        l2n,
        flatten=flatten,
        include_layers=include_layers,
        exclude_layers=exclude_layers,
        include_instances=include_instances,
        exclude_instances=exclude_instances,
    )


def l2n_to_json(
    l2n: kdb.LayoutToNetlist,
    *,
    flatten: bool = False,
    include_layers: Sequence[kdb.LayerInfo] | None = None,
    exclude_layers: Sequence[kdb.LayerInfo] | None = None,
    include_instances: Sequence[str] | None = None,
    exclude_instances: Sequence[str] | None = None,
    indent: int = 2,
) -> str:
    """Convenience wrapper around :func:`parse_l2n` returning a JSON string."""
    return _native.l2n_to_json(
        l2n,
        flatten=flatten,
        include_layers=include_layers,
        exclude_layers=exclude_layers,
        include_instances=include_instances,
        exclude_instances=exclude_instances,
        indent=indent,
    )
