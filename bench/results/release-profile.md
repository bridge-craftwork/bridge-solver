# The release profile: LTO and one codegen unit

Measured 2026-09-01 on an Apple M4 Pro (12 logical, 8P+4E), corpus `corpus-v1`,
best of 5. Supersedes `cache-fix.json` for per-board costs.

`cache-fix.md` closed the algorithmic gap and left one number outstanding: the
search tree was the same size as the C++ reference's, but each node cost
**1.120x** as much. That residue was not a Rust tax. It was Cargo's defaults.

## What was wrong

`Cargo.toml` had no `[profile.release]` section, so the release build took
Cargo's defaults: **`codegen-units = 16` and no LTO**. The crate was split into
sixteen units with no inlining across the splits, while the reference is built
`-O3` from a single translation unit. The search is one hot call graph spread
over `search.rs`, `bridge_solver.rs`, `cards.rs` and `pattern.rs`, so the split
falls exactly across the hot path.

```toml
[profile.release]
lto = "fat"
codegen-units = 1
```

## What it bought

Controlled A/B, both binaries built and run in the same session minutes apart,
best of 3 over the full corpus:

```
overall wall: 0.923x (-7.7%)
overall cpu : 0.923x (-7.7%)
```

Every one of the ten boards improved, in a tight band from -7.1% to -8.9%. That
consistency is what makes it attributable: a codegen change should move all
boards by roughly the same fraction, and it does.

Against the last recorded result (`cache-fix.json`), on a quiet machine:

```
overall wall: 0.914x (-8.6%)
overall cpu : 0.915x (-8.5%)
```

## Per node, against the C++ reference

The change is pure codegen: it cannot alter the search tree, and `verify`
confirms all ten tables are unchanged. Node counts are therefore identical to
those in `cache-fix.md`, and the per-node cost scales with the CPU figure:

| | ours / C++ |
|---|---|
| nodes searched | 0.994x (unchanged) |
| ns per node | 1.120x → **~1.03x** |

That row is **derived, not re-measured** — 1.120 x 0.923 — and should be
confirmed by a fresh per-cell run before it is quoted as a measurement.

A caveat on the reference build itself: its makefile specifies profile-guided
optimisation (`-fprofile-generate`, then `-fprofile-use`), but that flow is
GCC-specific — it moves a `.gcda` file, which clang does not produce — and on
macOS `g++` is Apple clang. No built binary or profile data survives in the
reference tree, so **how the binary behind the 1.120x figure was actually built
is unknown**. If it was plain `-O3`, PGO is an advantage neither side has taken.

## Scaling, one to twelve threads

Board 4, equal work per thread, best of 5:

| threads | speedup | cores busy | vs `cache-fix` |
|---|---|---|---|
| 1 | 1.00x | 1.00 | 1.00x |
| 2 | 1.93x | 2.00 | 1.00x |
| 3 | 2.82x | 3.00 | 1.02x |
| 4 | 3.70x | 3.99 | 1.03x |
| 5 | 4.61x | 4.99 | 1.03x |
| 6 | 5.50x | 5.99 | 1.04x |
| 7 | 6.35x | 6.96 | 1.03x |
| 8 | 7.25x | 7.95 | 1.04x |
| 9 | 7.44x | 8.83 | 1.02x |
| 10 | 7.69x | 9.66 | — |
| 11 | 7.74x | 10.20 | — |
| 12 | **7.97x** | 10.61 | — |

Monotonic throughout, near-linear to 8, then a slow climb as the four
efficiency cores join. Ordinary saturation, the shape `bench/README.md`
describes, and slightly *better* than `cache-fix` at every thread count.

An earlier recording of this same sweep showed a turnover to 0.84x at twelve
threads. That was contention, not the code: a Parallels VM and other load were
running, and the affected rows carried 33-42% spreads. With the machine quiet
the curve is clean. Recorded here as a worked example of the failure mode the
README warns about.

## Against DDS 2.9, single-threaded

Best of 5, all ten tables agreeing:

```
board     ours 1t    dds 1t  ours/dds1
    1        29.1      27.0      1.08x
    2        67.0      83.3      0.80x
    3       203.4     115.4      1.76x
    4        90.0      94.6      0.95x
    5       130.5      92.9      1.40x
    6       114.7     107.6      1.07x
    7        79.3      88.4      0.90x
    8        15.0      18.3      0.82x
    9        22.0      30.1      0.73x
   10       262.8     176.1      1.49x

Per core we are 1.06x DDS's cost, geometric mean over 10 boards.
```

Two figures, because they answer different questions. The **geometric mean is
1.06x** — the typical board. The **corpus total is 1.22x** (1013.8 ms against
833.7 ms), because the sum is dominated by boards 3 and 10, the two where we are
worst. Someone solving a large batch feels the second number.

The **spread is the interesting part**, and it is not noise: 0.73x to 1.76x. We
are faster than DDS on four boards and around half its speed on two. That is not
uniform per-node overhead — it is algorithmic, and it is where the remaining
work is. Worth checking whether node counts track DDS *per board* rather than
only in aggregate.

