import { describe, expect, it } from 'vitest'
import {
  formatBid,
  formatCard,
  getSuitClass,
  normalizeCardCode,
  parseCardCode,
  seatAtIndex,
  sideOf,
  sortSuitDescending,
} from './cards.js'
import { encodeDdTricks } from './solver.js'

describe('getSuitClass', () => {
  it('colours hearts and diamonds red, spades and clubs black', () => {
    expect(getSuitClass('hearts')).toBe('suit-red')
    expect(getSuitClass('diamonds')).toBe('suit-red')
    expect(getSuitClass('spades')).toBe('suit-black')
    expect(getSuitClass('clubs')).toBe('suit-black')
  })

  it('takes a letter as well as a name', () => {
    expect(getSuitClass('H')).toBe('suit-red')
    expect(getSuitClass('S')).toBe('suit-black')
  })
})

describe('formatCard', () => {
  it('spells the ten out and leaves everything else alone', () => {
    expect(formatCard('T')).toBe('10')
    expect(formatCard('A')).toBe('A')
    expect(formatCard('2')).toBe('2')
  })
})

describe('normalizeCardCode', () => {
  // Everything keys on this, so a disagreement means a badge silently misses.
  it('produces one spelling from the several that occur', () => {
    expect(normalizeCardCode('hk')).toBe('HK')
    expect(normalizeCardCode('HK')).toBe('HK')
    expect(normalizeCardCode(' dt ')).toBe('DT')
    expect(normalizeCardCode('D10')).toBe('DT')
    expect(normalizeCardCode('d10')).toBe('DT')
  })
})

describe('parseCardCode', () => {
  it('splits a code into suit and rank', () => {
    expect(parseCardCode('HK')).toEqual({ suit: 'H', rank: 'K' })
    expect(parseCardCode('d10')).toEqual({ suit: 'D', rank: 'T' })
  })
})

describe('sortSuitDescending', () => {
  it('orders a holding high to low', () => {
    expect(sortSuitDescending(['3', 'A', 'T', 'K'])).toEqual(['A', 'K', 'T', '3'])
  })

  it('accepts a ten written either way', () => {
    expect(sortSuitDescending(['10', 'A'])).toEqual(['A', 'T'])
  })

  it('does not mutate its input', () => {
    const input = ['3', 'A']
    sortSuitDescending(input)
    expect(input).toEqual(['3', 'A'])
  })
})

describe('formatBid', () => {
  it('renders a suit bid with a coloured symbol', () => {
    expect(formatBid('2H')).toEqual({ text: '2♥', html: '2<span class="red">♥</span>' })
    expect(formatBid('1S')).toEqual({ text: '1♠', html: '1<span class="black">♠</span>' })
  })

  it('renders notrump as letters, both spellings', () => {
    expect(formatBid('3N').text).toBe('3NT')
    expect(formatBid('3NT').text).toBe('3NT')
  })

  it('renders the non-bids', () => {
    expect(formatBid('Pass').text).toBe('Pass')
    expect(formatBid('p').text).toBe('Pass')
    expect(formatBid('X').text).toBe('X')
    expect(formatBid('XX').text).toBe('XX')
  })

  it('passes anything unrecognised through untouched', () => {
    expect(formatBid('???').text).toBe('???')
  })
})

describe('seatAtIndex', () => {
  it('walks clockwise', () => {
    expect(seatAtIndex('N', 1)).toBe('E')
    expect(seatAtIndex('E', 1)).toBe('S')
    expect(seatAtIndex('S', 1)).toBe('W')
    expect(seatAtIndex('W', 1)).toBe('N')
  })

  it('wraps past four', () => {
    expect(seatAtIndex('N', 4)).toBe('N')
    expect(seatAtIndex('N', 5)).toBe('E')
  })

  it('gives partner at two', () => {
    expect(seatAtIndex('N', 2)).toBe('S')
  })
})

describe('sideOf', () => {
  it('pairs the seats', () => {
    expect(sideOf('N')).toBe('NS')
    expect(sideOf('S')).toBe('NS')
    expect(sideOf('E')).toBe('EW')
    expect(sideOf('W')).toBe('EW')
  })
})

describe('encodeDdTricks', () => {
  /*
   * Conformance against an independently verified fixture.
   *
   * `9a8789a8784346543465` is Bridge-Classroom's DD test vector for the deal
   * N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72,
   * and this engine's CLI emits the same 20 characters for it. Encoding the
   * decoded table back to that string proves the row and column orders here
   * match the interchange format, which is the part easiest to get subtly wrong:
   * the string is seat-major over N,S,E,W with strains NT,S,H,D,C, while the
   * decoded table is N,E,S,W with strains C,D,H,S,NT — both orders differ.
   */
  it('reproduces the verified ddtricks string', () => {
    // Rows N,E,S,W; columns C,D,H,S,NT — as the engine returns them.
    const tricks = [
      [8, 7, 8, 10, 9], // N
      [5, 6, 4, 3, 4], // E
      [8, 7, 8, 10, 9], // S
      [5, 6, 4, 3, 4], // W
    ]
    expect(encodeDdTricks(tricks)).toBe('9a8789a8784346543465')
  })

  it('encodes ten and above as letters', () => {
    const all13 = Array.from({ length: 4 }, () => [13, 13, 13, 13, 13])
    expect(encodeDdTricks(all13)).toBe('d'.repeat(20))
    const all10 = Array.from({ length: 4 }, () => [10, 10, 10, 10, 10])
    expect(encodeDdTricks(all10)).toBe('a'.repeat(20))
  })
})
