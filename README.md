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

## Annotating PBN files

The `bridge-solver` binary (build with `--features cli`) fills in double-dummy
and par analysis on PBN files, writing Bridge Composer compatible tags:
`DoubleDummyTricks`, `OptimumResultTable`, and — when the board states its
vulnerability — `OptimumScore` and `ParContract`.

```sh
bridge-solver -i deals.pbn                 # analyze one file to stdout
bridge-solver -i deals.pbn -o out.pbn      # ...or to another file
bridge-solver -w -i Curated/               # annotate a tree in place
```

Directories are searched recursively for `*.pbn`, so a build can annotate a
whole collection in one command with no scripting around it. The pass is
designed to be safe to run over source material:

- **Only what is missing.** A board that already has a `DoubleDummyTricks` tag
  is passed through byte-for-byte. Use `--recalculate` to redo those too.
- **Nothing else is touched.** Files are edited line by line rather than
  reparsed and rewritten, so `%` directives (Bridge Composer's fonts, page
  setup and colours), `;` comments, and hand-authored `{...}` commentary all
  survive exactly as written. Annotating a collection only ever adds lines.
- **Incomplete deals are skipped.** Auction-only teaching boards, written as
  `[Deal "N:... ... ... ..."]`, parse into empty hands; they are left alone
  rather than stamped with a fabricated all-zero table.
- **`--mark-verified`** sets bit `0x00080000` ("double-dummy data has been
  verified") in each annotated board's `[BCFlags]`, adding the tag if absent and
  preserving every bit already there. This records provenance only — no
  documented BCFlags bit controls whether the DD table is *displayed*, and
  Bridge Composer does not set this bit itself when it computes a table.
- **Re-runs are no-ops.** Unchanged files are not rewritten, so mtimes do not
  churn. In-place writes go through a temporary file and a rename.
