# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

<!-- TODO: Add project description -->

## Build & Test Commands

**Use `./dev-build.sh` for local development builds, not bare cargo.** This repo depends on sibling bridge crates as git dependencies, with gitignored `[patch]` overrides in `.cargo/config.toml` redirecting them to the local checkouts in `../`. Cargo never lets a `[patch]` override an existing `Cargo.lock` pin, so bare `cargo build` silently compiles the GitHub revisions of those crates instead of your local edits — and if the patches do take effect, they rewrite `Cargo.lock` with local-path entries that must never be committed (CI has no sibling checkouts). The script keeps a separate local lock (`.cargo/dev.lock`), swaps it in around the cargo call, verifies each patched crate resolved to a local checkout, and leaves the committed `Cargo.lock` untouched.

```bash
./dev-build.sh                    # cargo build, against local sibling checkouts
./dev-build.sh build --release    # any cargo subcommand + args pass through
./dev-build.sh test               # cargo test
./dev-build.sh clippy -- -D warnings   # lint
cargo fmt --check                 # no dependency resolution; bare cargo is fine
```

For CI-parity builds (pre-commit checks, release verification) use `./dev-build.sh --ci test` (any cargo subcommand works after `--ci`) — it temporarily disables the local patches and builds with the committed lock's git pins. **Avoid bare cargo for anything that resolves dependencies** (build/test/check/run): with the patches present, a same-version patch is applied immediately and silently rewrites `Cargo.lock` to local-path entries, while a version mismatch makes the patches silently ignored — both wrong. The committed `Cargo.lock` must always pin `git+https://` sources for the internal crates; never commit a lock where those entries have lost their `source =` lines.

## Pre-commit Requirements

Before committing, always run and fix:
1. `cargo fmt --all` - Format all code
2. `./dev-build.sh --ci clippy --all-targets -- -D warnings` - Fix all clippy warnings
3. `./dev-build.sh --ci test` - Ensure all tests pass (CI parity: patches disabled, committed lock's git pins)

## Code Standards

- No `unwrap()` or `expect()` outside test code - use proper error handling
- No `println!()` in library code (CLI binaries are OK)
- All public functions must have doc comments (`///`)
- All `unsafe` blocks must have a comment explaining why they're safe
- Prefer editing existing files over creating new ones

## Git Configuration

Use SSH for all GitHub operations:
- Clone/push/pull: `git@github.com:bridge-craftwork/repo.git` (not `https://`)
- Remote URLs should use SSH format

## Related Projects

All located at `/Users/rick/Development/GitHub/`:

| Project | Description | Relationship |
|---------|-------------|--------------|
| [bridge-types](../bridge-types) | Core bridge types | upstream dependency |
| [Bridge-Parsers](../Bridge-Parsers) | PBN/LIN file parsing | sibling |
| [pbn-to-pdf](../pbn-to-pdf) | PDF generation | downstream |
| [bridge-wrangler](../bridge-wrangler) | CLI tool | downstream |
| [dealer3](../dealer3) | Hand generator | sibling |

## Notifications

Send Pushover notifications when work is blocked or completed:

```bash
pushover "message" "title"    # title defaults to "Claude Code"
```

**When to notify:**
- Waiting for user input or permission
- Task completed after extended work
- Build/test failures that need attention
- Any situation where work is paused and user may not notice

## Lock-step with the C++ reference

This is a port of `macroxue/bridge-solver`, and as of 2026-09-02 it searches the
same tree as that project at its 2026-01-31 state — **node for node, on all 200
deals of the reference's own corpus**. That is a property worth keeping, because
it turns any behavioural change into an immediately visible one.

Two invariants must hold after any change that could touch the search:

```bash
./dev-build.sh --ci run --release --features bench --bin solver-bench --   nodes fixtures/divergence/lockstep-200.pbn
```

must total exactly **296,689,028 nodes**, and

```bash
XRAY=<path to built xray/solver-xray> fixtures/divergence/first-divergence.sh
```

must report `none` for all twelve fixtures. See `fixtures/divergence/README.md`
for what each fixture isolates and how to build the instrumented reference.

Searching *fewer* nodes is a divergence too, not an improvement. Getting to
lock-step took five fixes, three of which were ordering bugs in the pattern
tree: `Pattern::lookup` returns the first child that matches, so child order is
semantics rather than housekeeping.

`bench/results/release-profile.md` is the full record — what was measured, what
was tried and found worthless, and where the remaining gap is.

## Performance work

Current standing, single-threaded over those 200 deals: the reference 21,630 ms,
this port ~22,720 ms, DDS 2.9 20,200 ms. With identical node counts, **~1.05x is
pure per-node cost** with nothing else mixed in.

The port's figure is the 23,480 ms recorded on 2026-09-01 scaled by the 0.968
measured for the `convert_suit` change (2026-09-02, alternating whole runs on a
busier machine: 23.90 s against 23.13 s, best of four rounds each). It is not a
fresh absolute measurement, and the three numbers were not taken in one
sitting — re-measure all three together before quoting the ratio anywhere it
matters.

Before optimising, read the "tried and found worthless" list in
`release-profile.md`. Hoisting the per-node atomics, deleting ~4,000
instructions of never-executed tracing, `-C target-cpu=native` and hoisting a
redundant cache-key hash all measured as exactly nothing.

Measurement discipline that this repo has learned the hard way:

- On a machine that is not idle, same-binary repeats swing 4–6% on the geometric
  mean. Build both binaries, keep both, and alternate them A/B/A/B. A single
  before/after pair has produced a confident wrong answer here more than once.
- **A/B on the full corpus, not `--quick`.** `run --no-sweep --runs 5` is ~5 s,
  so eight interleaved rounds a side is a couple of minutes, and the ten paired
  per-board ratios are what make a result attributable — a real change moves
  every board by roughly the same fraction, and noise does not. `--quick` is one
  board and so one ratio. Its board choice used to be an unrepeated timing that
  flipped between two boards under load, averaging two workloads into one
  number; it now ranks by nodes searched and is stable, but `--quick` numbers
  from before that change compare only with each other. See "Two ways the
  harness used to mislead you" in `release-profile.md`.
- `perf` does not exist on macOS. Use `samply record`, or Instruments' CPU
  Counters template, for a profile.
