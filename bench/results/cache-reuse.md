# Reusing the caches instead of rebuilding them

Measured 2026-09-02 on an Apple M4 Pro (12 logical, 8P+4E), native release,
corpus `corpus-v1`. Follows "The reference was being timed one process per
deal" in `release-profile.md`, which is where this was written down as the most
concrete lead still open.

## What was wrong

`solve_table_inner` built `CutoffCache::new(16)` and `PatternCache::new(16)`
fresh inside the per-strain loop: five cache pairs a deal, a thousand over the
200-deal lock-step corpus, five megabytes of table allocated, initialised and
thrown away every strain.

The C++ reference does not. Its `common_bounds_cache` and `cutoff_cache` are
process globals, and `Solve` only calls `Reset()` on them per trump — the
entries go, the capacity stays. It allocates once for the life of the process.

## What changed

Both caches gained a `reset` that empties them in place, and the pair moved out
of the strain loop into a `par::TableSolver` that owns them. The free functions
(`solve_dd_table` and the two node-counting variants) share one `TableSolver`
per thread, so the reuse spans deals as well as strains without any caller
having to thread a context through. `TableSolver` is public for a caller that
would rather hold one explicitly.

`drain_pool` now drops that shared solver before draining. It could not do what
it promises otherwise: the pattern-tree blocks a retained cache holds are
*live*, so a drain around them frees everything except the peak that matters.

## The lock-step invariants, unmoved

Both hold exactly, which is the whole constraint on this change:

| check | result |
|---|---|
| `solver-bench nodes fixtures/divergence/lockstep-200.pbn` | **296,689,028** nodes, unchanged |
| `fixtures/divergence/first-divergence.sh` | **`none` on all twelve** fixtures |
| `dev-build.sh --ci test` | 30 passed, 0 failed |

It is not luck, and it did not need to be measured to be believed — though it
was. **Neither cache can lose an entry.** A full table doubles rather than
replacing anything, and probing stops at the first empty slot, which insertion
never skips over. So a lookup hits exactly when the key was stored, whatever
size the table happens to be; the size decides only how often it doubles. That
is why cache capacity is invisible to the search, and why this port has always
been able to hold exact lock-step while starting from different cache sizes
than the reference does.

## Keeping the capacity unconditionally is a bad trade here

The obvious version of this change — reset and keep whatever size the table has
grown to, exactly as the reference does — **wins on the bench corpus and gives
the win back on the 200-deal run.** That is worth recording, because it is the
version the reference's own behaviour argues for, and because the corpus alone
would have waved it through.

Instrumenting the caches over the 200 deals says why. Per strain, the size a
cache built fresh for that strain alone would have reached:

| | base 2^16 | 2^17 | 2^18 |
|---|---|---|---|
| cutoff cache | 903 strains (99.2%) | 6 | 1 |
| pattern cache | 909 strains (99.9%) | 1 | — |

**Over 99% of strains never grow past the base size.** But the handful that do
leave the table there, and a retained cache is sticky at its worst case: 873 of
the 910 sampled strain resets ran against a cutoff cache at 2^18 and a pattern
cache at 2^17. Every one of those strains paid to clear twelve megabytes of
table on its way to using five. Over a thousand strains that is around seven
gigabytes of memory traffic bought for nothing.

So `reset` sizes the table to the load the last search actually reached, never
below the size the cache was built at. The common case is an in-place clear
with no allocator involvement at all; a genuine run of hard deals keeps its
grown table; a change of difficulty costs one allocation. There is no threshold
to tune — the size is read off the same three-quarters-full rule that `store`
and `get_or_create` already use to decide when to grow.

## The numbers

Three arms, built from the same tree and interleaved position-balanced (`A B C
C B A` per block, the ABBA discipline generalised to three), so each arm sees
each position within a block equally often:

- **A** — baseline, a fresh cache pair per strain
- **B** — hoisted, capacity kept unconditionally as the reference keeps it
- **C** — hoisted, capacity kept as far as the last search justified it

