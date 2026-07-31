<script setup>
/**
 * What every legal card was worth at one moment — the second tier of the analysis.
 *
 * The trace says a card cost a trick; this says what would not have. Cards are
 * ordered best-first rather than by suit, since the question being asked is what
 * should have been played.
 *
 * Sized for a corner of the table rather than a banner across the top. That is not
 * only a space decision: as a banner it appeared and disappeared above the hands,
 * so every click shoved the whole page down. In a reserved corner the hands stay
 * exactly where they were drawn.
 */
import { computed } from 'vue'
import { SUIT_SYMBOLS, formatCard, getSuitClass, parseCardCode } from '../lib/cards.js'
import { trickNumberOf } from '../lib/cardplay.js'
import { suitNameOf } from '../lib/errors.js'

const props = defineProps({
  /** `{ index, seat, card, cost, alternatives: [{ card, tricks, cost }] }`. */
  node: { type: Object, default: null },
  /** Shown when nothing is being inspected, so the corner is never a blank box. */
  idleHint: { type: String, default: 'Click any card, or a trick, to see what every alternative was worth here.' },
})

defineEmits(['close'])

const SEAT_NAMES = { N: 'North', E: 'East', S: 'South', W: 'West' }

const sorted = computed(() => {
  if (!props.node) return []
  return [...props.node.alternatives].sort((a, b) => a.cost - b.cost || b.tricks - a.tricks)
})

/** The best available, which is usually more than one card. */
const bestCost = computed(() => (sorted.value.length ? sorted.value[0].cost : 0))

/**
 * Whether the suit was playable and the card wrong, or the suit itself was the
 * mistake — the same distinction the error table colours, said in words.
 */
const verdict = computed(() => {
  const n = props.node
  if (!n || !(n.cost > 0) || !sorted.value.length) return ''
  const suit = parseCardCode(n.card).suit
  const inSuit = sorted.value.filter((a) => parseCardCode(a.card).suit === suit)
  if (!inSuit.length) return ''
  const name = suitNameOf(n.card)
  return inSuit.some((a) => a.cost === 0)
    ? `Another ${name} would have held it — the card was wrong, not the suit.`
    : `Every ${name} gave a trick away — the suit was the mistake.`
})

function glyph(code) {
  const { suit, rank } = parseCardCode(code)
  return { symbol: SUIT_SYMBOLS[suit], rank: formatCard(rank), cls: getSuitClass(suit) }
}
</script>

<template>
  <div class="inspector">
    <template v-if="node">
      <div class="head">
        <span class="title">
          Trick {{ trickNumberOf(node.index) }} · {{ SEAT_NAMES[node.seat] }}
          <span class="played-card" :class="glyph(node.card).cls">
            {{ glyph(node.card).symbol }}{{ glyph(node.card).rank }}
          </span>
        </span>
        <button type="button" class="close" title="Close" @click="$emit('close')">×</button>
      </div>

      <p v-if="node.cost > 0" class="verdict bad">
        −{{ node.cost }} {{ node.cost === 1 ? 'trick' : 'tricks' }}
      </p>
      <p v-else class="verdict good">Double-dummy best</p>

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
          :title="`${alt.tricks} tricks to the declaring side`"
        >
          <span class="alt-card" :class="glyph(alt.card).cls">
            {{ glyph(alt.card).symbol }}{{ glyph(alt.card).rank }}
          </span>
          <span class="alt-tricks">{{ alt.tricks }}</span>
        </li>
      </ul>

      <p v-if="verdict" class="why">{{ verdict }}</p>
      <p class="note">Tricks are what the declaring side takes from here.</p>
    </template>

    <p v-else class="idle">{{ idleHint }}</p>
  </div>
</template>

<style scoped>
.inspector {
  height: 100%;
  font-size: 12px;
}

.head {
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.title {
  font-weight: 700;
  font-size: 12px;
  line-height: 1.3;
}

.played-card {
  font-family: var(--font-cards);
  font-size: 15px;
  letter-spacing: 0.5px;
}

.close {
  margin-left: auto;
  border: none;
  background: transparent;
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
  color: var(--text-muted);
  padding: 0 2px;
}

.close:hover {
  color: var(--text);
}

.verdict {
  margin: 2px 0 6px;
  font-weight: 700;
  font-size: 12px;
}

.verdict.good {
  color: #2e7d32;
}

.verdict.bad {
  color: var(--cost-ink);
}

.alts {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
}

.alt {
  display: inline-flex;
  align-items: baseline;
  gap: 3px;
  padding: 1px 5px;
  border-radius: 4px;
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
  font-size: 14px;
  font-weight: 500;
}

.alt-tricks {
  font-size: 11px;
  font-weight: 600;
}

.why {
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--text-secondary);
}

.note,
.idle {
  margin: 5px 0 0;
  font-size: 10.5px;
  color: var(--text-muted);
  line-height: 1.35;
}

.idle {
  margin: 0;
}
</style>
