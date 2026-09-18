"""Port comparison flags and a lazy boundary to native geometry."""
from ._native import PortCheck


def check_connection(p1, p2, *, tolerance=0.1, angle_tolerance=0.01, snapped=False):
    from rlayout._native import _kfnetlist_extract
    return PortCheck(_kfnetlist_extract.check_connection(
        p1, p2, tolerance=tolerance, angle_tolerance=angle_tolerance, snapped=snapped
    ))
