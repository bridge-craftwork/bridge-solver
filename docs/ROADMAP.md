# Roadmap: double-dummy analysis in the browser

Goal: run cardplay analysis where the hand is being looked at, instead of sending
every hand to a server. Three deliverables, of which the first is done.

The workstreams below are **independent unless a dependency is stated**, so they
can be picked up in parallel.

---

## Established facts

Measured or verified, not assumed. Please don't re-derive these.

**Performance.** A full per-card analysis of a complete deal (41 cards, the
expensive mid-trick mode) costs **~90 ms native / ~128 ms in wasm** — about 1.4x,
not the 3–5x you might budget for. A repeat call off the position cache is
**0.2 ms**. The 20-cell DD table is ~250 ms in wasm. A 12-board tourney is
therefore a couple of seconds: fine, but belongs in a Web Worker so the page
stays responsive.

**Correctness.** `Analyzer::dd_table` was checked against Bridge Base Online's
own server-computed table for a real board — all 20 cells match, and
`contract_tricks` agrees. The 41-card trace found 5 costed errors with seat
attribution, including declarer's, consistent with the contract making one fewer
trick than double-dummy allowed. Keep using an external reference like this; the
service and the wasm build now share code, so comparing them proves nothing.

**`std::time::Instant` traps on wasm32.** It has no implementation there and
aborts at runtime while compiling perfectly happily. Fixed in
`solve_with_caches_and_partial`, but the trap is a property of the platform, so
any new timing code needs `#[cfg(not(target_arch = "wasm32"))]`. Same class of
problem: `rayon`/threads compile and then fail, and `reqwest`'s blocking client
does not exist for wasm at all.

**LIN parsing must come from `bridge-encodings`, not `bridge-parsers`.**
`bridge-parsers` cannot build for wasm — `error[E0433]: cannot find 'blocking' in
'reqwest'` — and is really an MS Access parser that has accumulated `scraper`,
`csv` and `rust_xlsxwriter`. `bridge-encodings` compiles for wasm32 with only
`bridge-types` + `thiserror`, and has the LIN module.

---

## Done — deliverable 1: engine + wasm build

- `bridge-solver` PR #1 — `analyse_play.rs` moved here behind the
  `play-analysis` feature; `wasm/` crate added; the `Instant` trap fixed.
- `bridge-solver-service` PR #7 — service now consumes the shared engine.
  **Merge PR #1 first.**

`wasm/` deliberately mirrors the service's HTTP contract so a client can change
transport without reshaping data:

| wasm | service route | engine |
|---|---|---|
| `Analyzer::dd_table` | `POST /dd` | `par::solve_dd_table` |
| `Analyzer::dd_play` | `POST /dd/play` | `analyse_play::running_trace` |
| `Analyzer::dd_play_node` | `POST /dd/play/node` | `analyse_play::node_alternatives` |

`Analyzer` holds the prefix-keyed position cache in memory for its lifetime,
standing in for the service's SQLite table. Hold **one instance** while stepping
through a hand and each new position costs one solve, the rest are hits.

---

## Workstream A — LIN input (blocks B and C)

Both remaining deliverables need to turn a LIN string or a BBO handviewer URL
into a `PlayRequest`. Put it in `wasm/` so the site and the extension share it.

Every piece already exists; this is wiring, not new logic:

```
handviewer URL  →  strip the `lin=` query param, percent-decode
LIN string      →  bridge_encodings::lin::parse_lin  →  LinData
LinData.dealer + .auction  →  bridge_types::auction::Auction::final_contract()
                           →  FinalContract { level, strain, declarer, .. }
leader = declarer.next()
LinData.deal    →  PBN string
LinData.play    →  Vec<Card>  →  ["C3", "CT", ...]
                           ↓
                    PlayRequest { dealstr, trump, declarer, leader, plays }
```

`LinData` also carries `player_names` (S, W, N, E order — BBO's convention, not
N, E, S, W) and `claim`, both of which the UI wants.

Roughly 50 lines plus tests. Add `bridge-encodings` to `wasm/Cargo.toml`.

**Watch out:** a claimed hand has fewer than 52 cards in `pc|`, and the sample
board claimed after 41. Trailing partial tricks are normal — don't assume 52.

---

## Workstream B — deliverable 2: the Pages site

A Vite + Vue 3 app in `web/`, deploying to this repo's GitHub Pages, following
the shape of `pdf-handouts`: paste a PBN, a LIN file or a handviewer URL, get the
DD table and a play trace with each error tagged by trick.

Depends on **A** for the LIN and URL paths. The PBN path needs nothing new and
is a reasonable first slice.