### Bench corpus, eight rounds an arm

`run --no-sweep --runs 5`, so each round is a best-of-five per board; the figure
below is the minimum of those eight per-board bests, and the headline is the
geometric mean over the ten boards.

| board | A ms | B ms | C ms | B/A | C/A |
|---|---|---|---|---|---|
| 1 | 25.78 | 24.28 | 24.32 | 0.9420 | 0.9435 |
| 2 | 63.85 | 62.19 | 62.28 | 0.9741 | 0.9755 |
| 3 | 167.86 | 165.30 | 165.71 | 0.9847 | 0.9872 |
| 4 | 78.41 | 76.56 | 76.74 | 0.9764 | 0.9787 |
| 5 | 123.87 | 121.55 | 121.50 | 0.9813 | 0.9808 |
| 6 | 106.73 | 104.38 | 104.95 | 0.9780 | 0.9833 |
| 7 | 73.75 | 71.81 | 72.01 | 0.9738 | 0.9764 |
| 8 | 13.94 | 12.56 | 12.55 | 0.9010 | 0.9001 |
| 9 | 30.85 | 29.35 | 29.44 | 0.9514 | 0.9544 |
| 10 | 225.24 | 222.32 | 222.45 | 0.9870 | 0.9876 |
| **geomean wall** | | | | **0.9646 (-3.5%)** | **0.9664 (-3.4%)** |
| **geomean cpu** | | | | 0.9647 (-3.5%) | 0.9667 (-3.3%) |

**Ten boards out of ten improved, on both arms.** The gain is a roughly fixed
cost per solve rather than a fraction of one, which is exactly the shape a
removed allocation should have: board 8 at 14 ms gains 10.0% and board 10 at
225 ms gains 1.2%, about 1.4 ms and 2.8 ms respectively.

B and C are 0.2% apart here, which is inside the noise on a single board and
well inside it after a geomean. The corpus cannot separate them, because its
ten boards all fit the base cache size and so never exercise the difference.

### The 200-deal lock-step corpus

Twelve rounds an arm, same interleave. This measurement is the honest one to
read *cautiously*: a whole 23-second run has no best-of inside it, and each
arm's own spread is 4-7%, so it resolves the sign of a one-percent effect and
not its size.

| arm | n | min s | median s | max s | spread |
|---|---|---|---|---|---|
| A | 12 | 23.26 | 23.54 | 24.16 | 3.9% |
| B | 12 | 23.16 | 23.80 | 24.73 | 6.8% |
| C | 12 | **22.81** | **23.43** | 23.97 | 5.1% |

| | vs A, on the minimum | vs A, on the median |
|---|---|---|
| B | -0.4% | **+1.1%** |
| C | **-1.9%** | -0.5% |

**C is the only arm ahead of A on both statistics, and B is behind A on the
median with the widest spread of the three.** That ordering is what the
instrumentation predicts: B clears roughly seven gigabytes more table than C
does over the thousand strains, which at the rate this machine sustains a
strided clear is a few tenths of a second — the size of the gap, and near
enough to the noise floor that the mechanism is better evidence than the clock.

The corpus and the 200 deals disagree about B for a reason worth keeping in
mind when reading either: the corpus solves ten fixed boards over and over, and
none of them grows a cache, so B never pays its tax there. It takes a run of
*varied* deals to expose it. A benchmark that repeats one workload cannot see a
cost that only a change of workload creates.

## What this does not do

It does not touch `Solver::solve`, `analyse_play`, or `solver-diag`, all of
which still build a cache pair per call. The first two are single-solve entry
points where there is nothing to reuse across; `solver-diag` is deliberately
left alone because `first-divergence.sh` compares it against the reference and
its per-strain lifecycle is part of what that check pins down.

It also leaves a retained cache resident on a thread that has stopped solving.
That was already true of the pattern-tree pool and has the same escape hatch:
`drain_pool`, which the wasm build already calls from `clear_cache` and
`release_memory`, and which now releases the caches too.
