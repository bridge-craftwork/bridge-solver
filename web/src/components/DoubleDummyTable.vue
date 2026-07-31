<script setup>
/**
 * The double-dummy table: tricks available to each seat in each strain.
 *
 * Re-vendored from Bridge-Classroom after its rewrite. Two things came with it.
 * Rows read `N, S, E, W` — partners adjacent, which is how a double-dummy table
 * is always drawn, and the ordering the collapse depends on. And identical
 * partnership rows merge into one `NS` / `EW` row, which is lossless (a pair only
 * merges when every cell already matches) and is the common case, since the two
 * hands of a partnership usually take the same tricks.
 *
 * Its `compact`, `rotated` and `par` props are not vendored: those exist to fit
 * the table into a corner of the classroom's grid arranger, and this page gives
 * it a column of its own. `diverged` is also dropped — it flagged a disagreement
 * with a bidding engine, which has no meaning on a played hand.
 */
import { computed } from 'vue'
import { buildDdRows, collapseDdRows, contractTarget, DISPLAY_STRAINS } from '../lib/ddTable.js'
import { SUIT_SYMBOLS, getSuitClass } from '../lib/cards.js'

const props = defineProps({
  /** The engine's `tricks[seat][strain]`, rows `N,E,S,W`, columns `C,D,H,S,NT`. */
  tricks: { type: Array, default: null },
  contract: { type: String, default: '' },
  declarer: { type: String, default: '' },
  /** Merge a partnership's rows when their tricks are identical. */
  collapse: { type: Boolean, default: true },
})

const SEAT_LABELS = {
  N: 'North',
  S: 'South',
  E: 'East',
  W: 'West',
  NS: 'North and South',
  EW: 'East and West',
}

const columns = computed(() =>
  DISPLAY_STRAINS.map((strain) => ({
    strain,
    label: strain === 'NT' ? 'NT' : SUIT_SYMBOLS[strain],
    cls: strain === 'NT' ? 'dd-nt' : getSuitClass(strain),
  }))
)

const rows = computed(() => {
  const built = buildDdRows(props.tricks, { contract: props.contract, declarer: props.declarer })
  return props.collapse ? collapseDdRows(built) : built
})

const target = computed(() => contractTarget(props.contract))

function cellTitle(seat, strain, cell) {
  const who = SEAT_LABELS[seat] || seat
  const what = strain === 'NT' ? 'notrump' : strain
  const base = `${who} ${seat.length > 1 ? 'take' : 'takes'} ${cell.tricks} tricks in ${what}`
  if (!cell.isContract || target.value == null) return base

  const diff = cell.tricks - target.value
  if (diff === 0) return `${base} — exactly makes ${props.contract}`
  if (diff > 0) return `${base} — makes ${props.contract} with ${diff} over`
  return `${base} — ${props.contract} is down ${-diff} on best play`
}
</script>

<template>
  <div v-if="rows" class="dd-wrap">
    <div class="dd-label">Double-dummy tricks</div>
    <table class="dd-table">
      <caption class="sr-only">
        Tricks available to each seat in each strain with both sides playing
        perfectly. A partnership shows as one row when both hands take the same
        tricks.
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
        <tr v-for="row in rows" :key="row.seat">
          <th scope="row" class="dd-seat">{{ row.seat }}</th>
          <td
            v-for="(cell, i) in row.cells"
            :key="i"
            :class="{ 'dd-contract': cell.isContract }"
            :title="cellTitle(row.seat, columns[i].strain, cell)"
          >
            {{ cell.tricks }}
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
  /* Room for a merged `NS` without the column jumping when it collapses. */
  min-width: 3ch;
}

.dd-contract {
  background: var(--green-pale);
  color: var(--green-ink);
  font-weight: 700;
}
</style>
