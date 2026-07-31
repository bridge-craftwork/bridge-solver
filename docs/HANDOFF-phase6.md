# Handoff: Phase 6 — the public stats page

State as of 2026-07-31. Phases 0, 1, 2, 4 and 5 of `~/Desktop/bridge-solver-plan.md`
are done and in production. Phase 3 was deliberately **not** built — see below.

---

## Live now

| | |
|---|---|
| Site | https://solver.bridge-classroom.org (Cloudflare Pages, proxied, CNAME-flattened) |
| Also live | https://bridge-craftwork.github.io/bridge-solver/ — **no redirect yet**, both serve |
| Account ID | `13691335358be0d5da6e79540083d975` |
| Pages project | `bridge-solver` |
| KV namespace | `SOLVER_STATS` = `b48796607e574d4cb345d0c2e99afda3` |
| AE dataset | `solver_events`, binding `SOLVER` — **confirmed receiving data** |
| Deploys | push to `main` → CI builds once, deploys GitHub Pages *and* Cloudflare |

`functions/api/stats.ts` is already deployed and returns **503** with
`{"status":"pending"}` until something writes the KV key `aggregate`. That is the
contract Phase 6 fills.

---

## What Phase 6 is

1. A **cron Worker**, daily (~06:00 UTC), that queries the Analytics Engine SQL API
   and writes one aggregate JSON blob to KV key `aggregate` in `SOLVER_STATS`.
2. A **`/stats` static page** that fetches `/api/stats` and renders it.

Queries the plan asks for: solves per day (90d), page loads per day and the
solves-per-load ratio, distinct embed origins with counts (30d), country
breakdown, solve-time **histogram** (not mean), browser/OS breakdown, UTC-offset
histogram. Suppress any bucket under ~5 events.

`/stats` must be a **static page**, not a Function — `/api/stats` is deliberately
at `/api/` precisely so `/stats` stays free for the page. Do not mount a Function
at `/stats`; it would shadow the page.

---

## The one trap that will cost you an hour

**Account-owned API tokens (`cfat_` prefix) cannot read Analytics Engine.** The
SQL API returns `403 Authorization error` and GraphQL returns `not authorized for
that account` — *even with Account Analytics: Read granted and visible in the
token's policy*. Verified empirically this session. The cause appears to be that
analytics is user-scoped (`viewer` resolves from a user) and an account token is a
service principal with no user behind it.

So the cron Worker needs a **user-owned** token, created at
**https://dash.cloudflare.com/profile/api-tokens** (a *different page* from the
account tokens), with **Account → Account Analytics → Read**. Store it as a Worker
secret (`wrangler secret put`), never in the repo or in a local env var — that
keeps the analytics credential inside Cloudflare, which is the whole reason the
architecture routes through a cron Worker rather than querying from the browser.

SQL API: `POST https://api.cloudflare.com/client/v4/accounts/<id>/analytics_engine/sql`
with `Authorization: Bearer <token>`, body is raw SQL.

There **is** a query UI in the dashboard (Workers & Pages → Analytics Engine),
useful for developing the queries before wiring them up.

---

## Schema written by `functions/t.ts`

| Field | Contents |
|---|---|
| `index1` | event type: `load` \| `solve` \| `bench` |
| `blob1`–`blob7` | browser family, OS family, country, embed origin (`''`=top-level), `Sec-Fetch-Dest`, app version, SIMD 0/1 |
| `double1`–`double8` | elapsed ms (bucketed to 100), cold 0/1, UTC offset mins, cards, bench score, wasm fetch ms, wasm compile ms, cancelled 0/1 |

Use `quantileWeighted(q)(double1, _sample_interval)` for percentiles — AE samples,
and ignoring `_sample_interval` silently understates counts.

---

## Verified facts — do not re-derive

* **Cold start is ~24 ms** (fetch 20, compile 4, instantiate 0). The analysis after
  it is ~377 ms. Compute dominates; delivery does not.
* **Across ten real boards: `dd_table` 64.4%, `running_trace` 25.0%, verdicts
  10.6%.** The single verified board (37.5/17.4/45.1) is **not representative** —
  it has five costed errors including the opening lead, the most expensive node
  there is. Do not tune against one board.
* **A worker pool is a 1.3x ceiling**, not the plan's "single largest UX win" —
  verdicts cannot start until the trace names the errors, and 81% of the verdict
  cost is one node. Phase 3 was skipped on this evidence.
* **Brotli on the wire is 106 kB vs GitHub Pages' gzip 111 kB.** The migration was
  worth doing for telemetry, not for speed.
* `web/src/lib/fixtures/bench-v2.json` — 10 real anonymised boards whose
  per-player error counts **reconcile exactly with EDGAR-Defense-Toolkit**. That is
  Workstream D's question, answered. Regenerate with
  `cd wasm && cargo run --release --example bench_boards`.
* `bench-v1.json` is a *different* fixture: the cheap deal the device probe times.
  Do not delete it.

---

## Build gotchas

* **`wasm/` is a separate workspace.** The root `cargo fmt --all` and clippy do
  **not** reach it; CI's WebAssembly job will catch what you miss. Run `cargo fmt`,
  `cargo clippy --all-targets -- -D warnings` and `cargo test` inside `wasm/`
  separately. Move `.cargo/config.toml` aside first and restore it after.
* **Pages Functions: export only method-specific handlers.** Exporting both
  `onRequest` and `onRequestPost` served HEAD correctly but hung GET into a 522,
  because `next()` from a catch-all falls through to static-asset resolution.
* **`web/public/_headers` must not set a CSP.** Each page carries a stricter one in
  a `<meta>` tag and the browser enforces both; `index.html` uses
  `default-src 'none'` while `gallery.html` needs `script-src 'unsafe-inline'`. A
  blanket `/*` policy adds nothing and **breaks the gallery**.
* CI deploys with `wrangler pages deploy --commit-dirty=true` — the generated wasm
  module makes the tree dirty and wrangler otherwise refuses.
* The workflow's `paths:` filter must keep `functions/**` and `wrangler.jsonc`, or
  a Function change deploys nothing while the site keeps serving.

---

## Before building charts, read this

**There is no real traffic yet.** As of handoff the dataset holds ~20 data points,
all from this session's testing: the `other` browser rows are curl, the Chrome
150/151 rows are Playwright. The `p50 800ms / p95 1000ms` figures describe the
test harness, not users.

A stats page built today renders a chart of our own smoke tests. Consider letting
it collect for a week first — the dashboard query UI answers "is there anything
real yet" without any code.

---

## Also open

* **No 301 from the GitHub Pages URL.** Both URLs serve the same site and will
  drift. The plan wants the redirect kept forever, since links get pasted into
  forum threads.
* **The 64% finding is the biggest remaining performance win** — get the
  double-dummy table off the critical path. Measurable against `bench-v2.json`.
* `/privacy` is published but high-level by choice, and deliberately carries no
  legal reasoning (no ePrivacy/controller claims) — that wants a professional if it
  is ever wanted at all. Contact line is a placeholder.
