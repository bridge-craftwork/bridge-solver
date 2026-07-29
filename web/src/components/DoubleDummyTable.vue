<script setup>
/**
 * The 20-cell double-dummy table: how many tricks each seat takes in each
 * strain.
 *
 * Vendored from Bridge-Classroom, adapted at one point. Its version takes the
 * 20-character `ddtricks` interchange string and decodes it with `buildDdRows`;
 * this engine returns the table already structured, so the decode step is gone.
 * `solver.js` keeps an `encodeDdTricks` for anyone who needs that string back.
 *
 * The classroom's `diverged` prop is dropped — it flagged a disagreement with a
 * bidding engine's auction, which has no meaning here.
 */
import { computed } from 'vue'
import { DD_SEATS, DD_STRAINS } from '../lib/solver.js'
import { SUIT_SYMBOLS, getSuitClass } from '../lib/cards.js'

const props = defineProps({
  /** Rows in `N,E,S,W` order, columns in `C,D,H,S,NT` order. */
  tricks: { type: Array, default: null },
  /** Highlight the cell this contract lands on. */
  contract: { type: String, default: '' },
  declarer: { type: String, default: '' },
})

const SEAT_NAMES = { N: 'North', E: 'East', S: 'South', W: 'West' }

const columns = computed(() =>
  DD_STRAINS.map((strain) => ({
    strain,
    label: strain === 'NT' ? 'NT' : SUIT_SYMBOLS[strain],
    cls: strain === 'NT' ? 'dd-nt' : getSuitClass(strain),
  }))
)

/** Which cell the contract sits in, so it can be marked. */
const contractCell = computed(() => {
  if (!props.contract || !props.declarer) return null
  const m = String(props.contract)
    .trim()
    .match(/^[1-7]\s*(NT?|S|H|D|C)/i)
  if (!m) return null
  const raw = m[1].toUpperCase()
  const strain = raw === 'N' || raw === 'NT' ? 'NT' : raw
  return { seat: props.declarer.toUpperCase(), strain }
})

function isContract(seat, strain) {
  return contractCell.value?.seat === seat && contractCell.value?.strain === strain
}

/** Tricks needed to make the contract, for reading a cell as made or beaten. */
const contractTricks = computed(() => {
  const m = String(props.contract || '')
    .trim()
    .match(/^([1-7])/)
  return m ? Number(m[1]) + 6 : null
})

function cellTitle(seat, strain, value) {
  const base = `${SEAT_NAMES[seat]} takes ${value} tricks in ${strain === 'NT' ? 'notrump' : strain}`
  if (!isContract(seat, strain) || contractTricks.value == null) return base
  const diff = value - contractTricks.value
  if (diff === 0) return `${base} — exactly makes ${props.contract}`
  if (diff > 0) return `${base} — makes ${props.contract} with ${diff} over`
  return `${base} — ${props.contract} is down ${-diff}`
}
</script>

<template>
  <div v-if="tricks" class="dd-wrap">
    <div class="dd-label">Double-dummy tricks</div>
    <table class="dd-table">
      <caption class="sr-only">
        Tricks available to each seat in each strain, with both sides playing
        perfectly.
      </caption>
      <thead>
        <tr>
          <th scope="col"><span class="sr-only">Seat</span></th>
          <th v-for="col in columns" :key="col.strain" scope="col" :class="col.cls">
            {{ col.label }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(seat, row) in DD_SEATS" :key="seat">
          <th scope="row" class="dd-seat">{{ seat }}</th>
          <td
            v-for="(col, i) in columns"
            :key="col.strain"
            :class="{ 'dd-contract': isContract(seat, col.strain) }"
            :title="cellTitle(seat, col.strain, tricks[row][i])"
          >
            {{ tricks[row][i] }}
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.dd-label {
  font-size: 11px;
  color: #666;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 4px;
}

.dd-table {
  border-collapse: collapse;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  background: var(--bg-white);
}

.dd-table th,
.dd-table td {
  border: 0.5px solid var(--rule);
  padding: 4px 10px;
  text-align: center;
}

.dd-table th {
  background: var(--surface-alt);
  color: #666;
  font-weight: 600;
}

.dd-table th.dd-nt {
  color: var(--suit-black);
}

.dd-seat {
  background: var(--surface-alt);
  color: var(--text) !important;
  font-weight: 600;
}

.dd-contract {
  background: var(--green-pale);
  color: var(--green-ink);
  font-weight: 700;
}
</style>
