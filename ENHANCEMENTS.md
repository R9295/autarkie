# FUTURE.md — Enhancements, rated and sorted

Merged and de-duplicated from `BESSER.md` (B1–B9) and `ENHANCEMENTS.md` (E1–E16).
The two lists overlap heavily; each entry below cites its source(s).

**Scales**
- **Ease (1–5):** 5 = trivial, localized (hours); 4 = small & contained (~a day);
  3 = moderate, some design (a few days); 2 = a new subsystem; 1 = large/architectural.
- **Effect (1–5):** expected increase in the fuzzer's *bug-finding power*.
  5 = transformational; 3 = solid; 1 = indirect (dev-velocity, safety, docs).
- **Score = Ease + Effect.** Sorted by Score ↓, then Effect ↓ (favor impact among
  equally cheap items). Items rated by reading the code, not the prose.

| # | Enhancement | Ease | Effect | Score | Sources |
|---|-------------|:----:|:------:|:-----:|---------|
| 1 | Robustness / UB pass (keep campaigns alive) | 5 | 4 | **9** | B5 |
| 2 | CMPLOG/I2S → first-class grammar solver | 3 | 5 | **8** | B2, E10 |
| 3 | Broaden grammar coverage of std types | 4 | 3 | **7** | B7 |
| 4 | Feedback-driven generation (bandit weighting) | 2 | 5 | **7** | B1 |
| 5 | Adaptive mutator scheduling | 3 | 4 | **7** | E5 |
| 6 | Structural / grammar coverage feedback | 3 | 4 | **7** | B3 |
| 7 | SCC-based recursion detection | 3 | 3 | **6** | E2 |
| 8 | Correct + structure-preserving minimization | 3 | 3 | **6** | B6, E11 |
| 9 | Performance: RNG, serialization, chunk cap | 3 | 3 | **6** | B8 |
| 10 | Stable type identity + chunk-store hardening | 3 | 3 | **6** | B4, E7 |
| 11 | Configurable corpus seeding | 3 | 3 | **6** | E8 |
| 12 | CI across all supported surfaces | 4 | 2 | **6** | E1 |
| 13 | Test suites (runtime + derive + property) | 4 | 2 | **6** | E3, E4, E16, B9 |
| 14 | Honor/remove dead config + lazy rendering | 4 | 2 | **6** | B9, E12 |
| 15 | Observability: stats.json + grammar report | 4 | 2 | **6** | E6, E9 |
| 16 | Documentation around target workflows | 5 | 1 | **6** | E15 |
| 17 | Hermetic libFuzzer runtime builds | 3 | 2 | **5** | E13 |
| 18 | Stable public API boundary | 3 | 1 | **4** | E14 |

---

## Tier A — Quick wins (do first: cheap, and they unblock the rest)

### 1. Robustness / UB pass — Ease 5, Effect 4
Localized fixes that stop the process from dying mid-campaign (longer runs = more
cumulative bugs): `from_file` → `maybe_deserialize` + skip; replace every
`random_range(0, n-1)` with `if n == 0 { Skipped }` + `below(NonZero::new(n)?)`
(kills the release-mode UB class); guard `get_string` on an empty pool; fix the
libFuzzer argv parse; fix numeric range mapping + divide-by-zero. *Already tracked
as P1 bug-fixes — list them here too because they gate every long campaign.*
`B5` (= FEHLER B1/B2/B3/B4/B7).

### 3. Broaden grammar coverage of std types — Ease 4, Effect 3
Several small, independent wins following the existing `Vec`/array patterns:
populate `BTreeMap` generation honoring `iterate_depth` (`tree.rs:821` is always
empty); add `HashMap`/`HashSet`/`VecDeque`; extend arrays past 32 (`serde.rs`
stops at `impl_node_serde_array!(32usize)`); let `#[autarkie_literal(...)]` accept
string/byte literals (it is `as`-cast / numeric-only at
`autarkie_derive/src/lib.rs:500`); wire the registered-but-unhandled
`#[autarkie_length(..)]`. Grammars using these types are under-explored today.
`B7`.

---

## Tier B — High-leverage (the capability jumps)

