// Working out what the user pasted, and turning it into boards.
//
// Three accepted forms: a PBN board or file, a LIN record or file, and a BBO
// handviewer URL. They are distinguishable without asking, so the input is one
// box rather than three tabs.

import { seatAtIndex } from './cards.js'
import { parseDealString, splitPbnBoards } from './deal.js'
import { parseLin, parseLinFile } from './solver.js'

/** What kind of thing a paste is. */
export const INPUT_KINDS = { PBN: 'pbn', LIN: 'lin', URL: 'url', DEAL: 'deal', UNKNOWN: 'unknown' }

/**
 * BBO's own link shortener, which is what "Export handviewer link" hands you.
 *
 * Worth recognising precisely because we cannot follow it, and the reason is
 * not a technical shortcoming to be worked around later:
 *
 * * **Not from the browser.** The redirect comes back without an
 *   `Access-Control-Allow-Origin` header, so script cannot read its `Location`
 *   cross-origin — and this page's own `connect-src 'self'` forbids the request
 *   before that even applies.
 * * **Not from a server of ours either.** Expanding it would be a few lines in
 *   a Function, and the expanded URL carries the deal *and the players'
 *   usernames* (`pn|…` is in every handviewer link). Routing that through our
 *   infrastructure would make the deal leave the device, which is the one thing
 *   this site promises never happens, and would specifically falsify the
 *   privacy page's claim that BBO usernames never reach us.
 *
 * So the short link is a dead end by design, and the useful thing to do is say
 * so and explain the one-step way around it.
 */
const BBO_SHORT_LINK = /(^|\/\/|\s)tinyurl\.bridgebase\.com\//i

/**
 * Classify input by its own markers rather than by guessing.
 *
 * Checked most specific first: a handviewer URL contains a `lin=` parameter, LIN
 * is pipe-delimited with known tokens, PBN has bracketed tags, and a bare deal
 * string is the `N:...` form on its own.
 */
export function detectKind(text) {
  const s = String(text || '').trim()
  if (!s) return INPUT_KINDS.UNKNOWN

  if (/[?&#]lin=/.test(s) || /^lin=/.test(s)) return INPUT_KINDS.URL
  // `md|` or `pn|` are present in every real LIN record; the pipe alone is not
  // enough, since PBN commentary can contain one.
  if (/(^|\|)(pn|md|qx|sv|mb|pc)\|/.test(s)) return INPUT_KINDS.LIN
  if (/\[\s*\w+\s+"/.test(s)) return INPUT_KINDS.PBN
  if (parseDealString(s)) return INPUT_KINDS.DEAL

  return INPUT_KINDS.UNKNOWN
}

/**
 * A board as the rest of the app consumes it.
 *
 * Both input paths converge here, so a component never needs to know whether it
 * is looking at a PBN board or a LIN record. `plays` may be empty (a deal with
 * no play record) and `contract` may be null (a deal with no auction), in which
 * case only the DD table is available and there is no trace to show.
 */
function board({
  hands,
  dealer = 'N',
  vulnerable = 'None',
  contract = null,
  declarer = null,
  leader = null,
  plays = [],
  auction = [],
  names = null,
  label = null,
  claim = null,
  source,
}) {
  return {
    hands,
    dealer,
    vulnerable,
    contract,
    declarer,
    /*
     * The opening lead is declarer's LHO, so a board that names a declarer implies
     * one even when it does not state it. PBN carries a contract without ever
     * carrying a leader, and without this the whole analysis was unavailable for
     * such a board — there was nothing wrong with it except a field nobody wrote
     * down.
     */
    leader: leader || (declarer ? seatAtIndex(declarer, 1) : null),
    plays,
    auction,
    names,
    label,
    claim,
    source,
  }
}

/**
 * Turn a paste into boards.
 *
 * Resolves to `{ kind, boards, problems }`. `problems` carries a message per
 * board that could not be read, so a file with one bad line reports it rather
 * than silently returning fewer boards than it has lines.
 *
 * @throws if the input cannot be recognised at all, or if a LIN input fails
 *   outright — the caller has nothing to display in either case.
 */
export async function parseInput(text) {
  const s = String(text || '').trim()
  const kind = detectKind(s)

  switch (kind) {
    case INPUT_KINDS.URL:
      return { kind, boards: [fromLinInput(await parseLin(s))], problems: [] }

    case INPUT_KINDS.LIN:
      return parseLinInput(s)

    case INPUT_KINDS.PBN:
      return parsePbnInput(s)

    case INPUT_KINDS.DEAL: {
      const hands = parseDealString(s)
      return { kind, boards: [board({ hands, source: 'deal' })], problems: [] }
    }

    default:
      if (BBO_SHORT_LINK.test(s)) {
        throw new Error(
          'That is a shortened BBO link, and this page cannot open it — expanding it ' +
            'would mean sending the deal to a server, and the hand never leaves your ' +
            'browser. Open the link in a new tab, then copy the full address from the ' +
            'address bar and paste that here instead.'
        )
      }
      throw new Error(
        'That does not look like a PBN board, a LIN record or a BBO handviewer URL.'
      )
  }
}

/** One line or many: a LIN file is line-oriented, so try the file path first. */
async function parseLinInput(text) {
  const lines = text.split('\n').filter((l) => l.trim() !== '')

  if (lines.length === 1) {
    return { kind: INPUT_KINDS.LIN, boards: [fromLinInput(await parseLin(text))], problems: [] }
  }

  const entries = await parseLinFile(text)
  const boards = []
  const problems = []
  entries.forEach((entry, i) => {
    if (entry.ok) boards.push(fromLinInput(entry.ok))
    else problems.push(`Board ${i + 1}: ${entry.error}`)
  })

  if (!boards.length) {
    throw new Error(
      problems.length
        ? `No board in that file could be analysed.\n${problems.join('\n')}`
        : 'No boards found in that LIN file.'
    )
  }
  return { kind: INPUT_KINDS.LIN, boards, problems }
}

function parsePbnInput(text) {
  const parsed = splitPbnBoards(text)
  if (!parsed.length) {
    throw new Error('No [Deal "..."] tag found in that PBN.')
  }
  const boards = parsed.map((b) =>
    board({
      hands: b.hands,
      dealer: b.dealer,
      vulnerable: b.vulnerable,
      contract: b.contract || null,
      declarer: b.declarer || null,
      label: b.board ? `Board ${b.board}` : null,
      source: 'pbn',
    })
  )
  return { kind: INPUT_KINDS.PBN, boards, problems: [] }
}

/**
 * Adapt the engine's parsed LIN to the shared board shape.
 *
 * The engine hands back a `request` carrying the PBN deal string, so the hands
 * are re-derived from it rather than duplicated — that string is also what the
 * position cache keys on, so there is exactly one source for it.
 */
function fromLinInput(parsed) {
  const hands = parseDealString(parsed.request.dealstr)
  const c = parsed.contract
  return board({
    hands,
    dealer: parsed.dealer,
    vulnerable: parsed.vulnerability,
    contract: c.description,
    declarer: c.declarer,
    leader: parsed.request.leader,
    plays: parsed.request.plays,
    auction: parsed.auction,
    names: parsed.player_names,
    label: parsed.board,
    claim: parsed.claim,
    source: 'lin',
  })
}
