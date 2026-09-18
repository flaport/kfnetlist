"""Behavior corpus for the Rust extraction port, including live native inputs."""

from dataclasses import dataclass
import gc
import itertools
import json
from pathlib import Path
from types import SimpleNamespace as NS
import warnings

import pytest
from rlayout import db
import kfnetlist
import kfnetlist.extract as candidate


CORPUS = json.loads((Path(__file__).parent / "data/extraction_parity.json").read_text())
CURRENT_CASE = None
OBSERVATION = 0


def wire(value):
    """Retain dictionary iteration order and tuple/list distinctions in JSON."""
    if isinstance(value, dict):
        return {"dict": [[wire(k), wire(v)] for k, v in value.items()]}
    if isinstance(value, tuple):
        return {"tuple": [wire(v) for v in value]}
    if isinstance(value, list):
        return [wire(v) for v in value]
    return value


def assert_same(actual):
    global OBSERVATION
    expected = CORPUS[CURRENT_CASE][OBSERVATION]
    assert wire(actual) == expected
    OBSERVATION += 1


@pytest.fixture(autouse=True)
def corpus_case(request):
    global CURRENT_CASE, OBSERVATION
    CURRENT_CASE = request.node.name
    OBSERVATION = 0
    yield
    assert OBSERVATION == len(CORPUS[CURRENT_CASE]), "missing frozen observations"


@dataclass
class XS:
    main_layer: object
    width: int = 500


