# Three solvers, three ways of using them

Measured 2026-09-02 on an Apple M4 Pro (8 performance cores, 4 efficiency),
macOS 25.5. Method, and why each choice was made, in `METHODOLOGY.md`. Each
solver is built the way its own project builds it: DDS 2.9 with its
`Makefile_Mac_clang_static` (`-O3 -flto`), macroxue's reference at `75b4619`
with the PGO its makefile defaults to, this port with `cargo build --release`.
That last asymmetry is measured rather than waved at: PGO is worth about 1% to
the reference and 1.7% to this port, so building both with it would move these
tables by about a point and not change any ordering in them. See "PGO on this
port" in `bench/results/release-profile.md` for why this port does not ship it.
Cases 2 and 3 were re-measured later the same day, after this port changed its
unit of parallel work; case 2 says what that was and what it moved. Case 1 is
unaffected by it -- one board on one thread has nothing to schedule -- and its
numbers are from the original run.

**DDS 2.9 is 1.00 in every table**, being the one everybody already has.

The short version: **which solver is fastest depends on what you are doing with
it, and the differences that exist are mostly invisible.** There is no single
number here, and anyone quoting one — including us — should say which of these
three questions they answered.

---

## 1. One board, someone waiting

2,000 random deals, full twenty-cell table each, single-threaded, one at a
time. Milliseconds, fastest of an adaptive number of repeats.

| | this port | DDS 2.9 | C++ reference |
|---|---|---|---|
| p50 | 78.7 &nbsp;`1.17x` | **67.1** | 75.3 &nbsp;`1.12x` |
| p80 | 162.0 &nbsp;`1.06x` | 153.5 | **147.0** &nbsp;`0.96x` |
| p90 | 234.9 &nbsp;`1.05x` | 223.7 | **213.1** &nbsp;`0.95x` |
| p95 | 324.7 &nbsp;`1.08x` | 299.5 | **297.2** &nbsp;`0.99x` |
| p99 | 637.6 &nbsp;`0.91x` | 703.2 | **569.6** &nbsp;`0.81x` |
| max | 1480.0 &nbsp;`0.56x` | 2643.1 | **1278.6** &nbsp;`0.48x` |

**Those rows are not paired.** Each column is its own percentile over its own
2,000 timings, so `p95` compares the deal *we* find 95th-hardest with the deal
*DDS* finds 95th-hardest, which are rarely the same deal. It is the right shape
for "what does a user wait for", and the wrong one for "which is faster on this
board" — read as a paired ratio it flatters whichever solver has the shorter
tail. The table below is the paired view, and the two disagree by enough to
matter: `p95` reads 0.99x for the reference, where paired it has been ahead of
DDS since about DDS's p70.

![per-deal latency](case1-latency.svg)

**There is a crossover, and it is the only finding here that matters.** Grouped
by how hard DDS finds a deal. The three ratio columns are each pair's total
time over the same deals; the last two count deals on which that solver beat
DDS outright:

| DDS takes | deals | this port | C++ reference | ours/ref | this port wins | reference wins |
|---|---|---|---|---|---|---|
| under 50 ms (to DDS p39) | 774 | 1.32x | 1.28x | 1.03x | 26% | 25% |
| 50–100 ms (to p65) | 525 | 1.23x | 1.15x | 1.06x | 31% | 37% |
| 100–200 ms (to p87) | 450 | 1.08x | **0.99x** | 1.09x | 47% | 56% |
| 200–400 ms (to p97) | 197 | **1.02x** | 0.92x | 1.11x | 53% | 69% |
| 400–800 ms (to p99) | 40 | 0.92x | 0.82x | 1.13x | 72% | 75% |
| over 800 ms | 14 | **0.73x** | **0.64x** | 1.14x | 86% | 86% |

DDS is quickest on cheap deals and **nobody can perceive the difference** — 67
ms against 79 ms is not a thing a person notices. The two tree searches pull
ahead on expensive deals, which is exactly where a person does notice. Of these
2,000 deals the one DDS finds hardest takes it **2.6 seconds**, where this port
takes 1.5 and the reference 1.3. The reference crosses DDS at about DDS's p70
and this port at about p75, so both spend the top quarter of the distribution
ahead.

**The `ours/ref` column runs the wrong way, and it is the one to watch.** We
track the reference to within 3% where deals are cheap and fall to 14% behind
where they are expensive, monotonically. The single figure in `CLAUDE.md` --
1.067x over the lock-step corpus, 1.086x over these 2,000 -- averages that
trend away. It is the same fact as the reference beating us by most on the
hardest deals, seen from the other side, and it is the one place in this
document where a user could plausibly notice the difference between the two
tree searches: the remaining per-node cost is paid most where the search is
longest.

So the useful summary for a user is not a ratio, it is: **all three are
instant on easy deals, and on the deals that make you wait, the tree searches
are about a third quicker.**

That also means a mean over a corpus lands wherever the corpus happens to sit
on this curve, which is worth remembering when reading anyone's headline
figure, this repo's included.

---

## 2. One event, 27 boards, someone waiting

The 27 boards of a club session, solved back to back. Deals per second; **above
1.00 is faster than DDS**.

