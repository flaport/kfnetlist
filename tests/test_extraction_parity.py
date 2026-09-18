"""Behavior corpus for the Rust extraction port, including live native inputs."""
from dataclasses import dataclass
import gc
import itertools
import json
from types import SimpleNamespace as NS
import warnings

import pytest
from rlayout import db
import kfnetlist
import kfnetlist.extract as candidate
from extraction_reference import reference


@dataclass
class XS:
    main_layer: object
    width: int = 500


def port(angle=0, x=0, y=0, *, complex=False, mirror=False, width=500, layer=1, kind="optical"):
    return NS(
        trans=None if complex else db.Trans(angle, mirror, x, y),
        dcplx_trans=db.DCplxTrans(1, angle * 90, mirror, x * .001, y * .001) if complex else None,
        cross_section=XS(db.LayerInfo(layer, 0), width),
        port_type=kind, kcl=NS(dbu=.001),
    )


def observed(call):
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            result = call()
            value = ("ok", result)
        except Exception as exc:
            value = ("error", type(exc).__name__, str(exc))
    return value, [(w.category.__name__, str(w.message)) for w in caught]


def test_port_flags():
    assert [(p.name, int(p)) for p in kfnetlist.PortCheck] == [
        (p.name, int(p)) for p in reference().PortCheck
    ]


PORT_CASES = list(itertools.product(range(4), range(4), (False, True), ("integer", "complex", "mixed"), (False, True)))


@pytest.mark.parametrize("a,b,mirror,kind,snapped", PORT_CASES)
def test_port_checks(a, b, mirror, kind, snapped):
    p1 = port(a, complex=kind == "complex")
    p2 = port(b, complex=kind != "integer", mirror=mirror)
    expected = reference().check_connection(p1, p2, snapped=snapped)
    assert kfnetlist.check_connection(p1, p2, snapped=snapped) == expected


@pytest.mark.parametrize("change", [dict(x=1), dict(width=501), dict(layer=2), dict(kind="RF")])
def test_port_mismatches(change):
    a, b = port(), port(2, **change)
    assert kfnetlist.check_connection(a, b) == reference().check_connection(a, b)


@pytest.mark.parametrize("tolerance", [0, .1, 1, -1])
@pytest.mark.parametrize("distance", [0, .000099, .0001, .000101])
def test_complex_tolerance_boundary(tolerance, distance):
    a, b = port(complex=True), port(2, complex=True)
    b.dcplx_trans = db.DCplxTrans(1, 180, False, distance, 0)
    assert kfnetlist.check_connection(a, b, tolerance=tolerance) == reference().check_connection(a, b, tolerance=tolerance)


def test_missing_transform_error():
    a, b = port(), port()
    b.trans = None
    assert observed(lambda: kfnetlist.check_connection(a, b)) == observed(lambda: reference().check_connection(a, b))


def optical_port(name, angle=0, **kwargs):
    value = port(angle, **kwargs)
    return NS(name=name, port_type=value.port_type, base=value,
              cross_section=NS(base=value.cross_section))


@pytest.mark.parametrize("case", ["empty", "open", "pair", "displaced", "width", "duplicate", "ignored"])
@pytest.mark.parametrize("allow_width_mismatch", [False, True])
def test_optical(case, allow_width_mismatch):
    ports = [] if case == "empty" else [optical_port("a")]
    if case not in ("empty", "open"):
        ports.append(optical_port("a" if case == "duplicate" else "b", 2,
                                  x=1 if case == "displaced" else 0,
                                  width=501 if case == "width" else 500,
                                  kind="RF" if case == "ignored" else "optical"))
    cell = NS(ports=ports, insts=[])
    def run(fn):
        return observed(lambda: [n.to_dict() for n in fn(cell, allow_width_mismatch=allow_width_mismatch)])
    expected = run(reference().extract._geometry.get_optical_nets)
    if case != "duplicate":
        assert expected[0][0] == "ok", expected
    assert run(candidate.get_optical_nets) == expected


