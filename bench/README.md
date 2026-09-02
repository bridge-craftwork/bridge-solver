# Performance harness

A characterization, not a proof. It answers "is the search getting cheaper, and
are we using the machine we are on?" — two questions that move for different
reasons and must not be collapsed into one headline.

Run on demand. Not in CI: a full run takes a couple of minutes and the numbers
are only comparable on an otherwise-idle machine.

## The three questions

| | What it measures | How stable |
|---|---|---|
| **Per-board cost** | How efficient the search is, one core on one board | Very — repeats land within 1–2%, because a single-threaded process gets a full core even on a busy machine |
| **Thread sweep** | How much of the machine concurrent solves can actually use | Noisy at high thread counts, where the run contends with everything else; read it to two significant figures |
| **Verify** | That the solver still gets the right answer | Exact |

## Commands

```bash
./dev-build.sh --ci run --release --features bench --bin solver-bench -- run
./dev-build.sh --ci run --release --features bench --bin solver-bench -- run --quick --no-sweep
./dev-build.sh --ci run --release --features bench --bin solver-bench -- sweep
./dev-build.sh --ci run --release --features bench --bin solver-bench -- verify
./dev-build.sh --ci run --release --features bench --bin solver-bench -- compare BASE.json NOW.json
```

Release matters. A debug build is roughly two orders of magnitude slower and
does not even rank the boards in the same order, so its "hardest board" is not
the release build's hardest board.

`--quick` measures only the median-cost board, where cost is **nodes searched**
rather than wall time. Both rank the same ten solves, but nodes is a property of
the deal and the search, so two `--quick` runs agree on which board they mean
however busy the machine is. Ranking by a single unrepeated timing did not: two
boards 6% apart traded places from run to run, and an A/B over them averaged two
different workloads.

Use `--quick` while iterating. Use the full corpus for anything recorded, and
for any A/B -- ten paired per-board ratios are what make a difference
attributable, because a real change moves every board by roughly the same
fraction and noise does not.

## Wall clock and CPU, always both

Every measurement records wall clock *and* CPU time (`getrusage`, so it counts
every thread). This is not redundancy. Wall clock alone cannot tell "we used
four cores well" from "we got faster on one core", and those are separate
pieces of work with separate fixes.

The `cores` column is CPU divided by wall: 1.00 is single-threaded, 4.00 means
four cores were saturated for the whole run. Measured, never inferred from a
thread-count flag — inferring it is precisely how the first round of figures in
issue #13 went wrong.

`compare` reports the two geometric means separately for the same reason. A
change that moves the CPU figure made the search itself cheaper or dearer; a
change that moves only wall clock changed how well we parallelise.

## Reading the thread sweep

The sweep runs **equal work per thread** — every thread solves the same board
the same number of times. Never one board per thread: board cost in this corpus
spans 16 ms to 300 ms, nearly 20x, so a board-per-thread sweep measures the
spread of board difficulty and reports a number that looks like scaling and is
not. An early version of this measurement did exactly that and reported 1.29x
where the truth was 6.9x.

The distinction that matters in the curve is **plateau versus regression**:

- A curve that **flattens** past the physical performance-core count is
  ordinary saturation. On an 8P+4E machine, expect near-linear scaling to 8 and
  then a slow climb as the efficiency cores contribute perhaps a third of a
  performance core each.
- A curve that **turns down** — more threads, less throughput — is contention
  on shared state, and that is a bug. The harness says so explicitly.

**Sweep runs must be long enough or the curve is fiction.** At a few hundred
milliseconds a run is mostly thread startup and the final join. The harness
pilots the *widest* configuration (the fastest in wall-clock terms, so the one
most at risk of being all overhead) and grows the workload until even that runs
past `--min-sweep-seconds`, so the trap is closed by default.

To check a curve is real, vary the workload and see whether its shape holds. If
the turnover moves with workload size, the cost is per-run; if it stays put, it
is contention.

Because each run sizes its own workload, **absolute sweep times are not
comparable between runs** — only the speedup curves are, each being normalised
to its own single-threaded point. `compare` says so when the two differ.

## Best of N

The fastest run is the least contaminated by whatever else the machine was
doing, so it is the headline. The median and the spread are kept beside it, and
a spread over 15% is flagged with `!` — that means the machine was busy, not
that the code changed. `compare` treats a change smaller than either run's own
spread as "within noise" rather than reporting it as a result.

## The corpus

`corpus.json` — ten real boards from `fixtures/bench-boards.lin`, spanning 16 ms
to 300 ms single-threaded. Committed, so results are comparable across
revisions.

Every board carries its double-dummy table, and `verify` re-checks all twenty
entries. Those tables were not taken on trust: they are the ones independently
pinned in `web/src/lib/fixtures/bench-v2.json`, and the deals here were
converted from the LIN fixture and confirmed to reproduce them exactly. So the
corpus is a correctness fixture as well as a workload, and a run that is fast
and wrong says so.

## Files

```
bench/
  corpus.json                     the boards, with their pinned DD tables
  README.md                       this file
  results/
    BASELINE.md                   the current reference numbers, and what they mean
    <name>.json                   a recorded run
```

Results default to `bench/results/<git describe>.json`. Note that two runs from
different dirty working trees describe identically, so pass `--json` explicitly
when recording a before-and-after that is not yet committed.

`compare` warns when two results were taken against different corpus versions
or on different machines, because in either case the per-board numbers describe
different work.
