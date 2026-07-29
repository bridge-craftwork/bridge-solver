<script setup>
/** A seat's box: who sits there, and what they hold. */
import HandDisplay from './HandDisplay.vue'

defineProps({
  seat: { type: String, required: true },
  hand: { type: Object, default: null },
  /** Player name from a LIN record, if there is one. */
  name: { type: String, default: '' },
  marks: { type: Object, default: null },
  inspectable: { type: Boolean, default: false },
  /** Frame this seat — used for the seat on lead at an inspected node. */
  active: { type: Boolean, default: false },
  /** Note beside the seat label, e.g. `Declarer` or `Dummy`. */
  role: { type: String, default: '' },
})

defineEmits(['card-click'])

const SEAT_NAMES = { N: 'North', E: 'East', S: 'South', W: 'West' }
</script>

<template>
  <div class="seat-panel" :class="{ active }">
    <div class="seat-head">
      <span class="seat-label">{{ SEAT_NAMES[seat] }}</span>
      <span v-if="role" class="seat-role">{{ role }}</span>
      <span v-if="name" class="seat-name" :title="name">{{ name }}</span>
    </div>
    <HandDisplay
      :hand="hand"
      :marks="marks"
      :inspectable="inspectable"
      @card-click="$emit('card-click', $event)"
    />
  </div>
</template>

<style scoped>
.seat-panel {
  background: var(--surface);
  border-radius: 8px;
  padding: calc(12px * var(--table-scale));
  /* Transparent by default so framing the active seat never shifts layout. */
  border: 2px solid transparent;
  min-width: min(calc(210px * var(--table-scale)), 100%);
}

.seat-panel.active {
  background: #e3f2fd;
  border-color: #2196f3;
}

.seat-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: calc(6px * var(--table-scale));
}

.seat-label {
  font-size: calc(12px * var(--table-scale));
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-secondary);
}

.seat-role {
  font-size: calc(10px * var(--table-scale));
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--green);
}

.seat-name {
  font-size: calc(12px * var(--table-scale));
  color: var(--text-muted);
  margin-left: auto;
  max-width: 11ch;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
