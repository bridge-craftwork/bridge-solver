// Deals: PBN in, the suit-keyed hand shape the components render, PBN back out.
//
// The hand shape is Bridge-Classroom's: `{ spades: [...], hearts: [...],
// diamonds: [...], clubs: [...] }` with single-character ranks and `T` for the
// ten. Keeping it identical is what lets its components drop in here.

import { SEAT_ORDER, SUIT_ORDER, sortSuitDescending } from './cards.js'

/** An empty hand — a void in every suit. */
export function emptyHand() {
  return { spades: [], hearts: [], diamonds: [], clubs: [] }
}

/**
 * Parse a PBN deal string into hands keyed by seat.
 *
 * Accepts the full `"N:AKQ.J6.KJ42.95 ..."` form. The leading seat says which
 * hand comes first and the rest follow clockwise, so `"E:"` is as valid as
 * `"N:"` and both produce the same seat mapping.
 *
 * Returns `null` rather than throwing: input comes from a paste box, so a
 * malformed deal is an expected outcome, not an exceptional one.
 */
export function parseDealString(dealstr) {
  if (typeof dealstr !== 'string') return null
  const s = dealstr.trim()
  const m = s.match(/^([NESW])\s*:\s*(.+)$/i)
  if (!m) return null

  const first = m[1].toUpperCase()
  const parts = m[2].trim().split(/\s+/)
  if (parts.length !== 4) return null

  const hands = {}
  let seatIdx = SEAT_ORDER.indexOf(first)
  for (const part of parts) {
    const suits = part.split('.')
    if (suits.length !== 4) return null
    const hand = emptyHand()
    SUIT_ORDER.forEach((suitName, i) => {
      // A PBN suit is a bare run of rank characters; `10` never appears.
      hand[suitName] = sortSuitDescending(suits[i].split('').filter((c) => c.trim() !== ''))
    })
    hands[SEAT_ORDER[seatIdx % 4]] = hand
    seatIdx += 1
  }

  return hands
}

/**
 * Serialise hands to the PBN body, `N,E,S,W` order without the `N:` prefix.
 *
 * Matches Bridge-Classroom's `handsToPbnString` exactly, including that it does
 * not sort — callers hold hands in descending order already.
 */
export function handsToPbnString(hands) {
  return SEAT_ORDER.map((seat) => {
    const h = hands[seat] || emptyHand()
    return [h.spades, h.hearts, h.diamonds, h.clubs]
      .map((arr) => arr.map((r) => (r === '10' ? 'T' : r)).join(''))
      .join('.')
  }).join(' ')
}

/** The full deal string the solver takes, always North-anchored. */
export function dealStringFrom(hands) {
  return 'N:' + handsToPbnString(hands)
}

/** How many cards the deal holds, for spotting a partial paste. */
export function countCards(hands) {
  return SEAT_ORDER.reduce((total, seat) => {
    const h = hands[seat]
    if (!h) return total
    return total + SUIT_ORDER.reduce((n, s) => n + h[s].length, 0)
  }, 0)
}

/**
 * Pull the interesting tags out of a PBN board.
 *
 * Only the handful the analysis needs — deal, dealer, vulnerability, contract,
 * declarer, board number — plus the `[Play]` section if one is present. A PBN
 * file is line-oriented and tolerant, so anything unrecognised is ignored.
 */
export function parsePbnBoard(text) {
  const tags = {}
  const tagRe = /\[\s*(\w+)\s+"([^"]*)"\s*\]/g
  let m
  while ((m = tagRe.exec(text)) !== null) {
    tags[m[1]] = m[2]
  }
  if (!tags.Deal) return null

  const hands = parseDealString(tags.Deal)
  if (!hands) return null

  return {
    hands,
    dealer: (tags.Dealer || 'N').toUpperCase(),
    vulnerable: normaliseVulnerability(tags.Vulnerable),
    contract: tags.Contract || '',
    declarer: (tags.Declarer || '').toUpperCase(),
    board: tags.Board || null,
    auction: [],
    plays: [],
  }
}

/** Split a PBN file into its boards. A board starts at each `[Deal ...]`. */
export function splitPbnBoards(text) {
  const boards = []
  // PBN separates games with a blank line, but files in the wild are looser;
  // keying on the Deal tag is what actually holds.
  const chunks = text.split(/\n\s*\n/)
  for (const chunk of chunks) {
    if (!/\[\s*Deal\s+"/.test(chunk)) continue
    const board = parsePbnBoard(chunk)
    if (board) boards.push(board)
  }
  // A single board with no blank-line separation at all.
  if (boards.length === 0) {
    const one = parsePbnBoard(text)
    if (one) boards.push(one)
  }
  return boards
}

function normaliseVulnerability(v) {
  if (!v) return 'None'
  switch (v.trim().toUpperCase()) {
    case 'NONE':
    case '-':
    case 'LOVE':
      return 'None'
    case 'NS':
    case 'N-S':
      return 'NS'
    case 'EW':
    case 'E-W':
      return 'EW'
    case 'ALL':
    case 'BOTH':
      return 'All'
    default:
      return 'None'
  }
}
