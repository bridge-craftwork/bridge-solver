// The public stats blob.
//
// Served at `/api/stats`, deliberately *not* `/stats` — that route belongs to
// the static page a reader actually visits, and a Function mounted there would
// shadow it. The page fetches this.
//
// This reads a pre-aggregated blob from KV rather than querying Analytics
// Engine per request, for two reasons. The AE SQL API needs an account-scoped
// API token, which must never reach a browser; and a cached blob keeps the page
// fast and immune to a traffic spike. A scheduled worker refreshes the blob
// once a day and holds that token as its own secret.
//
// Publishing these numbers openly is the strongest available backing for the
// privacy claim: anyone can see exactly what is collected, because all of it is
// on the screen.

interface Env {
  SOLVER_STATS: KVNamespace
}

/** The key the scheduled aggregation writes. */
const KEY = 'aggregate'

export const onRequestGet: PagesFunction<Env> = async ({ env }) => {
  const blob = await env.SOLVER_STATS.get(KEY, 'text')

  if (blob === null) {
    // Before the first aggregation has run there is genuinely nothing to show.
    // Say so plainly rather than serving an empty object that would render as
    // "0 solves" and read as a factual claim about usage.
    return Response.json(
      { status: 'pending', detail: 'No aggregate has been written yet.' },
      { status: 503, headers: { 'Cache-Control': 'no-store' } }
    )
  }

  return new Response(blob, {
    headers: {
      'Content-Type': 'application/json; charset=utf-8',
      // The blob changes once a day, so an hour of edge caching costs nothing
      // in freshness and takes the read off KV entirely for most visitors.
      'Cache-Control': 'public, max-age=3600',
    },
  })
}
