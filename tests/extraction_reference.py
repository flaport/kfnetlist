"""Temporary, pinned Python oracle; never imported by the production package."""
from functools import lru_cache
from pathlib import Path
import subprocess
import sys
import types

REFERENCE = "d831f0b"
ROOT = Path(__file__).resolve().parents[1]


@lru_cache(maxsize=1)
def reference():
    import kfnetlist

    name = "_kfnetlist_reference"
    package = types.ModuleType(name)
    package.__path__ = []
    # Existing Rust model types are shared; every remaining Python algorithm
    # comes from the pinned commit, including references between modules.
    for export in kfnetlist.__all__:
        setattr(package, export, getattr(kfnetlist, export))
    sys.modules[name] = package
    extract = types.ModuleType(name + ".extract")
    extract.__path__ = []
    sys.modules[extract.__name__] = extract
    package.extract = extract
    for suffix in ("_flatten", "port_check", "extract._geometry", "extract._l2n",
                   "extract._parser", "extract._shorts", "extract._settings", "extract._algo"):
        path = "src/kfnetlist/" + suffix.replace(".", "/") + ".py"
        source = subprocess.check_output(
            ["git", "-C", str(ROOT), "show", f"{REFERENCE}:{path}"], text=True
        ).replace("from kfnetlist", "from " + name)
        # The model extension remains the independent, already-ported core.
        source = source.replace("from ._native", "from kfnetlist._native")
        module = types.ModuleType(name + "." + suffix)
        module.__file__ = f"{REFERENCE}:{path}"
        module.__package__ = module.__name__.rpartition(".")[0]
        sys.modules[module.__name__] = module
        exec(compile(source, module.__file__, "exec"), module.__dict__)
        parent, _, leaf = module.__name__.rpartition(".")
        setattr(sys.modules[parent], leaf, module)
        if suffix == "_flatten":
            package.flatten_netlists = module.flatten_netlists
        elif suffix == "port_check":
            package.PortCheck = module.PortCheck
            package.check_connection = module.check_connection
    return package