DDS's threading backend here is GCD, which `IsIMPL()` does not cover, so
`thrMax = min(1, ncores) = 1` and the column is genuinely single-threaded.

## Against DDS across threads — not yet a fair comparison

DDS's own scaling, corpus total, best of 5, one process per thread count:

| DDS threads | corpus ms | speedup |
|---|---|---|
| 1 | 816.0 | 1.00x |
| 2 | 474.0 | 1.72x |
| 3 | 254.1 | 3.21x |
| 4 | 221.0 | 3.69x |
| 6 | 298.6 | 2.73x |
| 8 | 276.6 | 2.95x |
| 10 | 193.9 | 4.21x |
| 12 | 297.6 | 2.74x |

The curve is erratic and non-monotonic. That is DDS, not the machine: our own
single-threaded anchor, measured in the same eight runs, held between 991 and
1005 ms — 1.4% — so conditions were stable throughout.

**Do not read this as "we scale to 8x and DDS only manages 4x".** The two
numbers measure different work. `dds.rs` calls `CalcAllTablesPBN` with
`no_of_tables = 1`, so DDS is parallelising *within* one twenty-entry table
whose entries have very unequal cost; wall time is set by the slowest chunk and
by scheduling luck, which is exactly why the curve jumps around. Our sweep gives
our solver N *independent* deals, which is an embarrassingly parallel workload
by comparison.

DDS has a batch API — the same call takes up to 40 tables at once — and given
ten boards it would parallelise across them and almost certainly scale far
better than this. **A fair deal-throughput comparison needs the harness to pass
the whole corpus in one `CalcAllTablesPBN` call.** Until it does, the honest
claim is limited to the single-threaded column above.

### A note on measuring DDS at more than one thread count

DDS cannot survive a second `SetResources`: it tears its per-thread memory down
with `memory.Resize(0, ...)` before rebuilding, and the rebuild does not always
happen — the next solve then hits `Memory::GetPtr: 0 vs. 0` and DDS calls
`exit(1)`, taking the harness with it. `dds.rs` now skips redundant calls and
`main.rs` seeds the initial call with the first thread count to be measured, so
a single-valued `--dds-threads` needs exactly one. **Several thread counts means
one process each**; a multi-valued `--dds-threads` still dies.

## Three things that looked wrong and were not

Recorded because each is a plausible next guess, and each cost a measurement to
rule out.

**The per-node atomic reads.** `evaluate_playable_cards` carried 32 loads of
`XRAY_LIMIT`/`XRAY_COUNT` and `search_with_cache` another 17, and an atomic load
can be neither hoisted out of a loop nor CSEd. Hoisting them into `Search`
fields read once at construction: **no win**, slower within noise.

**The dead tracing.** Between them the two hottest functions held 61 calls into
`format_inner`, `_eprint` and `__rust_dealloc` for `eprintln!` blocks that never
fire — around 4,000 instructions of never-executed code interleaved with the hot
path. Feature-gating it away removed all of it (61 → 3 calls, binary 16 KB
smaller) and was **performance-neutral**: best-of-15 interleaved A/B, 83.4 /
83.8 / 83.8 ms against 86.2 / 83.2 / 82.6 ms. Cold never-taken branches are free
on this core, and the I-cache never fetches the lines they sit on.

**`-C target-cpu=native`.** Neutral. rustc already defaults to `apple-m1` on
`aarch64-apple-darwin`, and popcount and bit-scan are ARM64 baseline. This lever
only matters on x86-64, where the default target lacks `popcnt` and BMI.

Already clean, and worth not re-checking: there is **no per-node allocation**
(`OrderedCards` is a fixed `[u8; 13]`, both caches are flat `Box<[T]>`,
`PartialTrick`'s `Vec` is construction-only) and **no `HashMap`** —
`CutoffCache` is a hand-rolled open-addressed table with linear probing.

## Still open

- **A fair threaded comparison against DDS**, via its batch API. This is the
  one that matters for "how do we stack up on someone else's machine".
- **The per-board spread** against both references, above.
- **39 `panic_bounds_check` sites** in the two hot functions. One concrete
  instance: `CutoffCache::lookup` and `store` index
  `self.entries[(base_index + d) & self.mask]`, and LLVM cannot prove
  `mask + 1 == entries.len()` because `mask` is a separate field. Taking the
  mask from `self.entries.len() - 1` at the point of use would elide those
  checks without any unsafe code. Unmeasured.
- **`panic = "abort"`** was not tried. It would remove the unwind landing pads
  behind those checks, but it changes `cargo test` semantics and needs its own
  profile.
- **PGO** was considered and declined: real gains are plausible on a search this
  branchy, but the two-stage build is ongoing overhead this project does not
  want yet.

## A note on method

Below about 6%, the default `run` cannot attribute anything on a machine that is
not idle. Same-binary repeats swung 4-6% on the geometric mean under load, and
one such swing initially read as a 5.6% regression from a change that turned out
to be neutral.

What worked instead: build both binaries, keep both, and alternate them
A/B/A/B/A/B with `--quick --runs 15`. That held to roughly 1% even under load,
because the two builds see the same interference. `compare` cannot do this
today — it reads two JSON files recorded at different times, which is precisely
the design that cannot interleave.
