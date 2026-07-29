<script setup>
/**
 * Costed errors per player.
 *
 * Dummy's cards are credited to declarer, who actually chose them, and dummy
 * itself is scored as not applicable. That is the convention BBO's own BSOL
 * analysis uses, and matching it means these numbers can be compared against it
 * directly — verified against a real board where BSOL reports North 1, South 1
 * and declarer 3, which is this engine's per-seat 1/1/1 with dummy's 2 folded in.
 *
 * Attributing by card holder instead would show the same total split across two
 * rows, and read as though dummy had made mistakes of its own.
 */
import { computed } from 'vue'

const props = defineProps({
  /** `[{ index, seat, card, cost }]` from the running trace. */
  trace: { type: Array, default: () => [] },
  declarer: { type: String, default: null },
  /** Seat names from a LIN record, if there are any. */
  names: { type: Object, default: null },
})

const SEAT_ORDER = ['N', 'E', 'S', 'W']
const SEAT_NAMES = { N: 'North', E: 'East', S: 'South', W: 'West' }
const NAME_KEY = { N: 'north', E: 'east', S: 'south', W: 'west' }

const dummy = computed(() => {
  if (!props.declarer) return null
  return { N: 'S', S: 'N', E: 'W', W: 'E' }[props.declarer]
})

const rows = computed(() => {
  const cost = { N: 0, E: 0, S: 0, W: 0 }
  const count = { N: 0, E: 0, S: 0, W: 0 }

  for (const e of props.trace) {
    if (e.cost <= 0) continue
    // Fold dummy's cards into declarer.
    const seat = e.seat === dummy.value && props.declarer ? props.declarer : e.seat
    cost[seat] += e.cost
    count[seat] += 1
  }

  return SEAT_ORDER.map((seat) => ({
    seat,
    label: SEAT_NAMES[seat],
    name: props.names?.[NAME_KEY[seat]] || '',
    isDummy: seat === dummy.value,
    isDeclarer: seat === props.declarer,
    cost: cost[seat],
    count: count[seat],
  }))
})

const total = computed(() => rows.value.reduce((n, r) => n + r.cost, 0))
</script>

<template>
  <div v-if="trace.length" class="players">
    <div class="players-label">Tricks given away</div>
    <table class="players-table">
      <caption class="sr-only">
        Tricks each player gave away against double-dummy play. Dummy's cards
        count towards declarer, who chose them.
      </caption>
      <tbody>
        <tr v-for="row in rows" :key="row.seat" :class="{ dummy: row.isDummy }">
          <th scope="row">
            {{ row.seat }}
            <span v-if="row.name" class="pname">{{ row.name }}</span>
          </th>
          <td class="role">
            <span v-if="row.isDeclarer">decl</span>
            <span v-else-if="row.isDummy">dummy</span>
          </td>
          <td v-if="row.isDummy" class="na" title="Declarer plays dummy's cards">—</td>
          <td v-else class="count" :class="{ clean: row.cost === 0, costly: row.cost > 0 }">
            {{ row.cost }}
          </td>
        </tr>
      </tbody>
    </table>
    <p class="players-total">{{ total }} in total</p>
  </div>
</template>

<style scoped>
.players-label {
  font-size: 11px;
  color: #666;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 4px;
}

.players-table {
  border-collapse: collapse;
  font-size: 13px;
  background: var(--bg-white);
  min-width: calc(160px * var(--table-scale));
}

.players-table th,
.players-table td {
  border: 0.5px solid var(--rule);
  padding: 3px 8px;
  text-align: left;
}

.players-table th {
  background: var(--surface-alt);
  font-weight: 600;
}

.pname {
  font-weight: 400;
  color: var(--text-muted);
  font-size: 11px;
  margin-left: 4px;
  display: inline-block;
  max-width: 10ch;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}

.role {
  font-size: 10px;
  color: var(--green);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.count {
  text-align: center;
  font-variant-numeric: tabular-nums;
  font-weight: 700;
}

.count.clean {
  color: var(--green);
}

.count.costly {
  color: #c62828;
  background: var(--cost-mild);
}

.na {
  text-align: center;
  color: var(--text-muted);
}

tr.dummy th {
  color: var(--text-secondary);
}

.players-total {
  margin: 5px 0 0;
  font-size: 11px;
  color: var(--text-muted);
}
</style>
