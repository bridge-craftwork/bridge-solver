# The Bridge Composer oracle

Three files, and a human round trip that cannot be repeated by CI. They are the
reason this repository knows where Bridge Composer puts a supplemental tag, how
wide it writes the `OptimumResultTable` header, and what it means when a
`ParContract` names a single seat rather than a side.

| File | What it is |
|------|------------|
| `pbn-order-test.pbn` | The input. Eight boards, hand-written here. |
| `pbn-order-test-bc.pbn` | The same file after Bridge Composer 5.118.2 opened it, took a trivial edit, and saved. |
| `pbn-order-test-bc-dd.pbn` | The same again, after Bridge Composer additionally ran *its own* double-dummy analysis. |

All three are CRLF, which is what Bridge Composer writes; `.gitattributes`
marks them `-text` so git leaves them alone. Do not reformat them, do not
re-wrap them, and do not let an editor strip the carriage returns — a diff
against these files is only meaningful byte for byte.

## How they were obtained

`pbn-order-test.pbn` was written by hand to be maximally awkward:

- Every board carries all 15 mandatory tags, **deliberately shuffled** into a
  different arrangement on each board.
- Each board puts the four double-dummy tags somewhere different, and says so
  in a `; TEST n:` comment — above the auction, below it, split around it,
  reverse-alphabetical, wedged mid-block, and so on.
- Board 7 brackets the DD tags with custom one-line tags `AAACustom` and
  `ZZZCustom`, which Bridge Composer has never heard of.
- Board 8 carries a custom *section*, `AAATable`, whose name sorts before
  `OptimumResultTable`.

That file was then opened in Bridge Composer 5.118.2 on macOS, edited trivially,
and saved as `pbn-order-test-bc.pbn`; Bridge Composer's own DD analysis was then
run over it and the result saved as `pbn-order-test-bc-dd.pbn`. **A GUI did
that.** Nothing in this repository can regenerate these files, which is why they
are committed rather than produced by a script.

## What each proves

**The control: Bridge Composer normalises, it does not copy.** Every board comes
back with its 15 mandatory tags in the standard's fixed order —
`Event, Site, Date, Board, West, North, East, South, Dealer, Vulnerable, Deal,
Scoring, Declarer, Contract, Result` — however they were shuffled going in.
Without that, none of the placement below would be evidence of anything: it
could all have been the input order surviving.

**The tag layout.** In every board of the output, in this order:

1. the 15 mandatory tags, in the standard's order;
2. supplemental **tag pairs**, sorted alphabetically — including tags Bridge
   Composer does not know. `AAACustom` sorts above `BCFlags` and
   `DoubleDummyTricks`, `ZZZCustom` below `ParContract`, on board 7, whatever
   order they were written in;
3. `[Auction]` and its calls;
4. `[Play]` and its cards;
5. supplemental **sections**, sorted alphabetically among themselves — board 8's
   custom `AAATable` comes out before `OptimumResultTable`, and both come out
   after the auction even though `AAATable` was written above it.

So `DoubleDummyTricks`, `OptimumScore` and `ParContract` are group 2 and
`OptimumResultTable` is group 5: the one-liners go up among the identification
tags, the twenty-row table goes to the bottom. This repository wrote all four
above `[Auction]` until the change that added these fixtures.

**The `Result` column width.** Bridge Composer writes
`Result\1R` on a board where no declarer takes ten tricks and `Result\2R` where
one does — four boards of each here, no exceptions — and pads the twenty data
rows to the width it declared (`N NT 5` against `N NT  9`).
`bridge-encodings`' `optimum_result_table_header(&DdTable)` and
`optimum_result_table_rows(&DdTable)` are that rule, from one shared width.

**Two independent solvers agree.** `pbn-order-test.pbn`'s double-dummy values
were produced by this solver. Bridge Composer *recomputed* them rather than
copying them — it rewrote the header widths and it recomputed par, so it was
not passing our strings through — and every one came back unchanged: all 8
`DoubleDummyTricks` strings, all 160 `OptimumResultTable` cells, all 8
`OptimumScore` values, byte for byte.

**`ParContract` is not `OptimumScore` with a contract attached.** The scores
agreed on all 8 boards; the contracts did not, in two distinct ways.

| Board | We wrote | Bridge Composer |
|-------|----------|-----------------|
| 1 | `EW 2SX-1` | `EW 2SX-1; EW 3CX-1` |
| 5 | `NS 3N=` | `N 3N=` |
| 6 | `EW 2S=` | `E 2S=` |
| 7 | `NS 4H=` | `NS 4H=; NS 4S=` |

- **Ties are all listed**, separated by `"; "`. Board 1's two sacrifices both
  cost 100; board 7's two games both score 420.
- **A single seat is named when only one partner can take the tricks.** Board 5:
  North makes nine at notrump and South only eight, so only North can declare
  `3N=`. Board 6: East takes eight spades, West seven. When both partners take
  the same number the side is named, which is why board 1 and board 7 stay `EW`
  and `NS`.

## What we still do not match, and why

Comparing our `--recalculate` output against `pbn-order-test-bc-dd.pbn` is the
best end-to-end check available, but perfect equality is not the goal:

- Bridge Composer reorders the 15 mandatory tags. We deliberately do not touch
  tags we did not write.
- It adds a `%`-directive preamble of its own settings, a leading template
  board, and a `[BCFlags]` tag to every board.
- It rewrites `; comment` lines as `{ ... }` commentary, and moves them.
- It moves a *section* the board already carried: board 8's `AAATable`, written
  above the auction, comes back below it. We place what we write and leave the
  rest where it was.

What must match, and does — byte for byte, on all eight boards, checked by
`the_bridge_composer_fixture_round_trips`: the `DoubleDummyTricks`,
`OptimumScore` and `ParContract` values; their placement relative to
`[Auction]` and `[Play]`; and the whole `OptimumResultTable`, header width and
all twenty rows.
