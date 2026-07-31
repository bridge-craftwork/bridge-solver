// Reading the running trace: who a mistake belongs to, which way it moved the
// result, and whether the suit or only the card was wrong.

import { SUIT_NAME, parseCardCode } from './cards.js'

/** The seat opposite. */
export function partnerOf(seat) {
  return { N: 'S', S: 'N', E: 'W', W: 'E' }[seat] || null
}

/** Dummy is declarer's partner. */
export function dummyOf(declarer) {
  return declarer ? partnerOf(declarer) : null
}

/** Whether a seat is on the declaring side. */
export function isDeclaringSide(seat, declarer) {
  if (!declarer) return false
  return seat === declarer || seat === dummyOf(declarer)
}

/**
 * Who is answerable for a card.
 *
 * Dummy's cards are declarer's choice, so an error on one is declarer's. This is
 * the convention BBO's own analysis uses, and matching it means these counts can
 * be compared against it directly.
 */
export function blameFor(seat, declarer) {
  return declarer && seat === dummyOf(declarer) ? declarer : seat
}

/**
 * How a mistake moved declarer's trick total.
 *
 * A cost is always "tricks this player's side gave away", which makes declarer's
 * errors and the defence's directly comparable but says nothing about direction.
 * Signed against declarer, they add up: declarer's mistakes take tricks off the
 * contract, the defence's hand them over.
 */
export function signedEffect(entry, declarer) {
  return isDeclaringSide(entry.seat, declarer) ? -entry.cost : entry.cost
}

/**
 * Split the trace's costs by side.
 *
 * `net` is the two combined, and it is exactly the distance between the
 * double-dummy result from the opening lead and what declarer actually took:
 *
 *     actual = contractTricks - declarer + defenders
 *
 * so a hand's whole story reconciles. Worth stating because it is checkable — on
 * a board where double-dummy gives 8, declarer gave away 3 and the defence 2, the
 * 7 that fall out is the number that was claimed.
 */
export function summariseCosts(trace, declarer) {
  let declarerCost = 0
  let defenderCost = 0
  let errors = 0

  for (const e of trace || []) {
    if (e.cost <= 0) continue
    errors += 1
    if (isDeclaringSide(e.seat, declarer)) declarerCost += e.cost
    else defenderCost += e.cost
  }

  return {
    errors,
    declarerCost,
    defenderCost,
    total: declarerCost + defenderCost,
    net: defenderCost - declarerCost,
  }
}

/** Declarer's actual tricks, derived from the double-dummy result and the costs. */
export function trickTotalFrom(contractTricks, summary) {
  if (contractTricks == null) return null
  return contractTricks + summary.net
}

/**
 * Was the suit wrong, or only the card within it?
 *
 * Given a node's alternatives, look at the cards in the same suit as the one
 * actually played. If any of them cost nothing, the suit was playable and the
 * choice within it was the mistake. If every card in that suit gave a trick away,
 * the suit itself was the mistake — a different and usually worse error, and the
 * distinction a player most wants drawn.
 *
 * Returns `'card'`, `'suit'`, or `null` when there is nothing to judge (the play
 * cost nothing, or no alternatives are known yet).
 */
export function suitVerdict(node) {
  if (!node || !Array.isArray(node.alternatives) || !node.alternatives.length) return null
  if (!(node.cost > 0)) return null

  const suit = parseCardCode(node.card).suit
  const inSuit = node.alternatives.filter((a) => parseCardCode(a.card).suit === suit)
  if (!inSuit.length) return null

  return inSuit.some((a) => a.cost === 0) ? 'card' : 'suit'
}

/** Long name of the suit a card belongs to, for a readable explanation. */
export function suitNameOf(card) {
  return SUIT_NAME[parseCardCode(card).suit]
}
