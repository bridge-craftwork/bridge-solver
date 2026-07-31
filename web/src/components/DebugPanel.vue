<script setup>
/**
 * The cold-start and capability readout, behind `?debug=1`.
 *
 * This is the panel you ask someone to screenshot when they say the page is
 * slow on their device. A single total cannot distinguish a slow network from a
 * slow CPU from an old browser refusing to instantiate, and those have nothing
 * in common as fixes — so every segment is shown separately, with the device's
 * own capability readout underneath.
 *
 * Read once on mount and refreshed on demand rather than polled: these are
 * page-lifetime facts, and a panel that re-rendered continuously would perturb
 * the very timings it exists to report.
 */
import { ref, watch } from 'vue'
import { capabilities, poolSize, solveHistory, timings, warmupRatio } from '../lib/perf.js'

const props = defineProps({
  /** Wall clock to the trace, in ms — the wait that actually matters. */
  traceMs: { type: Number, default: 0 },
  /** Wall clock for the whole solve, in ms. */
  solveMs: { type: Number, default: 0 },
  /** Wall clock for the verdict pass, in ms. */
  verdictMs: { type: Number, default: 0 },
  /** Device speed score from the probe, reference machine = 100. */
  benchScore: { type: Number, default: null },
})

const caps = ref(capabilities())
const segments = ref(timings())
const solves = ref(solveHistory())
const warmup = ref(warmupRatio())

function refresh() {
  caps.value = capabilities()
  segments.value = timings()
  solves.value = solveHistory()
  warmup.value = warmupRatio()
}

/*
 * The cold-start segments do not exist yet when this mounts — the wasm is still
 * being fetched — so a panel that only read once showed four dashes for the
 * whole page's life, which is exactly the information it exists to carry.
 * Re-read whenever an analysis finishes, which is the point by which every
 * segment has landed.
 */
watch(() => [props.traceMs, props.solveMs, props.verdictMs], refresh)

/** A timing that has not been taken reads as unmeasured, not as instant. */
const ms = (n) => (n == null || n === 0 ? '—' : `${Math.round(n)} ms`)

/** Everything in one block, so a bug report can be pasted rather than described. */
function copy() {
  const lines = [
    `bridge-solver debug`,
    `wasm_fetch        ${ms(segments.value.wasmFetch)}`,
    `wasm_compile      ${ms(segments.value.wasmCompile)}`,
    `wasm_instantiate  ${ms(segments.value.wasmInstantiate)}`,
    `first_solve       ${ms(segments.value.firstSolve)}`,
    `trace             ${ms(props.traceMs)}`,
    `solve (total)     ${ms(props.solveMs)}`,
    `verdicts          ${ms(props.verdictMs)}`,
    `solves            ${solves.value.map((n) => Math.round(n)).join(', ') || '—'}`,
    `warmup 1st/3rd    ${warmup.value ? `${warmup.value.toFixed(2)}x` : '—'}`,
    `bench score       ${props.benchScore ?? '—'}`,
    `cores             ${caps.value.cores ?? '—'} (pool ${poolSize(caps.value)})`,
    `deviceMemory      ${caps.value.memoryGb ? `${caps.value.memoryGb} GB` : '—'}`,
    `simd              ${caps.value.simd ? 'yes' : 'no'}`,
    `crossOriginIsolated ${caps.value.crossOriginIsolated ?? '—'}`,
    `userAgent         ${navigator.userAgent}`,
  ]
  navigator.clipboard?.writeText(lines.join('\n')).catch(() => {})
}
</script>

<template>
  <section class="debug">
    <header>
      <h2>Debug</h2>
      <div>
        <button type="button" @click="refresh">Refresh</button>
        <button type="button" @click="copy">Copy</button>
      </div>
    </header>

    <div class="cols">
      <div>
        <h3>Cold start</h3>
        <dl>
          <dt>wasm fetch</dt>
          <dd>{{ ms(segments.wasmFetch) }}</dd>
          <dt>wasm compile</dt>
          <dd>{{ ms(segments.wasmCompile) }}</dd>
          <dt>wasm instantiate</dt>
          <dd>{{ ms(segments.wasmInstantiate) }}</dd>
          <dt>first solve</dt>
          <dd>{{ ms(segments.firstSolve) }}</dd>
        </dl>
      </div>

      <div>
        <h3>This analysis</h3>
        <dl>
          <dt>trace</dt>
          <dd>{{ ms(traceMs) }}</dd>
          <dt>solve (total)</dt>
          <dd>{{ ms(solveMs) }}</dd>
          <dt>verdicts</dt>
          <dd>{{ ms(verdictMs) }}</dd>
          <dt title="First solve divided by the third, on this page load">warm-up 1st/3rd</dt>
          <dd>{{ warmup ? `${warmup.toFixed(2)}x` : '—' }}</dd>
          <dt title="Device speed from the probe; the reference machine is 100">bench score</dt>
          <dd>{{ benchScore ?? '—' }}</dd>
        </dl>
      </div>

      <div>
        <h3>Device</h3>
        <dl>
          <dt>cores</dt>
          <dd>{{ caps.cores ?? '—' }} <small>(pool {{ poolSize(caps) }})</small></dd>
          <dt>deviceMemory</dt>
          <dd>{{ caps.memoryGb ? `${caps.memoryGb} GB` : '—' }}</dd>
          <dt>SIMD</dt>
          <dd>{{ caps.simd ? 'yes' : 'no' }}</dd>
          <dt>crossOriginIsolated</dt>
          <dd>{{ caps.crossOriginIsolated ?? '—' }}</dd>
        </dl>
      </div>
    </div>

    <p v-if="solves.length" class="history">
      Solves so far: {{ solves.map((n) => Math.round(n)).join(' · ') }} ms
    </p>

    <p class="note">
      None of this is transmitted. <code>cores</code> and <code>deviceMemory</code> are the two
      strongest passive fingerprinting signals here, and are read only to size the worker pool.
    </p>
  </section>
</template>

<style scoped>
.debug {
  margin: 1rem 0;
  padding: 0.75rem 1rem;
  border: 1px solid var(--border, #ccc);
  border-radius: 6px;
  background: var(--surface-2, #f6f6f6);
  font-size: 0.85rem;
}

header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 1rem;
}

h2 {
  margin: 0;
  font-size: 0.95rem;
}

h3 {
  margin: 0 0 0.35rem;
  font-size: 0.8rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  opacity: 0.7;
}

.cols {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
  gap: 1rem;
  margin-top: 0.75rem;
}

dl {
  display: grid;
  grid-template-columns: auto auto;
  gap: 0.15rem 0.6rem;
  margin: 0;
}

dt {
  opacity: 0.75;
}

dd {
  margin: 0;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

button {
  font: inherit;
  margin-left: 0.4rem;
}

.history,
.note {
  margin: 0.6rem 0 0;
  font-size: 0.78rem;
  opacity: 0.75;
}
</style>
