<script setup>
/**
 * A quiet line at the foot of the page: where the solve happened, and how long
 * it took.
 *
 * This used to carry the argument as well — three ways to prove nothing was
 * uploaded, a snippet that tried to leak and got refused by the CSP, a caveat
 * about `connect-src`. All of that is gone, deliberately.
 *
 * The reasoning is the same one that demoted the privacy claim from the top of
 * the page to a footnote in the first place. A bridge hand is four hands of
 * cards from a game already played, usually already public on BBO. Nobody is
 * anxious about it. Mounting a defence nobody asked for does not reassure — it
 * invites the question of why we are raising it at all, and leaves a reader
 * wondering what the fuss is about. The fact is worth stating once, plainly,
 * and then getting out of the way.
 *
 * What does earn its place is the solve time, because that is the part you
 * would otherwise not believe: the analysis really did happen here, and it took
 * a fraction of a second. The Privacy link covers anyone who wants the detail.
 */
defineProps({
  /** Wall-clock milliseconds for the last solve, if one has run. */
  elapsedMs: { type: Number, default: 0 },
})
</script>

<template>
  <p class="note">
    <span class="claim">
      This solve happened in your browser<span v-if="elapsedMs > 0" class="timing">
        in {{ (elapsedMs / 1000).toFixed(2) }} s</span >
    </span>
    <span class="links">
      <a href="./stats.html">Statistics</a>
      <a href="./privacy.html">Privacy</a>
    </span>
  </p>
</template>

<style scoped>
.note {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-wrap: wrap;
  margin: 0;
  padding: 9px 14px;
  font-size: 13px;
  color: var(--text-secondary);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: var(--bg-white);
}

.timing {
  color: var(--green);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.links {
  margin-left: auto;
  display: flex;
  gap: 12px;
}

.links a {
  color: var(--text-muted);
  font-size: 12px;
}
</style>
