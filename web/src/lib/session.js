// Where a hand comes from when the page opens, and what is remembered.
//
// Three sources, most specific first: the URL, then the last hand you looked at,
// then nothing. The URL wins because a link is an explicit instruction — someone
// sent you *that* hand — while storage is only a convenience.
//
// The URL form is also what makes the page embeddable. A host site can point an
// iframe at it with a hand and a set of options and get a working analysis with no
// scripting, which is why the parameters are plain and documented rather than an
// opaque blob.

/** Every option the URL can carry, with the default it falls back to. */
export const DEFAULTS = {
  /** Turn the table so declarer sits at the bottom. */
  declarerSouth: true,
  /** Strip the page down to the analysis, for embedding in another site. */
  embed: false,
  /** Constrain the app to a fixed pixel box, e.g. to preview a viewport. */
  width: null,
  height: null,
  /** Show the cold-start and device readout. Diagnostic, never persisted. */
  debug: false,
}

/**
 * The hand and options carried by the current URL.
 *
 * Accepted for the hand, in order: `hand`, `lin`, `pbn`, `url`, `board`. They are
 * all the same thing — a paste — and named variously because a host site will
 * reach for whichever word describes what it holds. The value is a full LIN
 * record, a PBN board, a handviewer URL, or a bare deal string; the parser works
 * out which.
 *
 * One wrinkle worth knowing: a handviewer URL contains its own `lin=` parameter,
 * so passing one raw would collide with this page's. Hosts should encode it
 * (`encodeURIComponent`), and `URLSearchParams` decodes it back before the hand
 * parser sees it, so a correctly-encoded URL survives intact.
 */
export function readUrl(search, hash) {
  // Read the location lazily and defensively: these run under a test runner and
  // inside a worker-less module graph as well as in a page, and a default argument
  // referencing `window` is evaluated even when only the other one is supplied.
  const loc = typeof window === 'undefined' ? null : window.location
  search = search ?? loc?.search ?? ''
  hash = hash ?? loc?.hash ?? ''

  // Options may live in the query or after the hash, so a static host that cannot
  // rewrite query strings can still drive it.
  const params = new URLSearchParams(search)
  const hashQuery = hash.includes('?') ? hash.slice(hash.indexOf('?') + 1) : ''
  for (const [k, v] of new URLSearchParams(hashQuery)) {
    if (!params.has(k)) params.set(k, v)
  }

  const hand = ['hand', 'lin', 'pbn', 'url', 'board']
    .map((key) => params.get(key))
    .find((v) => v && v.trim())

  /*
   * Only options the URL actually mentions. Filling in the rest with defaults here
   * would make "absent" indistinguishable from "explicitly set to the default",
   * and an absent parameter would then silently overwrite a stored preference —
   * which is exactly what it did: `declarerSouth` came back on every reload
   * however you had left it.
   */
  const options = {}
  if (params.has('declarerSouth')) options.declarerSouth = readBool(params, 'declarerSouth')
  if (params.has('embed')) options.embed = readBool(params, 'embed')
  if (params.has('debug')) options.debug = readBool(params, 'debug')
  const width = readInt(params, ['width', 'vw', 'w'])
  if (width !== null) options.width = width
  const height = readInt(params, ['height', 'vh', 'h'])
  if (height !== null) options.height = height

  return {
    hand: hand ? hand.trim() : null,
    options,
    /** Whether the URL asked for anything at all. */
    present: Boolean(hand) || Object.keys(options).length > 0,
  }
}

/**
 * `1`/`true`/`yes`/`on` are all true, and the bare presence of the key is true —
 * `?embed` reads as `embed=1`, which is what anyone hand-writing a URL expects.
 */
function readBool(params, key) {
  const raw = (params.get(key) || '').trim().toLowerCase()
  if (raw === '') return true
  return ['1', 'true', 'yes', 'on'].includes(raw)
}

function readInt(params, keys) {
  for (const key of keys) {
    if (!params.has(key)) continue
    const n = Number.parseInt(params.get(key), 10)
    if (Number.isFinite(n) && n > 0) return n
  }
  return null
}

const STORAGE_KEY = 'bridge-solver.session.v1'

/**
 * Remember the hand and options, so returning to the page resumes where you left
 * off.
 *
 * Wrapped because storage throws rather than degrades in the cases that matter:
 * Safari's private mode, a browser configured to block it, or a quota that is
 * full. None of those should cost you the analysis you are looking at.
 */
export function save(hand, options) {
  try {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ hand, options, savedAt: new Date().toISOString() })
    )
  } catch {
    // Storage is a convenience; losing it is not worth a broken page.
  }
}

/** The last hand and options, or `null` if there is nothing usable stored. */
export function load() {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw)
    if (!parsed?.hand || typeof parsed.hand !== 'string') return null
    return {
      hand: parsed.hand,
      // Only known keys, so a stored blob from an older version cannot inject
      // options this build does not understand.
      options: pickOptions(parsed.options),
      savedAt: parsed.savedAt || null,
    }
  } catch {
    return null
  }
}

export function forget() {
  try {
    window.localStorage.removeItem(STORAGE_KEY)
  } catch {
    // Nothing to do — see `save`.
  }
}

function pickOptions(stored) {
  const out = {}
  for (const key of Object.keys(DEFAULTS)) {
    if (stored && key in stored) out[key] = stored[key]
  }
  return out
}

/**
 * Resolve what to open with: the URL if it says anything, else what was stored.
 *
 * The viewport box, embed flag and debug flag are deliberately *not* restored
 * from storage — they describe the frame the page is in, not the hand you were
 * looking at, and carrying them over would leave someone stuck in a 390px box
 * they cannot get out of, or reading a diagnostic panel they asked for once.
 */
export function resolveInitial(url = readUrl(), stored = load()) {
  const options = { ...DEFAULTS, ...url.options }

  if (url.hand) return { hand: url.hand, options, source: 'url' }

  if (stored?.hand) {
    const { width, height, embed, debug, ...carried } = stored.options || {}
    return {
      hand: stored.hand,
      // A URL option still beats a stored one.
      options: { ...DEFAULTS, ...carried, ...url.options },
      source: 'storage',
      savedAt: stored.savedAt,
    }
  }

  return { hand: null, options, source: 'none' }
}
