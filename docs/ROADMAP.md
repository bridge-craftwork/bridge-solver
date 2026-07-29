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

## Done — workstream A: LIN input

`wasm/src/lin_input.rs`. `parse_lin(input)` takes a LIN string *or* a handviewer
URL and returns the `PlayRequest` plus contract, seat names (reordered to
N,E,S,W), dealer, vulnerability, board, claim and auction.
`parse_lin_file(content)` does a board per line, reporting an unanalysable board
in place rather than dropping it. 16 tests.

It was not pure wiring. Three things the plan below did not anticipate:

- **`Auction::final_contract()` gets declarer wrong** — it credits whoever made
  the *last* bid, but declarer is the first of that side to name the strain. Over
  `1S - P - 4S` it says South. That inverts declarer on most ordinary auctions,
  which swaps the opening leader and invalidates every trick count downstream.
  `resolve_contract` in `lin_input.rs` does it correctly; **`bridge-types` still
  needs the upstream fix**, and anything else calling `final_contract` is
  suspect.
- **LIN spells doubles `d`/`r`**, which `Call::from_pbn` rejects — it wants PBN's
  `X`/`XX`. Both spellings occur in real files (BBO writes the first, tools that
  generate LIN from PBN write the second), so both are accepted.
- **`BidWithAnnotation` is `bridge-encodings`' own type**, not `AnnotatedCall`,
  so the auction has to be rebuilt call by call rather than handed over.

Declarer attribution is checked against three boards whose contract and declarer
were recorded independently by `bridge-bots`, in the spirit of the external
reference the DD table got.

The deal string is anchored on North whatever the dealer, because the position
cache keys on it with only case and whitespace normalised — a deal written from
another seat would key differently and re-solve from scratch.

CI gained a `WebAssembly` job. `wasm/` is a separate workspace, so the existing
`--workspace` jobs never touched it: its tests did not run and a wasm32-only
regression could ship unnoticed.

<details>
<summary>The original plan, for reference</summary>

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

</details>

Confirmed while implementing, so don't re-derive: `parse_md` fills in the fourth
hand itself when `md|` carries only three; `Suit::from_char` and
`Rank::from_char` both uppercase, so vugraph LIN's lowercase `pc|dA|` parses;
`Direction::next` is clockwise, so `declarer.next()` is the correct leader; and
`Vulnerability::to_pbn()` yields exactly `None|NS|EW|All`, which is worth using
over matching the variants because those were renamed between the pinned and
local revisions.

Real fixtures live in `EDGAR-Defense-Toolkit/tests/fixtures/input/` (12 boards,
with per-player DD error counts alongside) and
`bridge-bots/bridgebots/tests/resources/` (vugraph, multi-line records).

---

## Done — deliverable 2: the Pages site

`web/`. Vite 7 + Vue 3, deploying via `.github/workflows/pages.yml`. Paste a PBN
board or file, a LIN record or file, or a BBO handviewer URL; get the DD table,
the auction, a play trace with every error tagged by trick, a per-player summary,
and click-any-card alternatives. 70 JS tests.

Verified working in a real browser under the real CSP, not just built: the wasm
loads in its worker, the trace renders, and clicking a card returns all 13 legal
alternatives with their costs.

Departures from the plan below, all deliberate:

- **The solver runs in a Web Worker.** The roadmap wanted this for a tourney;
  it turned out to be needed anyway, and it also gives the position cache a
  natural home for the page's lifetime.
- **`--table-scale` is the one thing worth keeping from the classroom's CSS.**
  Its table components hard-code their colours and consume only that variable, so
  copying `design-tokens.css` alone would not reproduce the look. The literals are
  tokens here and used as tokens.
- **`HandDisplay`'s measured-fit machinery is gone** — the probe row, the
  `ResizeObserver`, the double-rAF settle, `--suit-scale`, the `+N` popup. All of
  it existed to fit a live table into an arbitrary viewport; this site picks its
  own width. That was ~40% of the file and every moving part in it. The marks
  contract is kept exactly, because that is what the overlay renders through.
- **The 877-line grid arranger is not vendored.** `BridgeTable`'s legacy compass
  branch does the same job with the same `marksFor` merge and no config.
- **No Google Fonts.** A third-party font CDN in the waterfall of a page whose
  claim is that nothing leaves your browser is a bad look. System fonts, and the
  card glyphs were always going to be `'Segoe UI'`/`system-ui` anyway.
- **Dummy's errors are credited to declarer**, who chose them, with dummy scored
  as not applicable. That is BBO's own BSOL convention — see the verification
  below — and attributing by card holder instead reads as though dummy had made
  mistakes of its own.

**Two traps worth knowing.** `postMessage` cannot clone a `Proxy`, and Vue
reactive state *is* proxies — passing a reactive `plays` array to the worker
fails with `DataCloneError`, not a wrong answer, so the analysis silently
vanishes. `playRequest` copies to plain data and a test pins it. Relatedly, the
classroom client's null-on-any-failure discipline is right for the UI but
discards the reason; `optional()` keeps the `null` and logs why, which is the only
reason that bug was findable.

Vite 8 is not usable here: it builds on rolldown, whose `darwin-arm64` native
binding would not install. Vite 7 is rollup-based and outside the advisory range
(`<=6.4.2`), and `vitest` is pinned past its own (`<=3.2.5`) — `npm audit` is
clean, and should stay that way.

<details>
<summary>The original plan, for reference</summary>

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

</details>

**Verified against BBO's own analysis.** A bridgewebs BSOL payload for a real
board — 3NT claimed after 41 cards — carries both a DD table and a per-player
error count, and this engine reproduces all of it: the `ddtricks` string
`45544465449789987899` byte for byte, 5 costed errors, and per-player counts of
North 1, South 1, declarer 3 once dummy's two are folded in. It is also the
auction that most needs checking, `1NT - Pass - 2C - Pass - 2H - Pass - 3NT`:
East bid the final 3NT but West named notrump first, so **West declares** — the
exact case `final_contract` gets wrong. Pinned as
`matches_bsol_on_a_real_board`. Keep using references like this.

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
`--features cli,play-analysis`, because the 13 `analyse_play` tests do not
compile without `play-analysis`, and the CLI binaries' tests do not build
without `cli`. **Not `--all-features`** — that switches on the behaviour-altering
debug features (`no_tricks_pruning`, `no_fast_tricks`, ...) together, which
changes solver results and fails the suite.

The committed `Cargo.lock` must cover the optional features' dependencies
(`serde`, `sha2`), or every `play-analysis` build re-resolves and the CI cache
never hits.

The toolchain is deliberately **not pinned**. Drift gets fixed as it appears
rather than saved up for one large repin.
