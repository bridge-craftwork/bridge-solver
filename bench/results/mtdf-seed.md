# MTD(f) seed carry-forward

Measured 2026-09-01 on an Apple M4 Pro (12 logical, 8P+4E), corpus `corpus-v1`,
native release. Follows the bounds-cache fix (`cache-fix.md`).

## What changed

The C++ reference carries its MTD(f) seed between the cells of a table:

```cpp
guess_tricks = std::min(ns_tricks + 1, TOTAL_TRICKS);
```

Each cell starts its search from the previous cell's answer, rather than from a
fresh heuristic guess. We recomputed the guess every time. Now three call sites
carry it forward within a strain — `solve_dd_table`, the CLI's `solve_deal`,
and `solver-diag`'s multi-leader run — through a new
`Solver::solve_with_caches_seeded`, with `Solver::seed_from` building the seed.

**The seed cannot change an answer.** MTD(f) converges to the same value from
any starting point; a poor guess costs iterations and nothing else. That makes
this independent of #14 and unable to reintroduce it.

## Effect

Nodes searched are deterministic, so this is the measurement to trust — the
wall-clock difference is small enough to sit near the noise floor.

Whole-strain runs, four leaders sharing caches, summed over all five strains:

| board | unseeded | seeded | change |
|---|---|---|---|
| 1 | 416,148 | 352,103 | **-15.4%** |
| 2 | 1,124,378 | 1,080,473 | -3.9% |
| 3 | 2,223,983 | 1,951,843 | **-12.2%** |
| 4 | 1,038,226 | 1,007,459 | -3.0% |
| 5 | 1,631,816 | 1,589,973 | -2.6% |
| 6 | 1,413,124 | 1,392,954 | -1.4% |
| 7 | 962,827 | 922,119 | -4.2% |
| 8 | 237,998 | 228,171 | -4.1% |
| 9 | 349,718 | 321,379 | -8.1% |
| 10 | 2,554,254 | 2,557,095 | +0.1% |
| **total** | **11,952,472** | **11,403,569** | **-4.6%** |

Wall clock over the same corpus came out at **0.975x (-2.5%)**, which is
consistent with the node reduction but close enough to the noise floor that the
node count is the honest figure. Board 1 showed a 27% spread on that run and
should be read from the node column, not the clock.

## Correctness

Unchanged, as expected:

- corpus tables: **10 / 10**
- corpus tables vs DDS 2.9: **10 / 10**
- board 6 and #14's repro still match the C++ reference exactly
- full test suite passes
