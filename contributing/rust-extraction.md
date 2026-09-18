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

### Stage 3 validated, 2026-09-18

KFNetlist `e3c830a`, parent integration `12ee958`: 485 KFNetlist tests pass,
including the original 269 differential cases / 279 observations / zero
unacceptable differences. An additional reference-observed empty-instance
placement regression passes without replacing any frozen expectations.
The affected guarded KFactory netlist/L2N/schematic modules pass 28 / skip 1.
Parent Python tests pass 179, parent Rust tests 159, model core tests 11 and
extraction tests 4. Workspace check/format and native ownership sanitizers pass.
The complete `optical` Rust example executes with `PYO3_NO_PYTHON=1`, and its
normal dependency tree, dynamic dependencies and imported symbols contain no
Python dependency or runtime. Logs: parent `target/kfnetlist-stage3-*.log`.

Post-push corrections addressed native wrapper selection, typed borrow guards,
list-based parser filters, private metadata helper compatibility and the
reference's missing-bounds error. The Python reference/golden files were not
changed. Stage 4 now verifies standalone wheel packaging, public signatures,
typing and independent frozen-fixture replay before removing the test oracle.

## Stage 4 cutover (awaiting validation)

Python retains the original function signatures, defaults and structural typing
as thin forwarders. Model and flag imports remain independent of RLayout.
The temporary Git-loaded Python oracle and corpus-recording switch are removed;
269 cases now replay the same 279 independently captured observations directly.
`tests/data/extraction_parity.json` is unchanged. To reproduce characterization,
use harness revision `60810814` and oracle `662c5a99532e760680a18f7d6f34e47ff89f01a3`.

Build this model wheel and the matching parent RLayout wheel with Maturin.
The parent `scripts/check-kfnetlist-wheels.py MODEL_WHEEL RLAYOUT_WHEEL` creates
a fresh environment, proves independent model imports, then tests both import
orders, complete extraction and retained native graph lifetime. Rust extraction
still uses only `kfnetlist-extract`/`rlayout`; PyO3 exists in the binding crates.

### Stage 4 validated, 2026-09-18

At KFNetlist `8ed5d21` / parent `146cefb`, all 485 tests pass, including the
269-case/279-observation frozen replay with unchanged expectations. All 16 typed
forwarders preserve original signatures. Fresh paired wheel installations pass
in both import orders, including complete extraction and native lease/lifetime
checks; the model wheel also works with RLayout absent. The Rust example still
builds/runs without Python discovery or linkage. Parent Rust/Python checks pass.
Logs and artifacts: parent `target/kfnetlist-stage4-*`. Final consumer acceptance
remains Stage 5; no broad suite claim is made from the focused checks.

## Final acceptance sequence

Use the validated paired wheels for the guarded KFactory netlist, L2N,
schematic, metadata and PDK modules, then the complete suite. Execute the four
schematic documentation notebooks with the named `rlayout` kernel and the
upstream-KLayout import guard. Parent `scripts/check-kfnetlist-notebooks.py`
records executed artifacts; `work.md` will record the final report and counts.
The broad migration acceptance gate remains pending until those runs finish.

### Model source-build isolation

The CI retry showed that path overrides alone do not reliably prevent Cargo
from fetching the original Git source during fresh lock resolution. The model
workspace now contains only core/model bindings. Extraction and its companion
binding use separate crate workspaces with explicit package metadata. Building
or importing the model must not resolve RLayout. Parent Cargo integration still
links extraction into the shared engine image, with no domain/source changes.
Validate from a fresh source archive with no lockfiles, no Git cache and no
network, not only from an already-resolved developer checkout.

Final acceptance exposed a pre-existing RLayout instance-refresh bottleneck.
The user authorized a bounded native bookkeeping fix and a timed full KFactory
CI gate. The extraction corpus still passes 485 tests and the four schematic
notebooks pass with release wheels. Final acceptance awaits the native fix and
full-suite run; neither interrupted routing probes nor partial runs count as a
full pass. The parent `work.md` records the measured baseline and validation plan.


### Final migration acceptance (2026-09-18)

Runtime parent `d630d8d` with KFNetlist `892e358` and KFactory `53c43c1` passes
all **3,134 KFactory tests / 4 skips** in **168.25 seconds** (184.80 seconds
including startup/shutdown/reporting). Parent report:
`target/kfactory-reports/kfactory-full-20260918-143914.log`, exit 0.
All **485 KFNetlist tests** pass, including **269 frozen cases / 279 reference
observations / zero differences**. The four affected schematic notebooks pass
through the named `rlayout` kernel (35 original code cells plus four guards).
Parent Rust/Python tests and native ownership sanitizers also pass.

The pre-existing native instance bottleneck is fixed using stable editable
references and explicit replacement-alias updates. A 512-instance retained
append/property/transform workload falls from 2.0397 to 0.01854 seconds. The
parent CI now runs the guarded full suite against its release wheel with a
7-minute test limit and retained reports, and gates documentation publication
on success. Hosted run `35347114416` passes 3,134 / skips 4 in 315.87 seconds
(332.20 seconds including startup/shutdown); its report artifact is retained.

https://github.com/doplaydo/rlayout/actions/runs/35347114416

This completes acceptance of the Rust migration. The Rust-only extraction
example, independent model installation, paired-wheel import orders, unchanged
signatures, fixture replay and fresh model source-build isolation are validated
as recorded above. No Python extraction algorithm or runtime fallback remains.