| threads | this port | DDS 2.9 | C++ reference | ours/dds | ref/dds |
|---|---|---|---|---|---|
| 1 | 10.2 | **12.4** | 10.4 | 0.82x | 0.84x |
| 4 | 37.1 | **46.6** | 35.5 | 0.80x | 0.76x |
| 8 | 70.8 | **90.1** | 59.3 | 0.79x | 0.66x |
| 12 | 77.6 | **98.4** | 68.2 | 0.79x | 0.69x |

**DDS still wins this one, but no longer for the reason it used to.** The first
measurement of this case put us at 59.6 deals/sec on twelve threads against
DDS's 92.2 — 0.65x, and gaining essentially nothing between eight threads and
twelve, which is the signature of load imbalance rather than of a slow search.
Twenty-seven deals is not much to spread over twelve threads when deal cost
spans tenfold: this port
handed *whole deals* to threads from a shared cursor, so the run ended when the
slowest deal ended and several threads had long since gone idle. DDS's
`CalcAllTablesPBN` decomposes into one work item per (deal, strain) pair — 135
items rather than 27 — and packed them far better.

**The fix was to use the same unit.** A strain is the smallest piece of a table
that can move to another thread without changing the search: its four declarers
share a reset pair of caches and a chain of MTD(f) seeds, and nothing crosses
the boundary between one strain and the next. `TableSolver::solve_strain` is
that unit, and the numbers above are the same 27 boards with the cursor running
over 135 pairs instead of 27 deals, measured against the unchanged binary in
two interleaved passes on an otherwise idle machine. Scaling from one thread to
twelve went from 5.9x to 7.6x, against DDS's 7.9x in the same pass, and the
ratio at twelve threads from 0.60x to 0.79x. Node counts are unchanged to the
last node, which is what says this was scheduling and not search.

What is left is per-node cost, the 1.067x recorded in `CLAUDE.md`, and that is
a different problem. The whole event is under half a second on any of these
solvers, so none of it is a difference anybody will feel.

---

## 3. A file of deals, nobody watching

500 random deals. Deals per second; above 1.00 is faster than DDS.

| threads | this port | DDS 2.9 | C++ reference | ours/dds | ref/dds |
|---|---|---|---|---|---|
| 1 | 8.8 | **9.3** | 9.2 | 0.94x | 0.99x |
| 4 | 31.8 | 34.7 | **35.0** | 0.92x | 1.01x |
| 8 | 60.6 | 61.3 | **64.3** | 0.99x | 1.05x |
| 12 | 71.0 | 67.3 | **78.3** | 1.06x | 1.16x |

With enough work to hide the imbalance the picture inverts: past eight threads
both tree searches are ahead, and the reference most of all. Scaling from one
thread to twelve is 8.1x for this port, 7.2x for DDS, 8.5x for the reference —
against eight performance cores plus four efficiency cores contributing perhaps
a third each.

Case 2's finer work unit does nothing here, as expected and as checked: 2,500
(deal, strain) pairs and 500 deals are both far more items than there are
threads, so there is no tail to trim. Measured before and after, the columns
move by -1.5%, +1.7% and -1.4% at 4, 8 and 12 threads, in no consistent
direction and inside the 1-2% these numbers wander anyway.

**The reference has no threads at all.** It is single-threaded and not
thread-safe — its caches and statistics are global mutable state — and it has
no multi-deal input either: `main` accepts one deal per invocation, and its own
`parallel_run_tests.sh` fans out with `xargs -P`, one process per deal. Its
column above is that, one process per deal, because that is how it ships. It
does very well at it. But using it on a file of deals means writing the
orchestration yourself, where DDS and this port are a library call.

---

## What to take away

- **Solving one board for someone who is waiting**: all three are
  indistinguishable on ordinary deals. The tree searches move ahead over the
  top quarter of the distribution and are furthest ahead at the very top —
  about two thirds of DDS's time over the hardest one percent — which is where
  waiting is actually perceptible. DDS's worst case here was 2.6 seconds
  against their 1.3–1.5.
- **Solving one event**: any of them, well under a second. DDS is still ahead
  on a small machine, but by the same margin it holds everywhere else now that
  we schedule by (deal, strain) pair as it does; what remains is per-node cost,
  not work granularity.
- **Solving a large file**: close, with the tree searches ahead once past eight
  threads. If you want that from the reference you will be writing a shell
  script; the other two give you a thread count.

## What is not controlled

**One machine, and it is not neutral.** The reference has an x86-only fast path
— `PackBits`/`UnpackBits` become `_pext_u64`/`_pdep_u64` under `__BMI2__`, and
its makefile adds `-mbmi2` when `/proc/cpuinfo` reports it — that aarch64
cannot have. Measured, it is worth about 5% to the reference, so its column
would improve by roughly that on a Zen 3 Linux machine. DDS has no
architecture-specific code at all and would not move. This port is in the same
position as the reference.

**One version of each**, at one moment: DDS 2.9, the reference at `75b4619`
(its 2026-01-31 state, which is what this port searches node-for-node), and
this port at the commit that added this file. Upstream's own README claims 1.5x
DDS for a later revision than the one measured here.

**Not an idle machine.** Case 1 uses the fastest of several repeats per deal,
which is what makes that tolerable; cases 2 and 3 are whole-machine
measurements and more exposed, and use the best of five and three rounds
respectively. Ratios rather than absolute figures are what should be read
across from here.

**Random deals**, from a fixed seed, generated by `gen-corpus.py`. Real
collections are not uniformly random, and a collection of hard deals would sit
further right on the case-1 curve than this one does.
