<script setup>
/**
 * The play record, one row per trick and one column per seat.
 *
 * It used to lay the columns out in the order the cards were played — first,
 * second, third, fourth. That reads a single trick fine and makes the hand as a
 * whole unreadable, because the lead rotates: a given seat wanders across the
 * table from row to row, and following one player's cards down the hand means
 * re-finding them thirteen times. Fixing a seat per column makes that vertical
 * scan free, which is the scan a review actually wants.
 *
 * What the fixed columns cost is the play order, so that comes back as colour:
 * the leader's cell and the winner's cell are tinted. Colour alone would be a
 * poor way to carry it, so both also state themselves in each cell's title, and
 * the legend spells them out.
 *
 * Columns run clockwise from the opening leader, which puts declarer on the
 * right and dummy between the two defenders — the same way round as the table
 * above, so the two do not have to be mentally transposed.
 */
import { computed } from 'vue'
import {
  SUIT_SYMBOLS,
  formatCard,
  getSuitClass,
  parseCardCode,
  seatAtIndex,
} from '../lib/cards.js'
import { TRICK_SIZE } from '../lib/cardplay.js'
import { isDeclaringSide, partnerOf, signedEffect } from '../lib/errors.js'

const props = defineProps({
  /** `[{ index, seat, card, cost }]` from the running trace. */
  trace: { type: Array, default: () => [] },
  /** Which play index is being inspected, if any. */
  selectedIndex: { type: Number, default: -1 },
  /** Per-trick `{ leader, winner, complete }` from the replay. */
  tricks: { type: Array, default: () => [] },
  /**
   * Needed to sign a cost against declarer, and to name the columns.
   *
   * Without it this panel showed every error as a bare `−n` while the error table
   * showed the same card signed — so a defender's mistake read as `−1` here and
   * `+1` there. One card, two numbers.
   */
  declarer: { type: String, default: null },
  /**
   * `'card'` or `'suit'` per play index — the same verdicts the error table
   * colours by, so a mistake looks like the same kind of mistake wherever you meet
   * it. Without this the trace tinted everything one shade of red and quietly
   * disagreed with the table above it.
   */
  verdicts: { type: Object, default: () => ({}) },
})

defineEmits(['select'])

/** Dummy is declarer's partner; both are the declaring side. */
const dummy = computed(() => partnerOf(props.declarer))

/**
 * Who led the very first trick.
 *
 * Taken from the replay when there is one, and otherwise from the first card in
 * the trace — a board can be mid-analysis, and a header that waits for the
 * replay would flicker through a wrong order on the way.
 */
const openingLeader = computed(
  () => props.tricks[0]?.leader ?? props.trace[0]?.seat ?? null
)

/**
 * The four columns, clockwise from the opening leader.
 *
 * That ordering is not a preference: the opening leader is declarer's left-hand
 * opponent, so going clockwise from them necessarily ends on declarer and puts
 * dummy in the middle. Left to right is therefore also the order a trick is
 * played in whenever the leader is on lead, which is every trick they win.
 */
const columns = computed(() => {
  const first = openingLeader.value
  if (!first) return []
  return [0, 1, 2, 3].map((i) => {
    const seat = seatAtIndex(first, i)
    return {
      seat,
      // Declarer and dummy are named by their role because that is what the
      // reader is tracking; the defenders keep their compass letters, which is
      // what identifies them.
      label: seat === props.declarer ? 'Dec' : seat === dummy.value ? 'Dum' : seat,
      title:
        seat === props.declarer
          ? `Declarer (${seat})`
          : seat === dummy.value
            ? `Dummy (${seat})`
            : `${seat} — defender`,
    }
  })
})

/**
 * Tricks, each carrying its cards keyed by seat and the running score after it.
 *
 * The running count is stated from the declaring side first, so it reads the way
 * a result is spoken — eight tricks to five, not five to eight.
 */
const grouped = computed(() => {
  const out = []
  let declaring = 0
  let defending = 0

  for (let i = 0; i < props.trace.length; i += TRICK_SIZE) {
    const entries = props.trace.slice(i, i + TRICK_SIZE)
    const number = Math.floor(i / TRICK_SIZE) + 1
    const replayed = props.tricks[number - 1]
    const winner = replayed?.winner || null

    if (winner) {
      if (isDeclaringSide(winner, props.declarer)) declaring += 1
      else defending += 1
    }

    const bySeat = {}
    for (const e of entries) bySeat[e.seat] = e

    out.push({
      number,
      entries,
      bySeat,
      leader: replayed?.leader ?? entries[0]?.seat ?? null,
      winner,
      cost: entries.reduce((n, e) => n + e.cost, 0),
      // A trick still in progress has not been won by anyone, so the score has
      // nothing new to say and repeating it would imply it had.
      running: winner ? `${declaring}–${defending}` : '',
    })
  }
  return out
})

