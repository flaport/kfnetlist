"""Additional reference-observed edge behavior beyond the frozen main corpus."""
import pytest
from kfnetlist.extract import extract
from kfnetlist.extract._algo import _placement_for


def test_empty_instance_placement_preserves_reference_error():
    # Observed at pinned Python oracle 662c5a9: a missing native bbox remains
    # missing, rather than becoming an explicit empty box during the port.
    import kfactory as kf
    kcl = kf.KCLayout("empty_placement_contract")
    top = kcl.kcell("TOP")
    child = kcl.kcell("EMPTY")
    inst = top << child
    inst.name = "empty"
    with pytest.raises(AttributeError, match="'NoneType' object has no attribute 'left'"):
        _placement_for(inst)
    with pytest.raises(AttributeError, match="'NoneType' object has no attribute 'left'"):
        extract(top, wrap_kdb_instance=lambda i: kf.Instance(kcl=kcl, instance=i), include_placement=True)