@pytest.mark.parametrize("value", [None, 1, 1.5, "text", [1, 2], ("x", 2), {3: [True, None]}])
def test_settings(value):
    assert candidate.serialize_setting(value) == reference().extract._settings.serialize_setting(value)


@pytest.mark.parametrize("factory", [lambda: db.Box(1, 2, 3, 4), lambda: db.DBox(1, 2, 3, 4),
                                    lambda: db.Trans(2, True, 3, 4), lambda: db.DCplxTrans(1, 33, True, 1.2, -3.4),
                                    lambda: db.Polygon(db.Box(10)), lambda: db.LayerInfo(1, 2)])
def test_native_settings(factory):
    value = {"native": factory()}
    assert candidate.serialize_setting(value) == reference().extract._settings.serialize_setting(value)


def make_hierarchy(array=False, electrical=False):
    import kfactory as kf
    layer = db.LayerInfo(1, 0, "M1" if electrical else "WG")
    kcl = kf.KCLayout(name="parity_pdk", connectivity=[(layer,)] if electrical else [])
    child = kcl.kcell("LEAF")
    child.shapes(layer).insert(db.Box(0, -250, 1000, 250))
    for name, x, angle in [("a", 0, 2), ("b", 1000, 0)]:
        child.create_port(name=name, width=500, trans=db.Trans(angle, False, x, 0),
                          layer_info=layer, port_type="electrical" if electrical else "optical")
    top = kcl.kcell("TOP")
    if array:
        first = top.create_inst(child, a=db.Vector(2000, 0), b=db.Vector(0, 1000), na=2, nb=2)
    else:
        first = top << child
    first.name = "first"
    first.info["nested"] = {"value": 1.5}
    second = top.create_inst(child, db.Trans(1000, 0))
    second.name = "second"
    second.purpose = "measurement"
    return kf, kcl, top


@pytest.mark.parametrize("array,electrical,placement,flatten", itertools.product((False, True), repeat=4))
def test_hierarchy(array, electrical, placement, flatten):
    kf, kcl, top = make_hierarchy(array, electrical)
    kwargs = dict(wrap_kdb_instance=lambda i: kf.Instance(kcl=kcl, instance=i),
                  include_placement=placement, flatten=flatten)
    def run(fn):
        return {name: value.to_dict() for name, value in fn(top, **kwargs).items()}
    before = str(top._base.kdb_cell.bbox()), len(list(top.insts))
    expected = run(reference().extract._algo.extract)
    assert run(candidate.extract) == expected
    assert (str(top._base.kdb_cell.bbox()), len(list(top.insts))) == before


@pytest.mark.parametrize("options", [dict(ignore_unnamed=True), dict(exclude_purposes=["measurement"]),
                                    dict(equivalent_ports={"LEAF": [["a", "b"]]}), dict(flatten=["LEAF"])])
def test_extract_options(options):
    kf, kcl, top = make_hierarchy(electrical=True)
    kwargs = dict(wrap_kdb_instance=lambda i: kf.Instance(kcl=kcl, instance=i), **options)
    expected = reference().extract._algo.extract(top, **kwargs)
    actual = candidate.extract(top, **kwargs)
    assert {k: v.to_dict() for k, v in actual.items()} == {k: v.to_dict() for k, v in expected.items()}


@pytest.mark.parametrize("options", [{}, {"flatten": True}, {"include_instances": ["LEAF"]},
                                    {"exclude_instances": ["LEAF"]}, {"include_layers": []}])
def test_electrical_parser_and_retained_result(options):
    kf, kcl, top = make_hierarchy(electrical=True)
    expected_l2n = reference().extract._l2n.l2n_elec(top)
    actual_l2n = candidate.l2n_elec(top)
    expected = reference().extract._parser.parse_l2n(expected_l2n, **options)
    # Electrical marking duplicates the layout. Results must outlive the source.
    del top, kcl
    gc.collect()
    assert candidate.parse_l2n(actual_l2n, **options) == expected
    assert candidate.l2n_to_json(actual_l2n, **options) == reference().extract._parser.l2n_to_json(expected_l2n, **options)
    assert candidate.detect_shorts(actual_l2n) == reference().extract._shorts.detect_shorts(expected_l2n)
