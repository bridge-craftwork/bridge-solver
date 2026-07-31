<script setup>
/**
 * One hand, as four rows of inline card glyphs.
 *
 * Vendored from Bridge-Classroom, with its measured-fit machinery removed: the
 * `.hd-probe` off-flow row, the `ResizeObserver`, the double-rAF settle, the
 * per-suit `--suit-scale` compression and the `+N` truncation popup existed to
 * fit a live table into an arbitrary viewport. This site lays out at a width it
 * chooses, so a suit row never needs compressing — that removed about 40% of
 * the file and every moving part in it.
 *
 * What is kept exactly is the marks contract, because that is what the
 * double-dummy overlay renders through.
 */
import { computed } from 'vue'
import { SUIT_LETTER, SUIT_ORDER, SUIT_SYMBOLS, formatCard, getSuitClass } from '../lib/cards.js'

const props = defineProps({
  /** `{ spades: [...], hearts: [...], diamonds: [...], clubs: [...] }`. */
  hand: { type: Object, default: null },
  /**
   * Per-card marks, keyed by normalised code (`"HK"`):
   * `{ played, current, badge, fill, chosen }`.
   *
   * `played` (struck through) and the DD channel (`badge`/`fill`) are separate
   * concerns and deliberately compose: a card can carry an error badge without
   * being struck through, which is what makes the overlay readable against the
   * original deal.
   */
  marks: { type: Object, default: null },
  /** Emit `card-click` when a card is clicked. */
  inspectable: { type: Boolean, default: false },
  /** Drop played cards from the row instead of striking them through. */
  hidePlayedCards: { type: Boolean, default: false },
})

const emit = defineEmits(['card-click'])

const suits = computed(() =>
  SUIT_ORDER.map((name) => ({
    name,
    letter: SUIT_LETTER[name],
    symbol: SUIT_SYMBOLS[name],
    cls: getSuitClass(name),
    cards: props.hand?.[name] || [],
  }))
)

function markFor(letter, rank) {
  return props.marks?.cards?.[letter + rank] || null
}

function onClick(letter, rank) {
  if (!props.inspectable) return
  emit('card-click', { suit: letter, rank, code: letter + rank })
}
</script>

<template>
  <div class="holding" :class="{ 'hide-played': hidePlayedCards }">
    <div v-for="suit in suits" :key="suit.name" class="suit-row">
      <span class="suit-symbol" :class="suit.cls" aria-hidden="true">{{ suit.symbol }}</span>
      <span class="sr-only">{{ suit.name }}:</span>
      <span class="cards" :class="suit.cls">
        <template v-if="suit.cards.length">
          <span
            v-for="rank in suit.cards"
            :key="rank"
            class="cell"
            :class="{
              played: markFor(suit.letter, rank)?.played,
              current: markFor(suit.letter, rank)?.current,
              chosen: markFor(suit.letter, rank)?.chosen,
              inspectable: inspectable,
            }"
            :style="
              markFor(suit.letter, rank)?.fill
                ? { backgroundColor: markFor(suit.letter, rank).fill }
                : null
            "
            :role="inspectable ? 'button' : null"
            :tabindex="inspectable ? 0 : null"
            :aria-label="inspectable ? `${suit.name} ${formatCard(rank)}` : null"
            @click="onClick(suit.letter, rank)"
            @keydown.enter.prevent="onClick(suit.letter, rank)"
            @keydown.space.prevent="onClick(suit.letter, rank)"
            >{{ formatCard(rank)
            }}<span v-if="markFor(suit.letter, rank)?.badge" class="cell-badge">{{
              markFor(suit.letter, rank).badge
            }}</span></span
          >
        </template>
        <span v-else class="void" aria-label="void">—</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.holding {
  display: flex;
  flex-direction: column;
  gap: calc(4px * var(--table-scale));
}

/*
 * Badges overhang the top-right of a glyph, so the room is reserved
 * unconditionally. Reserving it only when a hand *has* a badge made every hand
 * change height the moment the overlay moved — which is 8px of the page jumping on
 * every click.
 */
.holding {
  padding-top: calc(8px * var(--table-scale));
  padding-right: calc(7px * var(--table-scale));
}

.suit-row {
  display: flex;
  align-items: center;
  gap: calc(8px * var(--table-scale));
  font-family: var(--font-cards);
  font-size: calc(24px * var(--table-scale));
  /* The body's 1.55 spaces 24px glyphs nearly 40px apart, which reads as four
     separate lines rather than one holding. */
  line-height: 1.2;
}

.suit-symbol {
  font-size: calc(27px * var(--table-scale));
  width: calc(28px * var(--table-scale));
  text-align: center;
  flex: 0 0 auto;
}

.cards {
  font-weight: 500;
  white-space: nowrap;
  letter-spacing: 1px;
}

.void {
  color: var(--text-muted);
  letter-spacing: 0;
}

/*
 * Cards are inline text runs rather than boxes — that is what makes a holding
 * read as a holding. `position: relative` is only so a badge can anchor.
 */
.cell {
  display: inline;
  position: relative;
  border-radius: 3px;
}

.cell.played {
  opacity: 0.4;
  text-decoration: line-through;
  cursor: default;
  user-select: none;
}

.holding.hide-played .cell.played {
  display: none;
}

.cell.current {
  background: var(--focus-blue);
  box-shadow: 0 0 0 2px var(--focus-blue);
}

/* The card actually chosen at an inspected node. */
.cell.chosen {
  box-shadow: 0 0 0 2px var(--border-strong);
}

.cell.inspectable {
  cursor: pointer;
  transition: background 0.15s;
}

.cell.inspectable:hover {
  background: var(--focus-blue);
}

.cell.inspectable:active {
  background: var(--focus-blue-strong);
}

.cell.inspectable:focus-visible {
  outline: 2px solid var(--green);
  outline-offset: 1px;
}

.cell-badge {
  position: absolute;
  top: calc(-7px * var(--table-scale));
  right: calc(-3px * var(--table-scale));
  font-size: calc(10px * var(--table-scale));
  line-height: 1;
  font-weight: 700;
  letter-spacing: 0;
  color: #fff;
  background: var(--badge);
  border-radius: 8px;
  padding: calc(1px * var(--table-scale)) calc(4px * var(--table-scale));
}
</style>
