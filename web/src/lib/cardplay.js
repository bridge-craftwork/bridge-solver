// Replaying a card record: what each seat still holds at trick k, who won a
// trick, which trick a play index belongs to.
//
// Ported from Bridge-Classroom's `utils/cardplayRules.js`, minus the parts that
// only a live table needs (legal-card enforcement while bidding out a hand).

import { RANK_ORDER, SUIT_NAME, SUIT_ORDER, parseCardCode, seatAtIndex } from './cards.js'

/** Cards per trick, and the arithmetic that depends on it. */
export const TRICK_SIZE = 4

/** 1-based trick number for a 0-based play index. */
export function trickNumberOf(index) {
  return Math.floor(index / TRICK_SIZE) + 1
}

/** Play index at which the trick containing `index` began. */
export function trickStartOf(index) {
  return Math.floor(index / TRICK_SIZE) * TRICK_SIZE
}

/** Trump letter for a contract like `"4HX"`, or `null` for notrump. */
export function trumpFromContract(contract) {
  if (!contract) return null
  const m = String(contract)
    .trim()
    .match(/^[1-7]\s*(NT?|S|H|D|C)/i)
  if (!m) return null
  const strain = m[1].toUpperCase()
  return strain === 'N' || strain === 'NT' ? null : strain
}

/**
 * Turn a flat play record into per-seat card lists, seat by seat in play order.
 *
 * `plays` is `["HK", "H3", ...]` and `leader` is who played the first card;
 * everything else follows from trick winners, which is why this needs the
 * trump.
 *
 * Returns `{ bySeat, tricks, seatOf }` where `bySeat[seat]` is that seat's
 * cards in the order they were played, `tricks` is the record grouped into
 * tricks with a winner each, and `seatOf[i]` is who played `plays[i]`.
 */
export function replay(plays, leader, trump) {
  const bySeat = { N: [], E: [], S: [], W: [] }
  const seatOf = []
  const tricks = []

  let currentLeader = leader
  for (let start = 0; start < plays.length; start += TRICK_SIZE) {
    const chunk = plays.slice(start, start + TRICK_SIZE)
    const entries = chunk.map((code, i) => {
      const seat = seatAtIndex(currentLeader, i)
      const { suit, rank } = parseCardCode(code)
      seatOf[start + i] = seat
      bySeat[seat].push(suit + rank)
      return { index: start + i, seat, suit, rank, code: suit + rank }
    })

    // A trailing partial trick has no winner: the record just stops, which is
    // normal after a claim.
    const complete = entries.length === TRICK_SIZE
    const winner = complete ? trickWinner(entries, trump) : null
    tricks.push({ leader: currentLeader, plays: entries, winner, complete })
    if (!complete) break
    currentLeader = winner
  }

  return { bySeat, tricks, seatOf }
}

/**
 * Who takes a completed trick: highest trump if any were played, else highest
 * card of the suit led.
 */
export function trickWinner(entries, trump) {
  if (!entries.length) return null
  const led = entries[0].suit
  const trumped = trump ? entries.filter((e) => e.suit === trump) : []
  const contenders = trumped.length ? trumped : entries.filter((e) => e.suit === led)
  return contenders.reduce((best, e) =>
    RANK_ORDER.indexOf(e.rank) < RANK_ORDER.indexOf(best.rank) ? e : best
  ).seat
}

/**
 * What each seat still holds after the first `count` cards of the record.
 *
 * Returns suit-keyed hands, ready for `HandDisplay`. Unlike the classroom's
 * version this does not throw when a card is missing from a hand: input here
 * comes from a pasted file rather than a table this code was driving, so a
 * record that disagrees with its own deal should degrade rather than blow up.
 */
export function remainingHands(hands, plays, seatOf, count) {
  const out = {}
  for (const seat of ['N', 'E', 'S', 'W']) {
    const h = hands[seat]
    out[seat] = {
      spades: [...(h?.spades || [])],
      hearts: [...(h?.hearts || [])],
      diamonds: [...(h?.diamonds || [])],
      clubs: [...(h?.clubs || [])],
    }
  }

  for (let i = 0; i < Math.min(count, plays.length); i += 1) {
    const seat = seatOf[i]
    if (!seat) continue
    const { suit, rank } = parseCardCode(plays[i])
    const list = out[seat][SUIT_NAME[suit]]
    const at = list.indexOf(rank)
    if (at >= 0) list.splice(at, 1)
  }

  return out
}

/** Running trick counts for each side over a replayed record. */
export function tricksTaken(tricks) {
  const taken = { NS: 0, EW: 0 }
  for (const t of tricks) {
    if (!t.winner) continue
    taken[t.winner === 'N' || t.winner === 'S' ? 'NS' : 'EW'] += 1
  }
  return taken
}

/** Every card in a seat's hand, flattened to codes. */
export function handToCodes(hand) {
  if (!hand) return []
  return SUIT_ORDER.flatMap((suitName) => {
    const letter = { spades: 'S', hearts: 'H', diamonds: 'D', clubs: 'C' }[suitName]
    return hand[suitName].map((r) => letter + r)
  })
}
