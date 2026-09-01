# Baseline: bridge-solver at the bounds-cache fix, corpus `corpus-v1`

Measured 2026-09-01 on an Apple M4 Pro (12 logical, 8P+4E), best of 3 runs,
native release build.

Superseded by `cache-fix.json`; see `cache-fix.md`. The numbers below are the
concurrency work only, and were measured on a path that produced a wrong table
for board 6 (#14). They are kept because the scaling curve is still the
reference for threading, and because `node-counter-before.json` /
`node-counter-after.json` are a clean before-and-after for the node-counter
change on the same corpus and machine.

**For current per-board costs use `release-profile.json`; see `release-profile.md`.**

```
results   : node-counter-after.json
corpus    : corpus-v1 (10 boards)
machine   : Apple M4 Pro, 12 logical (8P+4E)

board  contract      wall ms     cpu ms   median   spread
----------------------------------------------------------
    1  3HX             34.9       34.8     34.9     4.2%
    2  1H              75.1       74.9     75.3     0.5%
    3  2N             224.9      224.5    225.3     0.8%
    4  2D             102.2      101.9    102.3     2.1%
    5  2S             141.4      141.3    143.5     1.8%
    6  3H             111.3      111.2    112.4     3.2%
    7  3N              85.6       85.5     85.7     0.3%
    8  5CX             16.2       16.2     16.2     1.1%
    9  2H              23.8       23.8     23.9     0.9%
   10  3N             295.6      294.7    298.2     3.2%
```

Cost spans 16 ms to 296 ms — a factor of 18 across ten ordinary boards. Any
measurement that treats boards as interchangeable units of work is wrong before
it starts, which is why the thread sweep uses equal work per thread rather than
one board per thread.

## Scaling

Thread sweep on board 4, equal work per thread:

| threads | speedup | cores busy |
|---|---|---|
| 1 | 1.00x | 1.00 |
| 2 | 1.94x | 2.00 |
| 4 | 3.75x | 3.99 |
| 6 | 5.56x | 5.98 |
| 8 | 7.12x | 7.86 |
| 10 | 7.91x | 9.56 |
| 12 | 8.07x | 10.39 |

Near-linear to 8 threads, then a slow climb as the four efficiency cores join —
they contribute roughly a third of a performance core each. That is ordinary
saturation, and the shape to expect on this machine. A curve that turns *down*
instead would be contention.

## What changed, and what did not

Against `node-counter-before.json`, same corpus, same machine:

| | before | after | change |
|---|---|---|---|
| Single-threaded, wall | — | — | **1.002x (within noise)** |
| Single-threaded, CPU | — | — | **1.001x (within noise)** |
| 4 threads | 1.90x | 3.75x | 1.97x |
| 8 threads | 1.74x | 7.12x | 4.08x |
| 12 threads | 1.27x | 8.07x | 6.33x |

The per-core cost did not move at all, which is the point: the fix removed a
shared per-node atomic counter, and shared counters cost almost nothing on one
thread and everything on twelve. Before the fix the curve peaked at 1.90x on 4
threads and then **fell** to 1.27x — the harness flags that automatically as a
regression rather than saturation.

## Known gaps

- **No external reference.** DDS is not measured here. It is now measured in
  `release-profile.md`, where we come out at 1.06x DDS per core; issue #13's
  figure of roughly 1.5x cheaper predates both the bounds-cache fix and the
  release profile.
- **One machine.** Every number above is an M4 Pro. The 8-thread knee is a
  property of 8P+4E silicon, not of the solver.