Components to vendor from `Bridge-Classroom` (Vue 3 + Vite, so they drop in):
`HandDisplay.vue`, `DoubleDummyTable.vue`, `AuctionTable.vue`, `TrickArea.vue`,
`DealNavigator.vue`. Copying accepts that a handful of components can drift from
the classroom app — that was a deliberate choice over extracting a shared
package.

Carry over from `pdf-handouts`, where it is all proven:

- A strict CSP via `<meta http-equiv>`; `connect-src 'self'` is the enforcement
  behind any "stays in your browser" claim. Note `'self'` also blocks `fetch()`
  on your own `blob:` URLs — downloads via `<a download>` are unaffected.
- A "check this yourself" section rather than a self-attestation badge: pull the
  network cable, watch the Network tab, paste a snippet that tries to leak.
- A Pages workflow that builds the wasm with `wasm-pack` and uploads `web/`.

**Watch out:** `input.files` and `dataTransfer.files` are live collections the
browser empties underneath an async handler. Snapshot with `Array.from` before
any `await` or you silently keep only the first file.

---

## Workstream C — deliverable 3: the BBO extension

Tag DD errors directly on BBO's handviewer, replacing BBOHelper's server
round-trip. Depends on **A**. Rick has existing BBO extensions and would rather
fold this into one of them than ship a fourth.

An extension is a *better* privacy story than the site: it bundles the `.wasm`
locally, so nothing is fetched at runtime and `connect-src 'none'` becomes
achievable — closing the one caveat the Pages site has to state.

Two interactions, matching what Bridge-Classroom already does:

1. `dd_play` → tag each error card with its trick number.
2. `dd_play_node` → on click, show the position and each legal card's DD cost.

---

## Workstream D — fold EDGAR onto this engine

`EDGAR-Defense-Toolkit/src/dd_analysis.rs` (875 lines) is a **second, independent
implementation** of this analysis. It shares exactly one function name
(`parse_trump`) with `analyse_play.rs`, so the two can silently disagree.

It is smaller than it looks. Surveying the call sites:

- `src/lib.rs` — `pub mod dd_analysis;`, so it is public API
- `src/pipeline.rs:3006` — one test, using `compute_dd_costs`
- **nothing else.** The `dd_analysis` identifiers throughout `pipeline.rs` and
  `bin/bbo_csv.rs` are a *local variable* holding a pre-computed CSV column
  (`get(dd_col)`), not the module. Those paths parse DD costs out of CSV and
  never invoke the engine.

So this is not a rewiring job. Decide first whether anything still needs
EDGAR's module at all; if not, delete it and keep `compute_dd_costs`'s behaviour
as a test against `analyse_play`. Note the API shapes differ — EDGAR's entry
point takes `bridge_parsers::lin::LinData`, this engine's takes a parsed
`PlayInput` — so a straight re-export will not work.

Worth doing before either implementation gains users: two analysers that
disagree about a player's mistakes is a bad thing to discover from a student.

---

## Known issues

**`tests::test_replaces_existing_dd_tags` is stale** (`src/bin/bridge-solver/main.rs`).
It asserts `[OptimumScore` and `[ParContract` are absent, commented "we don't
generate them" — but `a14e712` added par calculation and now generates both. CI
never caught it because the test job ran with default features (`default = []`),
so the `cli` binaries and their tests were never built. The test job currently
uses `--features play-analysis` and deliberately omits `cli`; add `cli` once the
assertions are updated to the intended behaviour.

**The Lint job was red on `main` from 2026-07-15 to 2026-07-29** on a `cargo fmt`
diff, and nobody was notified, because a failing job on the default branch sends
no signal. Worth a branch protection rule or a notification if this matters.

---

## Conventions

**Always use `./dev-build.sh`, never bare cargo.** This repo has gitignored
`[patch]` overrides in `.cargo/config.toml` pointing sibling bridge crates at
local checkouts. With them, bare cargo either silently rewrites `Cargo.lock`
with local paths that must never be committed, or silently ignores the patch and
builds the GitHub revisions instead of your edits. The script swaps two lockfiles
around the call and verifies the patches actually resolved. Check
`git status Cargo.lock` is clean before committing.

**CI** is standardized across the bridge repos:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

with two additions here: the test and second clippy passes name
`--features play-analysis`, because the 13 `analyse_play` tests do not compile
without it. **Not `--all-features`** — that switches on the behaviour-altering
debug features (`no_tricks_pruning`, `no_fast_tricks`, ...) together, which
changes solver results and fails the suite.

The toolchain is deliberately **not pinned**. Drift gets fixed as it appears
rather than saved up for one large repin.
