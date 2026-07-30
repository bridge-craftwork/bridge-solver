<script setup>
/**
 * Every costed error in the hand, one row per trick that contains one.
 *
 * The whole hand's mistakes at a glance. The play trace lists all thirteen
 * tricks in order, which is what you read to follow the hand; this is the
 * filtered view you read to find the moments worth arguing about.
 *
 * Each row shows the complete trick rather than just the offending card, because
 * a card is only wrong in the context of what was led and what had already been
 * played to it. Any card is clickable and lands on the same position as clicking
 * it in the trace.
 */
import { computed } from 'vue'
import { SUIT_SYMBOLS, formatCard, getSuitClass, parseCardCode } from '../lib/cards.js'
import { TRICK_SIZE, trickNumberOf } from '../lib/cardplay.js'

const props = defineProps({
  /** `[{ index, seat, card, cost }]` from the running trace. */
  trace: { type: Array, default: () => [] },
  /** Which play index is being inspected, if any. */
  selectedIndex: { type: Number, default: -1 },
  /** Per-trick winners, indexed by trick number - 1. */
  tricks: { type: Array, default: () => [] },
  /** Seat names from a LIN record, if there are any. */
  names: { type: Object, default: null },
  declarer: { type: String, default: null },
})

defineEmits(['select'])

const NAME_KEY = { N: 'north', E: 'east', S: 'south', W: 'west' }

const dummy = computed(() =>
  props.declarer ? { N: 'S', S: 'N', E: 'W', W: 'E' }[props.declarer] : null
)

/**
 * Who is answerable for a card.
 *
 * Dummy's cards are declarer's choice, so an error on one is declarer's — the
 * same convention the per-player counts and BBO's own analysis use.
 */
function blameFor(seat) {
  return seat === dummy.value && props.declarer ? props.declarer : seat
}

function nameFor(seat) {
  return props.names?.[NAME_KEY[seat]] || ''
}

/** Only the tricks that contain at least one costed card. */
const rows = computed(() => {
  const out = []
  for (let start = 0; start < props.trace.length; start += TRICK_SIZE) {
    const entries = props.trace.slice(start, start + TRICK_SIZE)
    const errors = entries.filter((e) => e.cost > 0)
    if (!errors.length) continue

    const number = trickNumberOf(start)
    out.push({
      number,
      entries,
      errors,
      cost: errors.reduce((n, e) => n + e.cost, 0),
      winner: props.tricks[number - 1]?.winner || null,
      // One trick can hold more than one error, and they can be different
      // players — list each once, in the order they happened.
      blamed: [...new Set(errors.map((e) => blameFor(e.seat)))],
    })
  }
  return out
})

const totalCost = computed(() => rows.value.reduce((n, r) => n + r.cost, 0))

function glyph(code) {
  const { suit, rank } = parseCardCode(code)
  return { symbol: SUIT_SYMBOLS[suit], rank: formatCard(rank), cls: getSuitClass(suit) }
}

function blameLabel(seat) {
  const name = nameFor(seat)
  return name ? `${seat} ${name}` : seat
}
</script>

<template>
  <section v-if="rows.length" class="errors" aria-labelledby="errors-heading">
    <div class="errors-head">
      <h2 id="errors-heading">Where it went wrong</h2>
      <span class="errors-total">
        {{ rows.length }} {{ rows.length === 1 ? 'trick' : 'tricks' }},
        {{ totalCost }} {{ totalCost === 1 ? 'trick' : 'tricks' }} given away
      </span>
      <span class="errors-hint">Click a card to see the alternatives at that moment</span>
    </div>

    <table class="errors-table">
      <caption class="sr-only">
        Each trick containing a card that gave away a trick, with the costed cards
        marked. Dummy's cards are credited to declarer.
      </caption>
      <thead>
        <tr>
          <th scope="col" class="col-trick">Trick</th>
          <th scope="col" class="col-cards">Cards played</th>
          <th scope="col" class="col-cost">Cost</th>
          <th scope="col" class="col-blame">Charged to</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.number">
          <th scope="row" class="col-trick">{{ row.number }}</th>

          <td class="col-cards">
            <button
              v-for="e in row.entries"
              :key="e.index"
              type="button"
              class="play"
              :class="{
                error: e.cost > 0,
                severe: e.cost >= 2,
                selected: e.index === selectedIndex,
              }"
              :title="
                e.cost > 0
                  ? `${e.seat} gave away ${e.cost} ${e.cost === 1 ? 'trick' : 'tricks'} here — click for the alternatives`
                  : `${e.seat} — double-dummy best. Click for the alternatives`
              "
              :aria-label="`Trick ${row.number}, ${e.seat} played ${glyph(e.card).rank}${
                e.cost > 0 ? `, cost ${e.cost}` : ', no cost'
              }`"
              @click="$emit('select', e.index)"
            >
              <span class="play-seat">{{ e.seat }}</span>
              <span class="play-card" :class="glyph(e.card).cls">
                {{ glyph(e.card).symbol }}{{ glyph(e.card).rank }}
              </span>
              <span v-if="e.cost > 0" class="play-cost">−{{ e.cost }}</span>
            </button>
          </td>

          <td class="col-cost">{{ row.cost }}</td>

          <td class="col-blame">
            <span v-for="seat in row.blamed" :key="seat" class="blame">
              {{ blameLabel(seat) }}
              <span v-if="seat === declarer" class="blame-role">decl</span>
            </span>
          </td>
        </tr>
      </tbody>
    </table>
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
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

h2 {
  font-size: 16px;
  margin: 0;
}

.errors-total {
  font-size: 13px;
  font-weight: 600;
  color: #c62828;
}

.errors-hint {
  font-size: 12px;
  color: var(--text-muted);
  margin-left: auto;
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
  border-bottom: 1px solid var(--rule);
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

.col-cost {
  text-align: center !important;
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  color: #c62828;
  width: 4ch;
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

.play.error {
  background: var(--cost-mild);
}

.play.severe {
  background: var(--cost-severe);
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

@media (max-width: 620px) {
  /* The blame column is the least load-bearing: the seat is on every card. */
  .col-blame {
    display: none;
  }
  .errors-hint {
    display: none;
  }
}
</style>
