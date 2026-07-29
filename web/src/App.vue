<script setup>
/**
 * Double-dummy analysis of a hand, in the browser.
 *
 * The shape of the page follows Bridge-Classroom's review view: the four hands
 * in a compass with the DD table and auction beside them, errors tagged on the
 * cards themselves, and clicking a card rewinding the table to that moment to
 * show what every legal card was worth.
 *
 * Two differences, both forced by this being a review page rather than a live
 * table. There is no bot and no turn-taking, so the hands show the deal as dealt
 * with the error overlay on top rather than a hand being played out. And the
 * play trace is listed trick by trick as well as tagged on the cards, because
 * the question here is where a hand went wrong, which is answered by scanning
 * tricks rather than hunting badges across four holdings.
 */
import { computed, ref, watch } from 'vue'
import AuctionTable from './components/AuctionTable.vue'
import BridgeTable from './components/BridgeTable.vue'
import DoubleDummyTable from './components/DoubleDummyTable.vue'
import InputPanel from './components/InputPanel.vue'
import NodePanel from './components/NodePanel.vue'
import PlayerErrors from './components/PlayerErrors.vue'
import PlayTrace from './components/PlayTrace.vue'
import VerifySection from './components/VerifySection.vue'
import { parseInput } from './lib/input.js'
import { fetchDdPlay, fetchDdPlayNode, fetchDoubleDummy, playRequest } from './lib/solver.js'
import {
  remainingHands,
  replay,
  tricksTaken,
  trickNumberOf,
  trumpFromContract,
} from './lib/cardplay.js'
import { normalizeCardCode } from './lib/cards.js'

const boards = ref([])
const boardIndex = ref(0)
const problems = ref([])
const error = ref('')
const busy = ref(false)

/** Analysis for the board on screen. */
const ddTable = ref(null)
const trace = ref(null)
const node = ref(null)

const board = computed(() => boards.value[boardIndex.value] || null)

/**
 * The play record replayed: who played each card, the tricks, the winners.
 *
 * Needs the trump and the opening leader, so it is only meaningful once a
 * contract is known.
 */
const replayed = computed(() => {
  const b = board.value
  if (!b?.plays?.length || !b.leader) return null
  return replay(b.plays, b.leader, trumpFromContract(b.contract))
})

const taken = computed(() => (replayed.value ? tricksTaken(replayed.value.tricks) : { NS: 0, EW: 0 }))

/**
 * Error badges on the cards, keyed by seat then card code.
 *
 * A costed card is tinted by severity and badged with the trick it happened on,
 * which is what makes the overlay answer "when" as well as "what". These marks
 * deliberately do not set `played`: the hands read as the original deal with the
 * errors marked on it, not as a hand struck through.
 */
const errorBadges = computed(() => {
  if (!trace.value || node.value) return null
  const out = { N: {}, E: {}, S: {}, W: {} }
  for (const e of trace.value.trace) {
    if (e.cost <= 0) continue
    out[e.seat][normalizeCardCode(e.card)] = {
      badge: String(trickNumberOf(e.index)),
      fill: e.cost >= 2 ? 'var(--cost-severe)' : 'var(--cost-mild)',
    }
  }
  return out
})

/**
 * While a node is being inspected, the overlay changes meaning entirely: the
 * acting seat's legal cards are tinted by what they were worth, and everything
 * else is cleared so there is one thing to read.
 */
const nodeBadges = computed(() => {
  const n = node.value
  if (!n) return null
  const cards = {}
  const best = Math.min(...n.alternatives.map((a) => a.cost))
  for (const alt of n.alternatives) {
    cards[normalizeCardCode(alt.card)] =
      alt.cost > best
        ? { fill: 'var(--alt-bad)', badge: '−' + alt.cost }
        : { fill: 'var(--alt-good)' }
  }
  // Ring the card actually chosen, whether or not it was the best one.
  const chosen = normalizeCardCode(n.card)
  cards[chosen] = { ...cards[chosen], chosen: true }
  return { N: {}, E: {}, S: {}, W: {}, [n.seat]: cards }
})

