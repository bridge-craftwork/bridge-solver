<script setup>
/**
 * The four cards of a trick, laid out by compass position.
 *
 * Vendored from Bridge-Classroom essentially unchanged — it was already the
 * cleanest of its table components and needed no adaptation.
 */
import { computed } from 'vue'
import { SUIT_SYMBOLS, formatCard, getSuitClass } from '../lib/cards.js'

const props = defineProps({
  /** `{ leader, plays: [{ seat, suit, rank }] }`. */
  trick: { type: Object, default: null },
  /** Side trick counts to show in the middle. */
  tricksTaken: { type: Object, default: () => ({ NS: 0, EW: 0 }) },
  showCounter: { type: Boolean, default: true },
  /** Seat whose turn it is — outlined rather than filled. */
  nextSeat: { type: String, default: null },
  /** Which seat won, once the trick is complete. */
  winner: { type: String, default: null },
})

const bySeat = computed(() => {
  const out = { N: null, E: null, S: null, W: null }
  for (const p of props.trick?.plays || []) out[p.seat] = p
  return out
})

function cardClass(play) {
  return play ? getSuitClass(play.suit) : ''
}
function symbolFor(play) {
  return SUIT_SYMBOLS[play.suit]
}
</script>

<template>
  <div class="trick-area">
    <div class="trick-grid">
      <div class="slot slot-n" :class="{ 'is-next': nextSeat === 'N' }">
        <div v-if="bySeat.N" class="card" :class="cardClass(bySeat.N)">
          {{ symbolFor(bySeat.N) }}{{ formatCard(bySeat.N.rank) }}
        </div>
      </div>

      <div class="slot slot-w" :class="{ 'is-next': nextSeat === 'W' }">
        <div v-if="bySeat.W" class="card" :class="cardClass(bySeat.W)">
          {{ symbolFor(bySeat.W) }}{{ formatCard(bySeat.W.rank) }}
        </div>
      </div>

      <div class="slot slot-c">
        <div v-if="showCounter" class="counter">
          <span>NS {{ tricksTaken.NS }}</span>
          <span>EW {{ tricksTaken.EW }}</span>
        </div>
        <div v-if="winner" class="last-winner">{{ winner }} won</div>
      </div>

      <div class="slot slot-e" :class="{ 'is-next': nextSeat === 'E' }">
        <div v-if="bySeat.E" class="card" :class="cardClass(bySeat.E)">
          {{ symbolFor(bySeat.E) }}{{ formatCard(bySeat.E.rank) }}
        </div>
      </div>

      <div class="slot slot-s" :class="{ 'is-next': nextSeat === 'S' }">
        <div v-if="bySeat.S" class="card" :class="cardClass(bySeat.S)">
          {{ symbolFor(bySeat.S) }}{{ formatCard(bySeat.S.rank) }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.trick-area {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: calc(200px * var(--table-scale));
  padding: 0 calc(8px * var(--table-scale));
}

.trick-grid {
  display: grid;
  grid-template-columns: calc(56px * var(--table-scale)) 1fr calc(56px * var(--table-scale));
  grid-template-rows: repeat(3, minmax(calc(44px * var(--table-scale)), auto));
  gap: calc(6px * var(--table-scale));
  width: calc(196px * var(--table-scale));
  grid-template-areas:
    '.  n  .'
    'w  c  e'
    '.  s  .';
}

.slot {
  display: flex;
  align-items: center;
  justify-content: center;
}
.slot-n {
  grid-area: n;
}
.slot-w {
  grid-area: w;
}
.slot-c {
  grid-area: c;
  flex-direction: column;
  gap: 2px;
}
.slot-e {
  grid-area: e;
}
.slot-s {
  grid-area: s;
}

.card {
  background: var(--bg-white);
  border: 0.5px solid #bbb;
  border-radius: 4px;
  padding: calc(6px * var(--table-scale)) calc(10px * var(--table-scale));
  font-family: var(--font-cards);
  font-weight: 500;
  font-size: calc(24px * var(--table-scale));
  letter-spacing: 1px;
  white-space: nowrap;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}

.slot.is-next .card {
  outline: 1.5px solid var(--green);
  outline-offset: 2px;
}

.counter {
  display: flex;
  flex-direction: column;
  align-items: center;
  color: #555;
  font-variant-numeric: tabular-nums;
  font-size: calc(14px * var(--table-scale));
  line-height: 1.3;
}

.last-winner {
  color: var(--green);
  font-size: calc(11px * var(--table-scale));
}

/* TrickArea used #222 for black where everything else uses #1a1a1a; unified. */
.suit-black {
  color: var(--suit-black);
}
</style>
