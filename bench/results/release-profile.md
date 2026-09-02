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

## Against the C++ reference, measured

The 1.120x per-node figure in `cache-fix.md` was the last soft number here, and
the arithmetic that turned it into "~1.03x after the release profile" was
wrong. Measured directly, on the upstream project's own 200 random deals
(`1k_deals/deal.1` to `deal.200`, converted to PBN and cross-checked cell by
cell against the C++ solver's own output), single-threaded, best of 3:

| solver | 200 deals | vs ours |
|---|---|---|
| ours | 23.96 s | — |
| C++ reference (macroxue) | 20.62 s | **1.16x** |
| DDS 2.9 | 20.03 s | **1.20x** |

The build, recorded so the number has provenance — the makefile's own PGO path
is GCC-specific (`.gcda`) and cannot run under Apple clang:

```
clang++ -std=c++17 -O3 -march=native -o solver solver.cc
```

The C++ solver is single-threaded and solves one deal per process, so it is
timed per invocation from its own internal clock. Startup was measured at
2.1 ms (`-o`, read-and-exit), 1.6% of a deal, and external wall minus startup
agreed with the self-reported sum to within 1% across two passes.

**PGO turned out not to matter.** Built the clang way — `-fprofile-generate`,
train on `hard_deals/deal.8` as the makefile does, `llvm-profdata merge`,
`-fprofile-use` — it produced identical tables and 20.96 s against 20.62 s, with
the order reversing on the second pass. Within noise. The earlier speculation
that PGO explained part of the gap is dead, and the plain `-O3` build above is
representative.

### Two conclusions

**We are 1.17x the C++ original, not 1.03x.** The old figure was ns-per-node
over 194 cells truncated at 1000 nodes each — a slice thin enough to be mostly
cache-cold startup, and it does not predict whole-table cost. This number is
whole tables on random deals, which is the workload that matters.

**The upstream 1.28x-faster-than-DDS claim does not reproduce on this machine.**
Here the C++ solver and DDS are within 2% of each other (1.02x). That claim was
measured on an AMD Ryzen 7 5800H, and DDS carries x86-specific paths, so the
most likely reading is that it is an x86 result that does not port to ARM — but
the comparison scripts were never committed (one commit added the README, two
PNGs and a 50,000-line log, and nothing in that repo invokes DDS), and no DDS
version or build flags were recorded, so it cannot be checked directly.

Their published log does reproduce *internally*: parsing all 5000 deals gives a
geometric mean of 1.24x and a median of 1.25x against a claimed 1.28x, and the
advantage grows with difficulty — 1.23x at 0.05-0.10 s rising to 1.53x above
half a second. A least-squares fit gives `dds = -32.5 ms + 1.60 x solver`, so a
fixed per-invocation overhead inflating DDS is ruled out: the intercept is
negative. Whatever produced their result, it was not measurement overhead.

### The gap decomposes: 3.8% extra search, 12% per node

`cache-fix.md` established that we walk the reference's tree node-for-node.
That was measured over 194 cells truncated at a thousand nodes each, and **it
does not hold on full solves**. Counting nodes on the same 200 deals -- both
counters increment at the top of `EvaluatePlayableCards`, so they count the
same event -- gives:

| | ours / C++ |
|---|---|
| nodes searched | **1.038x** |
| wall clock | **1.162x** |
| implied ns per node | **1.120x** |

Not one of the 200 deals matches node-for-node. The per-deal ratio runs from
0.699x to 1.384x with a median of 1.021x, and we search *fewer* nodes than the
reference on 70 of them. This is not a missing prune -- that would be one-sided
-- it is small divergences in move ordering and cache state that compound
differently on different deals.

They compound with size, which is the part that matters:

| C++ nodes | deals | ours / C++ |
|---|---|---|
| < 300k | 23 | 1.019x |
| 300k - 1M | 70 | 1.025x |
| 1M - 3M | 84 | 1.026x |
| 3M - 10M | 23 | **1.083x** |

That lines up with the two other places the hard deals stand out: our per-board
spread against DDS is worst on the corpus's two most expensive boards, and the
reference's own advantage over DDS widens on hard deals. Whatever diverges,
it diverges more the longer the search runs.

### One cause found and fixed: declarer order

The reference solves a strain's four cells in lead order W, E, N, S -- that is,
declarers **S, N, W, E**, partners adjacent. We solved them N, E, S, W,
alternating sides.

That is not cosmetic, because the caches are shared across a strain and the
MTD(f) seed chains from one cell to the next: `seed_from` hands the next search
`ns_tricks + 1`, so ordering partners adjacently means each seed comes from a
cell whose NS trick count is usually the same or within one. Alternating sides
means every second seed is derived from the opponents declaring, which is a
poor guess and costs MTD(f) iterations.

Matching the reference's order took 310.4M nodes to 307.9M -- **0.8% fewer, for
a two-line change**, with all ten corpus tables unchanged. It is the ordering
the reference chose on purpose.

### The rest is chaotic, not systematic

Per-cell counts over the first 50 deals (`solver-bench nodes --per-cell` against
the reference's `-S1` output, which prints one block per cell):

| strain | C++ nodes | ours | ratio |
|---|---|---|---|
| NT | 13,561,599 | 13,745,262 | 1.014x |
| spades | 16,127,483 | 18,942,747 | 1.175x |
| hearts | 11,613,030 | 12,023,012 | 1.035x |
| diamonds | 16,935,775 | 17,305,948 | 1.022x |
| clubs | 14,232,095 | 14,534,830 | 1.021x |

Spades looks like a systematic outlier and is not: `deal.8` alone supplies about
three quarters of that excess.

The individual cells say what is really going on. The worst are 1.6x to 2.5x
(`deal.1` NT East, 34,811 against 85,732) and the best are far more extreme in
our favour -- `deal.7` NT East is 56,343 nodes for the reference and **2,267**
for us, 0.04x. Deviations that large in both directions, cancelling to 1.056x in
aggregate, are the signature of chaotic sensitivity: a tie broken differently in
move ordering changes which cutoff fires, and the subtree either collapses or
does not. It is not a missing prune and it is not strain-specific.

### Traced: the fast-tricks estimate under-counts

`deal.72`, notrump, declarer South is the cleanest possible target -- the first
cell of its strain in both, so cold caches and an identical `GuessTricks` seed,
78,779 nodes against our 151,962, while the deal's four suit strains agree
within 1%.

Both solvers trace that one cell with a five-line deal file (the deal, then `N`,
then `W`), which `solver-diag -f` and the instrumented C++ both read as an
explicit trump and lead:

```
solver-xray -f deal72.ntw -m0 -X 40000     # xray/bridge-solver.cc
solver-diag -f deal72.ntw -X 40000
```

The traces agree line for line -- once `remaining=[...]`, which the two format
differently, is normalised out -- until **line 208**:

```
C++ :  FAST_TRICKS: depth=20 seat=1 raw=9 capped=8 trump=4
ours:  FAST_TRICKS: depth=20 seat=1 raw=8 capped=8 trump=4
```

**Our fast-tricks estimate is low.** Over the aligned prefix it differs on 7.1%
of calls and is *never* high: 20 cases of -1 and 4 of -2. The cap hides it every
time in that prefix, which is why the searches stay identical for another
hundred-odd records, and then the traces lose structural alignment.

That is the whole shape of the problem, and it explains every symptom:

- **It cannot produce a wrong answer.** A low fast-trick count is a conservative
  estimate; it only fails to prune. Correctness tests pass, node counts do not.
- **It is chaotic.** `min(fast_tricks, remaining)` masks the difference most of
  the time. When it does not, a beta cut that the reference takes is missed and
  a whole subtree is searched — or the reverse, once cache contents diverge,
  which is why we are sometimes far *below* the reference (`deal.7` NT East,
  2,267 nodes against 56,343).
- **It compounds with depth**, matching the 1.019x → 1.083x trend by deal size.

The estimator is `fast_tricks_from_seat` in `search.rs`, against `FastTricks`
in the reference. Reading them side by side, the structure matches — the
`SuitFastTricks` cases, the `pd_entry` handling and the argument swap on the
second call are all faithful. Two differences are visible and neither is the
NT case above, so the cause is still to be found by instrumenting the per-suit
`my_tricks`/`pd_tricks` at this position:

1. `for card in all_suit.iter()` walks `self.hands.all_cards()`, the *live*
   remaining cards, where the reference walks `trick->all_cards`, the snapshot
   taken at the start of the trick. These agree at a trick boundary, which
   depth 20 is, so this is not the divergence above — but they part company
   mid-trick, and that is worth fixing regardless.
2. `max_suit_winners` starts at `self.num_tricks` where the reference uses
   `TOTAL_TRICKS`. Equal for a full deal, since `num_tricks` is the largest hand
   at construction, but not for `new_mid_trick` or `analyse_play`, and a smaller
   value truncates the suit harder and under-counts. Latent, and in the same
   direction as the symptom.

### Three mechanisms, not one

Tracing the same cell (notrump, lead West) on sixteen deals, first 6,000 xray
iterations, and classifying where each trace first parts company:

| first divergence | deals | what differs |
|---|---|---|
| none in the traced prefix | 8 | — |
| `FAST_TRICKS` | 4 (`.4`, `.10`, `.12`, `.72`) | our raw estimate is lower |
| `MOVE_ORDER_AFTER` | 2 (`.1`, `.134`) | same playable set, different order |
| `PATTERN_HIT` | 1 (`.38`) | the reference hits, we miss |

**Move ordering** (`deal.1`, depth 28): identical `playable=[H2 D9 CT C9 C6 C2]`
and no cutoff card, and the reference orders `[CT C2 H2 D9 C9 C6]` while we put
`D9` first. Relative order is otherwise preserved, so we are promoting `D9` into
a higher-priority bucket than the reference does — a lead-classification
difference, in `order_leads`.

**Pattern cache** (`deal.38`, depth 16): the reference reports
`PATTERN_HIT ... bounds=[0,7] adj_upper=10 UPPER_CUT` and prunes. We miss, and
go on to compute fast tricks and store.

Every preceding trace line is identical, which first read as "both caches hold
the same entries, so our lookup is narrower". **That was wrong.** Digesting the
cache contents shows they had already parted company 445 iterations earlier, at
1190 against the miss at 1635. The miss is a consequence of cache drift the
trace could not see, not a lookup bug, and the thing to find is the write that
differs around iteration 1190.

### Digesting the caches, not just the trace

The trace records what the search *computed*; it says nothing about what the
caches *kept*. Those diverge at different moments, so `solver-diag -C <n>` (and
the same flag on the instrumented reference) emits a content digest of both
caches every `n` iterations. It XORs across live slots -- table size, hash
function and probe order all differ between the two and must not matter -- and
walks each pattern tree pre-order, because the tree's shape is what decides
whether `lookup` matches.

Neither indicator dominates:

| deal | trace differs at | caches differ at |
|---|---|---|
| `deal.134` | iter 9 | iter 20 |
| `deal.1` | iter 29 | iter 40 |
| `deal.72` | iter 28 | **iter 1170** |
| `deal.38` | iter 1635 | **iter 1190** |

`deal.72`'s trace fires at 28 on a fast-trick estimate both sides then cap to
the same value: a visible difference that changes nothing, and the caches
rightly agree for another eleven hundred iterations. `deal.38` is the reverse,
and is why the pattern-cache conclusion above had to be withdrawn.

### Root cause of the pattern-cache divergence: no probing

Bisecting `deal.38` with `-C 1180:1195` pins it to a single step. The digests
are identical at iteration 1188 and differ at 1189:

```
c++   iter=1189 cutoff=59/eeb7d6b187ad13b2 pattern=74/63846b1f49f46558
ours  iter=1189 cutoff=59/eeb7d6b187ad13b2 pattern=73/6a0d824c48bed25b
```

The cutoff caches still agree. The reference's pattern cache goes from 73 live
entries to **74** — it took a fresh slot. Ours stays at **73** with a changed
digest — we wrote over an entry that was already there.

`PatternCache` does no probing at all. It is direct-mapped:

```rust
pub fn lookup(&mut self, shape: u64, seat_to_play: Seat) -> Option<&mut ShapeEntry> {
    let idx = self.index(hash);
    let entry = &mut self.entries[idx];
    if entry.hash == hash { Some(entry) } else { None }   // one slot, then give up
}

pub fn get_or_create(&mut self, shape: u64, seat_to_play: Seat) -> &mut ShapeEntry {
    let idx = self.index(hash);
    if self.entries[idx].hash != hash {
        self.entries[idx].reset(hash);                     // evict the incumbent
    }
    ...
}
```

The reference linear-probes on both paths, never evicts, and resizes at 75%
load:

```cpp
for (int d = 0; ; ++d) {
  Entry& entry = entries[(index + d) & (size - 1)];
  if (entry.hash == hash) return &entry;
  if (entry.hash == 0) { ++load_count; entry.Reset(hash); return &entry; }
}
```

So every hash collision costs us an entry the reference keeps. That is the
`PATTERN_HIT` miss at iteration 1635: we had evicted it.

It also explains the shape of the whole problem. Collisions get more frequent
as the table fills, so the loss grows with the length of the search — which is
the 1.019x to 1.083x trend by deal size, and why the reference's advantage over
DDS widens on hard deals while ours narrows. `CutoffCache` already probes and
resizes; the pattern cache is the one that did not get it.

Fixing it means giving `PatternCache` the same linear probing and 75% resize.
Unmeasured, but it is the strongest candidate for the ~3.8% node excess.

All three are safe and all three cost nodes. A low fast-trick estimate, a missed
pattern hit and a worse move order can only fail to prune, never return a wrong
answer, which is exactly why every correctness test passes while node counts do
not.

### Are they hand-specific? Is the order relevant?

**The mechanisms are deterministic properties of the code, not of particular
hands** — but which deals hit them varies enormously. Half the sample never
diverged in 6,000 iterations, and on `deal.72` the fast-tricks estimate differs
on 7.1% of calls against 1.3% on `deal.10` and 0.1% on `deal.4`.

**They are order-independent.** `fast_tricks_from_seat` and the lead ordering
are pure functions of the position — hands, trump, seat to play — and read no
cache, so a given position always diverges the same way however the search
reached it and whatever order the twenty cells are solved in. The traces
reproduce exactly, run to run.

**Their cost is not.** How many nodes a divergence ends up costing depends
entirely on the order, because caches are shared across a strain and the MTD(f)
seed chains from cell to cell. `deal.10` went from 1.36x to 1.02x purely from
solving declarers in the reference's order, with no estimator touched. So the
bugs are order-independent and the damage is not.

The ~12% per-node cost remains the larger half of the gap, and the 39
`panic_bounds_check` sites below are the only concrete lead on that half.

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

## Against DDS across threads

Deal-level throughput: 240 distinct generated deals, both solvers given the same
list at the same thread count, best of 3. `solver-bench reference
--dds-threads N --throughput-deals 240`, one process per point.

| threads | ours ms | cores | dds ms | cores | ours/dds |
|---|---|---|---|---|---|
| 1 | 28885 | 1.00 | 25806 | 1.00 | 1.12x |
| 2 | 15051 | 1.99 | 13186 | 1.99 | 1.14x |
| 4 | 8094 | 3.93 | 6883 | 3.94 | 1.18x |
| 6 | 5637 | 5.75 | 4775 | 5.74 | 1.18x |
| 8 | 4499 | 7.53 | 4012 | 7.45 | 1.12x |
| 10 | 4099 | 9.23 | 3693 | 8.80 | 1.11x |
| 12 | 3825 | 10.34 | 3484 | 9.77 | **1.10x** |

**Threading is a wash.** The two solvers put almost exactly the same number of
cores to work at every point, and the ratio stays between 1.10x and 1.18x from
one thread to twelve. Whatever gap remains is per-node cost, not parallelism,
and it does not grow with core count — which is the thing worth knowing before
telling anyone to run this on their own machine.

Note the ratio on random deals (1.12x at one thread) is worse than on the
curated corpus (1.06x geometric mean). The corpus was selected to span a range
of difficulty, not to be representative; random deals are the better model of a
real file, and the honest headline number.

Getting to this table took three corrections, each of which would have produced
a confidently wrong published figure.

**One deal per call gave DDS five work items.** `dds.rs` called
`CalcAllTablesPBN` with `no_of_tables = 1`. `CalcAllTables` flattens the request
into one item per (deal, strain) pair before scheduling, so a single deal offers
five items no matter how many threads are configured — most idle, wall time set
by the slowest strain. The old measurement showed DDS jumping between 2.7x and
4.2x; that was load imbalance, not scaling. `solve_tables` now batches, chunked
at forty (DDS enforces `count * noOfTables <= 200`, and all five strains means
forty deals).

**Repeating the corpus handed DDS a free win.** The first throughput workload
was the ten-board corpus repeated eight times, and it measured DDS at **4.70x
our speed**. That number was entirely an artefact: `DetectCalcDuplicates` and
`CopyCalcSingle` de-duplicate identical deals inside a batch and copy the
result rather than solving again, so DDS solved ten deals per chunk while we
solved eighty. The workload is now distinct deals from a seeded generator, and
five of them are checked against DDS before timing — a generator emitting
well-formed but wrongly ordered deals would otherwise produce plausible
timings for work nobody did.

**DDS's macOS default pins it to the efficiency cores.** With the batch fixed,
DDS still flatlined at 3.97 cores and an identical wall time from four threads
up, while reporting twelve threads made. The cause is in `System.cpp`:

```c
dispatch_apply(numThreads,
  dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_BACKGROUND, 0), ...)
```

Background QoS on Apple Silicon is confined to the efficiency cores, of which
an M4 Pro has exactly four. DDS's GCD backend therefore cannot exceed four
cores on this machine however it is configured. That is a property of how DDS
is packaged on macOS, not of DDS's scaling, and building a comparison on it
would have been indefensible. `reference` now calls `SetThreading(5)` — DDS's
STL backend, plain `std::thread` at default QoS, scheduled across all cores
like ours — and `--dds-threading 3` restores GCD for anyone who wants to see
the difference.

Worth flagging separately, because it is not our problem to fix but it is real:
**a Mac user driving DDS through its default GCD path gets four cores.**

### Measuring DDS at more than one thread count

DDS cannot survive a second `SetResources`: it tears its per-thread memory down
with `memory.Resize(0, ...)` before rebuilding, and the rebuild does not always
happen — the next solve then hits `Memory::GetPtr: 0 vs. 0` and DDS calls
`exit(1)`, taking the harness with it. `reference` now seeds the initial call
with the thread count it will measure and rejects a multi-valued
`--dds-threads` with an explanation rather than dying. **A curve means one
process per point.**

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

- **The per-board spread** against DDS, above: 0.73x to 1.76x on the curated
  corpus. Not noise, and not uniform overhead. Worth checking whether node
  counts track DDS per board rather than only in aggregate.
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