/**
 * The hands as shown.
 *
 * Normally the deal as dealt, so the error overlay has every card to land on.
 * While inspecting a node, rewound to that moment — the cards already gone are
 * gone, which is the only way the alternatives make sense.
 */
const shownHands = computed(() => {
  const b = board.value
  if (!b) return {}
  if (!node.value || !replayed.value) return b.hands
  return remainingHands(b.hands, b.plays, replayed.value.seatOf, node.value.index)
})

/** The trick in the middle: at a node, only the cards played before that seat acted. */
const shownTrick = computed(() => {
  const n = node.value
  if (!n || !replayed.value) return null
  const t = replayed.value.tricks[trickNumberOf(n.index) - 1]
  if (!t) return null
  return { leader: t.leader, plays: t.plays.filter((p) => p.index < n.index) }
})

/** Cards already played, so they can be struck through when a node is open. */
const playedCards = computed(() => {
  const n = node.value
  if (!n || !replayed.value) return null
  const out = { N: [], E: [], S: [], W: [] }
  for (let i = 0; i < n.index; i += 1) {
    out[replayed.value.seatOf[i]].push(normalizeCardCode(board.value.plays[i]))
  }
  return out
})

const request = computed(() => {
  const b = board.value
  if (!b?.contract || !b.declarer || !b.leader || !b.plays?.length) return null
  return playRequest({
    hands: b.hands,
    trump: trumpFromContract(b.contract) || 'NT',
    declarer: b.declarer,
    leader: b.leader,
    plays: b.plays,
  })
})

const contractSummary = computed(() => {
  const b = board.value
  if (!b?.contract) return null
  const made = trace.value?.contract_tricks
  return {
    contract: b.contract,
    declarer: b.declarer,
    ddTricks: made,
    claim: b.claim,
  }
})

async function analyse(text) {
  error.value = ''
  problems.value = []
  node.value = null
  trace.value = null
  ddTable.value = null

  if (!text.trim()) {
    boards.value = []
    return
  }

  busy.value = true
  try {
    const result = await parseInput(text)
    boards.value = result.boards
    problems.value = result.problems
    boardIndex.value = 0
    await loadBoard()
  } catch (e) {
    boards.value = []
    error.value = e?.message || String(e)
  } finally {
    busy.value = false
  }
}

/** Solve the current board: the table always, the trace when there is play. */
async function loadBoard() {
  const b = board.value
  if (!b) return

  node.value = null
  trace.value = null
  ddTable.value = null

  const table = await fetchDoubleDummy(b.hands)
  // Guard against a slow solve landing after the user moved on.
  if (board.value !== b) return
  ddTable.value = table

  if (request.value) {
    const result = await fetchDdPlay(request.value)
    if (board.value !== b) return
    trace.value = result
  }
}

async function inspect(index) {
  if (!request.value) return
  // Clicking the open node again closes it.
  if (node.value?.index === index) {
    node.value = null
    return
  }
  const result = await fetchDdPlayNode(request.value, index)
  if (result) node.value = result
}

/**
 * A card click opens that card's moment — or, if one is already open, closes it.
 *
 * Closing on any card is the important half. While a node is open the hands are
 * rewound to that moment, so the cards on screen are the ones still held; looking
 * one up in the full record finds where it was *eventually* played, which is
 * usually a later trick. Treating that as navigation means clicking almost
 * anything jumps somewhere unrelated instead of going back. Moving between
 * moments is what the play trace is for, where the tricks are laid out in order.
 */
function onCardClick({ code }) {
  if (node.value) {
    node.value = null
    return
  }
  const b = board.value
  if (!b?.plays?.length) return
  const target = normalizeCardCode(code)
  const index = b.plays.findIndex((p) => normalizeCardCode(p) === target)
  if (index >= 0) inspect(index)
}

