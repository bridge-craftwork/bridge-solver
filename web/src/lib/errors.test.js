import { describe, expect, it } from 'vitest'
import {
  blameFor,
  correctionStart,
  dummyOf,
  isDeclaringSide,
  signedEffect,
  suitVerdict,
  summariseCosts,
  trickTotalFrom,
} from './errors.js'

/*
 * The verified board: 3NT by West, claimed after 41 cards. Double-dummy gives
 * declarer 8; the five costed errors are North ♣3 (t1), East ♦Q (t2), South ♣9
 * (t5), East ♦10 (t6) and West ♥7 (t7). East is dummy.
 */
const TRACE = [
  { index: 0, seat: 'N', card: 'C3', cost: 1 },
  { index: 4, seat: 'E', card: 'DQ', cost: 1 },
  { index: 16, seat: 'S', card: 'C9', cost: 1 },
  { index: 20, seat: 'E', card: 'DT', cost: 1 },
  { index: 26, seat: 'W', card: 'H7', cost: 1 },
  { index: 1, seat: 'S', card: 'CT', cost: 0 },
]

describe('sides and blame', () => {
  it('makes dummy declarer partner', () => {
    expect(dummyOf('W')).toBe('E')
    expect(dummyOf('N')).toBe('S')
    expect(dummyOf(null)).toBeNull()
  })

  it('counts declarer and dummy as the declaring side', () => {
    expect(isDeclaringSide('W', 'W')).toBe(true)
    expect(isDeclaringSide('E', 'W')).toBe(true)
    expect(isDeclaringSide('N', 'W')).toBe(false)
    expect(isDeclaringSide('S', 'W')).toBe(false)
  })

  it('is false with no declarer rather than guessing', () => {
    expect(isDeclaringSide('N', null)).toBe(false)
  })

  it('charges dummy cards to declarer and leaves the rest alone', () => {
    expect(blameFor('E', 'W')).toBe('W')
    expect(blameFor('W', 'W')).toBe('W')
    expect(blameFor('N', 'W')).toBe('N')
    // With no declarer there is nobody to charge it to.
    expect(blameFor('E', null)).toBe('E')
  })
})

describe('signedEffect', () => {
  it('is negative for the declaring side and positive for the defence', () => {
    expect(signedEffect({ seat: 'W', cost: 1 }, 'W')).toBe(-1)
    // Dummy's card is declarer's mistake, so it moves the same way.
    expect(signedEffect({ seat: 'E', cost: 1 }, 'W')).toBe(-1)
    expect(signedEffect({ seat: 'N', cost: 1 }, 'W')).toBe(1)
    expect(signedEffect({ seat: 'S', cost: 2 }, 'W')).toBe(2)
  })
})

describe('summariseCosts', () => {
  const summary = summariseCosts(TRACE, 'W')

  it('splits the two sides', () => {
    // Declarer's own ♥7 plus dummy's ♦Q and ♦10.
    expect(summary.declarerCost).toBe(3)
    expect(summary.defenderCost).toBe(2)
    expect(summary.total).toBe(5)
    expect(summary.errors).toBe(5)
  })

  it('nets the two against each other', () => {
    expect(summary.net).toBe(-1)
  })

  /*
   * The reconciliation that makes the numbers trustworthy: double-dummy from the
   * opening lead was 8, declarer gave away 3 and the defence handed back 2, so 7
   * were taken — which is exactly what was claimed on this board.
   */
  it('reconciles to the tricks actually taken', () => {
    expect(trickTotalFrom(8, summary)).toBe(7)
  })

  it('has nothing to reconcile without a double-dummy result', () => {
    expect(trickTotalFrom(null, summary)).toBeNull()
  })

  it('ignores cards that cost nothing', () => {
    const clean = summariseCosts([{ seat: 'N', cost: 0 }], 'W')
    expect(clean).toMatchObject({ errors: 0, declarerCost: 0, defenderCost: 0, net: 0 })
  })

  it('copes with an empty or missing trace', () => {
    expect(summariseCosts([], 'W').total).toBe(0)
    expect(summariseCosts(null, 'W').total).toBe(0)
  })
})

