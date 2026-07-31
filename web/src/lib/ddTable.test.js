import { describe, expect, it } from 'vitest'
import { buildDdRows, collapseDdRows, contractTarget, DISPLAY_SEATS } from './ddTable.js'
import { seatAtPosition } from './cards.js'

// The engine's table for the verified deal: rows N,E,S,W, columns C,D,H,S,NT.
// N and S take the same tricks, as do E and W — the usual case.
const TRICKS = [
  [8, 7, 8, 10, 9], // N
  [5, 6, 4, 3, 4], // E
  [8, 7, 8, 10, 9], // S
  [5, 6, 4, 3, 4], // W
]

describe('buildDdRows', () => {
  it('reorders the engine rows into reading order', () => {
    const rows = buildDdRows(TRICKS)
    expect(rows.map((r) => r.seat)).toEqual(DISPLAY_SEATS)
    expect(DISPLAY_SEATS).toEqual(['N', 'S', 'E', 'W'])
  })

  it('keeps each seat cells with that seat', () => {
    const rows = buildDdRows(TRICKS)
    const bySeat = Object.fromEntries(rows.map((r) => [r.seat, r.cells.map((c) => c.tricks)]))
    expect(bySeat.N).toEqual([8, 7, 8, 10, 9])
    expect(bySeat.S).toEqual([8, 7, 8, 10, 9])
    expect(bySeat.E).toEqual([5, 6, 4, 3, 4])
    expect(bySeat.W).toEqual([5, 6, 4, 3, 4])
  })

  it('marks the cell the contract lands on', () => {
    const rows = buildDdRows(TRICKS, { contract: '4S', declarer: 'N' })
    const marked = rows.flatMap((r) =>
      r.cells.map((c, i) => (c.isContract ? `${r.seat}${i}` : null)).filter(Boolean)
    )
    // Spades is display column 3.
    expect(marked).toEqual(['N3'])
  })

  it('reads both notrump spellings', () => {
    for (const contract of ['3N', '3NT']) {
      const rows = buildDdRows(TRICKS, { contract, declarer: 'W' })
      const cell = rows.find((r) => r.seat === 'W').cells[4]
      expect(cell.isContract, contract).toBe(true)
    }
  })

  it('ignores doubling when placing the contract', () => {
    const rows = buildDdRows(TRICKS, { contract: '5CXX', declarer: 'E' })
    expect(rows.find((r) => r.seat === 'E').cells[0].isContract).toBe(true)
  })

  it('marks nothing when there is no contract or declarer', () => {
    const none = (opts) =>
      buildDdRows(TRICKS, opts).every((r) => r.cells.every((c) => !c.isContract))
    expect(none({})).toBe(true)
    expect(none({ contract: '4S' })).toBe(true)
    expect(none({ declarer: 'N' })).toBe(true)
    expect(none({ contract: 'Pass', declarer: 'N' })).toBe(true)
  })

  /*
   * A partial table would render as a grid of confident zeros — a table claiming
   * every contract makes nothing. No table is the honest rendering.
   */
  it('returns null for a table it cannot trust', () => {
    expect(buildDdRows(null)).toBeNull()
    expect(buildDdRows([])).toBeNull()
    expect(buildDdRows([[1, 2, 3, 4, 5]])).toBeNull()
    expect(buildDdRows([[1, 2, 3], [1, 2, 3], [1, 2, 3], [1, 2, 3]])).toBeNull()
  })
})

describe('collapseDdRows', () => {
  it('merges a partnership whose tricks match', () => {
    const rows = collapseDdRows(buildDdRows(TRICKS))
    expect(rows.map((r) => r.seat)).toEqual(['NS', 'EW'])
    expect(rows[0].cells.map((c) => c.tricks)).toEqual([8, 7, 8, 10, 9])
    expect(rows[1].cells.map((c) => c.tricks)).toEqual([5, 6, 4, 3, 4])
  })

  it('keeps all four rows when a pair differs', () => {
    const uneven = [
      [8, 7, 8, 10, 9], // N
      [5, 6, 4, 3, 4], // E
      [8, 7, 8, 10, 8], // S — one cell apart from N
      [5, 6, 4, 3, 4], // W
    ]
    const rows = collapseDdRows(buildDdRows(uneven))
    // NS cannot merge; EW still can, and they are independent.
    expect(rows.map((r) => r.seat)).toEqual(['N', 'S', 'EW'])
  })

  it('carries the contract highlight through a merge', () => {
    const rows = collapseDdRows(buildDdRows(TRICKS, { contract: '4S', declarer: 'N' }))
    const ns = rows.find((r) => r.seat === 'NS')
    expect(ns.cells[3].isContract).toBe(true)
  })

  it('passes null through', () => {
    expect(collapseDdRows(null)).toBeNull()
  })
})

describe('contractTarget', () => {
  it('is the level plus the book', () => {
    expect(contractTarget('1S')).toBe(7)
    expect(contractTarget('3NT')).toBe(9)
    expect(contractTarget('6C')).toBe(12)
    expect(contractTarget('7NT')).toBe(13)
  })

  it('is null when there is no level to read', () => {
    expect(contractTarget('')).toBeNull()
    expect(contractTarget('Pass')).toBeNull()
    expect(contractTarget(null)).toBeNull()
  })
})

describe('seatAtPosition', () => {
  it('leaves the compass alone by default', () => {
    for (const seat of ['N', 'E', 'S', 'W']) {
      expect(seatAtPosition(seat, null)).toBe(seat)
      expect(seatAtPosition(seat, 'S')).toBe(seat)
    }
  })

  it('puts the named seat at the bottom', () => {
    expect(seatAtPosition('S', 'W')).toBe('W')
    expect(seatAtPosition('S', 'N')).toBe('N')
    expect(seatAtPosition('S', 'E')).toBe('E')
  })

  /*
   * The property that makes turning the table safe rather than confusing: every
   * seat moves by the same number of steps, so going clockwise round the screen
   * still goes clockwise round the table. Each hand keeps its true LHO on its left
   * and its partner opposite.
   */
  it('preserves the clockwise order and so the geography', () => {
    for (const south of ['N', 'E', 'S', 'W']) {
      const order = ['N', 'E', 'S', 'W'].map((p) => seatAtPosition(p, south))
      // A rotation of the seating, not a reshuffle.
      expect(new Set(order).size).toBe(4)
      for (let i = 0; i < 4; i += 1) {
        const seat = order[i]
        const clockwise = order[(i + 1) % 4]
        const trueNext = { N: 'E', E: 'S', S: 'W', W: 'N' }[seat]
        expect(clockwise, `${south}: ${seat} clockwise`).toBe(trueNext)
      }
      // Partners stay opposite.
      expect({ N: 'S', S: 'N', E: 'W', W: 'E' }[order[0]]).toBe(order[2])
    }
  })

  it('ignores nonsense rather than dropping a hand', () => {
    expect(seatAtPosition('S', 'X')).toBe('S')
    expect(seatAtPosition('X', 'W')).toBe('X')
  })
})
