// Card and suit presentation. Kept deliberately close to Bridge-Classroom's
// `utils/cardFormatting.js` so the two apps read the same way and a component
// can move between them without a rewrite.

export const SUIT_SYMBOLS = {
  spades: '♠',
  hearts: '♥',
  diamonds: '♦',
  clubs: '♣',
  S: '♠',
  H: '♥',
  D: '♦',
  C: '♣',
}

/** Display order, high suit first. */
export const SUIT_ORDER = ['spades', 'hearts', 'diamonds', 'clubs']

/** Long suit name -> the single letter used in card codes. */
export const SUIT_LETTER = { spades: 'S', hearts: 'H', diamonds: 'D', clubs: 'C' }

/** Single letter -> long suit name. */
export const SUIT_NAME = { S: 'spades', H: 'hearts', D: 'diamonds', C: 'clubs' }

/** Descending, the order a hand is always held in. */
export const RANK_ORDER = ['A', 'K', 'Q', 'J', 'T', '9', '8', '7', '6', '5', '4', '3', '2']

export const SEAT_ORDER = ['N', 'E', 'S', 'W']

/** Red suits get one colour, black the other; nothing else is colour-coded. */
export function getSuitClass(suit) {
  const letter = suit.length === 1 ? suit.toUpperCase() : SUIT_LETTER[suit]
  return letter === 'H' || letter === 'D' ? 'suit-red' : 'suit-black'
}

/**
 * Render a rank for reading: the ten is the only one that is not its own code.
 * Card *codes* always use `T`; only display uses `10`.
 */
export function formatCard(rank) {
  return rank === 'T' ? '10' : rank
}

/** Sort ranks into holding order, tolerating `10` as an input spelling. */
export function sortSuitDescending(ranks) {
  return [...ranks]
    .map((r) => (r === '10' ? 'T' : r))
    .sort((a, b) => RANK_ORDER.indexOf(a) - RANK_ORDER.indexOf(b))
}

/**
 * Normalise a card code to the one spelling everything else keys on: uppercase
 * suit letter, uppercase rank, `T` for the ten.
 *
 * The mark maps, the DD trace and the alternatives list must all agree on this
 * or a badge silently fails to find its card.
 */
export function normalizeCardCode(code) {
  const s = String(code).trim().toUpperCase()
  const suit = s[0]
  const rank = s.slice(1) === '10' ? 'T' : s.slice(1)
  return suit + rank
}

/** Split a `"HK"` code into its parts. */
export function parseCardCode(code) {
  const c = normalizeCardCode(code)
  return { suit: c[0], rank: c.slice(1) }
}

/**
 * Render a call the way an auction column wants it: text for accessibility,
 * HTML so the suit symbol can carry its own colour.
 */
export function formatBid(bid) {
  const b = String(bid).trim()
  if (/^(p|pass)$/i.test(b)) return { text: 'Pass', html: '<span class="bid-pass">Pass</span>' }
  if (/^(x|dbl)$/i.test(b)) return { text: 'X', html: '<span class="double">X</span>' }
  if (/^(xx|rdbl)$/i.test(b)) return { text: 'XX', html: '<span class="redouble">XX</span>' }

  const m = b.match(/^([1-7])\s*(NT?|S|H|D|C)$/i)
  if (!m) return { text: b, html: b }

  const level = m[1]
  const strain = m[2].toUpperCase()
  if (strain === 'N' || strain === 'NT') {
    return { text: `${level}NT`, html: `${level}<span class="notrump">NT</span>` }
  }
  const cls = strain === 'H' || strain === 'D' ? 'red' : 'black'
  return {
    text: `${level}${SUIT_SYMBOLS[strain]}`,
    html: `${level}<span class="${cls}">${SUIT_SYMBOLS[strain]}</span>`,
  }
}

/** The seat `n` places clockwise from `seat`. */
export function seatAtIndex(seat, n) {
  const i = SEAT_ORDER.indexOf(seat)
  if (i < 0) return null
  return SEAT_ORDER[(i + n) % 4]
}

/** Which side a seat belongs to. */
export function sideOf(seat) {
  return seat === 'N' || seat === 'S' ? 'NS' : 'EW'
}
