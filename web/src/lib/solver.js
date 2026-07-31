// Client for the double-dummy engine.
//
// This is the wasm replacement for Bridge-Classroom's `utils/ddsClient.js`,
// keeping its three operations and their exact request and response shapes so
// the same components and decoding code work against either. What changes is
// that there is no `fetch` and no `SOLVER_URL`: the engine is compiled into the
// page, which is what lets the site claim nothing is uploaded and back it with
// `connect-src 'self'`.
//
// The classroom client's other discipline is kept too: every call resolves to
// `null` on any failure rather than throwing. Double-dummy here is an overlay
// on a hand you can already read, so a failed solve should cost you the
// annotation and nothing else.

import { dealStringFrom } from './deal.js'

/** Column order of a decoded DD table row. */
export const DD_STRAINS = ['C', 'D', 'H', 'S', 'NT']

/** Row order of a decoded DD table. */
export const DD_SEATS = ['N', 'E', 'S', 'W']

let worker = null
let nextId = 1
const pending = new Map()

function getWorker() {
  if (worker) return worker
  worker = new Worker(new URL('./solver.worker.js', import.meta.url), { type: 'module' })
  worker.onmessage = (event) => {
    const { id, ok, result, error } = event.data || {}
    const entry = pending.get(id)
    if (!entry) return
    pending.delete(id)
    ok ? entry.resolve(result) : entry.reject(new Error(error))
  }
  worker.onerror = (event) => {
    // A worker-level failure (the module failing to load, say) never resolves
    // the in-flight calls otherwise.
    const message = event.message || 'the solver worker failed to start'
    for (const [, entry] of pending) entry.reject(new Error(message))
    pending.clear()
  }
  return worker
}

function call(op, payload) {
  const id = nextId++
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
    getWorker().postMessage({ id, op, payload })
  })
}

/**
 * How long the last measured run of work took, in milliseconds.
 *
 * Wall clock from the main thread, so it includes the worker hop and the JSON at
 * both ends — which is the honest number, because that is what the page waited
 * for. Measured here rather than in Rust because `std::time::Instant` has no
 * wasm32 implementation and traps at runtime.
 */
let lastElapsedMs = 0

export function elapsedMs() {
  return lastElapsedMs
}

/** Run `work`, recording how long it took whether or not it succeeds. */
export async function timed(work) {
  const started = performance.now()
  try {
    return await work()
  } finally {
    lastElapsedMs = performance.now() - started
  }
}

/**
 * Swallow a failed analysis into `null`, but say why on the console.
 *
 * The `null` is deliberate — an overlay that cannot be computed should cost the
 * annotation and nothing else. Discarding the reason as well is not: it turns
 * "the trace silently did not appear" into something with no thread to pull.
 */
function optional(op, promise) {
  return promise.catch((error) => {
    console.warn(`[solver] ${op} failed: ${error?.message || error}`)
    return null
  })
}

/**
 * The 20-cell double-dummy table.
 *
 * Resolves to `{ tricks, total }` with `tricks` as rows in [`DD_SEATS`] order
 * and columns in [`DD_STRAINS`] order, or `null` if the deal could not be
 * solved.
 */
export function fetchDoubleDummy(hands) {
  return optional('dd table', call('ddTable', { dealstr: dealStringFrom(hands) }))
}

/**
 * Build the request body the two play operations share.
 *
 * `trump` takes the engine's spelling: a suit letter, or `NT`. Callers holding
 * a contract should pass `trumpFromContract(contract) || 'NT'`, since that
 * returns `null` for notrump.
 *
 * Everything here must be plain, structured-cloneable data: this crosses into a
 * worker, and `postMessage` cannot clone a Proxy. A caller passing Vue reactive
 * state — which is exactly what the app does — hands over proxied arrays, so
 * `plays` is copied into a plain one rather than forwarded. The failure mode if
 * this is missed is a `could not be cloned` DataCloneError, not a wrong answer.
 */
export function playRequest({ hands, trump, declarer, leader, plays }) {
  return {
    dealstr: dealStringFrom(hands),
    trump: trump || 'NT',
    declarer: String(declarer),
    leader: String(leader),
    plays: Array.from(plays, String),
  }
}

/**
 * The running trace: every card played, with what it cost.
 *
 * Resolves to `{ contract_tricks, trace: [{ index, seat, card, cost }], cached }`
 * or `null`. A `cost` above zero is a card that gave away that many tricks, and
 * it is comparable across seats — declarer's errors and the defence's are
 * counted the same way.
 */
export function fetchDdPlay(request) {
  return optional(
    'dd play',
    call('ddPlay', { request }).then((result) => {
      if (!Array.isArray(result?.trace)) throw new Error('response carried no trace')
      return result
    })
  )
}

/**
 * One decision point's alternatives.
 *
 * `node` indexes into `request.plays`. Resolves to
 * `{ index, seat, card, cost, alternatives: [{ card, tricks, cost }] }` or
 * `null`.
 */
export function fetchDdPlayNode(request, node) {
  return optional(
    'dd play node',
    call('ddPlayNode', { request, node }).then((result) => {
      if (!Array.isArray(result?.alternatives)) {
        throw new Error('response carried no alternatives')
      }
      return result
    })
  )
}

/**
 * A double-dummy-perfect continuation from one point in the hand.
 *
 * Resolves to `{ from, cards, seats, declaring_tricks }` or `null`. Started at the
 * first costed error, this is the correction for it: what should have happened.
 */
export function fetchOptimalLine(request, from) {
  return optional(
    'optimal line',
    call('ddOptimalLine', { request, from }).then((result) => {
      if (!Array.isArray(result?.cards)) throw new Error('response carried no line')
      return result
    })
  )
}

/**
 * Parse a LIN string or a BBO handviewer URL.
 *
 * Resolves to the parsed board, or rejects with the engine's message — unlike
 * the analysis calls, this one *is* load-bearing: if the input cannot be read
 * there is nothing to show, and the user needs to be told why.
 */
export function parseLin(input) {
  return call('parseLin', { input })
}

/**
 * Parse a multi-board LIN file.
 *
 * Resolves to an array of `{ ok }` / `{ error }` entries, one per line, so a
 * single unreadable board does not cost the rest of the file.
 */
export function parseLinFile(content) {
  return call('parseLinFile', { content })
}

/** How many positions the session cache is holding. */
export function cacheSize() {
  return optional('cache size', call('cacheSize', {})).then((n) => n ?? 0)
}

/**
 * Encode a decoded table back into the 20-character `ddtricks` string.
 *
 * Seat-major over `N, S, E, W` and strain over `NT, S, H, D, C` — note both
 * orders differ from the decoded table's. This is the interchange format the
 * classroom app and the older BSOL service both use, and keeping an encoder
 * here means a table from this engine can be compared against either.
 */
export function encodeDdTricks(tricks) {
  const seatRow = { N: 0, E: 1, S: 2, W: 3 }
  const strainCol = { NT: 4, S: 3, H: 2, D: 1, C: 0 }
  let out = ''
  for (const seat of ['N', 'S', 'E', 'W']) {
    for (const strain of ['NT', 'S', 'H', 'D', 'C']) {
      const n = tricks[seatRow[seat]][strainCol[strain]]
      out += n < 10 ? String(n) : String.fromCharCode(97 + n - 10)
    }
  }
  return out
}
