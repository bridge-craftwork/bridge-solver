<script setup>
/**
 * What every legal card was worth at one decision point.
 *
 * This is the second tier of the analysis: the trace says a card cost a trick,
 * this says what would not have. Cards are ordered best-first rather than by
 * suit, since the question being asked is "what should have been played".
 */
import { computed } from 'vue'
import { SUIT_SYMBOLS, formatCard, getSuitClass, parseCardCode } from '../lib/cards.js'
import { trickNumberOf } from '../lib/cardplay.js'

const props = defineProps({
  /** `{ index, seat, card, cost, alternatives: [{ card, tricks, cost }] }`. */
  node: { type: Object, default: null },
})

defineEmits(['close'])

const SEAT_NAMES = { N: 'North', E: 'East', S: 'South', W: 'West' }

const sorted = computed(() => {
  if (!props.node) return []
  return [...props.node.alternatives].sort((a, b) => a.cost - b.cost || b.tricks - a.tricks)
})

/** The cards that were as good as it gets — usually more than one. */
const bestCost = computed(() => (sorted.value.length ? sorted.value[0].cost : 0))

function glyph(code) {
  const { suit, rank } = parseCardCode(code)
  return { symbol: SUIT_SYMBOLS[suit], rank: formatCard(rank), cls: getSuitClass(suit) }
}
</script>

<template>
  <div v-if="node" class="node-panel">
    <div class="node-head">
      <span class="node-title">
        Trick {{ trickNumberOf(node.index) }} — {{ SEAT_NAMES[node.seat] }} played
        <span class="node-card" :class="glyph(node.card).cls">
          {{ glyph(node.card).symbol }}{{ glyph(node.card).rank }}
        </span>
      </span>
      <span v-if="node.cost > 0" class="node-verdict bad">
        gave away {{ node.cost }} {{ node.cost === 1 ? 'trick' : 'tricks' }}
      </span>
      <span v-else class="node-verdict good">double-dummy best</span>
      <button type="button" class="node-close" title="Close" @click="$emit('close')">×</button>
    </div>

    <ul class="alts">
      <li
        v-for="alt in sorted"
        :key="alt.card"
        class="alt"
        :class="{
          best: alt.cost === bestCost,
          bad: alt.cost > bestCost,
          chosen: alt.card === node.card,
        }"
      >
        <span class="alt-card" :class="glyph(alt.card).cls">
          {{ glyph(alt.card).symbol }}{{ glyph(alt.card).rank }}
        </span>
        <span class="alt-tricks">{{ alt.tricks }}</span>
        <span v-if="alt.cost > 0" class="alt-cost">−{{ alt.cost }}</span>
        <span v-else class="alt-cost alt-cost-none">best</span>
        <span v-if="alt.card === node.card" class="alt-played">played</span>
      </li>
    </ul>

    <p class="node-note">
      Tricks are what the declaring side takes from here with both sides playing
      perfectly. Click another card to move the analysis.
    </p>
  </div>
</template>

<style scoped>
.node-panel {
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  padding: 12px 14px;
}

.node-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 10px;
}

.node-title {
  font-weight: 600;
  font-size: 14px;
}

.node-card {
  font-family: var(--font-cards);
  font-size: 17px;
  letter-spacing: 0.5px;
}

.node-verdict {
  font-size: 13px;
  font-weight: 600;
}
.node-verdict.good {
  color: #2e7d32;
}
.node-verdict.bad {
  color: #c62828;
}

.node-close {
  margin-left: auto;
  border: none;
  background: transparent;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
  color: var(--text-muted);
  padding: 0 4px;
}
.node-close:hover {
  color: var(--text);
}

.alts {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.alt {
  display: inline-flex;
  align-items: baseline;
  gap: 5px;
  padding: 3px 7px;
  border-radius: 5px;
  border: 1px solid transparent;
  font-variant-numeric: tabular-nums;
}

.alt.best {
  background: var(--alt-good);
}
.alt.bad {
  background: var(--alt-bad);
}
.alt.chosen {
  border-color: var(--border-strong);
}

.alt-card {
  font-family: var(--font-cards);
  font-size: 16px;
  font-weight: 500;
  letter-spacing: 0.5px;
}

.alt-tricks {
  font-size: 13px;
  font-weight: 600;
}

.alt-cost {
  font-size: 10px;
  font-weight: 700;
  color: #fff;
  background: var(--badge);
  border-radius: 8px;
  padding: 0 4px;
}

.alt-cost-none {
  background: transparent;
  color: #1b5e20;
}

.alt-played {
  font-size: 10px;
  color: var(--text-secondary);
  font-style: italic;
}

.node-note {
  margin: 10px 0 0;
  font-size: 12px;
  color: var(--text-muted);
}
</style>
