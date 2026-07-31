<script setup>
/**
 * Every costed error in the hand, one row per trick that contains one.
 *
 * The whole hand's mistakes at a glance. The play trace lists all thirteen tricks
 * in order and is what you read to follow the hand; this is the filtered view you
 * read to find the moments worth arguing about.
 *
 * Each row shows the complete trick rather than just the offending card, because a
 * card is only wrong in the context of what was led and what had already gone to
 * it. Any card lands on the same position as clicking it in the trace.
 */
import { computed } from 'vue'
import { SUIT_SYMBOLS, formatCard, getSuitClass, parseCardCode } from '../lib/cards.js'
import { TRICK_SIZE, trickNumberOf } from '../lib/cardplay.js'
import {
  blameFor,
  isDeclaringSide,
  signedEffect,
  suitNameOf,
  summariseCosts,
  trickTotalFrom,
} from '../lib/errors.js'

const props = defineProps({
  /** `[{ index, seat, card, cost }]` from the running trace. */
  trace: { type: Array, default: () => [] },
  /** Which play index is being inspected, if any. */
  selectedIndex: { type: Number, default: -1 },
  /** Seat names from a LIN record, if there are any. */
  names: { type: Object, default: null },
  declarer: { type: String, default: null },
  /** The contract's double-dummy result from the opening lead. */
  contractTricks: { type: Number, default: null },
  /**
   * `'card'` or `'suit'` per play index, once the alternatives at that node are
   * known: whether a playable card existed in the suit, or the suit itself was
   * the mistake. Arrives after the trace, so rows render uncoloured first.
   */
  verdicts: { type: Object, default: () => ({}) },
})

defineEmits(['select'])

const NAME_KEY = { N: 'north', E: 'east', S: 'south', W: 'west' }

const summary = computed(() => summariseCosts(props.trace, props.declarer))
const actualTricks = computed(() => trickTotalFrom(props.contractTricks, summary.value))

/** Only the tricks that contain at least one costed card. */
const rows = computed(() => {
  const out = []
  for (let start = 0; start < props.trace.length; start += TRICK_SIZE) {
    const entries = props.trace.slice(start, start + TRICK_SIZE)
    const errors = entries.filter((e) => e.cost > 0)
    if (!errors.length) continue

    out.push({
      number: trickNumberOf(start),
      entries,
      // Signed against declarer, so the column adds up to the net swing.
      effect: errors.reduce((n, e) => n + signedEffect(e, props.declarer), 0),
      // One trick can hold errors from more than one player.
      blamed: [...new Set(errors.map((e) => blameFor(e.seat, props.declarer)))],
    })
  }
  return out
})

function nameFor(seat) {
  return props.names?.[NAME_KEY[seat]] || ''
}

function blameLabel(seat) {
  const name = nameFor(seat)
  return name ? `${seat} ${name}` : seat
}

function glyph(code) {
  const { suit, rank } = parseCardCode(code)
  return { symbol: SUIT_SYMBOLS[suit], rank: formatCard(rank), cls: getSuitClass(suit) }
}

/** Amber for a wrong card in a playable suit, red for a wrong suit. */
function costClass(entry) {
  if (!(entry.cost > 0)) return {}
  const verdict = props.verdicts[entry.index]
  return {
    error: true,
    'error-card': verdict === 'card',
    'error-suit': verdict === 'suit',
    'error-unknown': !verdict,
    severe: verdict === 'suit' && entry.cost >= 2,
  }
}

function cardTitle(entry) {
  if (!(entry.cost > 0)) return `${entry.seat} — double-dummy best. Click for the alternatives`
  const tricks = entry.cost === 1 ? 'trick' : 'tricks'
  const verdict = props.verdicts[entry.index]
  const why =
    verdict === 'card'
      ? ` A ${suitNameOf(entry.card)} would have worked — the card was the mistake, not the suit.`
      : verdict === 'suit'
        ? ` Every ${suitNameOf(entry.card)} gave a trick away — the suit itself was the mistake.`
        : ''
  return `${entry.seat} gave away ${entry.cost} ${tricks}.${why} Click for the alternatives`
}

function signed(n) {
  return n > 0 ? `+${n}` : String(n)
}
</script>

