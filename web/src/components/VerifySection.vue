<script setup>
/**
 * The privacy claim, and how to check it — collapsed, at the foot of the page.
 *
 * `pdf-handouts` argues this case at the top and at length, and rightly: the
 * files it takes are your own documents. A bridge hand is not that. It is four
 * hands of cards from a game that has already been played, usually already public
 * on BBO, and there is nothing in it to protect. Claiming otherwise at the top of
 * the page would be selling a benefit nobody is buying.
 *
 * So the fact is stated in one line and the argument is folded away for anyone who
 * wants it. What earns its place in the summary is the solve time, because that is
 * the thing you would otherwise not believe: the analysis really did happen here,
 * and it took a fraction of a second.
 */
defineProps({
  /** Wall-clock milliseconds for the last solve, if one has run. */
  elapsedMs: { type: Number, default: 0 },
})
</script>

<template>
  <details class="verify" id="verify">
    <summary>
      <span class="claim">This solve happened entirely in your browser</span>
      <span v-if="elapsedMs > 0" class="timing">
        in {{ (elapsedMs / 1000).toFixed(2) }} s
      </span>
      <span class="more">how to check</span>
    </summary>

    <div class="body">
      <p class="intro">
        The solver is compiled to WebAssembly and shipped with the page. Nothing is
        uploaded, and nothing needs to be — here is how to confirm that without
        trusting us.
      </p>

      <ol class="checks">
        <li>
          <strong>Pull the plug.</strong> Load this page, then turn off your wifi.
          Paste a hand and analyse it. It still works, because the solver is
          already here — there is nothing left to talk to.
        </li>
        <li>
          <strong>Watch the network.</strong> Open developer tools, go to the
          Network tab, clear it, then analyse a hand. There are no requests. The
          only traffic this page makes is fetching itself, and the solver, when it
          first loads.
        </li>
        <li>
          <strong>Make it try to leak.</strong> This page ships a
          <a href="https://developer.mozilla.org/docs/Web/HTTP/CSP">
            Content Security Policy
          </a>
          the <em>browser</em> enforces — the page cannot switch it off. Paste this
          into the console and watch every attempt get refused:
          <pre><code>fetch('https://example.com/x', {method:'POST', body:'test'});
new WebSocket('wss://example.com/x');
navigator.sendBeacon('https://example.com/x', 'test');
new Image().src = 'https://example.com/x?d=test';</code></pre>
        </li>
      </ol>

      <p class="caveat">
        The one caveat, stated plainly: the policy is
        <code>connect-src 'self'</code> rather than <code>'none'</code>, because the
        WebAssembly solver is itself fetched from this origin when the page loads.
        That origin is GitHub Pages, which serves static files and has nothing that
        could receive an upload.
      </p>
    </div>
  </details>
</template>

<style scoped>
.verify {
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: var(--bg-white);
}

summary {
  cursor: pointer;
  padding: 9px 14px;
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
  font-size: 13px;
  border-radius: var(--radius-card);
}

summary:hover .more {
  color: var(--green-hover);
  text-decoration: underline;
}

summary:focus-visible {
  outline: 2px solid var(--green);
  outline-offset: -2px;
}

.claim {
  color: var(--text-secondary);
}

.timing {
  color: var(--green);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.more {
  margin-left: auto;
  color: var(--text-muted);
  font-size: 12px;
}

.body {
  padding: 0 14px 14px;
  border-top: 1px solid var(--border);
  margin-top: 2px;
  padding-top: 12px;
}

.intro {
  margin: 0 0 10px;
  font-size: 13px;
  color: var(--text-secondary);
  max-width: 74ch;
}

.checks {
  margin: 0;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 9px;
  font-size: 13px;
  color: var(--text-secondary);
}

.checks li {
  max-width: 74ch;
}

pre {
  margin: 7px 0 0;
  font-size: 11px;
}

.caveat {
  margin: 12px 0 0;
  font-size: 12px;
  color: var(--text-muted);
  max-width: 74ch;
}

.caveat code {
  background: var(--bg-warm);
  padding: 1px 4px;
  border-radius: 3px;
  border: 1px solid var(--border);
}
</style>
