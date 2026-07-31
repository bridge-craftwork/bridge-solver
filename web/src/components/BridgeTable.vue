<script setup>
/**
 * The table: four hands in compass positions, the trick in the middle, and the
 * four corners available to whatever the page wants to put there.
 *
 * The corners are the whole point of the layout. A compass leaves them empty by
 * construction, which on a 1180-wide iPad is four sizeable holes in the middle of
 * the screen while the summaries queue up below the fold. Filling them puts the
 * auction, the double-dummy table, the per-player tally and the inspector all
 * within the same glance as the cards.
 *
 * Corner cells hold their size whether or not they have content, which is what
 * keeps **the hand stationary**. Before, the inspector appeared above the table
 * and every card click shoved the hands and the trick list down the page; now it
 * opens into a reserved box and nothing else moves.
 *
 * This owns `marksFor`, the load-bearing part of the double-dummy overlay: it
 * merges played state, current-trick state and the DD badges into the single
 * `marks` object `HandDisplay` renders from. The merge order matters and is the
 * same as Bridge-Classroom's. Its grid arranger — 877 lines of per-region scale
 * clamps for fitting a live table into arbitrary viewports — is deliberately not
 * vendored; this is the legacy compass branch, same merge, no configuration.
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

function marksFor(seat) {
  const cards = {}

  for (const code of props.playedCards?.[seat] || []) {
    cards[normalizeCardCode(code)] = { played: true }
  }

  // Current-trick cards highlight rather than strike through.
  for (const code of props.currentCards?.[seat] || []) {
    cards[normalizeCardCode(code)] = { current: true }
  }

  // The DD overlay merges *onto* whatever is there without setting `played`, so an
  // error badge can sit on a card that is not struck through. Keeping the strike
  // and the overlay separate is the whole reason the overlay is readable.
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
    <div class="corner corner-nw"><slot name="nw" /></div>
    <div class="corner corner-ne"><slot name="ne" /></div>
    <div class="corner corner-sw"><slot name="sw" /></div>
    <div class="corner corner-se"><slot name="se" /></div>

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
    'nw n ne'
    'w  c e'
    'sw s se';
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
.corner-nw {
  grid-area: nw;
}
.corner-ne {
  grid-area: ne;
}
.corner-sw {
  grid-area: sw;
}
.corner-se {
  grid-area: se;
}

/*
 * Corners hold their box whether filled or not. This is what makes the hand
 * stationary: the inspector opens and closes inside a reserved space, so nothing
 * around it reflows. Content that outgrows the box scrolls rather than pushing.
 */
.corner {
  /*
   * A fixed box, not a minimum. The inspector goes from one line of hint text to
   * thirteen alternatives, and if the corner grew to fit, the centred grid would
   * slide sideways every time it opened. Content that outgrows the box scrolls.
   */
  width: calc(215px * var(--table-scale));
  height: calc(200px * var(--table-scale));
  overflow: auto;
}

/*
 * Below the table's natural width the compass has to give: everything stacks in
 * reading order and the centre trick goes, having no meaning without positions
 * around it.
 *
 * The threshold is the width the compass actually needs — three 215px columns plus
 * gaps is 665px, so ~700px with the page's padding. It was set at 900px, which
 * stacked the table on a 720px half-screen that had room for it and made the page
 * 2667px tall instead of about half that. Measure the layout, don't guess it.
 */
@media (max-width: 700px) {
  .bridge-table {
    grid-template-areas:
      'n'
      'w'
      'e'
      's'
      'nw'
      'ne'
      'sw'
      'se';
    grid-template-columns: minmax(0, 1fr);
  }

  .cell-c {
    display: none;
  }

  .corner {
    width: auto;
    height: auto;
    overflow: visible;
  }

  /* An empty corner would otherwise leave a gap in the stack. */
  .corner:empty {
    display: none;
  }
}
</style>