function goTo(i) {
  if (i < 0 || i >= boards.value.length) return
  boardIndex.value = i
}

watch(boardIndex, loadBoard)
</script>

<template>
  <header class="masthead">
    <div class="wrap">
      <h1>Bridge double-dummy analysis</h1>
      <p class="tagline">
        Paste a hand and see the double-dummy table, then every card that gave a
        trick away and what should have been played instead.
      </p>
      <p class="privacy" role="note">
        <strong>The hand never leaves this page.</strong> The solver is compiled
        into the page and runs in your browser — nothing is sent to a server.
        <a href="#verify">Don't take our word for it →</a>
      </p>
    </div>
  </header>

  <main class="wrap">
    <InputPanel :busy="busy" :error="error" @analyse="analyse" />

    <p v-if="problems.length" class="problems" role="note">
      {{ problems.length }} board{{ problems.length === 1 ? '' : 's' }} in that file
      could not be analysed:
      <span v-for="p in problems" :key="p" class="problem">{{ p }}</span>
    </p>

    <template v-if="board">
      <nav v-if="boards.length > 1" class="boards" aria-label="Boards">
        <button
          type="button"
          class="nav-btn"
          :disabled="boardIndex === 0"
          @click="goTo(boardIndex - 1)"
        >
          ‹ Previous
        </button>
        <span class="board-count">
          {{ board.label || `Board ${boardIndex + 1}` }}
          <span class="of">{{ boardIndex + 1 }} of {{ boards.length }}</span>
        </span>
        <button
          type="button"
          class="nav-btn"
          :disabled="boardIndex === boards.length - 1"
          @click="goTo(boardIndex + 1)"
        >
          Next ›
        </button>
      </nav>

      <section class="board-head" aria-label="Board details">
        <span v-if="board.label && boards.length === 1" class="chip">{{ board.label }}</span>
        <span class="chip">Dealer {{ board.dealer }}</span>
        <span class="chip">Vul {{ board.vulnerable }}</span>
        <span v-if="contractSummary" class="chip chip-contract">
          {{ contractSummary.contract }} by {{ contractSummary.declarer }}
        </span>
        <span v-if="contractSummary?.ddTricks != null" class="chip">
          Double-dummy: {{ contractSummary.ddTricks }} tricks
        </span>
        <span v-if="contractSummary?.claim != null" class="chip">
          Claimed {{ contractSummary.claim }}
        </span>
      </section>

      <NodePanel v-if="node" :node="node" @close="node = null" />

      <!--
        Three columns on a wide screen: the play record, the table, and the
        summaries. The trace belongs beside the hands rather than under them —
        finding where a hand went wrong means reading tricks in order while
        looking at what each seat held, and putting it below the fold makes that
        two glances instead of one.
      -->
      <div class="layout">
        <section v-if="trace" class="trace-col" aria-label="Play record">
          <PlayTrace
            :trace="trace.trace"
            :tricks="replayed?.tricks || []"
            :selected-index="node?.index ?? -1"
            @select="inspect"
          />
        </section>

        <div class="table-col">
          <BridgeTable
            :hands="shownHands"
            :names="board.names"
            :card-badges="node ? nodeBadges : errorBadges"
            :played-cards="playedCards"
            :trick="shownTrick"
            :tricks-taken="taken"
            :active-seat="node?.seat || null"
            :declarer="board.declarer"
            :inspectable="!!request"
            @card-click="onCardClick"
          />

          <p v-if="request && !node" class="table-hint">
            Cards that gave a trick away are tinted and badged with the trick
            number. Click any card to see what every alternative was worth.
          </p>

          <p v-if="board.plays?.length && !request" class="note">
            This board has a play record but no contract, so there is nothing to
            cost the cards against. The double-dummy table still applies.
          </p>
          <p v-else-if="!board.plays?.length" class="note">
            No play record in this board — the double-dummy table is all there is
            to show. A LIN record or a BBO handviewer URL carries the cards
            played.
          </p>
        </div>

        <aside class="side-col">
          <DoubleDummyTable
            :tricks="ddTable?.tricks"
            :contract="board.contract || ''"
            :declarer="board.declarer || ''"
          />
          <PlayerErrors
            v-if="trace"
            :trace="trace.trace"
            :declarer="board.declarer"
            :names="board.names"
          />
          <AuctionTable :bids="board.auction" :dealer="board.dealer" />
        </aside>
      </div>
    </template>

    <VerifySection />
  </main>

  <footer class="wrap">
    <p>
      Built on
      <a href="https://github.com/bridge-craftwork/bridge-solver">bridge-solver</a>,
      compiled to WebAssembly. Unlicense.
    </p>
  </footer>
