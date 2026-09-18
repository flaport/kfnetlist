# kfnetlist

**kfnetlist** is a standalone, Rust-backed netlist schema for
[kfactory](https://github.com/gdsfactory/kfactory) and netlist tooling.

It provides a fast, type-safe data model for circuit connectivity — instances,
nets, ports, and arrays — with full JSON/dict serialization and Pydantic v2
integration. A Python-independent Rust crate owns the core types and algorithms;
a separate PyO3 crate exposes them as native Python classes.

---

## Installation of this fork

This branch moves extraction into Rust. Build the model wheel with Maturin from
this checkout, or install the KFNetlist wheel produced alongside an RLayout
checkout that pins this commit. The model package has no Python dependencies.
Extraction and geometric port checking additionally require the **matching
RLayout wheel**. These paired development wheels are not a PyPI release.

```bash
python -m pip install /path/to/wheels/kfnetlist-*.whl /path/to/wheels/rlayout-*.whl
```

The validated extraction platform is x86-64 GNU/Linux. Build requirements and
remaining shared system libraries are documented in the RLayout repository.
Do not combine this branch's extraction adapters with an arbitrary older RLayout
wheel: the checked live-object bridge is compiled into their shared engine image.
Python callers still use Python; Rust callers do not require it.

## Quick Example

```python
from kfnetlist import Netlist, NetlistPort, PortRef

nl = Netlist()

# Add instances
nl.create_inst("wg1", kcl="MY_PDK", component="straight",
               settings={"width": 500, "length": 10_000})
nl.create_inst("wg2", kcl="MY_PDK", component="straight",
               settings={"width": 500, "length": 10_000})

# Add a top-level port and connect it
p_in = nl.create_port("in")
nl.create_net(p_in, PortRef(instance="wg1", port="o1"))

# Internal net
nl.create_net(
    PortRef(instance="wg1", port="o2"),
    PortRef(instance="wg2", port="o1"),
)

# Serialize to JSON
print(nl.to_json())
```

## Connectivity Verification

kfnetlist provides tools for LVS-style connectivity verification: detecting
opens, comparing against reference netlists, and finding geometric shorts.

```python
# Check for open circuits
opens = extracted_nl.detect_opens()
if opens["unconnected_ports"]:
    print(f"Unconnected: {opens['unconnected_ports']}")

# Compare against schematic
diff = extracted_nl.find_net_difference(schematic_nl)
if diff["missing"]:
    print(f"{len(list(diff['missing']))} nets missing from layout")

# Geometric short detection (requires the paired RLayout wheel)
from kfnetlist.extract import detect_shorts
shorts = detect_shorts(l2n)
for s in shorts:
    print(f"Short: {s.net_a} <-> {s.net_b} on {s.layer}")
```

For a complete walkthrough, see the
[LVS Verification Guide](https://gdsfactory.github.io/kfnetlist/guides/lvs_verification/).

## Key Features

- **Rust core** — Netlist, Net, and port types are implemented in Rust for
  speed and memory safety, exposed via PyO3
- **Zero runtime dependencies** — the base package has no Python dependencies
- **Full serialization** — `to_json()` / `from_json()` and `to_dict()` /
  `from_dict()` on every type
- **Pydantic v2 support** — all types implement `__get_pydantic_core_schema__`
- **Equivalent ports** — `Netlist.normalize()` folds electrically-equivalent
  ports into canonical names for netlist comparison
- **Hierarchical flattening** — `Netlist.flatten()` replaces instances by the
  contents of their own cell's netlist (and `flatten_netlists()` does it for a
  whole `{cell name: netlist}` mapping), rewiring nets across both levels
- **Instance removal** — `Netlist.remove_instances()` deletes sub-cell
  instances, merging the nets they touched
- **Port checking** — `PortCheck` bitmask and `check_connection()` for
  geometric port-pair comparison (requires the paired RLayout wheel)
- **Connectivity verification** — `detect_opens()`, `find_net_difference()`,
  and `detect_shorts()` for LVS-style verification workflows
- **Netlist extraction** — `kfnetlist.extract` subpackage extracts hierarchical
  netlists from KFactory/RLayout cells (requires the paired RLayout wheel)

## Rust usage

The Cargo workspace contains the independent `kfnetlist-core` model and
`kfnetlist-extract` domain libraries, plus their two PyO3 binding crates.
`kfnetlist-extract` depends directly on the RLayout Rust crate. Rust consumers can
depend on either library without a Python installation or PyO3 dependency:

```toml
[dependencies]
kfnetlist-core = { path = "/path/to/kfnetlist/crates/kfnetlist-core" }
serde_json = "1"
```

```rust
use kfnetlist_core::{Netlist, NetMember, PortRef};
use serde_json::json;

let mut nl = Netlist::default();
nl.create_inst("wg1".into(), "PDK".into(), "straight".into(),
               json!({"width": 0.5}), 1, 1)?;
let input = nl.create_port("in".into());
nl.create_net([
    NetMember::Port(input),
    NetMember::Ref(PortRef { instance: "wg1".into(), port: "o1".into() }),
])?;
let opens = nl.detect_opens();
let serialized = kfnetlist_core::to_json(&nl)?;
```

Run the complete example with `cargo run -p kfnetlist-core --example connectivity`.
Run Rust tests with `cargo test -p kfnetlist-core`, and Python compatibility tests
with `uv run --extra dev --with pydantic pytest`. Maturin uses the binding manifest configured in
`pyproject.toml`, so source and wheel builds still run from the repository root.
See [the Rust API guide](contributing/rust-core.md) for ownership, serialization,
and compatibility details.

## Extraction architecture

```text
Rust callers -> kfnetlist-extract -> rlayout
                       |
                       v
                 kfnetlist-core <- kfnetlist._native <- Python model callers
                       ^
                       |
Python extraction -> thin adapters -> rlayout._native._kfnetlist_extract
```

The shared RLayout extension links `rlayout-python` and
`kfnetlist-extract-python` into one native engine image. The shim reads KFactory
metadata, borrows checked native handles, calls Rust and converts results. It
retains owners and preserves stale-handle rejection and extraction read leases.
The separate model extension never exchanges engine pointers with it.

For a full Rust-only extraction example, run:

```bash
PYO3_NO_PYTHON=1 cargo run -p kfnetlist-extract --example optical
```

The pinned RLayout dependency currently uses SSH access to its development
repository. In an RLayout checkout, `scripts/cargo-kfnetlist.py` substitutes the
local Rust crates. The independent core example remains available without any
RLayout dependency. See `contributing/rust-extraction.md` for the parity corpus,
bridge ownership and validation evidence.

## Documentation

Full documentation: https://gdsfactory.github.io/kfnetlist

## License

kfnetlist is released under the [Apache License 2.0](LICENSE).
