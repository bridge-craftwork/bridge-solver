import { describe, expect, it } from 'vitest'
import {
  handToCodes,
  remainingHands,
  replay,
  trickNumberOf,
  trickStartOf,
  trickWinner,
  tricksTaken,
  trumpFromContract,
} from './cardplay.js'
import { parseDealString } from './deal.js'

describe('trick arithmetic', () => {
  it('numbers tricks from one', () => {
    expect(trickNumberOf(0)).toBe(1)
    expect(trickNumberOf(3)).toBe(1)
    expect(trickNumberOf(4)).toBe(2)
    expect(trickNumberOf(51)).toBe(13)
  })

  it('finds the start of a trick', () => {
    expect(trickStartOf(0)).toBe(0)
    expect(trickStartOf(3)).toBe(0)
    expect(trickStartOf(4)).toBe(4)
    expect(trickStartOf(7)).toBe(4)
  })
})

describe('trumpFromContract', () => {
  it('reads the strain', () => {
    expect(trumpFromContract('4H')).toBe('H')
    expect(trumpFromContract('6C')).toBe('C')
    expect(trumpFromContract('1S')).toBe('S')
    expect(trumpFromContract('3D')).toBe('D')
  })

  it('returns null for notrump, both spellings', () => {
    expect(trumpFromContract('3N')).toBeNull()
    expect(trumpFromContract('3NT')).toBeNull()
  })

  it('ignores doubling', () => {
    expect(trumpFromContract('5CX')).toBe('C')
    expect(trumpFromContract('4SXX')).toBe('S')
  })

  it('returns null rather than throwing on nonsense', () => {
    expect(trumpFromContract('')).toBeNull()
    expect(trumpFromContract(null)).toBeNull()
    expect(trumpFromContract('Pass')).toBeNull()
    expect(trumpFromContract('9H')).toBeNull()
  })
})

describe('trickWinner', () => {
  const entry = (seat, code) => ({ seat, suit: code[0], rank: code.slice(1) })

  it('gives the trick to the highest card of the suit led', () => {
    const t = [entry('W', 'H3'), entry('N', 'HK'), entry('E', 'H4'), entry('S', 'HA')]
    expect(trickWinner(t, null)).toBe('S')
  })

  it('lets a trump beat the suit led', () => {
    const t = [entry('W', 'HA'), entry('N', 'H3'), entry('E', 'S2'), entry('S', 'H4')]
    expect(trickWinner(t, 'S')).toBe('E')
  })

  it('gives it to the highest trump when several are played', () => {
    const t = [entry('W', 'HA'), entry('N', 'S9'), entry('E', 'S2'), entry('S', 'SJ')]
    expect(trickWinner(t, 'S')).toBe('S')
  })

  it('ignores a discard in a third suit', () => {
    const t = [entry('W', 'H3'), entry('N', 'D2'), entry('E', 'C9'), entry('S', 'H4')]
    expect(trickWinner(t, 'S')).toBe('S')
  })

  it('does not treat a trump lead as an overruff case', () => {
    const t = [entry('W', 'SK'), entry('N', 'S3'), entry('E', 'S2'), entry('S', 'S4')]
    expect(trickWinner(t, 'S')).toBe('W')
  })
})

describe('replay', () => {
  // Contract 1NT by North; East leads. Two complete tricks.
  const plays = ['D8', 'DK', 'DA', 'D2', 'H2', 'H4', 'HJ', 'HQ']

  it('attributes each card to the seat that played it', () => {
    const { seatOf } = replay(plays, 'E', null)
    // First trick starts with the leader and goes clockwise.
    expect(seatOf.slice(0, 4)).toEqual(['E', 'S', 'W', 'N'])
  })

  it('makes the trick winner lead the next one', () => {
    const { tricks, seatOf } = replay(plays, 'E', null)
    // DA wins trick one for West, so West leads trick two.
    expect(tricks[0].winner).toBe('W')
    expect(tricks[1].leader).toBe('W')
    expect(seatOf[4]).toBe('W')
  })

  it('collects each seat cards in play order', () => {
    const { bySeat } = replay(plays, 'E', null)
    expect(bySeat.E).toEqual(['D8', 'HJ'])
    expect(bySeat.W).toEqual(['DA', 'H2'])
  })

  it('handles a trailing partial trick without inventing a winner', () => {
    // Nine cards: two full tricks then one card, which is what a claim leaves.
    const { tricks } = replay([...plays, 'S5'], 'E', null)
    expect(tricks).toHaveLength(3)
    expect(tricks[2].complete).toBe(false)
    expect(tricks[2].winner).toBeNull()
    expect(tricks[2].plays).toHaveLength(1)
  })

  it('copes with an empty record', () => {
    const { tricks, bySeat } = replay([], 'E', null)
    expect(tricks).toEqual([])
    expect(bySeat.N).toEqual([])
  })
})