const totalErrors = computed(() => props.trace.filter((e) => e.cost > 0).length)
const totalCost = computed(() => props.trace.reduce((n, e) => n + e.cost, 0))

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

/**
 * The card's effect on declarer's total, matching the error table exactly.
 *
 * Signed from the declaring side throughout: a trick declarer threw away is
 * negative and red, a trick the defence handed back is positive and green. The
 * colours therefore mean "which way did this move the contract", not "who erred"
 * — the column already says who.
 */
function signed(entry) {
  const n = signedEffect(entry, props.declarer)
  return { text: n > 0 ? `+${n}` : String(n), good: n > 0 }
}

function glyph(code) {
  const { suit, rank } = parseCardCode(code)
  return { symbol: SUIT_SYMBOLS[suit], rank: formatCard(rank), cls: getSuitClass(suit) }
}

/** What a cell says about itself, for anyone not reading the tint. */
function cellTitle(t, seat, entry) {
  const role = []
  if (t.leader === seat) role.push('led')
  if (t.winner === seat) role.push('won the trick')
  const where = role.length ? ` (${role.join(', ')})` : ''
  if (!entry) return `${seat}${where}`
  return entry.cost > 0
    ? `${seat}${where} gave away ${entry.cost} ${entry.cost === 1 ? 'trick' : 'tricks'} — click to see the alternatives`
    : `${seat}${where} — double-dummy best. Click to see the alternatives`
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

    <table class="tricks">
      <caption class="sr-only">
        The play, one row per trick and one column per seat, in clockwise order
        from the opening leader. The running score after each trick is given from
        the declaring side first.
      </caption>
      <thead>
        <tr>
          <th scope="col" class="col-no"><span class="sr-only">Trick</span></th>
          <th v-for="c in columns" :key="c.seat" scope="col" class="col-seat" :title="c.title">
            {{ c.label }}
          </th>
          <th scope="col" class="col-run" title="Tricks won so far — declaring side first">
            Tricks
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="t in grouped" :key="t.number" class="trick" :class="{ 'has-cost': t.cost > 0 }">
          <!-- Clicking the trick moves to its opening lead, so you can walk the
               hand trick by trick without having to aim at a particular card. -->
          <th scope="row" class="col-no">
            <button
              type="button"
              class="trick-no"
              :class="{ selected: t.entries.some((e) => e.index === selectedIndex) }"
              :title="`Go to the start of trick ${t.number}`"
              @click="$emit('select', t.entries[0].index)"
            >
              {{ t.number }}
            </button>
          </th>

          <td
            v-for="c in columns"
            :key="c.seat"
            class="cell"
            :class="{ lead: t.leader === c.seat, won: t.winner === c.seat }"
            :title="cellTitle(t, c.seat, t.bySeat[c.seat])"
          >
            <!-- Purely decorative: the cell's title already says "led", so this
                 must not be announced a second time. -->
            <span v-if="t.leader === c.seat" class="lead-chevron" aria-hidden="true"></span>
            <button
              v-if="t.bySeat[c.seat]"
              type="button"
              class="play"
              :class="{
                ...costClass(t.bySeat[c.seat]),
                selected: t.bySeat[c.seat].index === selectedIndex,
              }"
              @click="$emit('select', t.bySeat[c.seat].index)"
            >
              <span class="play-card" :class="glyph(t.bySeat[c.seat].card).cls">
                {{ glyph(t.bySeat[c.seat].card).symbol }}{{ glyph(t.bySeat[c.seat].card).rank }}
              </span>
              <span
                v-if="t.bySeat[c.seat].cost > 0"
                class="play-cost"
                :class="{ good: signed(t.bySeat[c.seat]).good }"
              >
                {{ signed(t.bySeat[c.seat]).text }}
              </span>
            </button>
          </td>

          <td class="col-run">{{ t.running }}</td>
        </tr>
      </tbody>
    </table>

    <!-- Colour is doing real work above, so say what it means rather than
         leaving it to be inferred. -->
    <p class="legend">
      <span><i class="swatch lead"></i>led</span>
      <span><i class="swatch won"></i>won the trick</span>
      <span><i class="dot bad">−1</i>declarer lost a trick</span>
      <span><i class="dot good">+1</i>defence gave one back</span>
    </p>
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
  width: 100%;
  border-collapse: separate;
  /* The 2px gap between cells is what separates the tinted ones, so a leader
     next to a winner stays two marks rather than one wide band. */
  border-spacing: 0 2px;
}