</template>

<style scoped>
.wrap {
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 18px;
}

.masthead {
  background: var(--bg-white);
  border-bottom: 1px solid var(--border);
  padding: 26px 0 20px;
  margin-bottom: 22px;
}

h1 {
  font-size: 26px;
}

.tagline {
  margin: 0 0 10px;
  color: var(--text-secondary);
  max-width: 62ch;
}

.privacy {
  margin: 0;
  font-size: 14px;
  background: #f0faf5;
  border: 1px solid #c9e9d8;
  border-radius: var(--radius-button);
  padding: 8px 12px;
  display: inline-block;
}

main > * {
  margin-bottom: 18px;
}

.problems {
  font-size: 13px;
  color: #8a6d1f;
  background: #fdf6e3;
  border: 1px solid #eadfae;
  border-radius: var(--radius-button);
  padding: 8px 12px;
}

.problem {
  display: block;
  font-family: var(--font-mono);
  font-size: 12px;
}

.boards {
  display: flex;
  align-items: center;
  gap: 12px;
}

.nav-btn {
  font: inherit;
  font-size: 13px;
  padding: 5px 12px;
  border: 1px solid var(--border);
  background: var(--bg-white);
  border-radius: var(--radius-button);
  cursor: pointer;
}
.nav-btn:disabled {
  opacity: 0.45;
  cursor: default;
}
.nav-btn:not(:disabled):hover {
  border-color: var(--green);
}

.board-count {
  font-weight: 600;
  font-size: 14px;
}
.board-count .of {
  font-weight: 400;
  color: var(--text-muted);
  margin-left: 6px;
  font-size: 12px;
}

.board-head {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.chip {
  font-size: 12px;
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 3px 10px;
  color: var(--text-secondary);
}

.chip-contract {
  font-weight: 700;
  color: var(--green-ink);
  background: var(--green-pale);
  border-color: #b9dfc6;
}

/*
 * Trace | table | summaries. The outer columns size to their content and the
 * table takes the rest, so the compass stays centred in whatever is left rather
 * than drifting against one edge.
 */
.layout {
  display: grid;
  grid-template-columns: minmax(210px, 250px) minmax(0, 1fr) auto;
  gap: 26px;
  align-items: start;
}

.trace-col {
  position: sticky;
  /* Keeps the tricks in view while the eye is on the hands. */
  top: 12px;
  max-height: calc(100vh - 24px);
  overflow-y: auto;
}

.table-col {
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.side-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* Two columns: the trace moves under the table, the summaries stay beside it. */
@media (max-width: 1180px) {
  .layout {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .trace-col {
    grid-row: 2;
    grid-column: 1 / -1;
    position: static;
    max-height: none;
  }
}

@media (max-width: 720px) {
  .layout {
    grid-template-columns: minmax(0, 1fr);
  }
}

.table-hint,
.note {
  font-size: 13px;
  color: var(--text-muted);
  max-width: 60ch;
  margin: 10px 0 0;
}

.note {
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: var(--radius-button);
  padding: 10px 12px;
  color: var(--text-secondary);
}

footer {
  border-top: 1px solid var(--border);
  margin-top: 30px;
  padding-top: 14px;
  padding-bottom: 30px;
  font-size: 13px;
  color: var(--text-muted);
}
</style>
