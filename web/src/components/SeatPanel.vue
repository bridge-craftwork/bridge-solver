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
  /** Note beside the seat badge, e.g. `Declarer` or `Dummy`. */
  role: { type: String, default: '' },
})

defineEmits(['card-click'])

const SEAT_NAMES = { N: 'North', E: 'East', S: 'South', W: 'West' }
</script>

<template>
  <div class="seat-panel" :class="{ active }">
    <div class="seat-head">
      <!--
        A circled letter rather than the word, following the classroom's
        SeatIndicator badge. Four boxes headed NORTH / EAST / SOUTH / WEST spent
        more ink on the labels than on the cards, and the compass is already
        legible from the layout — the badge is confirmation, not information.
      -->
      <span class="seat-badge" :title="SEAT_NAMES[seat]">{{ seat }}</span>
      <span class="sr-only">{{ SEAT_NAMES[seat] }}</span>
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
  padding: calc(10px * var(--table-scale));
  /* Transparent by default so framing the active seat never shifts layout. */
  border: 2px solid transparent;
  min-width: min(calc(200px * var(--table-scale)), 100%);
}

.seat-panel.active {
  background: #e3f2fd;
  border-color: #2196f3;
}

.seat-head {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: calc(5px * var(--table-scale));
}

.seat-badge {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: calc(22px * var(--table-scale));
  height: calc(22px * var(--table-scale));
  border-radius: 50%;
  background: var(--bg-white);
  color: var(--text);
  font-weight: 700;
  font-size: calc(13px * var(--table-scale));
  line-height: 1;
  border: 1.5px solid rgba(0, 0, 0, 0.15);
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
  max-width: 12ch;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