thead th {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  padding: 0 0 2px;
  text-align: center;
}

.col-no {
  width: 2.5ch;
}

.col-run {
  width: 5ch;
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.trick > * {
  background: var(--bg-white);
  border-top: 1px solid var(--rule);
  border-bottom: 1px solid var(--rule);
  padding: 2px 0;
}

.trick > *:first-child {
  border-left: 1px solid var(--rule);
  border-radius: 4px 0 0 4px;
}

.trick > *:last-child {
  border-right: 1px solid var(--rule);
  border-radius: 0 4px 4px 0;
}

.trick.has-cost > * {
  border-color: #f0c4c4;
}

.cell {
  text-align: center;
  /* Anchors the lead chevron, which is positioned rather than laid out so it
     costs no column width — these cells are about 70px and cannot spare any. */
  position: relative;
}

/* The play order the fixed columns gave up, handed back as colour. The tint sits
   on the cell while an error tint sits on the chip inside it, so a leader who
   also erred shows both rather than one overwriting the other. */
.cell.lead {
  background: #e8eef5;
}

.cell.won {
  background: var(--green-pale);
}

/*
 * Leading and winning the same trick is not an edge case — whoever wins leads
 * the next one, so on a hand where one side keeps the lead it is most of the
 * rows (six of thirteen on the board this was built against). Two backgrounds
 * cannot both show, and dropping one would silently lose it, so the lead also
 * carries a bar down the left edge. It is drawn on every led cell rather than
 * only the overlapping ones, so "led" looks like one thing throughout.
 */
.cell.lead {
  box-shadow: inset 3px 0 0 #5b86b5;
}

/*
 * The direction cue on the seat that led.
 *
 * A clip-path rather than the CSS border-triangle trick, which cannot produce a
 * flat left edge to fuse against the bar — it only makes triangles whose sides
 * are all borders of one box. This is a plain rectangle with the right-pointing
 * third of it clipped away, so its left edge is genuinely flat and meets the bar
 * cleanly.
 *
 * Absolutely positioned, so it overlays the cell's left margin and adds nothing
 * to the column width. The chip inside is centred with roughly 20px of clearance
 * either side, so the two do not meet.
 *
 * The proportions are the anti-aliasing constraint, not taste: a wide, short
 * triangle puts the hypotenuse near horizontal, where the clip is stepped across
 * many pixels and reads as fuzzy. Tall and narrow keeps it steep — 4.5px across
 * 14px of height puts the edge about 33° off vertical, comfortably in the range
 * that stays crisp. Widening this without lengthening it is what would spoil it.
 */
.lead-chevron {
  position: absolute;
  left: 3px;
  top: 50%;
  transform: translateY(-50%);
  width: 4.5px;
  height: 14px;
  background: #5b86b5;
  clip-path: polygon(0 0, 0 100%, 100% 50%);
  pointer-events: none;
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
  font-variant-numeric: tabular-nums;
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

.play {
  display: inline-flex;
  align-items: center;
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

.play-card {
  font-size: 15px;
  font-weight: 500;
  letter-spacing: 0.5px;
}

/*
 * Signed from the declaring side: red for a trick declarer lost, green for one
 * the defence handed back. Round, so it reads as a token against the card rather
 * than as part of it.
 */
.play-cost {
  font-size: 10px;
  font-weight: 700;
  color: #fff;
  background: #c62828;
  border-radius: 999px;
  min-width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 3px;
  font-family: var(--font-body);
}

.play-cost.good {
  background: #1d7a3f;
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--text-muted);
}

.legend span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.swatch {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  border: 1px solid var(--rule);
  display: inline-block;
}

/* The swatch carries the chevron too, or the legend would explain a mark the
   table does not actually show. */
.swatch.lead {
  background: #e8eef5;
  box-shadow: inset 3px 0 0 #5b86b5;
  position: relative;
}

.swatch.lead::after {
  content: '';
  position: absolute;
  left: 3px;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 9px;
  background: #5b86b5;
  clip-path: polygon(0 0, 0 100%, 100% 50%);
}

.swatch.won {
  background: var(--green-pale);
}

.dot {
  font-size: 9px;
  font-weight: 700;
  font-style: normal;
  color: #fff;
  background: #c62828;
  border-radius: 999px;
  min-width: 15px;
  height: 15px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 3px;
}

.dot.good {
  background: #1d7a3f;
}
</style>
