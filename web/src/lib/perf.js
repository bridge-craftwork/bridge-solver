// Cold-start instrumentation.
//
// Everything about this page's perceived speed is calibrated against four
// segments, and until they are measured separately any tuning is guesswork:
//
//   wasm_fetch        bytes over the wire (or out of the HTTP cache)
//   wasm_compile      WebAssembly.compile on those bytes
//   wasm_instantiate  binding the compiled module to its imports, per worker
//   first_solve       the first real analysis, which also pays for V8 warm-up
//
// Reported as one number they are indistinguishable, and they have completely
// different fixes: fetch wants compression and caching, compile wants a smaller
// binary, instantiate is per-worker and is the cost a worker pool multiplies,
// and first_solve is the only one the solver itself can do anything about.
//
// Marks go through `performance.mark`/`measure` as well as this module's own
// store, so a run is legible in devtools' performance timeline without the
// debug panel.

/** Segment timings for the current page load, in milliseconds. */
const segments = {
  wasmFetch: null,
  wasmCompile: null,
  wasmInstantiate: null,
  firstSolve: null,
}

/**
 * Solve times in call order, so warm-up can be seen rather than assumed.
 *
 * V8 tiers up a hot function over its first few calls, so an engine's first
 * solve can be several times its third on the same page load with nothing else
 * having changed. Anything quoting a single cold number is quoting the worst
 * case; anything quoting a warm one is quoting a case the user rarely gets.
 */
const solveTimes = []

/** Record `value` ms against `name`, and mirror it into the performance timeline. */
export function record(name, value) {
  if (name in segments) segments[name] = value
  try {
    performance.measure(`bridge-solver:${name}`, {
      start: performance.now() - value,
      duration: value,
    })
  } catch {
    // `measure` with a start/duration needs User Timing L3; a browser without
    // it still gets the number in `segments`, which is what the panel reads.
  }
  return value
}

/** Run `work`, recording how long it took under `name` whether or not it throws. */
export async function timeSegment(name, work) {
  const started = performance.now()
  try {
    return await work()
  } finally {
    record(name, performance.now() - started)
  }
}

/** Note that a solve took `ms`, for the warm-up ratio. */
export function recordSolve(ms) {
  solveTimes.push(ms)
  if (segments.firstSolve === null) record('firstSolve', ms)
}

/**
 * How much faster the third solve is than the first, or `null` before three.
 *
 * The plan asks whether the first solve completes before the engine tiers up.
 * This is that question as a number: 1.0 means warm-up costs nothing, 3.0 means
 * the first solve pays three times over and a warm-up solve would be worth it.
 */
export function warmupRatio() {
  if (solveTimes.length < 3 || !solveTimes[2]) return null
  return solveTimes[0] / solveTimes[2]
}

/** Every solve recorded so far, oldest first. */
export function solveHistory() {
  return solveTimes.slice()
}

/** A snapshot of the four segments. */
export function timings() {
  return { ...segments }
}

// A module whose body needs v128 to validate: `i32.const 0`, `i8x16.splat`,
// `i8x16.popcnt`. `WebAssembly.validate` answers without instantiating, so this
// is safe to run on a browser that would reject the opcode.
const SIMD_PROBE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7b, 0x03,
  0x02, 0x01, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00, 0x41, 0x00, 0xfd, 0x0f, 0xfd, 0x62, 0x0b,
])

/** Whether this browser accepts WebAssembly SIMD (`simd128`). */
export function hasSimd() {
  try {
    return WebAssembly.validate(SIMD_PROBE)
  } catch {
    return false
  }
}

/**
 * What this device says about itself.
 *
 * Used locally for pool sizing and for the debug panel. Deliberately *not*
 * transmitted: `hardwareConcurrency` and `deviceMemory` are the two strongest
 * passive fingerprinting signals in this list, and the telemetry design turns
 * on carrying no identifier.
 */
export function capabilities() {
  return {
    cores: navigator.hardwareConcurrency || null,
    memoryGb: navigator.deviceMemory || null,
    simd: hasSimd(),
    crossOriginIsolated: typeof crossOriginIsolated === 'boolean' ? crossOriginIsolated : null,
    // A worker pool is the plan's main lever, and it is worth showing when the
    // platform has quietly withheld it.
    workers: typeof Worker === 'function',
  }
}

/**
 * The pool size to use on this device.
 *
 * Not `hardwareConcurrency` itself. On every big.LITTLE phone and on Apple
 * silicon that count includes efficiency cores, which run a solve 2–3x slower
 * than a performance core; scheduling a share of the work onto one makes it the
 * straggler that gates the whole run. Half the reported count, capped at four,
 * keeps the work on cores that can carry it.
 *
 * Halved again when the device admits to 4 GB or less, because each worker
 * carries its own position cache and a phone that swaps mid-solve is slower
 * than one that never forked.
 */
export function poolSize({ cores, memoryGb } = capabilities()) {
  let n = Math.min(4, Math.ceil((cores || 2) / 2))
  if (memoryGb && memoryGb <= 4) n = Math.min(n, 2)
  return Math.max(1, n)
}

/** Reset every recorded timing. Test seam. */
export function reset() {
  for (const key of Object.keys(segments)) segments[key] = null
  solveTimes.length = 0
}