describe('tricksTaken', () => {
  it('counts by side and ignores an unfinished trick', () => {
    const tricks = [
      { winner: 'N' },
      { winner: 'E' },
      { winner: 'S' },
      { winner: 'W' },
      { winner: 'N' },
      { winner: null },
    ]
    expect(tricksTaken(tricks)).toEqual({ NS: 3, EW: 2 })
  })
})

describe('remainingHands', () => {
  const DEAL = 'N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72'
  const hands = parseDealString(DEAL)

  it('leaves the deal untouched at index zero', () => {
    const plays = ['D8', 'DK', 'DA', 'D2']
    const { seatOf } = replay(plays, 'E', null)
    expect(remainingHands(hands, plays, seatOf, 0)).toEqual(hands)
  })

  it('removes exactly the cards played so far', () => {
    // W leads D8, N plays DK... wait: leader E. E has no D8, so use real cards.
    // East holds DA/DQ/D8/D7; South D T; West D9/D6/D5/D3; North DK/DJ/D4/D2.
    const plays = ['D8', 'DT', 'D9', 'DK']
    const { seatOf } = replay(plays, 'E', null)
    const after = remainingHands(hands, plays, seatOf, 4)

    expect(after.E.diamonds).toEqual(['A', 'Q', '7'])
    expect(after.S.diamonds).toEqual([])
    expect(after.W.diamonds).toEqual(['6', '5', '3'])
    expect(after.N.diamonds).toEqual(['J', '4', '2'])
    // Untouched suits are intact.
    expect(after.N.spades).toEqual(['A', 'K', 'Q', 'T', '3'])
  })

  it('stops partway through a trick', () => {
    const plays = ['D8', 'DT', 'D9', 'DK']
    const { seatOf } = replay(plays, 'E', null)
    const after = remainingHands(hands, plays, seatOf, 2)
    expect(after.E.diamonds).toEqual(['A', 'Q', '7'])
    expect(after.S.diamonds).toEqual([])
    // West and North have not played yet.
    expect(after.W.diamonds).toEqual(['9', '6', '5', '3'])
    expect(after.N.diamonds).toEqual(['K', 'J', '4', '2'])
  })

  it('does not mutate the hands it was given', () => {
    const plays = ['D8', 'DT', 'D9', 'DK']
    const { seatOf } = replay(plays, 'E', null)
    remainingHands(hands, plays, seatOf, 4)
    expect(hands.E.diamonds).toEqual(['A', 'Q', '8', '7'])
  })

  it('degrades rather than throwing when a card is not in the hand', () => {
    // A record that disagrees with its own deal. The classroom version throws;
    // here the input is a pasted file, so it must not take the page down.
    const plays = ['SA', 'SA', 'SA', 'SA']
    const { seatOf } = replay(plays, 'E', null)
    expect(() => remainingHands(hands, plays, seatOf, 4)).not.toThrow()
  })
})

describe('handToCodes', () => {
  it('flattens a hand to card codes in suit order', () => {
    const codes = handToCodes({ spades: ['A', 'K'], hearts: ['Q'], diamonds: [], clubs: ['2'] })
    expect(codes).toEqual(['SA', 'SK', 'HQ', 'C2'])
  })

  it('copes with no hand', () => {
    expect(handToCodes(null)).toEqual([])
  })
})
