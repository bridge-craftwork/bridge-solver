# Performance baseline

Phase 0 of the telemetry/performance plan: measure before changing anything, so
everything downstream is calibrated against numbers rather than intuition.

Reference machine: Mac mini M4 Pro, 12 cores, 32 GB. Native figures are release
builds; browser figures are Chrome against the production `vite build` output.

Reproduce with:

```bash
cargo run --release --example bench_fixtures --features play-analysis -- units
cargo run --release --example bench_fixtures --features play-analysis -- survey 200
```

and open the app with `?debug=1` for the in-browser breakdown.

---

## The headline

**Compute dominates; delivery does not.** Cold start — fetching, compiling and
instantiating the engine — is about **24 ms**. The analysis that follows is
**377 ms**. Anything that makes the binary smaller or arrive sooner is chasing
6% of the problem.

**The single biggest win available was ordering, not parallelism.** It is
already applied, and it cost two lines: solve the play trace before the
double-dummy table instead of after. Time to the answer went from ~377 ms to
**138 ms**, because the table costs roughly twice what the trace costs and the
trace was queued behind it for no reason.

---

## Cold start (browser)

| Segment | Time |
|---|---|
| `wasm_fetch` | 20 ms |
| `wasm_compile` | 4 ms |
| `wasm_instantiate` | 0 ms |
| **total** | **~24 ms** |

Binary size: **254.7 kB raw, 111.2 kB gzip, 90.1 kB brotli.**

Two consequences. Moving to Cloudflare buys brotli over GitHub Pages' gzip — a
21 kB saving, which at these sizes is tens of milliseconds. **The migration is
justified by telemetry, not by performance**; it should not be sold internally as
a speed fix. And the plan's slow-connection reassurance is real but small: even
on a link slow enough to take 2 s over the wasm, the solve that follows is the
larger wait on any device.

Vite already emits the `.wasm` under a content-hashed name
(`bridge_solver_wasm_bg-D2fw8TBB.wasm`), so the plan's immutable far-future
`Cache-Control` is safe to set as written.

---

## The analysis, broken into work units

On the verified board (BBO board 3, 3NT by West, 41 cards). The `ddtricks`
assertion in the harness re-checks the answer against BSOL's own table on every
run, so a timing regression and a correctness regression cannot be confused.

| Unit | Native | Share | Can it be split? |
|---|---|---|---|
| `dd_table` | 222.7 ms | 37.5% | 5 ways — caches are already per-strain |
| `running_trace` (cold) | 103.5 ms | 17.4% | **No** — each position builds on the last |
| verdicts | 267.6 ms | 45.1% | Per node, sharing no cache at all |
| **total sequential** | **593.8 ms** | | |

`running_trace` off a warm cache is **~0 ms** — about 17,000x cheaper. The prefix
cache works exactly as designed.

Browser equivalents: trace 138 ms, whole solve 377 ms, verdicts 575 ms. The wasm
penalty is not uniform — about 1.2x on the solve but **2.1x on the verdict
pass**, so the stage that already costs the most is also the one that degrades
worst off the reference machine.

### Why a worker pool buys much less than it looks like it should

The verdict pass is not evenly divisible. Search depth falls as the hand is
played, so the nodes are wildly uneven:

| Node | Alternatives | Time |
|---|---|---|
| 0 (opening lead) | 13 | **217.4 ms** |
| 4 | 12 | 44.2 ms |
| 16 | 9 | 3.1 ms |
| 20 | 8 | 1.9 ms |
| 26 | 4 | 0.9 ms |

**81% of the verdict pass is one node.** Add the ordering constraint — verdicts
cannot start until the trace has said which cards were errors — and the ceiling
for splitting *whole units* across workers is:

```
max(dd_table, running_trace) + dearest single node
= max(222.7, 103.5) + 217.4  =  440 ms   vs 594 ms sequential  =  1.3x
```

That is the ceiling with **unlimited** workers, not with four. So the plan's
"single largest UX win" is, measured, a 1.3x win — against real complexity: N
workers means N position caches, N instantiations, and a cancellation story.

Going past 1.3x means splitting the *dearest node itself*. Its 13 alternatives
are independent full-depth solves, so that is the granularity worth
parallelising — not one node per worker.

---

## The plan's open questions, answered

**1. Does the solver share a transposition table across card solves?**
Partly, and the distinction matters. `running_trace` takes a prefix-keyed cache
and reuses it thoroughly (~17,000x on a repeat). `node_alternatives` — the
verdict pass — **takes no cache parameter at all** and shares nothing between
nodes. So the hypothesised "3–10x win" is already banked for the trace and does
not exist for the verdicts. The good news is that this makes the expensive half
embarrassingly parallel, with no cache to lose by splitting it.

**2. Is the build SIMD-enabled?** **No.** `rustc --target wasm32-unknown-unknown
--print cfg` lists `bulk-memory`, `multivalue`, `mutable-globals`,
`nontrapping-fptoint`, `reference-types` and `sign-ext` — no `simd128`. So there
is no Safari 16.4 floor and the old iPads are not silently excluded on that
count. The real floor is roughly **Safari 15**, set by `bulk-memory` and
`reference-types`. Catching an instantiation failure and saying "your browser is
too old" is still worth doing, but it is not the emergency the question assumed.

**3. What is the p99 hand difficulty?** Over 200 seeded random deals, DD table
only:

| | p0 | p50 | p90 | p95 | p99 | p100 |
|---|---|---|---|---|---|---|
| native | 8.9 ms | 97.7 ms | 281.7 ms | 315.8 ms | 460.8 ms | 545.9 ms |

Mean 127.0 ms; spread p99/p50 = **4.7x**. The distribution is bounded — there is
no pathological tail — so **per-card timeouts are not needed**. A progress
indicator is enough.

**4. Does the hand travel in the URL query string?** **Yes** — `hand`, `lin`,
`pbn`, `url` and `board` all carry it, and the gallery and every embed use that
path. `Referrer-Policy: no-referrer` is therefore load-bearing and must not be
tidied away later.

---

## The fixture set

`web/src/lib/fixtures/bench-v1.json` — ten deals spanning the measured
distribution, each with its `ddtricks` table. Difficulty is defined by what a
deal actually cost to solve rather than by a guess at what makes one hard.

`referenceMs` is native on the reference machine, so treat it as a **ranking**
rather than a target. The `ddtricks` values are the load-bearing part: a device
reporting a different table has a genuine wasm correctness bug, which is worth
more than any timing it also reports.

---

## What this suggests doing next

1. **Defer or background the double-dummy table.** It is 37.5% of the work and
   it is a reference the reader consults, not the thing they came for. Ordering
   already stopped it blocking the trace; not solving it until it is looked at
   would remove it from the critical path entirely.
2. **Split the opening-lead node's alternatives**, if more is wanted after that.
   13 independent full-depth solves is where the remaining concurrency is.
3. **Phase 4's ETA still stands**, and is cheap — but note the extrapolation in
   the plan (`elapsed / done * remaining`) assumes units of roughly equal cost,
   and the verdict nodes are not. Weight by remaining cards, or it will
   over-estimate badly after the first node and read as broken.
4. **Treat the Cloudflare work as telemetry work**, sequenced on its own merits.
