# The bounds-cache correctness fix (#14)

Measured 2026-09-01 on an Apple M4 Pro (12 logical, 8P+4E), best of 3, native
release, corpus `corpus-v1`.

Three divergences from the C++ reference (`macroxue/bridge-solver`, via the
local xray fork) were producing wrong double-dummy tables. All three are fixed,
and **the fix is faster than the code it replaces**.

## What was wrong

Found by diffing xray traces against the reference, which is exactly what that
instrumentation exists for. Each fix pushed the first divergence later: line
708 → 9951 → 20862 → 28463 → none.

**1. A missing fast-tricks upgrade** (`search.rs`). The reference upgrades a
zero fast-trick count from the side to play's own perspective before the fast
cuts:

```cpp
if (fast_tricks == 0 && trump != NOTRUMP)
  fast_tricks = SlowTrumpTricks(own, partner, LHO, RHO, true);
```

We omitted it. That is not merely a lost prune: the cut it enables returns a
different `rank_winners` set, and the bounds cache keys its entries on exactly
those cards.

**2. No promotion of a matched pattern into the root slot** (`pattern.rs`). The
reference's `ShapeEntry::Lookup` writes a matched child's hands and bounds back
into the entry's root. `Pattern::update` then compares candidates against that
root, so a root left stale grows a differently-shaped tree and stores
generalisations the reference never would.

**3. A run-length computed as a bit position** (`pattern.rs`,
`compute_pattern_hands`). Extending the bottom rank winner to its highest
equivalent card needs the *run* of consecutive cards held at and below it,
stopping at the first gap — `clz(~(x << (63 - i)))` in the reference. We used
the position of the highest card below it, which over-subtracts whenever that
run is broken, retaining cards the reference drops and so storing patterns that
match positions their bound does not hold for.

The third was the one that actually flipped answers; the first two were
prerequisites for the traces lining up far enough to find it.

## Correctness

| check | result |
|---|---|
| corpus tables (`solver-bench verify`) | **10 / 10** |
| individual cells vs the C++ reference | **200 / 200**, zero disagreements |
| corpus tables vs DDS 2.9 | **10 / 10** |
| #14's original repro (cold solve, South NT) | 9 → **8**, correct |
| board 6 hearts, shared caches | 5 5 → **7 7 6 6**, matches reference |

Both symptoms in #14 are gone: the cold-cache case it reported, and the
shared-cache case found later on board 6. They were the same three bugs seen
from opposite sides.

## Performance

Against the previous fast-but-wrong path (`node-counter-after.json`):

```
overall wall: 0.952x (-4.8%)
overall cpu : 0.953x (-4.7%)
```

**Correctness came out 4.8% faster.** Fix 1 restores a prune we were not
taking, and fixes 2 and 3 make cache entries match the reference's, so more
lookups hit. Nine of ten boards improved; board 6 is 1.3% slower, within noise.

This supersedes the earlier estimate that fixing #14 would cost ~2.1x. That
figure was for the *naive* fix — giving every cell its own caches, abandoning
reuse entirely. Against that naive version this fix is **2.3x faster**
(`0.439x` wall), because it keeps cross-cell cache sharing and makes it sound
rather than giving it up.

Scaling is unchanged: 7.90x at 12 threads, versus 8.07x before, within the
noise of the threaded rows.

## Against the reference, per cell

194 cells over 1000 nodes, cold caches, both solvers:

| | ours / C++ |
|---|---|
| nodes searched | **0.994x** |
| ns per node | **1.120x** |

The search tree is now marginally smaller than the reference's, and the whole
remaining single-core gap is 12% of per-node execution cost. There is no
algorithmic deficit left to close.
