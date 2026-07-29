import { describe, expect, it } from 'vitest'
import {
  countCards,
  dealStringFrom,
  handsToPbnString,
  parseDealString,
  parsePbnBoard,
  splitPbnBoards,
} from './deal.js'

// The deal from Bridge-Classroom's verified DD fixture, which is also the deal
// this engine's CLI was checked on.
const DEAL = 'N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72'

describe('parseDealString', () => {
  it('reads a North-anchored deal into seats', () => {
    const hands = parseDealString(DEAL)
    expect(hands.N.spades).toEqual(['A', 'K', 'Q', 'T', '3'])
    expect(hands.N.hearts).toEqual(['J', '6'])
    expect(hands.N.diamonds).toEqual(['K', 'J', '4', '2'])
    expect(hands.N.clubs).toEqual(['9', '5'])
    expect(hands.E.hearts).toEqual(['A', 'K', '4', '2'])
    expect(hands.S.clubs).toEqual(['A', 'K', '8', '6', '3'])
    expect(hands.W.spades).toEqual(['9', '8'])
  })

  it('honours a leading seat other than North', () => {
    // Same four holdings, written from East: each should land one seat later.
    const north = parseDealString(DEAL)
    const east = parseDealString(
      'E:652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72 AKQT3.J6.KJ42.95'
    )
    expect(east).toEqual(north)
  })

  it('handles voids', () => {
    const hands = parseDealString('N:AKQJT98765432... .AKQJT98765432.. ..AKQJT98765432. ...AKQJT98765432')
    expect(hands.N.spades).toHaveLength(13)
    expect(hands.N.hearts).toEqual([])
    expect(hands.E.hearts).toHaveLength(13)
  })

  it('sorts a holding into descending order', () => {
    const hands = parseDealString('N:2TKA3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72')
    expect(hands.N.spades).toEqual(['A', 'K', 'T', '3', '2'])
  })

  it('returns null rather than throwing on bad input', () => {
    expect(parseDealString('')).toBeNull()
    expect(parseDealString('nonsense')).toBeNull()
    // Three hands, not four.
    expect(parseDealString('N:AKQ.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863')).toBeNull()
    // A hand with three suits, not four.
    expect(parseDealString('N:AKQ.J6.KJ42 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72')).toBeNull()
  })
})

describe('handsToPbnString', () => {
  it('round-trips a deal', () => {
    expect(dealStringFrom(parseDealString(DEAL))).toBe(DEAL)
  })

  it('always writes N,E,S,W regardless of how the deal was written', () => {
    const east = parseDealString(
      'E:652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72 AKQT3.J6.KJ42.95'
    )
    expect(dealStringFrom(east)).toBe(DEAL)
  })

  it('normalises a ten written as 10', () => {
    const hands = parseDealString(DEAL)
    hands.N.spades = ['A', 'K', 'Q', '10', '3']
    expect(handsToPbnString(hands)).toContain('AKQT3')
  })
})

describe('countCards', () => {
  it('counts a full deal', () => {
    expect(countCards(parseDealString(DEAL))).toBe(52)
  })
})

describe('parsePbnBoard', () => {
  const pbn = `[Event "Test"]
[Board "7"]
[Dealer "E"]
[Vulnerable "NS"]
[Deal "${DEAL}"]
[Declarer "N"]
[Contract "4S"]
`

  it('pulls out the tags the analysis needs', () => {
    const board = parsePbnBoard(pbn)
    expect(board.dealer).toBe('E')
    expect(board.vulnerable).toBe('NS')
    expect(board.contract).toBe('4S')
    expect(board.declarer).toBe('N')
    expect(board.board).toBe('7')
    expect(countCards(board.hands)).toBe(52)
  })

  it('normalises the vulnerability spellings PBN allows', () => {
    const at = (v) => parsePbnBoard(`[Vulnerable "${v}"]\n[Deal "${DEAL}"]`).vulnerable
    expect(at('None')).toBe('None')
    expect(at('-')).toBe('None')
    expect(at('Love')).toBe('None')
    expect(at('N-S')).toBe('NS')
    expect(at('E-W')).toBe('EW')
    expect(at('Both')).toBe('All')
    expect(at('All')).toBe('All')
  })

  it('defaults a missing dealer and vulnerability rather than failing', () => {
    const board = parsePbnBoard(`[Deal "${DEAL}"]`)
    expect(board.dealer).toBe('N')
    expect(board.vulnerable).toBe('None')
  })

  it('returns null without a Deal tag', () => {
    expect(parsePbnBoard('[Event "Test"]\n[Dealer "N"]')).toBeNull()
  })
})

describe('splitPbnBoards', () => {
  it('splits a file on blank lines', () => {
    const file = `[Board "1"]\n[Deal "${DEAL}"]\n\n[Board "2"]\n[Deal "${DEAL}"]\n`
    const boards = splitPbnBoards(file)
    expect(boards).toHaveLength(2)
    expect(boards[0].board).toBe('1')
    expect(boards[1].board).toBe('2')
  })

  it('reads a single board with no separator', () => {
    expect(splitPbnBoards(`[Board "1"]\n[Deal "${DEAL}"]\n`)).toHaveLength(1)
  })

  it('skips chunks with no deal', () => {
    const file = `[Event "Only a header"]\n\n[Board "1"]\n[Deal "${DEAL}"]\n`
    expect(splitPbnBoards(file)).toHaveLength(1)
  })

  it('returns nothing for a file with no deals at all', () => {
    expect(splitPbnBoards('[Event "Empty"]')).toEqual([])
  })
})