### 2. CMPLOG/I2S → first-class grammar solver — Ease 3, Effect 5
Comparisons are the major barrier for structured fuzzers, and Autarkie has a
unique edge: it can map an operand back to a *typed field*. **Prereq:** fix the
byte path (FEHLER A1/A2/B5 — "find `left` → write `right`", absolute offsets,
empty-needle guard). Then route AFL CMPLOG bytes and the `-x` dict to the
*field* level so structure stays valid: pairwise operand replacement, string
fields, numeric width/endian/sign-aware matching, per-field hit counters. There's
already a `// TODO: I2S for AFL` at `fuzzer.rs:480`. Biggest win for
magic-value-gated and text-format targets (the SQL guide). `B2`, `E10`.

### 4. Feedback-driven generation (bandit weighting) — Ease 2, Effect 5
Variant selection in `Visitor::generate` is **uniform** today
(`visitor.rs:283-288`); the rich `MutationMetadata` accounting is written to
`stats.json` but never fed back. Add a per-`(type, variant)` weight updated from
which variants appear in newly-covering inputs (multi-armed-bandit / frequency
model) and bias generation + `GenerateReplace`/`RecursiveReplace` toward
productive/under-explored variants. The `ty_generate_map` infra already exists;
the cost is plumbing feedback back into the `Visitor`. Single biggest
effectiveness lever per the audit. `B1`.

### 5. Adaptive mutator scheduling — Ease 3, Effect 4
Track per-mutator metrics (attempts, skips, evaluated, finds, exec cost, recent
yield) and adjust mutator weights by recent usefulness. Grammar-aware mutators
vary wildly in value by target; this stops wasting cycles on low-yield classes
(and pairs naturally with fixing the skip-heavy mutators in TODO). `E5`.

### 6. Structural / grammar coverage feedback — Ease 3, Effect 4
Pure edge coverage under-explores rare variants that don't immediately move
edges. Track a `(type, variant)` and `(parent → child)` "seen" map and treat
newly-exercised productions as interesting (a cheap secondary `MapFeedback`-style
signal). Drives the fuzzer to exercise the whole grammar — the entire premise of
"the type *is* the grammar." `B3`.

---

## Tier C — Solid, moderate-effort improvements

