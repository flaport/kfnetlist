# Rust extraction migration

Status: Stage 1 validated on 2026-09-18; Stage 2 is next.

The parent RLayout repository's `work.md` is the implementation plan and parity
contract. Preserve the existing Python signatures and observable outputs while
moving all domain logic into Rust. The model crate remains independent of
RLayout; a new extraction crate will use RLayout directly. PyO3 is permanent
conversion/binding code. The first target remains x86-64 GNU/Linux.

## Baseline

- Parent plan: `89c189a9e32b8e4f30075d4ee3c52b505645e946`.
- Initial KFNetlist: `494eb6eb80961798d96fc9be4b3f76672dfa03e7`.
- KFactory: `53c43c14a9d66a33f6f991db7bb05f0bb36a1cc6`.
- Python oracle: `662c5a99532e760680a18f7d6f34e47ff89f01a3`, the initial implementation with explicit RLayout
  binding adaptations for parser/shorts internal-layout access, native array
  element transforms and test setup.
- KFNetlist 0.3.0 uses PyO3/pythonize 0.23; RLayout uses PyO3 0.29.2.

The oracle loader in `tests/extraction_reference.py` reads only pinned Git
objects, without network access or a production fallback. It keeps the existing
Rust model classes, while loading all remaining Python algorithms under a
separate test-only namespace. It fails if the reference commit is unavailable.

## Boundary inventory

There are eleven named migration entry points: `extract`, `get_optical_nets`,
`l2n_elec`, `parse_l2n`, `l2n_to_json`, `detect_shorts`, `serialize_setting`,
`check_connection`, `PortCheck`, `ShortResult`, and `flatten_netlists`.
The model's existing public types/methods are covered by the unchanged model,
flatten, placement, instance-info and binding compatibility tests.

| Python implementation | Destination | Python boundary work |
| --- | --- | --- |
| `port_check.py` | extraction crate: flags and geometry comparisons | read port/XS metadata |
| `extract/_geometry.py` | extraction crate: adjacency, arrays, open ports | read ports and native instance handles |
| `extract/_l2n.py` | extraction crate: duplicate/mark/connect/extract | unwrap layout and copy metadata |
| `extract/_parser.py` | extraction crate: hierarchy, filters, shape serialization | wrap dictionaries/JSON |
| `extract/_shorts.py` | extraction crate: native intersections | wrap live Region results |
| `extract/_settings.py` | Rust serialization rules | convert arbitrary Python values |
| `extract/_algo.py` | extraction crate: hierarchy/equivalence/placement/orchestration | read KFactory metadata; preserve naming hook |
| `_flatten.py` | existing core flattening engine and Rust mapping orchestration | convert mapping inputs/outputs |

KFactory's direct extraction seam consists of three calls in `kcell.py`:
`l2n_elec`, `extract`, and `get_optical_nets`. Its `checks.py` has two `check_connection` calls and imports
`check_connection` and `PortCheck`; the remaining imported KFNetlist classes are
model consumers. Preserve the required `wrap_kdb_instance` naming hook at the
Python boundary, without putting Python callbacks into the Rust extraction API.

## Characterization

`tests/test_extraction_parity.py` supplies paired port comparisons, tolerance
boundaries, malformed transforms, optical opens/duplicates/mismatches, nested
and native settings, array and ordinary hierarchies, electrical extraction,
placement, flattening, purpose filters, equivalent ports, parser filters and
returned extraction lifetime checks. Each case executes both implementations.
Keep ordering, values, exception categories/messages and warnings intact.

Existing parser golden files remain independent reference artifacts; do not
replace them with candidate output. Existing tests now import RLayout and use
its explicit constructors. No assertions were weakened.

Run from the parent repository, after pushing the stage and installing its pin:

```sh
.venv/bin/python scripts/test-kfnetlist.py vendor/kfnetlist/tests -q
just kfactory-test tests/test_netlist.py tests/test_l2n.py tests/test_schematic.py -q
```

Record exact totals and blockers after execution. The new differential cases
must not silently pass when both implementations unexpectedly raise; valid-input
cases require a successful reference execution. The explicitly malformed cases
compare exceptions and warnings.

### Baseline corrections found during characterization

Native array transforms use `Instance.element_transform(...).s_trans()` instead
of constructing an unbound `InstElement`. Parser transforms and recognized
native settings use native string serialization instead of Python's default
object-address repr. These are backend adapter repairs, made before porting;
address strings are not normalized away by the differential harness.

The parent also repairs extraction Region ownership: native delegate transfer
preserves registered deep-layer identity, and materialization releases only lazy
source leases, retaining extraction owners. Existing parser golden files pass
unchanged after that repair.

## Stage 1 acceptance evidence

Validated with KFNetlist runtime `808cdc11`, Python oracle `662c5a9`, parent
native fixes through `85ffa51`, and KFactory `53c43c1`:

- Guarded KFNetlist suite: **484 passed**, one existing deprecated-method warning.
- Differential corpus: **269 cases / 279 observations / zero differences**.
  `tests/data/extraction_parity.json` records the reference results, retaining
  dictionary order and tuple/list distinctions. A separate process replayed
  every recorded case successfully. No object addresses are normalized away.
