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
