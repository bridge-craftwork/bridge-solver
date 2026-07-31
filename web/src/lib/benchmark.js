// A device speed probe, and what to say about a slow one.
//
// The self-calibrating estimate — elapsed so far, extrapolated over what is
// left — is the right way to drive a progress bar, but it cannot say anything
// *before* the first piece of work finishes. That is exactly when a warning is
// worth showing, because on a slow device the first piece of work is itself a
// long wait. So one small fixed solve runs first and the warning is decided off
// that.
//
// The probe is the cheapest deal in the frozen fixture set rather than a
// synthetic position, which means it doubles as a correctness check: the
// expected table is frozen alongside it, and a device computing a different one
// has a real wasm bug. That is worth more than the timing it also produces.

import fixtures from './fixtures/bench-v1.json'

/** The cheapest deal in the set — measured, not assumed to be easy. */
const PROBE = fixtures.deals[0]

/**
 * What the probe costs on the reference machine, in milliseconds.
 *
 * Measured, not budgeted: Chrome on a Mac mini M4 Pro against the production
 * build, ~41 ms across repeated runs. A score of 100 is that machine; 25 means
 * roughly four times slower.
 *
 * Worth knowing that this is an end-to-end figure — it includes the worker hop
 * and the JSON at both ends, and the deal is the cheapest in the set, so a
 * meaningful part of it is fixed overhead rather than search. That compresses
 * the dynamic range a little. It is left that way deliberately: the threshold
 * below is calibrated against the same end-to-end measure, and what the warning
 * needs to predict is how slow a *whole small analysis* is on this device,
 * which is what this measures.
 */
const REFERENCE_MS = 41

/**
 * Below this score, warn before starting.
 *
 * Set so the warning appears when a full analysis would run past a handful of
 * seconds. Calibrated from the reference numbers in docs/performance-baseline.md
 * and deliberately conservative: a warning nobody needed is a smaller failure
 * than a page that looks hung.
 */
export const SLOW_SCORE = 25

/** The deal the probe solves, and the table it must produce. */
export const probeDeal = () => ({ deal: PROBE.deal, ddtricks: PROBE.ddtricks })

/**
 * Turn a probe time into a score, with the reference machine at 100.
 *
 * Clamped at the bottom so an absurd outlier — a device that was asleep, a tab
 * that was backgrounded mid-probe — cannot produce a zero that reads as a
 * broken measurement rather than a slow one.
 */
export function scoreFrom(ms) {
  if (!ms || ms <= 0) return null
  return Math.max(1, Math.round((REFERENCE_MS / ms) * 100))
}

/**
 * Roughly how long a full analysis will take on a device with this score.
 *
 * The reference numbers are a 377 ms solve and a 575 ms verdict pass, so about
 * a second in total on the reference machine. Scaled inversely by the score.
 * Deliberately quoted as a range by the caller — this is a warning, not a
 * promise, and a single number invites being held to it.
 */
export function expectedTotalMs(score) {
  if (!score) return null
  return Math.round((950 * 100) / score)
}

/** Whether a device with this score deserves a warning before it starts. */
export function isSlow(score) {
  return score !== null && score < SLOW_SCORE
}

/**
 * The warning text for a score, or `null` when none is warranted.
 *
 * A range rather than a point, and it says results arrive as they are found —
 * which is true, and is the part that stops the wait reading as a hang.
 */
export function slowMessage(score) {
  if (!isSlow(score)) return null
  const total = expectedTotalMs(score)
  /*
   * Widened from the obvious ±30% after measuring against CPU throttling. The
   * probe's fixed overhead means the score flatters a slow device — 20x
   * throttling scores 8 rather than 5 — so a symmetric range centred on the
   * estimate had the real wait sitting on its top edge. Skewed upwards so the
   * measured cases land inside it rather than at the boundary.
   */
  const low = Math.max(2, Math.round((total * 0.8) / 1000))
  const high = Math.round((total * 2.2) / 1000)
  return `This device will take a while — roughly ${low}–${high} seconds. Results appear as they are found.`
}

export { PROBE, REFERENCE_MS }
