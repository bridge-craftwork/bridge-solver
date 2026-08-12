# bridge-solver

A fast double-dummy solver for bridge, with par-contract scoring, cardplay
analysis, PBN file processing, and a WebAssembly build for the browser.

## Attribution and License

The double-dummy solver core is a Rust reimplementation of
[macroxue/bridge-solver](https://github.com/macroxue/bridge-solver), which is
licensed GPL-2.0. Copyright in that work is held by its authors. This work is
therefore GPL-2.0-only — see [LICENSE](LICENSE).

GPL-2.0-only rather than "or later": upstream ships a bare copy of the GPL-2.0
text with no "or (at your option) any later version" notice applied to the work,
so there is no or-later grant to pass on.

The search engine corresponds to upstream commit
[`75b4619`](https://github.com/macroxue/bridge-solver/commit/75b4619).

### Not derived from upstream

The following are original work with no upstream counterpart:

- `src/par.rs` — par contract and optimum score calculation
- `src/analyse_play.rs` — double-dummy cardplay analysis
- `wasm/` — WebAssembly bindings and LIN parsing
- `web/` — the browser application
- PBN input/output, and the `bin/bridge-solver` PBN-processing binary