- Original parser golden files: pass unchanged.
- Guarded KFactory netlist/L2N/schematic modules: **28 passed / 1 skipped**.
- KFNetlist standalone Rust core: **11 passed**.
- Parent Rust workspace: **159 passed**; Python suite: **176 passed**.
- Parent workspace check/format and native ASan/UBSan/LSan harness: passed.

Parent logs are under `target/kfnetlist-stage1-*.log`. To regenerate this initial
reference corpus, explicitly set `KFNETLIST_RECORD_CORPUS=1` while running the
complete `test_extraction_parity.py` module. After Rust porting starts, a changed
reference needs an explained contract decision; do not regenerate from candidate
behavior to hide a difference. The recording helper reads expected results from
the pinned oracle and only writes after successful comparisons.

## Stage 2: single native image

`kfnetlist-extract` is a Python-independent crate over RLayout. Its
`connected_geometry` primitive borrows a layout, validates owned cell/layer
identities and returns the native owner-retaining extraction context. Typed port
and cell metadata carry values, never Python objects or callbacks. The standalone
example retains its graph after dropping the source and extraction wrappers.

`kfnetlist-extract-python` is an rlib registered by `rlayout-extension`. Its
safe Rust interop guards borrow the existing RLayout Python wrapper. Both
bindings and the engine live in one extension image. The independent model
extension stays engine-free. Do not pass Rust objects or capsules between these
images. Model results cross the boundary as ordinary Python values.

Both binding crates use PyO3 0.29; model conversion uses pythonize 0.29. Migration
reference: https://pyo3.rs/main/migration and https://docs.rs/pythonize/0.29.0/ .
The extraction dependencies pin RLayout's integration bootstrap. The parent
workspace patches those packages to its current local crates, so integration
uses exactly one Rust type/engine identity. No Cargo dependency cycle exists:
rlayout-extension -> extraction bindings -> rlayout-python -> rlayout.

The model-only Python package still imports without RLayout. Extraction is
provided by the common RLayout wheel. Stage 2 introduces the bridge but does not
yet replace the public extraction algorithms; Stage 3 handles that cutover.
Stage 2 implementation awaits post-push validation.

The private RLayout source pin uses SSH, matching the parent checkout. Cargo
fetches may need `CARGO_NET_GIT_FETCH_WITH_CLI=true` with the user's existing
Git credentials. The parent already configures that setting.

### Stage 2 validated, 2026-09-18

Runtime KFNetlist `62d30bc`, parent implementation `bab93d5`.
The parent Rust workspace passes 159 tests; core/extraction crates pass 11 + 2.
The standalone example runs with `PYO3_NO_PYTHON=1`. `readelf`, `ldd`, and
`nm -D` show neither Python runtime dependencies nor Python API symbols.
Parent check/format, native static executable/cdylib tests and ASan/UBSan/LSan
checks pass. Maturin rebuilds the shared extension and upgrades model bindings.

Parent Python: 179 passed. Guarded KFNetlist suite plus bridge tests: 487 passed,
including 269 differential cases / 279 observations / zero differences.
Bridge tests exercise a real KFactory layout, source write rejection, retained
native results, and foreign/deleted handle rejection. The model-only extension
continues to import independently; clean wheel packaging is the Stage 4 gate.

Reproduce from the parent with `cargo check --workspace`,
`cargo test --workspace`, `cargo test -p kfnetlist-extract -p kfnetlist-core`,
`python3 scripts/cargo-kfnetlist.py run -p kfnetlist-extract --example standalone --target-dir target`,
`.venv/bin/python -m unittest discover -s tests/python`, and
`.venv/bin/python scripts/test-kfnetlist.py vendor/kfnetlist/tests tests/python/test_kfnetlist_bridge.py -q`.
Authoritative command logs: parent `target/kfnetlist-stage2-*.log`.

## Stage 3 implementation (awaiting validation)

All public extraction operations now dispatch into `kfnetlist-extract` through
`kfnetlist-extract-python`: port flags/geometry, ordered optical adjacency,
electrical marker extraction, ordered L2N parsing, shorts, settings encoding,
placement and hierarchical assembly. Collection flattening moved to the existing
model core. Source algorithms remain reproducible only through the pinned test
oracle; production Python files are exports and the lazy port-check boundary.

Metadata equality is interned at the boundary. Native extraction matches are
returned to the binding for the existing KFactory name hook, then resolved names
are supplied to Rust finalization; no Python callback enters the Rust API. Native
geometry access uses checked wrapper guards, not method calls through `rlayout.db`.
Python JSON encoding and opaque-value representation stay boundary conversions.
The Rust parser's ordered tree is independently serializable. Original native
wrapper formatting and opaque identity are preserved by setting conversion.

`examples/optical.rs` exercises the complete standalone extraction API. The
frozen 269-case corpus and existing parser goldens remain unchanged. Stage 3 is
awaiting its post-push compilation and parity checks.
