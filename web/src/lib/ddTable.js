// Turning a solved double-dummy table into display rows.
//
// Vendored from Bridge-Classroom's `utils/handAnalysis.js` (`buildDdRows`,
// `collapseDdRows`), adapted at one point: the classroom decodes the 20-character
// `ddtricks` interchange string, while this engine returns the table already
// structured. The row and column orders of the output are identical, so the
// component is otherwise the same.
//
// Note the two orders differ and both matter. The engine returns rows in
// `N, E, S, W`; a double-dummy table is always *read* `N, S, E, W`, partners
// adjacent, which is what makes the collapse below legible.

/** Row order of the engine's table. */
const ENGINE_SEATS = ['N', 'E', 'S', 'W']

/** Row order of the display: partners adjacent. */
export const DISPLAY_SEATS = ['N', 'S', 'E', 'W']

/** Column order, low strain first — the engine's, and the display's. */
export const DISPLAY_STRAINS = ['C', 'D', 'H', 'S', 'NT']

/**
 * Build the display grid, marking the cell the contract lands on.
 *
 * `tricks` is the engine's `tricks[seat][strain]` with rows [`ENGINE_SEATS`] and
 * columns [`DISPLAY_STRAINS`]. Returns `null` for a missing or malformed table:
 * a partial one would render as a grid of confident zeros, and no table is the
 * honest rendering.
 */
export function buildDdRows(tricks, { contract = '', declarer = '' } = {}) {
  if (!Array.isArray(tricks) || tricks.length !== 4) return null
  if (!tricks.every((row) => Array.isArray(row) && row.length === 5)) return null

  const strainIdx = contractStrainIndex(contract)
  const declarerSeat = (declarer || '').toUpperCase()

  return DISPLAY_SEATS.map((seat) => {
    const row = tricks[ENGINE_SEATS.indexOf(seat)]
    return {
      seat,
      cells: row.map((n, ci) => ({
        tricks: n,
        isContract: seat === declarerSeat && ci === strainIdx,
      })),
    }
  })
}

/**
 * Merge a partnership's rows when their trick counts match — `N`+`S` → `NS`.
 *
 * Lossless by construction: a pair only merges when every cell already agrees, so
 * nothing is hidden, and it halves the height in the common case. When they
 * differ — the interesting case, and the one worth looking at — all four rows
 * stay.
 *
 * `isContract` survives as the OR of the pair, so a merged highlight means "this
 * partnership, this strain". The declarer is named beside the table, so the seat
 * is not lost from the display as a whole.
 */
export function collapseDdRows(rows) {
  if (!rows) return rows
  const bySeat = Object.fromEntries(rows.map((r) => [r.seat, r]))

  const same = (a, b) =>
    a && b && a.cells.length === b.cells.length && a.cells.every((c, i) => c.tricks === b.cells[i].tricks)

  const merge = (a, b, seat) => ({
    seat,
    cells: a.cells.map((c, i) => ({
      tricks: c.tricks,
      isContract: c.isContract || b.cells[i].isContract,
    })),
  })

  const out = []
  if (same(bySeat.N, bySeat.S)) out.push(merge(bySeat.N, bySeat.S, 'NS'))
  else {
    if (bySeat.N) out.push(bySeat.N)
    if (bySeat.S) out.push(bySeat.S)
  }
  if (same(bySeat.E, bySeat.W)) out.push(merge(bySeat.E, bySeat.W, 'EW'))
  else {
    if (bySeat.E) out.push(bySeat.E)
    if (bySeat.W) out.push(bySeat.W)
  }
  return out
}

/** Which display column a contract's strain sits in, or -1. */
function contractStrainIndex(contract) {
  const m = String(contract || '')
    .trim()
    .match(/^[1-7]\s*(NT?|[CDHS])/i)
  if (!m) return -1
  const raw = m[1].toUpperCase()
  return DISPLAY_STRAINS.indexOf(raw === 'N' ? 'NT' : raw)
}

/** Tricks the contract needs, for reading a cell as made or beaten. */
export function contractTarget(contract) {
  const m = String(contract || '')
    .trim()
    .match(/^([1-7])/)
  return m ? Number(m[1]) + 6 : null
}
