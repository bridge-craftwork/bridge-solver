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
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import AuctionTable from './components/AuctionTable.vue'
import BridgeTable from './components/BridgeTable.vue'
import DoubleDummyTable from './components/DoubleDummyTable.vue'
import ErrorSummary from './components/ErrorSummary.vue'
import InputPanel from './components/InputPanel.vue'
import NodePanel from './components/NodePanel.vue'
import PlayerErrors from './components/PlayerErrors.vue'
import PlayTrace from './components/PlayTrace.vue'
import VerifySection from './components/VerifySection.vue'
import { parseInput } from './lib/input.js'
import {
  elapsedMs,
  fetchDdPlay,
  fetchDdPlayNode,
  fetchDoubleDummy,
  fetchOptimalLine,
  playRequest,
  timed,
} from './lib/solver.js'
import { suitVerdict, summariseCosts, trickTotalFrom } from './lib/errors.js'
import { resolveInitial, save as saveSession } from './lib/session.js'
import { TRICK_SIZE, nextToPlay, trickStartOf } from './lib/cardplay.js'
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

/**
 * The trace of what was actually played, kept while a draft is open.
 *
 * A correction needs to know where the first mistake was, and any alternative line
 * is only interesting next to the real one — so the original analysis survives
 * exploring.
 */
const originalTrace = ref(null)

/** Wall clock for the last solve, shown so the claim is checkable. */
const solveMs = ref(0)

/**
 * An alternative line, when one is being explored.
 *
 * Both extra modes are the same idea, so they share one mechanism: a replacement
 * play record that branches from the original at `from`. The engine composes it for
 * a correction; you compose it a card at a time in free play. Everything downstream
 * — the board, the trace, the costs — reads the active record without caring which
 * of the two produced it.
 *
 * `{ from, plays, source: 'corrected' | 'free', result }`
 */
const draft = ref(null)

/** Whether clicking a card adds it to the line rather than inspecting it. */
const freePlay = ref(false)

/** What the URL asked for, or what was open last time. */
const initial = resolveInitial()

/** The text of the current hand, kept so it can be stored and restored. */
const currentInput = ref('')

/**
 * Strip the page to the analysis, for embedding in another site. Set by `?embed`.
 */
const embed = ref(initial.options.embed)

/**
 * A fixed pixel box, set by `?width` / `?height`. Used by the gallery to preview a
 * viewport, and by a host site that wants the analysis to sit in a known space.
 */
const boxWidth = ref(initial.options.width)
const boxHeight = ref(initial.options.height)

/**
 * Turn the table so declarer sits at the bottom.
 *
 * On by default: a hand under review is conventionally read from declarer's side,
 * and the alternative — a fixed compass — makes you re-orient on every board as
 * declarer moves around. Turning it only relabels which slot each hand occupies,
 * so the badges keep naming real seats and the geography is unchanged.
 */
const declarerSouth = ref(initial.options.declarerSouth)

/**
 * `'card'` or `'suit'` per costed play index: whether a playable card existed in
 * the suit, or the suit itself was the mistake.
 *
 * Filled in after the trace, one `dd_play_node` per error. Each of those costs a
 * solve per legal card, so they are deliberately not part of the headline timing
 * and the rows colour in as they land.
 */
const verdicts = ref({})

const board = computed(() => boards.value[boardIndex.value] || null)

/**
 * The play record replayed: who played each card, the tricks, the winners.
 *
 * Needs the trump and the opening leader, so it is only meaningful once a
 * contract is known.
 */
/** The record under analysis: the draft when one is open, else what was played. */
const activePlays = computed(() => draft.value?.plays ?? board.value?.plays ?? [])

const replayed = computed(() => {
  const b = board.value
  if (!activePlays.value.length || !b?.leader) return null
  return replay(activePlays.value, b.leader, trumpFromContract(b.contract))
})

