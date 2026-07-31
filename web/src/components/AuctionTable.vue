<script setup>
/**
 * The auction, in W-N-E-S columns with the dealer's call in the right one.
 *
 * Vendored from Bridge-Classroom with its teaching features removed — the
 * bidding-engine divergence toggles, the wrong/correct-bid marking and the
 * turn indicator all belong to a practice drill, not to reviewing a played
 * hand. What is kept is the column layout, the dealer offset and the glyph
 * scaling.
 *
 * The suit symbol is set at 1.28em deliberately: in an auction the suit is read
 * rather than recognised by position, so it is sized to match the level digit's
 * cap height.
 */
import { computed } from 'vue'
import { formatBid } from '../lib/cards.js'

const props = defineProps({
  /** Calls in order from the dealer: `[{ call, alert, annotation }]` or strings. */
  bids: { type: Array, default: () => [] },
  dealer: { type: String, default: 'N' },
})

/** Fixed display order; the dealer's column is found within it. */
const COLUMNS = ['W', 'N', 'E', 'S']

const dealerColumn = computed(() => Math.max(0, COLUMNS.indexOf(props.dealer.toUpperCase())))

const normalised = computed(() =>
  props.bids.map((b) => (typeof b === 'string' ? { call: b, alert: false, annotation: null } : b))
)

/**
 * Lay the calls out into rows of four, leaving the cells before the dealer
 * empty.
 */
const rows = computed(() => {
  const cells = Array(dealerColumn.value).fill(null)
  normalised.value.forEach((bid, i) => cells.push({ ...bid, index: i }))
  while (cells.length % 4 !== 0) cells.push(null)

  const out = []
  for (let i = 0; i < cells.length; i += 4) out.push(cells.slice(i, i + 4))
  return out
})

/** Calls that carry an explanation, listed under the grid. */
const annotated = computed(() =>
  normalised.value.map((b, i) => ({ ...b, index: i })).filter((b) => b.annotation)
)

function render(bid) {
  return formatBid(bid.call).html
}
function label(bid) {
  return formatBid(bid.call).text
}
</script>

<template>
  <div v-if="bids.length" class="auction">
    <div class="auction-label">Auction</div>
    <div class="auction-table">
      <div class="header" role="row">
        <div v-for="seat in COLUMNS" :key="seat" class="header-cell" role="columnheader">
          {{ seat }}
        </div>
      </div>
      <div v-for="(row, r) in rows" :key="r" class="round" role="row">
        <div v-for="(cell, c) in row" :key="c" class="bid-cell" role="cell">
          <template v-if="cell">
            <span class="bid" :aria-label="label(cell)" v-html="render(cell)" />
            <span v-if="cell.alert" class="alert-dot" :title="cell.annotation || 'Alerted'">!</span>
          </template>
        </div>
      </div>
    </div>
    <!--
      Explanations sit under the grid rather than in a tooltip: on a review page
      they are worth reading through, and a hover target is no use on a phone.
    -->
    <ul v-if="annotated.length" class="annotations">
      <li v-for="a in annotated" :key="a.index">
        <span class="ann-bid" v-html="render(a)" />
        <span class="ann-text">{{ a.annotation }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.auction-label {
  font-size: 11px;
  color: #666;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 4px;
}

.auction-table {
  background: var(--bg-white);
  border: 2px solid var(--border-strong);
  border-radius: 4px;
  overflow: hidden;
  /*
   * Fits its container rather than demanding a width. It sits in a 215px table
   * corner, and a 220px minimum meant the last column was clipped off the right
   * edge — measured at iPad landscape, where the corner is exactly its budget.
   */
  width: 100%;
  min-width: 0;
}

.header {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  background: var(--border-strong);
  color: #fff;
}

.header-cell {
  font-size: calc(11px * var(--table-scale));
  padding: calc(2px * var(--table-scale)) calc(4px * var(--table-scale));
  font-weight: 600;
  text-align: center;
}

.round {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  border-bottom: 1px solid var(--rule);
}
.round:last-child {
  border-bottom: none;
}

.bid-cell {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-cards);
  font-size: calc(24px * var(--table-scale));
  font-weight: 500;
  min-height: calc(34px * var(--table-scale));
  border-right: 1px solid #eee;
}
.bid-cell:last-child {
  border-right: none;
}

.bid-cell :deep(.red) {
  color: var(--suit-red);
}
.bid-cell :deep(.black) {
  color: var(--suit-black);
}
/* Sized to the level digit's cap height — see the note in the script block. */
.bid-cell :deep(.red),
.bid-cell :deep(.black) {
  font-size: 1.28em;
  line-height: 1;
}
.bid-cell :deep(.notrump) {
  font-size: 0.78em;
  font-weight: 600;
}
.bid-cell :deep(.bid-pass) {
  font-size: 0.72em;
  font-weight: 500;
  color: #555;
}
.bid-cell :deep(.double) {
  color: #ff5722;
  font-weight: bold;
}
.bid-cell :deep(.redouble) {
  color: #2196f3;
  font-weight: bold;
}

.alert-dot {
  position: absolute;
  top: 1px;
  right: 3px;
  font-size: calc(10px * var(--table-scale));
  font-weight: 700;
  color: var(--badge);
}

.annotations {
  list-style: none;
  margin: 8px 0 0;
  padding: 0;
  font-size: 12px;
  color: var(--text-secondary);
}

.annotations li {
  display: flex;
  gap: 6px;
  margin-bottom: 3px;
}

.ann-bid {
  font-family: var(--font-cards);
  font-weight: 600;
  color: var(--text);
  flex: 0 0 auto;
}
</style>
