# Handoff: Phase 6 — the public stats page

State as of 2026-08-01. Phases 0, 1, 2, 4, 5 and now 6 of
`~/Desktop/bridge-solver-plan.md` are built. Phase 3 was deliberately **not**
built — see below.

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

---

## What Phase 6 shipped

* **`/stats`** — `web/public/stats.html` + `stats.js`. A static page, copied
  verbatim by Vite rather than bundled, so the JS must run as written. Charts are
  hand-built SVG because the page's own CSP is `default-src 'none'` and could not
  load a charting library even if we wanted one.
* **`/api/stats`** — `functions/api/stats.ts`, rewritten. Queries Analytics
  Engine itself and caches the result in KV for 15 minutes.

### The cron Worker in the plan was not built, on purpose

The plan called for a daily cron Worker to aggregate into KV. Its stated
justification was that the AE SQL API needs a token that must not reach a
browser — true, but `/api/stats` is *already* server-side, so the token is
equally safe there. The other two justifications (cheap, spike-immune) are cache
arguments, and a cache does not need a clock.

So `/api/stats` serves the KV blob and refreshes past a TTL. Same properties,
plus: no queries at all on a day nobody visits, self-healing where a dead cron
would silently serve stale numbers forever, and **one** deploy artifact —
[Pages Functions cannot carry cron triggers](https://developers.cloudflare.com/workers/configuration/cron-triggers/),
so a cron would have meant a whole second Worker.

**The one thing this gives up:** history outliving AE's
[three-month retention](https://developers.cloudflare.com/analytics/analytics-engine/limits/).
Everything the page shows sits inside that window, so today every figure is
recomputable on demand and the KV blob is a pure cache with no independent
value. If a record longer than three months is ever wanted, that is the change
to make — **and it has to be made before the data ages out, not after.**

---

## Still to do — the credential

`/api/stats` needs `AE_TOKEN` set as a Pages secret. Until it is, the endpoint
returns 503 `{"status":"pending"}` and the page renders a clean "Not collecting
yet" card. That state is verified in a browser; nothing breaks.

```bash
npx wrangler pages secret put AE_TOKEN --project-name bridge-solver
```

`CF_ACCOUNT_ID` is already set as a plain var in `wrangler.jsonc` — it is not a
credential.

### The "account tokens cannot read AE" trap does not reproduce

The previous handoff recorded, as verified, that account-owned `cfat_` tokens get
403 from the Analytics Engine SQL API even with Account Analytics: Read granted,
and that a **user**-owned token was therefore required.

**That is not true of the token currently in `CLOUDFLARE_API_TOKEN`.** It is
`cfat_`-prefixed and it read the dataset successfully — repeatedly, across all
sixteen dashboard queries. Either the token's policy was edited after that test
or the original 403 had another cause; there is no way to tell now, and a freshly
minted `cfat_` token has **not** been re-tested.

Practical consequence: ownership is not the constraint it was recorded as. Scope
still is. The existing token is the *deploy* credential (Pages write, KV write);
giving it to `/api/stats` hands the Function deploy rights it never uses. Prefer
a separate token scoped to **Account Analytics: Read** alone.

---

## Schema written by `functions/t.ts`

| Field | Contents |
|---|---|
| `index1` | event type: `load` \| `solve` \| `bench` |
| `blob1`–`blob7` | browser family, OS family, country, embed origin (`''`=top-level), `Sec-Fetch-Dest`, app version, SIMD 0/1 |
| `double1`–`double8` | elapsed ms (bucketed to 100), cold 0/1, UTC offset mins, cards, bench score, wasm fetch ms, wasm compile ms, cancelled 0/1 |

Use `quantileWeighted(q)(double1, _sample_interval)` for percentiles — AE samples,
and ignoring `_sample_interval` silently understates counts.

### The SQL dialect is a subset — verified empirically, not from docs

The published SQL reference is a table of contents. These were probed against the
live API:

**Works:** `quantileWeighted(q)(col, _sample_interval)`, `intDiv`, `floor`,
`round`, `toDate`, `toStartOfInterval`, `NOW() - INTERVAL '90' DAY`,
`toDateTime('…')`, `if()` (including nested), `count(DISTINCT x)`, `avg/min/max`,
`HAVING`, `LIMIT`, arithmetic inside an aggregate (`double1 / double4`).

**Rejected:** `uniq()`, `uniqExact()` — *unknown function call*.
`CASE WHEN … THEN … END` — *unsupported expression type*.
`quantilesWeighted` (plural form) — *unknown function call*.

`FORMAT JSONEachRow` returns newline-delimited objects with **no** envelope;
without it you get the enveloped `{meta, data, rows}` form. Counts come back as
strings (`UInt64`), timings as numbers.

---

## The finding the dashboard immediately produced

**The pre-solve warning over-predicts by about 2.2× at the median**, and worse on
slower devices. Reported independently from a newer iPad Air: warning said ~4 s,
actual analysis ~0.5 s. Two compounding causes, both in
`web/src/lib/benchmark.js`:

1. **Cold probe against a warm reference.** `runBenchmark()` is the *first* wasm
   call the page ever makes (`App.vue`, posted before the analysis), and its timer
   starts before the worker `postMessage` — so it includes the worker hop and
   pre-JIT execution. `REFERENCE_MS = 41` is documented as measured "across
   repeated runs", i.e. warm steady-state. Cold-vs-warm depresses every device's
   score, worst on mobile JSC.
2. **The multiplier rests on the board this repo's own docs disown.**
   `expectedTotalMs` uses 950 ms = "377 ms solve + 575 ms verdict pass", and
   `docs/performance-baseline.md` says the 575 ms verdict figure is "true of the
   verified board and false in general" — verdicts are 10.6% across ten real
   boards, not 45%. The ordering fix also took time-to-answer to 138 ms.

A depressed score divides an inflated constant, so the errors multiply. **Neither
is fixed.** The "Is the pre-solve warning honest?" panel on `/stats` measures the
gap continuously, and needs no telemetry change to do it: the prediction is a
pure function of the bench score, which is already collected as `double5`.

> If `expectedTotalMs` changes, change `PREDICTED_MS_AT_100` in
> `web/public/stats.js` too. That file is not bundled and cannot import it, so
> the panel would otherwise silently grade against a formula the app no longer
> uses.

---

## Input: BBO short links, and the Paste button

BBO's **"Export handviewer link"** hands you a shortened link
(`https://tinyurl.bridgebase.com/5cyerrh5`), not the handviewer URL. It used to
fall through to the generic "that does not look like a hand" error, which is the
worst possible answer to the most likely first paste.

**We cannot expand it, and should not want to.** Both halves matter:

* **Not from the browser.** The 301 comes back with no
  `Access-Control-Allow-Origin`, so script cannot read its `Location`
  cross-origin — and the page's own `connect-src 'self'` forbids the request
  first. Verified against the live endpoint.
* **Not from a Function either.** It would be a few lines, and it would be a
  breach. The expanded URL carries the deal *and the players' usernames* —
  `pn|wino john,snowball5,kemistry,swaddee` in the reported example. Routing that
  through our infrastructure makes the deal leave the device, which is the site's
  central promise, and specifically falsifies privacy.html's claim that BBO
  usernames "are not transmitted, not stored, and not something we could look at
  even if we wanted to."

So `input.js` recognises the short link and says what to do instead: open it,
then copy the full address from the address bar. Tested in a browser, not only
in unit tests.

**A browser extension is the right home for this.** An extension has host
permissions and is not bound by CORS, so it could resolve the short link and
hand over the expanded URL without anything touching our servers — the deal would
go BBO → the reader's own browser, exactly as the promise requires. That is a
genuinely good fit and the only design that both works and keeps the promise.
Not built.

### There is no text field any more

`InputPanel.vue` has no input box. Nobody reads back a LIN record or a
handviewer URL, so echoing it on screen spent the top of the page on machine
text. A status line names what loaded ("Handviewer link loaded", "12 lines
loaded") via `detectKind`, so it cannot disagree with what the parser then does.

Three ways in, and the ordering matters:

1. **Pasting anywhere on the page** — a window-level `paste` listener. This is
   the path that always works, and the reason the field could go. Per the
   [Clipboard API spec](https://w3c.github.io/clipboard-apis/), an event handler
   may read clipboard data when "the action that triggers the event is invoked
   from the user-agent's own user interface" — **no permission check**, unlike
   the async API, which "must run … check clipboard read permission" and rejects
   without it. So ⌘V/Ctrl+V works in an embed, in every browser, with no prompt.
2. **The Paste button** — the async API, for touch and for click-users.
   Feature-detected and hidden when `navigator.clipboard?.readText` is absent
   (which includes any non-secure context).
3. **A file**, dropped on the panel or chosen.

Verified in a browser: ⌘V loads and analyses on the top-level page, **and inside
an iframe carrying no `allow` attribute**. That iframe was same-origin — no
second origin was available locally — but the paste event does not consult
Permissions Policy at all, so origin is not the variable.

Two corrections worth keeping, both of which were wrong before being checked:

* **"A Paste-only UI would break every embed" was wrong in the case that
  matters.** `App.vue` renders `<InputPanel v-if="!embed">`, so the documented
  embed (`?embed=1`, what the gallery uses) has no input panel at all. The
  concern only applies to someone iframing the *full* page — real, but much
  narrower than claimed.
* **"Firefox does not support `readText()` for web content" was wrong.** It does,
  behind a per-use prompt. Checked against MDN rather than assumed.

One behaviour found only by testing: **Chrome prompts on first use of the
button**, and the promise sits *pending* until answered — so the first Paste is
two clicks and appears to do nothing in between. Firefox and Safari prompt every
time instead, since they do not implement the `clipboard-read` permission.

Not tested: Firefox, Safari, touch devices, or a genuinely cross-origin embed.

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
* **`wrangler pages dev --kv X` replaces the config's whole binding set** rather
  than adding to it, so `env.SOLVER` goes undefined and `/t` answers 500 locally.
  Analytics Engine has no local emulation either way. That 500 is a local-dev
  artifact, not a regression — do not "fix" it.

### Running the stats page locally

```bash
npm --prefix web run build
npx wrangler@4 pages dev --port 8788 \
  --binding AE_TOKEN="$CLOUDFLARE_API_TOKEN" CF_ACCOUNT_ID="1369…d975" \
  --kv SOLVER_STATS
```

Pass the token as a `--binding` rather than writing `.dev.vars`, so the
credential never lands on disk. `rm -rf .wrangler` clears the local KV to
re-test the cold path.

---

## The data is still mostly our own testing

As of this handoff the dataset holds ~29 points, nearly all from development:
`other`/iOS rows are spoofed-UA curl, `Chrome 150/151` is Playwright, the single
`GB` row is a spoof. Every percentile on the page describes the test harness.

Telemetry carries no identifier by design, so **our traffic can never be
separated from real traffic except by timestamp**. `DATA_START` in
`functions/api/stats.ts` is the only lever for that. It is currently wide open
(`2000-01-01`), and the page says so in a caveat banner at the top.

**Set `DATA_START` to the moment real traffic begins.** It cannot be applied
retroactively by any other means.

---

## Also open

* **No 301 from the GitHub Pages URL.** Both URLs serve the same site and will
  drift. The plan wants the redirect kept forever, since links get pasted into
  forum threads.
* **A browser extension for BBO short links** — see above. The only design that
  resolves them without breaking the privacy promise, and it would also let the
  reader go straight from a BBO table to an analysis.
* **The benchmark calibration above is unfixed** and is now the most visible
  user-facing defect: people are told to expect several seconds and get half of
  one.
* **The 64% finding is the biggest remaining performance win** — get the
  double-dummy table off the critical path. Measurable against `bench-v2.json`.
* `/privacy` is published but high-level by choice, and deliberately carries no
  legal reasoning (no ePrivacy/controller claims) — that wants a professional if it
  is ever wanted at all. Contact line is a placeholder. It now links `/stats`.
* The stats page is **light-only**, matching the rest of the site. Its two series
  colours are from a validated categorical palette checked against the page's
  white surface (worst-pair CVD ΔE 24.7). Re-validate before changing them.