/** Who is on turn, and what they may play — only meaningful in free play. */
const turn = computed(() => {
  const b = board.value
  if (!freePlay.value || !b?.hands || !b.leader) return null
  return nextToPlay(b.hands, activePlays.value, b.leader, trumpFromContract(b.contract))
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
/**
 * In free play, mark the cards the seat on turn may legally play, so the next move
 * is visible rather than something you discover by clicking.
 */
const turnBadges = computed(() => {
  const t = turn.value
  if (!t) return null
  const cards = {}
  for (const code of t.legal) cards[code] = { current: true }
  return { N: {}, E: {}, S: {}, W: {}, [t.seat]: cards }
})

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
 *
 * Because the rewind removes them outright, there is nothing left to strike
 * through: `HandDisplay` still supports a `played` mark, but this page never
 * needs to send one.
 */
const shownHands = computed(() => {
  const b = board.value
  if (!b) return {}
  // In free play the table shows the position you have played to; otherwise the
  // deal as dealt, unless a node is being inspected.
  if (freePlay.value && turn.value) return turn.value.remaining
  if (!node.value || !replayed.value) return b.hands
  return remainingHands(b.hands, activePlays.value, replayed.value.seatOf, node.value.index)
})

/** The trick in the middle: at a node, only the cards played before that seat acted. */
const shownTrick = computed(() => {
  const n = node.value
  if (!n || !replayed.value) return null
  const t = replayed.value.tricks[trickNumberOf(n.index) - 1]
  if (!t) return null
  return { leader: t.leader, plays: t.plays.filter((p) => p.index < n.index) }
})

const request = computed(() => {
  const b = board.value
  if (!b?.contract || !b.declarer || !b.leader || !activePlays.value.length) return null
  return playRequest({
    hands: b.hands,
    trump: trumpFromContract(b.contract) || 'NT',
    declarer: b.declarer,
    leader: b.leader,
    plays: activePlays.value,
  })
})

/**
 * The original record's request, for the reference figures a draft is compared
 * against. Kept separate so switching lines does not lose what actually happened.
 */
const originalRequest = computed(() => {
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

/** Which seat the table is turned to put at the bottom, if any. */
const southSeat = computed(() =>
  declarerSouth.value && board.value?.declarer ? board.value.declarer : null
)

/** The real hand's split of costs, for comparing a draft against. */
const originalSummary = computed(() => summariseCosts(originalTrace.value?.trace, board.value?.declarer))

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
  currentInput.value = text
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

/**
 * Remember the hand and the options, so coming back resumes where you left off.
 *
 * Only once a hand has actually parsed: storing a paste that failed would greet
 * you with the same error next time.
 */
watch([currentInput, declarerSouth], () => {
  if (boards.value.length && currentInput.value.trim()) {
    saveSession(currentInput.value, { declarerSouth: declarerSouth.value })
  }
})

/**
 * Walk the play with the keyboard.
 *
 * Up and down move a whole trick, keeping the position within it, so you can
 * follow one seat down the hand. Left and right move a single card and cross trick
 * boundaries, so the whole record is one continuous sequence. Nothing is selected
 * yet means starting at the opening lead.
 */
function onKeydown(event) {
  if (event.metaKey || event.ctrlKey || event.altKey) return
  // Never steal keys from the paste box or any other field.
  const tag = event.target?.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || event.target?.isContentEditable) return

  const step = {
    ArrowDown: TRICK_SIZE,
    ArrowUp: -TRICK_SIZE,
    ArrowRight: 1,
    ArrowLeft: -1,
  }[event.key]
  if (step === undefined) return

  const total = board.value?.plays?.length || 0
  if (!total || !request.value) return

  event.preventDefault()

  const from = node.value ? node.value.index : null
  const next = from === null ? 0 : from + step
  if (next < 0 || next >= total) return
  inspect(next, false)
}

/**
 * Re-analyse whenever the active line changes.
 *
 * The costs have to describe the line on screen, or the tally would be talking
 * about a hand nobody is looking at. Each new position is one solve and the prefix
 * cache covers everything before it, so walking a line forward is cheap.
 */
watch(activePlays, async (plays) => {
  if (!draft.value) return
  if (!plays.length || !request.value) {
    trace.value = null
    return
  }
  const b = board.value
  const result = await fetchDdPlay(request.value)
  if (board.value !== b) return
  trace.value = result
})

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  // A hand from the URL, or the one open last time.
  if (initial.hand) analyse(initial.hand)
})

onUnmounted(() => window.removeEventListener('keydown', onKeydown))

/** Solve the current board: the table always, the trace when there is play. */
async function loadBoard() {
  const b = board.value
  if (!b) return

  node.value = null
  trace.value = null
  originalTrace.value = null
  ddTable.value = null
  verdicts.value = {}
  solveMs.value = 0
  draft.value = null
  freePlay.value = false

  await timed(async () => {
    const table = await fetchDoubleDummy(b.hands)
    // Guard against a slow solve landing after the user moved on.
    if (board.value !== b) return
    ddTable.value = table

    if (request.value) {
      const result = await fetchDdPlay(request.value)
      if (board.value !== b) return
      trace.value = result
      originalTrace.value = result
    }
  })
  if (board.value !== b) return
  solveMs.value = elapsedMs()
}

/**
 * Work out, for each costed card, whether the suit or only the card was wrong.
 *
 * One node analysis per error, and each of those solves every legal card from that
 * position — far more work than the trace itself. Deliberately not awaited: the page
 * is already complete and correct without it, and each answer colours its own row as
 * it arrives.
 *
 * Driven by the trace rather than by loading a board, because an explored line has
 * costed cards of its own that deserve the same explanation. It also has to be:
 * verdicts are keyed by play index, so a stale one from the original record would
 * otherwise be applied to whatever card a draft happens to have at that index.
 */
watch(trace, async (current) => {
  verdicts.value = {}
  if (!current || !request.value) return

  const b = board.value
  const forTrace = current
  for (const e of current.trace.filter((x) => x.cost > 0)) {
    const result = await fetchDdPlayNode(request.value, e.index)
    // Abandon quietly if the board or the line moved on while these were queued.
    if (board.value !== b || trace.value !== forTrace) return
    const verdict = suitVerdict(result)
    if (verdict) verdicts.value = { ...verdicts.value, [e.index]: verdict }
  }
})

/**
 * Open the analysis at one play index.
 *
 * `toggle` closes it when the same index is already open, which is what a click on
 * it should do. Keyboard movement passes `false`: arrowing onto the current card
 * should stay there rather than dismiss the panel.
 */
async function inspect(index, toggle = true) {
  if (!request.value) return
  if (toggle && node.value?.index === index) {
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
  // While composing a line, a click on a legal card plays it.
  if (freePlay.value) {
    playCard(normalizeCardCode(code))
    return
  }
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

/** The trick a correction would start at, given what is selected. */
const correctFromTrick = computed(() => {
  const anchor = node.value ? node.value.index : firstErrorIndex.value
  return anchor < 0 ? 1 : trickNumberOf(anchor)
})

/** The first card that gave a trick away — the default place to correct from. */
const firstErrorIndex = computed(() => {
  const first = (originalTrace.value?.trace || []).find((e) => e.cost > 0)
  return first ? first.index : -1
})

/**
 * Replace the play from the selected trick onwards with what should have happened.
 *
 * Every earlier trick is kept exactly as played — the correction starts at the trick
 * you are looking at, so you can ask "what if it had gone right from *here*" at any
 * point rather than only from the hand's first mistake. With nothing selected it
 * falls back to that first mistake, which is the most likely thing to want.
 *
 * It always begins at a trick boundary rather than mid-trick: a correction that
 * started on the third card of a trick would leave the two before it standing and
 * read as a different, stranger question.
 *
 * The engine plays it out for both sides, so this is not "what a good player might
 * have tried" but the line that was actually available.
 */
async function showCorrectedLine() {
  const b = board.value
  if (!originalRequest.value) return

  const anchor = node.value ? node.value.index : firstErrorIndex.value
  if (anchor < 0) return
  const from = trickStartOf(anchor)

  const line = await fetchOptimalLine(originalRequest.value, from)
  if (!line || board.value !== b) return

  freePlay.value = false
  node.value = null
  draft.value = {
    from,
    plays: [...b.plays.slice(0, from), ...line.cards],
    source: 'corrected',
    result: line.declaring_tricks,
  }
}

/** Take the hand over from a point and play it on yourself. */
function startFreePlay(from = node.value?.index ?? 0) {
  const b = board.value
  if (!b?.plays?.length) return
  node.value = null
  freePlay.value = true
  draft.value = { from, plays: b.plays.slice(0, from), source: 'free', result: null }
}

/** Add a card to the line being composed. */
function playCard(code) {
  const t = turn.value
  if (!t || !t.legal.has(code)) return
  draft.value = { ...draft.value, plays: [...draft.value.plays, code] }
}

/** Back to what was actually played. */
function resetLine() {
  draft.value = null
  freePlay.value = false
  node.value = null
  // The draft watcher does not run for the original line, so restore its analysis
  // rather than leaving the page with the alternative's costs — or with none.
  trace.value = originalTrace.value
}

/** Undo the last card of a line you are composing. */
function undoCard() {
  const d = draft.value
  if (!d || d.plays.length <= d.from) return
  draft.value = { ...d, plays: d.plays.slice(0, -1) }
}

function goTo(i) {
  if (i < 0 || i >= boards.value.length) return
  boardIndex.value = i
}

watch(boardIndex, loadBoard)
</script>

<template>
  <div
    class="app"
    :class="{ 'is-embed': embed }"
    :style="{
      width: boxWidth ? boxWidth + 'px' : null,
      height: boxHeight ? boxHeight + 'px' : null,
    }"
  >
  <header v-if="!embed" class="masthead">
    <div class="wrap">
      <h1>Bridge double-dummy analysis</h1>
      <p class="tagline">
        Paste a hand and see the double-dummy table, then every card that gave a
        trick away and what should have been played instead.
      </p>
    </div>
  </header>

  <main class="wrap">
    <InputPanel
      v-if="!embed"
      :busy="busy"
      :error="error"
      :hand="currentInput"
      @analyse="analyse"
    />

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

        <label v-if="board.declarer" class="chip chip-toggle">
          <input v-model="declarerSouth" type="checkbox" />
          Declarer at the bottom
        </label>
      </section>

      <!--
        Two ways to leave the record behind: have the engine show what should have
        happened, or take the hand over and try something. Both produce an
        alternative line, and the analysis follows it.
      -->
      <section v-if="originalRequest" class="modes" aria-label="Explore the hand">
        <button
          v-if="firstErrorIndex >= 0 || node"
          type="button"
          class="mode-btn"
          :class="{ active: draft?.source === 'corrected' }"
          :title="
            `Keep every trick before ${correctFromTrick}, then play it out perfectly from there`
          "
          @click="showCorrectedLine"
        >
          Correct it from trick {{ correctFromTrick }}
        </button>
        <button
          type="button"
          class="mode-btn"
          :class="{ active: freePlay }"
          :title="
            node
              ? `Take over from trick ${trickNumberOf(node.index)} and play it yourself`
              : 'Take the hand over from the start and play it yourself'
          "
          @click="startFreePlay()"
        >
          {{ node ? `Play on from trick ${trickNumberOf(node.index)}` : 'Play it yourself' }}
        </button>

        <template v-if="draft">
          <button
            v-if="freePlay"
            type="button"
            class="mode-btn"
            :disabled="draft.plays.length <= draft.from"
            @click="undoCard"
          >
            Undo
          </button>
          <button type="button" class="mode-btn" @click="resetLine">
            Back to what was played
          </button>
        </template>

        <span v-if="draft" class="mode-status">
          <template v-if="draft.source === 'corrected'">
            Corrected from trick {{ trickNumberOf(draft.from) }} —
            <strong>{{ draft.result }} tricks</strong> instead of
            {{ trickTotalFrom(contractSummary?.ddTricks, originalSummary) }}
          </template>
          <template v-else-if="turn">
            Your line · {{ turn.seat }} to play ·
            {{ 52 - draft.plays.length }} cards left
          </template>
          <template v-else>Your line · the hand is complete</template>
        </span>
      </section>

      <!--
        Errors first, before the table: the question this page exists to answer
        is where the hand went wrong, and this answers it without reading
        anything else.
      -->
      <ErrorSummary
        v-if="trace"
        :trace="trace.trace"
        :selected-index="node?.index ?? -1"
        :names="board.names"
        :declarer="board.declarer"
        :contract-tricks="trace.contract_tricks"
        :verdicts="verdicts"
        @select="inspect"
      />

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
            :declarer="board.declarer"
            :verdicts="verdicts"
            @select="inspect"
          />
        </section>

        <div class="table-col">
          <!--
            The four corners a compass leaves empty. On a 1180-wide iPad those were
            four holes in the middle of the screen while the summaries queued up
            below the fold; now everything is in one glance. The inspector goes NE
            in a reserved box, which is what keeps the hands from moving when it
            opens.
          -->
          <BridgeTable
            :hands="shownHands"
            :names="board.names"
            :card-badges="freePlay ? turnBadges : node ? nodeBadges : errorBadges"
            :trick="shownTrick"
            :tricks-taken="taken"
            :active-seat="turn?.seat || node?.seat || null"
            :declarer="board.declarer"
            :south-seat="southSeat"
            :inspectable="!!originalRequest"
            @card-click="onCardClick"
          >
            <template #nw>
              <AuctionTable :bids="board.auction" :dealer="board.dealer" />
            </template>

            <template #ne>
              <NodePanel
                v-if="originalRequest"
                :node="node"
                :idle-hint="
                  freePlay
                    ? 'Playing your own line. Click a highlighted card to play it.'
                    : undefined
                "
                @close="node = null"
              />
            </template>

            <template #sw>
              <PlayerErrors
                v-if="trace"
                :trace="trace.trace"
                :declarer="board.declarer"
                :names="board.names"
              />
            </template>

            <template #se>
              <DoubleDummyTable
                :tricks="ddTable?.tricks"
                :contract="board.contract || ''"
                :declarer="board.declarer || ''"
              />
            </template>
          </BridgeTable>

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
      </div>
    </template>

    <VerifySection v-if="!embed" :elapsed-ms="solveMs" />
  </main>

  <footer v-if="!embed" class="wrap">
    <p>
      Built on
      <a href="https://github.com/bridge-craftwork/bridge-solver">bridge-solver</a>,
      compiled to WebAssembly. Unlicense.
    </p>
  </footer>
  </div>
</template>

<style scoped>
.wrap {
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 18px;
}

/*
 * A fixed pixel box, when `?width`/`?height` ask for one. The gallery uses it to
 * preview a viewport; a host site uses it to give the analysis a known space.
 * Scrolls internally rather than pushing the host page around.
 */
.app {
  overflow: auto;
}

/* Embedded: no masthead, no paste box, no footer — just the analysis. */
.is-embed {
  padding: 10px 0;
}

.is-embed .wrap {
  padding: 0 10px;
}

.is-embed main > * {
  margin-bottom: 12px;
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
  margin: 0;
  color: var(--text-secondary);
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

.modes {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.mode-btn {
  font: inherit;
  font-size: 13px;
  padding: 5px 12px;
  border: 1px solid var(--border);
  background: var(--bg-white);
  border-radius: var(--radius-button);
  cursor: pointer;
}

.mode-btn:not(:disabled):hover {
  border-color: var(--green);
}

.mode-btn:disabled {
  opacity: 0.45;
  cursor: default;
}

.mode-btn.active {
  background: var(--green-pale);
  border-color: #b9dfc6;
  color: var(--green-ink);
  font-weight: 600;
}

.mode-status {
  font-size: 13px;
  color: var(--text-secondary);
}

.chip-toggle {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  user-select: none;
}

.chip-toggle input {
  margin: 0;
  cursor: pointer;
}

.chip-contract {
  font-weight: 700;
  color: var(--green-ink);
  background: var(--green-pale);
  border-color: #b9dfc6;
}

/*
 * Trace beside the table. The summaries used to need a third column; they now sit
 * in the table's own corners, which is both tighter and closer to the cards.
 */
.layout {
  display: grid;
  grid-template-columns: minmax(320px, 350px) minmax(0, 1fr);
  gap: 22px;
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

/*
 * The table with its corners measures 665px and the trace column 350px, so the two
 * fit side by side from about 1060px. The previous 1240px breakpoint collapsed them
 * on an iPad in landscape (1180px) — the one case the corner layout exists for —
 * which pushed the trace below the fold and made the page over two screens tall.
 */
@media (max-width: 1060px) {
  .layout {
    grid-template-columns: minmax(0, 1fr);
  }

  .trace-col {
    grid-row: 2;
    position: static;
    max-height: none;
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
