<script setup>
/**
 * The four hands in compass positions with the trick in the middle.
 *
 * This owns `marksFor`, the load-bearing part of the double-dummy overlay: it
 * merges played state, current-trick state and the DD badges into the single
 * `marks` object `HandDisplay` renders from. The merge order matters and is the
 * same as Bridge-Classroom's.
 *
 * The classroom's grid arranger — 877 lines of per-region scale clamps and layout
 * ledgers for fitting a live table into arbitrary viewports — is deliberately not
 * vendored. Its legacy compass branch, which is what this is, renders the same
 * thing with the same merge and no configuration.
 */
import { computed } from 'vue'
import SeatPanel from './SeatPanel.vue'
import TrickArea from './TrickArea.vue'
import { normalizeCardCode, seatAtPosition } from '../lib/cards.js'

const props = defineProps({
  hands: { type: Object, default: () => ({}) },
  /** Names by seat, from a LIN record. */
  names: { type: Object, default: null },
  /** Cards already played, by seat: `{ N: ['SK', ...] }`. Struck through. */
  playedCards: { type: Object, default: null },
  /** Cards in the trick on the table, by seat. Highlighted, not struck. */
  currentCards: { type: Object, default: null },
  /** DD overlay marks by seat: `{ N: { SK: { badge, fill, chosen } } }`. */
  cardBadges: { type: Object, default: null },
  /** Seat to frame — the one on lead at an inspected node. */
  activeSeat: { type: String, default: null },
  /** Whether clicking a card does anything. */
  inspectable: { type: Boolean, default: false },
  trick: { type: Object, default: null },
  tricksTaken: { type: Object, default: () => ({ NS: 0, EW: 0 }) },
  trickWinner: { type: String, default: null },
  declarer: { type: String, default: null },
  /**
   * Turn the table so this seat sits at the bottom. The badges keep naming real
   * compass seats, so what changes is the viewpoint, not the geography.
   */
  southSeat: { type: String, default: null },
})

defineEmits(['card-click'])

/**
 * Merge the three mark channels for one seat.
 *
 * Order is deliberate: played first, then current-trick cards overwrite it
 * (dummy's led card should highlight rather than strike through), then the DD
 * badges merge *onto* whatever is there without setting `played` — so an error
 * badge can sit on a card that is not struck through. Keeping the strike and the
 * overlay separate is the whole reason the overlay is readable.
 */
function marksFor(seat) {
  const cards = {}

  for (const code of props.playedCards?.[seat] || []) {
    cards[normalizeCardCode(code)] = { played: true }
  }

  for (const code of props.currentCards?.[seat] || []) {
    cards[normalizeCardCode(code)] = { current: true }
  }

  for (const [code, mark] of Object.entries(props.cardBadges?.[seat] || {})) {
    const key = normalizeCardCode(code)
    cards[key] = { ...cards[key], ...mark }
  }

  return { cards }
}

/** Declarer and dummy are worth labelling; the defenders need no note. */
function roleOf(seat) {
  if (!props.declarer) return ''
  if (seat === props.declarer) return 'Declarer'
  const partner = { N: 'S', S: 'N', E: 'W', W: 'E' }[props.declarer]
  return seat === partner ? 'Dummy' : ''
}

function nameOf(seat) {
  const key = { N: 'north', E: 'east', S: 'south', W: 'west' }[seat]
  return props.names?.[key] || ''
}

/** Each screen position, resolved to the seat it shows. */
const slots = computed(() =>
  ['N', 'E', 'S', 'W'].map((position) => {
    const seat = seatAtPosition(position, props.southSeat)
    return {
      position,
      seat,
      hand: props.hands[seat],
      name: nameOf(seat),
      marks: marksFor(seat),
      role: roleOf(seat),
      active: props.activeSeat === seat,
    }
  })
)
</script>

<template>
  <div class="bridge-table">
    <div v-for="slot in slots" :key="slot.position" :class="`cell-${slot.position.toLowerCase()}`">
      <SeatPanel
        :seat="slot.seat"
        :hand="slot.hand"
        :name="slot.name"
        :marks="slot.marks"
        :role="slot.role"
        :active="slot.active"
        :inspectable="inspectable"
        @card-click="$emit('card-click', { seat: slot.seat, ...$event })"
      />
    </div>

    <div class="cell-c">
      <TrickArea
        :trick="trick"
        :tricks-taken="tricksTaken"
        :winner="trickWinner"
        :next-seat="activeSeat"
        :south-seat="southSeat"
      />
    </div>
  </div>
</template>

<style scoped>
.bridge-table {
  display: grid;
  grid-template-areas:
    '.  n  .'
    'w  c  e'
    '.  s  .';
  grid-template-columns: auto auto auto;
  justify-content: center;
  align-items: start;
  gap: calc(10px * var(--table-scale));
}

.cell-n {
  grid-area: n;
}
.cell-w {
  grid-area: w;
}
.cell-c {
  grid-area: c;
}
.cell-e {
  grid-area: e;
}
.cell-s {
  grid-area: s;
}

/*
 * Below the table's natural width the compass has to give: stack the seats in
 * reading order and drop the centre trick, which has no meaning without positions
 * around it.
 */
@media (max-width: 720px) {
  .bridge-table {
    grid-template-areas:
      'n'
      'w'
      'e'
      's';
    grid-template-columns: minmax(0, 1fr);
  }

  .cell-c {
    display: none;
  }
}
</style>
