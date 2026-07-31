<script setup>
/**
 * The play record trick by trick, with each error tagged.
 *
 * The classroom app tags errors on the hands themselves and nothing else; a
 * review page wants the list too, because the useful question is "where did this
 * hand go wrong" and that is answered by scanning tricks, not by hunting badges
 * across four holdings. Clicking any card here inspects that node, the same as
 * clicking it in a hand.
 */
import { computed } from 'vue'
import { SUIT_SYMBOLS, formatCard, getSuitClass, parseCardCode } from '../lib/cards.js'
import { TRICK_SIZE } from '../lib/cardplay.js'
import { signedEffect } from '../lib/errors.js'

const props = defineProps({
  /** `[{ index, seat, card, cost }]` from the running trace. */
  trace: { type: Array, default: () => [] },
  /** Which play index is being inspected, if any. */
  selectedIndex: { type: Number, default: -1 },
  /** Per-trick winners from the replay, indexed by trick number - 1. */
  tricks: { type: Array, default: () => [] },
  /**
   * Needed to sign a cost against declarer.
   *
   * Without it this panel showed every error as a bare `−n` while the error table
   * showed the same card signed — so a defender's mistake read as `−1` here and
   * `+1` there. One card, two numbers.
   */
  declarer: { type: String, default: null },
})

defineEmits(['select'])

/** Group the trace into tricks, carrying each trick's total cost. */
const grouped = computed(() => {
  const out = []
  for (let i = 0; i < props.trace.length; i += TRICK_SIZE) {
    const entries = props.trace.slice(i, i + TRICK_SIZE)
    const number = Math.floor(i / TRICK_SIZE) + 1
    out.push({
      number,
      entries,
      cost: entries.reduce((n, e) => n + e.cost, 0),
      winner: props.tricks[number - 1]?.winner || null,
    })
  }
  return out
})

const totalErrors = computed(() => props.trace.filter((e) => e.cost > 0).length)
const totalCost = computed(() => props.trace.reduce((n, e) => n + e.cost, 0))

/** The card's effect on declarer's total, matching the error table exactly. */
function signed(entry) {
  const n = signedEffect(entry, props.declarer)
  return n > 0 ? `+${n}` : String(n)
}

function glyph(code) {
  const { suit, rank } = parseCardCode(code)
  return { symbol: SUIT_SYMBOLS[suit], rank: formatCard(rank), cls: getSuitClass(suit) }
}
</script>

<template>
  <div v-if="trace.length" class="trace">
    <div class="trace-head">
      <span class="trace-label">Play</span>
      <span v-if="totalErrors" class="trace-summary">
        {{ totalErrors }} costed {{ totalErrors === 1 ? 'error' : 'errors' }},
        {{ totalCost }} {{ totalCost === 1 ? 'trick' : 'tricks' }} given away
      </span>
      <span v-else class="trace-summary trace-clean">No costed errors</span>
    </div>

    <ol class="tricks">
      <li v-for="t in grouped" :key="t.number" class="trick" :class="{ 'has-cost': t.cost > 0 }">
        <!-- Clicking the trick moves to its opening lead, so you can walk the hand
             trick by trick without having to aim at a particular card. -->
        <button
          type="button"
          class="trick-no"
          :class="{ selected: t.entries.some((e) => e.index === selectedIndex) }"
          :title="`Go to the start of trick ${t.number}`"
          @click="$emit('select', t.entries[0].index)"
        >
          {{ t.number }}
        </button>
        <span class="trick-cards">
          <button
            v-for="e in t.entries"
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
                ? `${e.seat} gave away ${e.cost} ${e.cost === 1 ? 'trick' : 'tricks'} — click to see the alternatives`
                : `${e.seat} — double-dummy best. Click to see the alternatives`
            "
            @click="$emit('select', e.index)"
          >
            <span class="play-seat">{{ e.seat }}</span>
            <span class="play-card" :class="glyph(e.card).cls">
              {{ glyph(e.card).symbol }}{{ glyph(e.card).rank }}
            </span>
            <span v-if="e.cost > 0" class="play-cost">{{ signed(e) }}</span>
          </button>
        </span>
        <span class="trick-won">{{ t.winner || '' }}</span>
      </li>
    </ol>
  </div>
</template>

<style scoped>
.trace-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 6px;
}

.trace-label {
  font-size: 11px;
  color: #666;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.trace-summary {
  font-size: 12px;
  color: #c62828;
  font-weight: 600;
}

.trace-summary.trace-clean {
  color: var(--green);
}

.tricks {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.trick {
  display: grid;
  grid-template-columns: 2ch 1fr 2ch;
  align-items: center;
  gap: 8px;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--bg-white);
  border: 1px solid var(--rule);
}

.trick.has-cost {
  border-color: #f0c4c4;
}

.trick-no,
.trick-won {
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.trick-no {
  font: inherit;
  font-size: 11px;
  border: 1px solid transparent;
  background: transparent;
  border-radius: 3px;
  padding: 1px 2px;
  cursor: pointer;
  color: var(--text-muted);
}

.trick-no:hover {
  background: var(--focus-blue);
  color: var(--text);
}

.trick-no.selected {
  border-color: var(--border-strong);
  color: var(--text);
  font-weight: 700;
}

.trick-no:focus-visible {
  outline: 2px solid var(--green);
  outline-offset: 1px;
}

.trick-cards {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.play {
  display: inline-flex;
  align-items: baseline;
  gap: 3px;
  border: 1px solid transparent;
  background: transparent;
  border-radius: 4px;
  padding: 1px 4px;
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
  font-size: 15px;
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
</style>