def port(
    angle=0,
    x=0,
    y=0,
    *,
    complex=False,
    mirror=False,
    width=500,
    layer=1,
    kind="optical",
):
    return NS(
        trans=None if complex else db.Trans(angle, mirror, x, y),
        dcplx_trans=db.DCplxTrans(1, angle * 90, mirror, x * 0.001, y * 0.001)
        if complex
        else None,
        cross_section=XS(db.LayerInfo(layer, 0), width),
        port_type=kind,
        kcl=NS(dbu=0.001),
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
    assert_same([(p.name, int(p)) for p in kfnetlist.PortCheck])


PORT_CASES = list(
    itertools.product(
        range(4),
        range(4),
        (False, True),
        ("integer", "complex", "mixed"),
        (False, True),
    )
)


@pytest.mark.parametrize("a,b,mirror,kind,snapped", PORT_CASES)
def test_port_checks(a, b, mirror, kind, snapped):
    p1 = port(a, complex=kind == "complex")
    p2 = port(b, complex=kind != "integer", mirror=mirror)
    assert_same(kfnetlist.check_connection(p1, p2, snapped=snapped))


@pytest.mark.parametrize(
    "change", [dict(x=1), dict(width=501), dict(layer=2), dict(kind="RF")]
)
def test_port_mismatches(change):
    a, b = port(), port(2, **change)
    assert_same(kfnetlist.check_connection(a, b))


@pytest.mark.parametrize("tolerance", [0, 0.1, 1, -1])
@pytest.mark.parametrize("distance", [0, 0.000099, 0.0001, 0.000101])
def test_complex_tolerance_boundary(tolerance, distance):
    a, b = port(complex=True), port(2, complex=True)
    b.dcplx_trans = db.DCplxTrans(1, 180, False, distance, 0)
    assert_same(kfnetlist.check_connection(a, b, tolerance=tolerance))


def test_missing_transform_error():
    a, b = port(), port()
    b.trans = None
    assert_same(observed(lambda: kfnetlist.check_connection(a, b)))


def optical_port(name, angle=0, **kwargs):
    value = port(angle, **kwargs)
    return NS(
        name=name,
        port_type=value.port_type,
        base=value,
        cross_section=NS(base=value.cross_section),
    )


@pytest.mark.parametrize(
    "case", ["empty", "open", "pair", "displaced", "width", "duplicate", "ignored"]
)
@pytest.mark.parametrize("allow_width_mismatch", [False, True])
def test_optical(case, allow_width_mismatch):
    ports = [] if case == "empty" else [optical_port("a")]
    if case not in ("empty", "open"):
        ports.append(
            optical_port(
                "a" if case == "duplicate" else "b",
                2,
                x=1 if case == "displaced" else 0,
                width=501 if case == "width" else 500,
                kind="RF" if case == "ignored" else "optical",
            )
        )
    cell = NS(ports=ports, insts=[])

    def run(fn):
        return observed(
            lambda: [
                n.to_dict() for n in fn(cell, allow_width_mismatch=allow_width_mismatch)
            ]
        )

    actual = run(candidate.get_optical_nets)
    if case != "duplicate":
        assert actual[0][0] == "ok", actual
    assert_same(actual)


@pytest.mark.parametrize(
    "value", [None, 1, 1.5, "text", [1, 2], ("x", 2), {3: [True, None]}]
)
def test_settings(value):
    assert_same(candidate.serialize_setting(value))


@pytest.mark.parametrize(
    "factory",
    [
        lambda: db.Box(1, 2, 3, 4),
        lambda: db.DBox(1, 2, 3, 4),
        lambda: db.Trans(2, True, 3, 4),
        lambda: db.DCplxTrans(1, 33, True, 1.2, -3.4),
        lambda: db.Polygon(db.Box(10)),
        lambda: db.LayerInfo(1, 2),
    ],
)
def test_native_settings(factory):
    value = {"native": factory()}
    assert_same(candidate.serialize_setting(value))


def make_hierarchy(array=False, electrical=False):
    import kfactory as kf

    layer = db.LayerInfo(1, 0, "M1" if electrical else "WG")
    kcl = kf.KCLayout(
        name="parity_pdk",
        connectivity=[(layer, db.LayerInfo(2, 0, "M2"))] if electrical else [],
    )
    child = kcl.kcell("LEAF")
    child.shapes(layer).insert(db.Box(0, -250, 1000, 250))
    for name, x, angle in [("a", 0, 2), ("b", 1000, 0)]:
        child.create_port(
            name=name,
            width=500,
            trans=db.Trans(angle, False, x, 0),
            layer_info=layer,
            port_type="electrical" if electrical else "optical",
        )
    top = kcl.kcell("TOP")
    if array:
        first = top.create_inst(
            child, a=db.Vector(2000, 0), b=db.Vector(0, 1000), na=2, nb=2
        )
    else:
        first = top << child
    first.name = "first"
    first.info["nested"] = {"value": 1.5}
    second = top.create_inst(child, db.Trans(1000, 0))
    second.name = "second"
    second.purpose = "measurement"
    return kf, kcl, top


@pytest.mark.parametrize(
    "array,electrical,placement,flatten",
    list(itertools.product((False, True), repeat=4)),
)
def test_hierarchy(array, electrical, placement, flatten):
    kf, kcl, top = make_hierarchy(array, electrical)
    kwargs = dict(
        wrap_kdb_instance=lambda i: kf.Instance(kcl=kcl, instance=i),
        include_placement=placement,
        flatten=flatten,
    )

    def run(fn):
        return {name: value.to_dict() for name, value in fn(top, **kwargs).items()}

    before = str(top._base.kdb_cell.bbox()), len(list(top.insts))
    assert_same(run(candidate.extract))
    assert (str(top._base.kdb_cell.bbox()), len(list(top.insts))) == before


@pytest.mark.parametrize(
    "options",
    [
        dict(ignore_unnamed=True),
        dict(exclude_purposes=["measurement"]),
        dict(equivalent_ports={"LEAF": [["a", "b"]]}),
        dict(flatten=["LEAF"]),
    ],
)
def test_extract_options(options):
    kf, kcl, top = make_hierarchy(electrical=True)
    kwargs = dict(
        wrap_kdb_instance=lambda i: kf.Instance(kcl=kcl, instance=i), **options
    )
    actual = candidate.extract(top, **kwargs)
    assert_same({k: v.to_dict() for k, v in actual.items()})


@pytest.mark.parametrize(
    "options",
    [
        {},
        {"flatten": True},
        {"include_instances": ["LEAF"]},
        {"exclude_instances": ["LEAF"]},
        {"include_layers": []},
    ],
)
def test_electrical_parser_and_retained_result(options):
    kf, kcl, top = make_hierarchy(electrical=True)
    actual_l2n = candidate.l2n_elec(top)
    # Electrical marking duplicates the layout. Results must outlive the source.
    del top, kcl
    gc.collect()
    assert_same(candidate.parse_l2n(actual_l2n, **options))
    assert_same(candidate.l2n_to_json(actual_l2n, **options))
    assert_same(candidate.detect_shorts(actual_l2n))


@pytest.mark.parametrize("case", ["empty", "library", "virtual"])
def test_cell_relationships(case):
    import kfactory as kf

    kcl = kf.KCLayout(name="parity_relationships")
    top = kcl.kcell("ROOT")
    if case == "library":
        source = kf.KCLayout(name="parity_source")
        child = source.kcell("LIB_LEAF")
        child.shapes(db.LayerInfo(1, 0)).insert(db.Box(1000))
        inst = top << child
        inst.name = "proxy"
        assert inst.cell.is_library_cell()
    elif case == "virtual":
        virtual = kcl.vkcell("VIRTUAL")
        virtual.shapes(kcl.layer(1, 0)).insert(db.DPolygon(db.DBox(1)))
        kf.VInstance(virtual).insert_into(top)
    kwargs = dict(
        wrap_kdb_instance=lambda i: kf.Instance(kcl=kcl, instance=i),
        include_placement=True,
    )
    actual = candidate.extract(top, **kwargs)
    assert_same({k: v.to_dict() for k, v in actual.items()})
