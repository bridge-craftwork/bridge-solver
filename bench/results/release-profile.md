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
`g++ -O3` from a single translation unit. The search is one hot call graph
spread over `search.rs`, `bridge_solver.rs`, `cards.rs` and `pattern.rs`, so the
split falls exactly across the hot path.

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

Every one of the ten boards improved, in a tight band from -7.1% to -8.9%.
That consistency is what makes this attributable: a codegen change should move
all boards by roughly the same fraction, and it does.

Against the last recorded result (`cache-fix.json`) the figure is larger:

```
overall wall: 0.876x (-12.4%)
overall cpu : 0.875x (-12.5%)
```

**Use -7.7% as the attribution.** The -12.4% is measured across two recording
sessions on a machine whose load was not the same in both, and it carries that
drift as well as the change.

## Per node, against the C++ reference

The change is pure codegen: it cannot alter the search tree, and `verify`
confirms all ten tables are unchanged. Node counts are therefore identical to
those in `cache-fix.md`, and the per-node cost scales with the CPU figure:

| | ours / C++ |
|---|---|
| nodes searched | 0.994x (unchanged) |
| ns per node | 1.120x → **~1.03x** |

That row is **derived, not re-measured** — 1.120 x 0.923 — and should be
confirmed by a fresh per-cell run against the reference before it is quoted as
a measurement.

One thing to weigh against whatever residue remains: the reference's makefile
builds it with **profile-guided optimisation** (`-fprofile-generate`, then
`-fprofile-use`). PGO is typically worth several percent on branchy search
code, so a few points of any remaining gap are a build technique we have not
adopted rather than a language or porting cost.

## Against DDS 2.9

The `dds-reference` comparison could not run at all before this change — not
for performance reasons, but because it crashed. DDS's `SetResources` tears
down its per-thread memory with `memory.Resize(0, ...)` before rebuilding it,
and on this build the rebuild does not always happen; the next solve then hit
`Memory::GetPtr: 0 vs. 0` and DDS called `exit(1)`, taking the harness with it.
`dds.rs` now skips redundant calls, which is enough for a single fixed thread
count. Changing `--dds-threads` mid-run still trips it, so **the DDS thread
sweep remains unusable**.

Single-threaded, best of 5, all ten tables agreeing:

```
board     ours 1t    dds 1t  ours/dds1
    1        28.3      25.9      1.09x
    2        63.6      79.5      0.80x
    3       191.8     110.6      1.74x
    4        86.4      90.9      0.95x
    5       126.5      88.3      1.43x
    6       109.8     102.9      1.07x
    7        78.9      87.2      0.90x
    8        14.3      17.4      0.82x
    9        21.2      29.0      0.73x
   10       260.5     172.8      1.51x

Per core we are 1.06x DDS's cost, geometric mean over 10 boards.
```

DDS's threading backend here is GCD, which `IsIMPL()` does not cover, so
`thrMax = min(1, ncores) = 1` and the column is genuinely single-threaded.

The **spread is the interesting part**, and it is not noise: 0.73x to 1.74x
across ten boards. We are faster than DDS on four of them and half as fast on
two. That is not uniform per-node overhead — it is algorithmic, and it is where
the remaining work is. Worth checking whether node counts track DDS *per
board* rather than only in aggregate.

## Three things that looked wrong and were not

Recorded because each is a plausible next guess, and each cost a measurement to
rule out.

**The per-node atomic reads.** `evaluate_playable_cards` carried 32 loads of
`XRAY_LIMIT`/`XRAY_COUNT` and `search_with_cache` another 17, and an atomic
load can be neither hoisted out of a loop nor CSEd. Hoisting them into `Search`
fields read once at construction: **no win**, slower within noise.

**The dead tracing.** Between them the two hottest functions held 61 calls into
`format_inner`, `_eprint` and `__rust_dealloc` for `eprintln!` blocks that never
fire — around 4,000 instructions of never-executed code interleaved with the
hot path. Feature-gating it away removed all of it (61 → 3 calls, binary 16 KB
smaller) and was **performance-neutral**: best-of-15 interleaved A/B, 83.4 /
83.8 / 83.8 ms against 86.2 / 83.2 / 82.6 ms. Cold never-taken branches are
free on this core, and the I-cache never fetches the lines they sit on.

**`-C target-cpu=native`.** Neutral. rustc already defaults to `apple-m1` on
`aarch64-apple-darwin`, and popcount and bit-scan are ARM64 baseline. This lever
only matters on x86-64, where the default target lacks `popcnt` and BMI.

Already clean, and worth not re-checking: there is **no per-node allocation**
(`OrderedCards` is a fixed `[u8; 13]`, both caches are flat `Box<[T]>`,
`PartialTrick`'s `Vec` is construction-only) and **no `HashMap`** — `CutoffCache`
is a hand-rolled open-addressed table with linear probing.

## Still open

- **39 `panic_bounds_check` sites** remain in the two hot functions. One
  concrete instance: `CutoffCache::lookup` and `store` index
  `self.entries[(base_index + d) & self.mask]`, and LLVM cannot prove
  `mask + 1 == entries.len()` because `mask` is a separate field. Taking the
  mask from `self.entries.len() - 1` at the point of use would elide those
  checks without any unsafe code. Unmeasured.
- **`panic = "abort"`** was not tried. It would remove the unwind landing pads
  behind those checks, but it changes `cargo test` semantics and needs its own
  profile.
- **The per-board spread against DDS**, above.

## A note on the conditions

The machine was at load average 5.7 throughout (`XprotectService`,
`WindowServer`, `BetterDisplay`, a Parallels VM). Same-binary repeats swung up
to 4-6% on the geometric mean, and one such swing initially read as a 5.6%
regression from a change that turned out to be neutral.

**Below about 6%, the default `run` is not sufficient to attribute anything.**
Use `--quick --runs 15` and alternate the two binaries A/B/A/B; that held to
roughly 1% even under this load.

The thread sweep in `release-profile.json` should be **read as invalid**, not as
a regression. It shows 0.84x at 12 threads against `cache-fix.json`, but the
spreads on those rows are 33%, 42% and 28% — the harness was contending with the
rest of the machine, which is precisely the condition `bench/README.md` says
makes sweep rows meaningless. Nothing in this change touches shared state. Re-run
the sweep on an idle machine before drawing any conclusion from it.
