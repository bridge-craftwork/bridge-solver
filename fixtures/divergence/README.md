# Divergence fixtures

Seven positions where our search does not follow the C++ reference
(`macroxue/bridge-solver`) node for node, each one reduced to a single search
from a cold cache.

They exist because the equivalence recorded in `bench/results/cache-fix.md` —
"we walk the same path, node for node" — was established over 194 cells
truncated at a thousand nodes each. It does not hold on full solves. We search
1.038x the reference's nodes for 1.162x its wall clock, so roughly a quarter of
the remaining gap is extra work rather than slower work.

## Running them

```bash
UPSTREAM=~/src/bridge-solver XRAY=~/src/bridge-solver-xray/xray/solver-xray \
  fixtures/divergence/first-divergence.sh
```

Each row reports where the two traces first differ, and whether that matches
what `manifest.tsv` records. `FIXED (no divergence)` means a change closed it;
`MOVED` means it changed shape and the trace is worth re-reading.

Build the reference's instrumented solver with:

```bash
clang++ -std=c++17 -O3 -o solver-xray xray/bridge-solver.cc
```

## Why the deals are named and not committed

`1k_deals` belongs to `macroxue/bridge-solver`, which is GPL-2.0; this repo is
MIT OR Apache-2.0. Randomly generated deals are almost certainly uncopyrightable
data rather than authorship, so vendoring them is very likely fine — but that is
a call to make deliberately, not one to inherit from a fixture directory. The
manifest names them instead and the script reads them from a checkout.

Regenerating equivalent positions from `solver-bench`'s own seeded generator
would make the set self-contained and moot the question.

## What each position isolates

Every row is one search: one deal, one trump, one opening leader, cold caches,
and the MTD(f) guess straight from `GuessTricks`. Nothing precedes it, so there
is no accumulated cache state or cell ordering to reproduce — the divergence is
a property of that position alone.

**`FAST_TRICKS` (deal.72, .12, .10, .4)** — our fast-trick estimate is lower
than the reference's. On `deal.72` it differs on 7.1% of calls and is never
high: twenty cases of -1 and four of -2. `min(fast_tricks, remaining)` masks it
most of the time, which is why the traces survive another hundred records before
losing alignment.

**`MOVE_ORDER_AFTER` (deal.1, .134)** — identical playable set, identical
cutoff cards, different order out. On `deal.1` at depth 28 the reference orders
`[CT C2 H2 D9 C9 C6]` and we put `D9` first, with everything else keeping its
relative position, so we promote `D9` into a higher-priority bucket than
`order_leads` should.

**`PATTERN_HIT` (deal.38)** — at depth 16 the reference reports
`bounds=[0,7] adj_upper=10 UPPER_CUT` and prunes; we miss and go on to compute
fast tricks and store. Every preceding line is identical, so both caches hold
the same entries and our lookup is the narrower one.

All three fail safely. A low fast-trick estimate, a worse move order and a
missed pattern hit can only fail to prune — none can return a wrong answer,
which is why the correctness suite stays green while node counts do not.

## Determinism

Both solvers are deterministic here, and this was checked rather than assumed:
three runs of each of the seven positions, on both sides, gave byte-identical
traces, and the first-divergence line was identical every time.

The mechanisms themselves are also independent of processing order.
`fast_tricks_from_seat` and `order_leads` are pure functions of the position —
hands, trump, seat to play — and read no cache, so a position diverges the same
way however the search reached it and whatever order the twenty cells of a table
are solved in.

What *is* order-dependent is the cost. Caches are shared across a strain and the
MTD(f) seed chains from cell to cell, so the same divergence is worth wildly
different numbers of nodes depending on where it lands. Solving declarers in the
reference's order (S, N, W, E — partners adjacent) rather than N, E, S, W took
`deal.10`'s whole-table count from 1.36x to 1.02x without touching any
estimator.