describe('suitVerdict', () => {
  it('blames the card when something in the suit would have worked', () => {
    // West held AK97 and played the 7; the ace or king cost nothing.
    const node = {
      card: 'H7',
      cost: 1,
      alternatives: [
        { card: 'HA', tricks: 8, cost: 0 },
        { card: 'HK', tricks: 8, cost: 0 },
        { card: 'H9', tricks: 7, cost: 1 },
        { card: 'H7', tricks: 7, cost: 1 },
      ],
    }
    expect(suitVerdict(node)).toBe('card')
  })

  it('blames the suit when every card in it gave a trick away', () => {
    const node = {
      card: 'D5',
      cost: 1,
      alternatives: [
        { card: 'D5', tricks: 7, cost: 1 },
        { card: 'D9', tricks: 7, cost: 1 },
        // Another suit was fine, which is what makes the diamond choice the error.
        { card: 'SA', tricks: 8, cost: 0 },
      ],
    }
    expect(suitVerdict(node)).toBe('suit')
  })

  it('judges nothing when the card cost nothing', () => {
    const node = {
      card: 'HA',
      cost: 0,
      alternatives: [{ card: 'HA', tricks: 8, cost: 0 }],
    }
    expect(suitVerdict(node)).toBeNull()
  })

  it('judges nothing without alternatives to compare', () => {
    expect(suitVerdict(null)).toBeNull()
    expect(suitVerdict({ card: 'HA', cost: 1, alternatives: [] })).toBeNull()
    expect(suitVerdict({ card: 'HA', cost: 1 })).toBeNull()
  })

  it('only considers the suit actually played', () => {
    // A zero-cost card exists, but in a different suit — the played suit was still
    // wrong, so this must not read as a card-level mistake.
    const node = {
      card: 'D5',
      cost: 2,
      alternatives: [
        { card: 'D5', tricks: 6, cost: 2 },
        { card: 'SA', tricks: 8, cost: 0 },
      ],
    }
    expect(suitVerdict(node)).toBe('suit')
  })
})

describe('correctionStart', () => {
  /*
   * The reported case: the mistake was the *fourth* card of trick one, and
   * correcting from the trick boundary replaced the opening lead — which changes
   * the hand rather than correcting it. The lead and the two cards after it were
   * all fine and must stand.
   */
  const trace = [
    { index: 0, cost: 0 },
    { index: 1, cost: 0 },
    { index: 2, cost: 0 },
    { index: 3, cost: 1 }, // fourth card of trick 1
    { index: 9, cost: 2 }, // second card of trick 3
  ]

  it('starts on the offending card, not the top of its trick', () => {
    expect(correctionStart(trace, 0)).toBe(3)
  })

  it('keeps playing past a clean trick to the next mistake', () => {
    // Trick 2 (indices 4–7) has nothing wrong in it, so pointing there carries
    // on to the mistake in trick 3 rather than rewriting a well-played trick.
    expect(correctionStart(trace, 4)).toBe(9)
  })

  it('takes a mistake that is already at the anchor', () => {
    expect(correctionStart(trace, 3)).toBe(3)
  })

  it('returns the anchor when nothing later was a mistake', () => {
    // Already double-dummy perfect from there, so there is nothing to correct
    // and playing it out from the anchor reproduces the same cards.
    expect(correctionStart(trace, 10)).toBe(10)
  })

  it('survives a missing or empty trace', () => {
    expect(correctionStart(undefined, 4)).toBe(4)
    expect(correctionStart([], 4)).toBe(4)
  })

  it('ignores entries that cost nothing', () => {
    expect(correctionStart([{ index: 2, cost: 0 }], 0)).toBe(0)
  })

  /*
   * A trace is not promised in play order — TRACE above is deliberately not —
   * so this must take the earliest qualifying card rather than whichever the
   * engine happened to list first. Getting this wrong corrects from a later
   * mistake and silently leaves the real one in the line.
   */
  it('takes the earliest mistake even when the trace is out of order', () => {
    const jumbled = [
      { index: 26, cost: 1 },
      { index: 3, cost: 1 },
      { index: 9, cost: 2 },
    ]
    expect(correctionStart(jumbled, 0)).toBe(3)
    expect(correctionStart(jumbled, 4)).toBe(9)
    expect(correctionStart(jumbled, 10)).toBe(26)
  })

  it('works against the real board fixture', () => {
    // North ♣3 at index 0, then East ♦Q at 4, South ♣9 at 16, East ♦10 at 20.
    expect(correctionStart(TRACE, 0)).toBe(0)
    expect(correctionStart(TRACE, 1)).toBe(4)
    expect(correctionStart(TRACE, 17)).toBe(20)
  })
})