### 7. SCC-based recursion detection — Ease 3, Effect 3
Replace the heuristic cycle-endpoint comparison (`visitor.rs:195-221`, which also
mis-handles equal variant counts — BUGS #3) with strongly-connected-component
analysis: mark variant edges that stay inside a recursive SCC, identify
terminating base variants, and report grammars with no terminating variant.
`petgraph` is already a dependency. Prevents stack overflows and makes
minimization predictable. `E2`.

### 8. Correct + structure-preserving minimization — Ease 3, Effect 3
Implement `RecursiveReplace` for `Vec`/`Cow<[T]>`/`BTreeMap` (currently `// TODO`
no-ops, FEHLER C5); switch the three minimizers from `evaluate_input` to raw
`run_target` + restore (no corpus pollution); fix the off-by-one so the last
collection element can drop. Then extend: shrink numerics toward boundaries,
strings via dictionary, enum variants to simpler ones, map/struct fields
independently. Smaller valid entries make splicing more effective and triage
easier. `B6`, `E11`.

### 9. Performance: RNG, serialization, chunk cap — Ease 3, Effect 3
`generate_bytes` burns a full `u64` draw per byte (`visitor.rs:58-63`);
`impl_hash!`/`generate_name` re-serialize the whole input repeatedly
(`afl.rs:64-67`); the chunk store writes one file per sub-node per interesting
input with **no cap** (inode/disk blow-up on big grammars or long runs). A reused
serialization buffer + an in-memory LRU chunk store with a size/count cap lift
throughput and prevent disk exhaustion. `B8`.

### 10. Stable type identity + chunk-store hardening — Ease 3, Effect 3
Replace `XxHash64(TypeId)` (`tree.rs:44-49`) with a hash of the **structural
fingerprint** (type name + field/variant shape the derive already knows), and
store metadata beside each chunk (codec, autarkie/grammar version, type id+name,
chunk kind, length+content hash); validate/discard stale chunks on load. Makes
`chunks/` and corpora reusable across rebuilds and shareable across machines —
turning long campaigns and corpus distribution into first-class features. `B4`,
`E7`.

### 11. Configurable corpus seeding — Ease 3, Effect 3
Knobs for the startup-generation phase (`fuzzer.rs:399-415`): attempts vs.
successful inputs, recursive-depth and iterable-length distributions, string-pool
/ dictionary / target-literal seeding, plus visibility into generation failures.
Better seeding markedly improves early coverage. `E8`.

---

## Tier D — Foundational / supporting (low direct effect, high enablement)

### 12. CI across all supported surfaces — Ease 4, Effect 2
Only `sql-fuzzer-build.yml` exists. Add jobs for `cargo check -p autarkie
--features derive,afl`, `cargo test -p autarkie_test`, `cargo bench --no-run`, and
small downstream AFL + cargo-fuzz/libFuzzer shim builds. The derive, runtime, and
stages must be checked *together* — version drift currently breaks the build
before compilation. `E1`.

### 13. Test suites: runtime + derive + property — Ease 4, Effect 2
Per-`Node` runtime tests (generation terminates, serialize round-trips, field
paths resolve, mutations are safe-or-`Skipped`), `trybuild` derive compile tests
across struct/enum/generic/recursive/union/attribute shapes, and property tests
for invariants (round-trip identity, valid field paths, bounded recursion, chunks
deserialize as their indexed type). Would have caught the drifted fixtures and
several TODO bugs mechanically. `E3`, `E4`, `E16`, `B9`.

### 14. Honor/remove dead config + lazy rendering — Ease 4, Effect 2
Make `-g`/`-e`/`-r` do what they claim or delete them (FEHLER C1/C2/C3). In
particular gate rendering on `self.render` (`context.rs:74-85` renders + writes
two files unconditionally today) and add explicit modes: render corpus / crashes
/ only-new / only-on-shutdown. Rendering is expensive for ASTs like SQL. `B9`,
`E12`.

### 15. Observability: stats.json + grammar report — Ease 4, Effect 2
`AutarkieStats` is just `BTreeMap<MutationMetadata, usize>` today. Expand it
(corpus size by core, chunk counts by type, per-mutator attempts/successes/skip
reasons, generation and CMPLOG success rates, render byte counts). Add a
human-readable grammar report from the existing type maps (variants, recursive
vs. non-recursive, iterable fields, weakly-supported types, fields never
reachable from generation). Lets users see whether their type became the grammar
they intended. `E6`, `E9`.

### 16. Documentation around target workflows — Ease 5, Effect 1
Document exact nightly/`llvm-tools`/AFL++/cargo-fuzz requirements, the
version-sync warning for local path deps, output-directory layout, crash-repro
paths (internal vs. rendered corpus), `TypeId`-chunk-reuse limits, and current
field-attribute status. Reduces failed first attempts. `E15`.

---

## Tier E — Lower priority

### 17. Hermetic libFuzzer runtime builds — Ease 3, Effect 2
Generate the runtime crate entirely under `OUT_DIR` for *both* path and version
modes (the local-path branch rewrites a checked-in `Cargo.toml` today — FEHLER
D2), include a grammar-source fingerprint, support concurrent cargo-fuzz targets,
and improve the `AUTARKIE_GRAMMAR_SRC` / missing-features error messages (replace
the `unreachable!` panics). No direct fuzzing power, but unblocks parallel
targets and stops dirtying the tree. `E13`.

### 18. Stable public API boundary — Ease 3, Effect 1
Separate user-facing API (`fuzz_afl!`, `fuzz_libfuzzer!`, `debug_grammar!`, derive
attributes) from internal `__autarkie_*` methods; document supported feature
combinations and a serialized-corpus compatibility policy. Maintainability;
reduces fixture/example drift. `E14`.

---

## Suggested sequencing

1. **#1 robustness pass** + the relevant TODO P1 fixes — cheap, and everything
   long-running depends on them.
2. **#2 CMPLOG fix→solver** and **#3 std-type coverage** — restore/extend features
   that are the most direct effectiveness gains for real targets.
3. **#4 feedback-driven generation** + **#6 structural coverage** + **#5 adaptive
   scheduling** — the levers that take Autarkie from "neat idea" to "competitive
   grammar fuzzer."
4. Backfill **#12–#13 CI + tests** early in parallel so the above land safely,
   then work down Tiers C–E as capacity allows.
</content>