<template>
  <section v-if="rows.length" class="errors" aria-labelledby="errors-heading">
    <div class="errors-head">
      <h2 id="errors-heading">Where it went wrong</h2>

      <!--
        Declarer and the defence separately, because they pull in opposite
        directions and the net is the whole story of the hand.
      -->
      <span class="tally">
        <span v-if="summary.declarerCost" class="tally-decl">
          declarer −{{ summary.declarerCost }}
        </span>
        <span v-if="summary.defenderCost" class="tally-def">
          defence +{{ summary.defenderCost }}
        </span>
        <span v-if="contractTricks != null" class="tally-net">
          {{ contractTricks }} on best play → <strong>{{ actualTricks }} taken</strong>
        </span>
      </span>
    </div>

    <table class="errors-table">
      <caption class="sr-only">
        Each trick containing a card that gave away a trick. Amber marks a wrong
        card in a suit that would have worked; red marks a suit that was wrong
        outright. Dummy's cards are charged to declarer.
      </caption>
      <thead>
        <tr>
          <th scope="col" class="col-trick">Trick</th>
          <th scope="col" class="col-cards">Cards played</th>
          <th scope="col" class="col-effect" title="Effect on declarer's trick total">
            Declarer
          </th>
          <th scope="col" class="col-blame">Charged to</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.number">
          <th scope="row" class="col-trick">
            <button
              type="button"
              class="trick-no"
              :title="`Go to the start of trick ${row.number}`"
              @click="$emit('select', row.entries[0].index)"
            >
              {{ row.number }}
            </button>
          </th>

          <td class="col-cards">
            <button
              v-for="e in row.entries"
              :key="e.index"
              type="button"
              class="play"
              :class="{ ...costClass(e), selected: e.index === selectedIndex }"
              :title="cardTitle(e)"
              @click="$emit('select', e.index)"
            >
              <span class="play-seat">{{ e.seat }}</span>
              <span class="play-card" :class="glyph(e.card).cls">
                {{ glyph(e.card).symbol }}{{ glyph(e.card).rank }}
              </span>
              <span v-if="e.cost > 0" class="play-cost">
                {{ signed(signedEffect(e, declarer)) }}
              </span>
            </button>
          </td>

          <td
            class="col-effect"
            :class="row.effect < 0 ? 'effect-down' : 'effect-up'"
          >
            {{ signed(row.effect) }}
          </td>

          <td class="col-blame">
            <span v-for="seat in row.blamed" :key="seat" class="blame">
              {{ blameLabel(seat) }}
              <span v-if="isDeclaringSide(seat, declarer)" class="blame-role">decl</span>
            </span>
          </td>
        </tr>
      </tbody>
    </table>

    <p class="legend">
      <span class="swatch swatch-card" aria-hidden="true"></span>
      the card was wrong, the suit would have worked
      <span class="swatch swatch-suit" aria-hidden="true"></span>
      every card in that suit gave a trick away
    </p>
  </section>
</template>

<style scoped>
.errors {
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  padding: 12px 14px;
}

.errors-head {
  display: flex;
  align-items: baseline;
  gap: 14px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

h2 {
  font-size: 16px;
  margin: 0;
}

.tally {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  font-size: 13px;
  align-items: baseline;
}

.tally-decl {
  color: var(--cost-ink);
  font-weight: 600;
}

.tally-def {
  color: #2e7d32;
  font-weight: 600;
}

.tally-net {
  color: var(--text-secondary);
}

.errors-table {
  border-collapse: collapse;
  font-size: 13px;
}

.errors-table th,
.errors-table td {
  padding: 3px 10px 3px 0;
  text-align: left;
  vertical-align: middle;
  border-bottom: 1px solid var(--rule);
}

.errors-table tbody tr:last-child th,
.errors-table tbody tr:last-child td {
  border-bottom: none;
}

.errors-table thead th {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

.col-trick {
  width: 3.5ch;
  text-align: center !important;
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
  padding-right: 12px !important;
}

.col-cards {
  white-space: nowrap;
}

/* The trick number is a handle for the whole trick, landing on its opening lead. */
.trick-no {
  font: inherit;
  border: 1px solid transparent;
  background: transparent;
  border-radius: 3px;
  padding: 1px 4px;
  cursor: pointer;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.trick-no:hover {
  background: var(--focus-blue);
  color: var(--text);
}

.trick-no:focus-visible {
  outline: 2px solid var(--green);
  outline-offset: 1px;
}

.col-effect {
  text-align: center !important;
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  width: 5ch;
}

.effect-down {
  color: var(--cost-ink);
}

.effect-up {
  color: #2e7d32;
}

.col-blame {
  font-size: 12px;
  color: var(--text-secondary);
}

.blame {
  display: inline-block;
  margin-right: 8px;
  white-space: nowrap;
}

.blame-role {
  font-size: 9px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--green);
  margin-left: 2px;
}

/* Same vocabulary as the play trace, so a card looks the same in both. */
.play {
  display: inline-flex;
  align-items: baseline;
  gap: 3px;
  border: 1px solid transparent;
  background: transparent;
  border-radius: 4px;
  padding: 1px 4px;
  margin-right: 3px;
  cursor: pointer;
  font: inherit;
  font-family: var(--font-cards);
}

.play:hover {
  background: var(--focus-blue);
}

.play:focus-visible {
  outline: 2px solid var(--green);
  outline-offset: 1px;
}

.play.error-card {
  background: var(--cost-card);
}

.play.error-suit {
  background: var(--cost-suit);
}

.play.error-suit.severe {
  background: var(--cost-suit-severe);
}

/* Until the alternatives come back the kind of mistake is unknown; mark it as an
   error without claiming which. */
.play.error-unknown {
  background: var(--cost-mild);
}

.play.selected {
  border-color: var(--border-strong);
}

.play-seat {
  font-size: 10px;
  color: var(--text-muted);
  font-family: var(--font-body);
  min-width: 1ch;
}

.play-card {
  font-size: 16px;
  font-weight: 500;
  letter-spacing: 0.5px;
}

.play-cost {
  font-size: 10px;
  font-weight: 700;
  color: #fff;
  background: var(--badge);
  border-radius: 8px;
  padding: 0 3px;
  font-family: var(--font-body);
}

.legend {
  margin: 8px 0 0;
  font-size: 11px;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
}

.swatch {
  display: inline-block;
  width: 11px;
  height: 11px;
  border-radius: 2px;
  flex: 0 0 auto;
}

.swatch:not(:first-child) {
  margin-left: 8px;
}

.swatch-card {
  background: var(--cost-card);
}

.swatch-suit {
  background: var(--cost-suit);
}

@media (max-width: 620px) {
  /* The blame column is the least load-bearing: the seat is on every card. */
  .col-blame {
    display: none;
  }
}
</style>
