//! Baseline measurement for the browser build's performance work.
//!
//! Everything in the performance plan is calibrated against numbers this
//! produces, so it exists before any tuning does. Three things come out of it:
//!
//! * **A difficulty distribution.** Deals are generated from a seeded RNG and
//!   ranked by what they actually cost to solve, rather than by a guess at what
//!   makes a deal hard. p50/p95/p99 off that ranking is what decides whether a
//!   per-card timeout is needed at all.
//! * **A frozen fixture set**, ten deals spanning that distribution, written out
//!   with their double-dummy tables so the set is a correctness check as well as
//!   a benchmark. A device reporting a different table has a real wasm bug,
//!   which is worth more than any timing.
//! * **A cost breakdown per work unit**, which is the measurement that decides
//!   whether a worker pool helps. The three units have very different cache
//!   behaviour and only two of them can be split without losing anything.
//!
//! ```text
//! cargo run --release --example bench_fixtures --features play-analysis -- survey
//! cargo run --release --example bench_fixtures --features play-analysis -- units
//! ```
//!
//! Release matters: a debug build is roughly two orders of magnitude slower and
//! its ranking of "hard" deals is not the release build's ranking.

use std::collections::HashMap;
use std::time::Instant;

use bridge_encodings::pbn::dd_table_to_pbn;
use bridge_solver::analyse_play::{node_alternatives, prefix_keys, running_trace, PlayInput};
use bridge_solver::{solve_dd_table, Hands};
use bridge_types::Deal;

/// The verified board: BBO board 3, 3NT by West, claimed after 41 cards.
///
/// Anchored on North, which is what the position cache keys on. Its
/// double-dummy table is pinned against BSOL's own `ddtricks` elsewhere in the
/// suite, so timing the real analysis here also re-checks the answer.
const VERIFIED_DEAL: &str = "N:J98.QT83.K6.J853 Q762.J4.QJT5.AT6 KT43.652.A984.94 A5.AK97.732.KQ72";

/// The 41 cards actually played on that board, in order.
const VERIFIED_PLAYS: &[&str] = &[
    "C3", "CT", "C4", "C2", "DQ", "D4", "D2", "DK", "SJ", "S2", "S3", "SA", "D3", "D6", "DJ", "DA",
    "C9", "C7", "C5", "CA", "DT", "D8", "D7", "S8", "HJ", "H6", "H7", "HQ", "S9", "SQ", "SK", "S5",
    "ST", "H9", "C8", "S6", "D9", "CQ", "CJ", "D5", "H5",
];

/// Seat indices as the solver numbers them.
const WEST: usize = 0;
const NORTH: usize = 1;
const NOTRUMP: usize = 4;

/// A tiny deterministic generator, so the fixture set is reproducible without
/// taking a dependency for it. SplitMix64 — good enough to shuffle a deck, and
/// short enough to read.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Deal 52 cards into a PBN deal string anchored on North.
fn random_deal(rng: &mut Rng) -> String {
    const RANKS: [char; 13] = [
        'A', 'K', 'Q', 'J', 'T', '9', '8', '7', '6', '5', '4', '3', '2',
    ];

    // Cards as (suit, rank) with suit 0=spades..3=clubs, matching PBN's order.
    let mut deck: Vec<(usize, usize)> = (0..4).flat_map(|s| (0..13).map(move |r| (s, r))).collect();
    for i in (1..deck.len()).rev() {
        deck.swap(i, rng.below(i + 1));
    }

    let mut hands: Vec<[Vec<char>; 4]> = (0..4).map(|_| Default::default()).collect();
    for (i, &(suit, rank)) in deck.iter().enumerate() {
        hands[i % 4][suit].push(RANKS[rank]);
    }

    let mut out = String::from("N:");
    for (h, hand) in hands.iter_mut().enumerate() {
        if h > 0 {
            out.push(' ');
        }
        for (s, suit) in hand.iter_mut().enumerate() {
            if s > 0 {
                out.push('.');
            }
            // PBN writes each holding high to low; the deck is already in that
            // order per suit, so sorting by index restores it after the shuffle.
            suit.sort_by_key(|c| RANKS.iter().position(|r| r == c).unwrap_or(0));
            out.extend(suit.iter());
        }
    }
    out
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Generate deals, rank them by measured solve cost, and print the distribution
/// plus a fixture set spanning it.
fn survey(count: usize) {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    let mut rows: Vec<(f64, String, String)> = Vec::with_capacity(count);

    eprintln!("solving {count} deals…");
    for i in 0..count {
        let dealstr = random_deal(&mut rng);
        let Some(deal) = Deal::from_pbn(&dealstr) else {
            eprintln!("  generated an unparseable deal, skipping: {dealstr}");
            continue;
        };
        let started = Instant::now();
        let table = solve_dd_table(&deal);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        rows.push((ms, dealstr, dd_table_to_pbn(&table)));
        if (i + 1) % 25 == 0 {
            eprintln!("  {}/{count}", i + 1);
        }
    }

    let mut times: Vec<f64> = rows.iter().map(|r| r.0).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    println!("\n=== DD table solve cost, {} deals ===", rows.len());
    for p in [0.0, 50.0, 90.0, 95.0, 99.0, 100.0] {
        println!("  p{p:<5.0} {:>9.1} ms", percentile(&times, p));
    }
    let mean = times.iter().sum::<f64>() / times.len().max(1) as f64;
    println!("  mean   {mean:>9.1} ms");
    println!(
        "  spread {:>9.1}x  (p99 / p50)",
        percentile(&times, 99.0) / percentile(&times, 50.0).max(0.001)
    );

    // Ten deals spanning the distribution, cheapest to dearest.
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    println!("\n=== fixture set (paste into web/src/lib/fixtures/bench-v1.json) ===");
    println!("{{");
    println!("  \"version\": \"bench-v1\",");
    println!("  \"generator\": \"examples/bench_fixtures.rs, seed 0x5EED1234ABCD0001\",");
    println!("  \"note\": \"ddtricks is seat-major N,S,E,W over strains NT,S,H,D,C. referenceMs is a native release build on the reference machine, NOT the browser: the wasm build runs about 1.4x slower, so treat these as a ranking rather than a target.\",");
    println!("  \"deals\": [");
    let picks = [0.0, 11.0, 22.0, 33.0, 44.0, 56.0, 67.0, 78.0, 89.0, 100.0];
    for (n, p) in picks.iter().enumerate() {
        let idx = ((p / 100.0) * (rows.len() - 1) as f64).round() as usize;
        let (ms, dealstr, ddtricks) = &rows[idx.min(rows.len() - 1)];
        let comma = if n + 1 == picks.len() { "" } else { "," };
        println!(
            "    {{ \"id\": \"p{p:.0}\", \"referenceMs\": {ms:.1}, \"deal\": \"{dealstr}\", \"ddtricks\": \"{ddtricks}\" }}{comma}"
        );
    }
    println!("  ]");
    println!("}}");
}

/// Break the page's actual workload into the units a worker pool could split.
fn units() {
    let deal = Deal::from_pbn(VERIFIED_DEAL).expect("the verified deal parses");
    let hands = Hands::from_pbn(VERIFIED_DEAL).expect("the verified deal parses");

    println!("=== the page's workload, on the verified board ===\n");

    // 1. The double-dummy table.
    let started = Instant::now();
    let table = solve_dd_table(&deal);
    let table_ms = started.elapsed().as_secs_f64() * 1000.0;
    let ddtricks = dd_table_to_pbn(&table);
    println!("dd_table              {table_ms:>8.1} ms   ddtricks {ddtricks}");
    assert_eq!(
        ddtricks, "45544465449789987899",
        "the verified board's table changed — that is a correctness regression, not a timing one"
    );

    // 2. The running trace, cold and then warm.
    let plays: Vec<usize> = VERIFIED_PLAYS
        .iter()
        .map(|c| bridge_solver::analyse_play::parse_card(c).expect("a fixture card parses"))
        .collect();
    let input = PlayInput {
        hands,
        trump: NOTRUMP,
        declarer: WEST,
        leader: NORTH,
        plays: plays.clone(),
    };
    let keys = prefix_keys(VERIFIED_DEAL, NOTRUMP, NORTH, &plays);

    let cold = HashMap::new();
    let started = Instant::now();
    let trace = running_trace(&input, &keys, &cold).expect("the verified trace replays");
    let trace_cold_ms = started.elapsed().as_secs_f64() * 1000.0;

    let mut warm: HashMap<String, u8> = HashMap::new();
    for (key, value) in &trace.new_entries {
        warm.insert(key.clone(), *value);
    }
    let started = Instant::now();
    let _ = running_trace(&input, &keys, &warm).expect("the verified trace replays");
    let trace_warm_ms = started.elapsed().as_secs_f64() * 1000.0;

    let errors: Vec<usize> = trace
        .trace
        .iter()
        .filter(|e| e.cost > 0)
        .map(|e| e.index)
        .collect();
    println!("running_trace  cold   {trace_cold_ms:>8.1} ms   {} cards, {} costed errors, {} positions cached",
        trace.trace.len(), errors.len(), trace.new_entries.len());
    println!(
        "running_trace  warm   {trace_warm_ms:>8.1} ms   ({:.0}x cheaper off the cache)",
        trace_cold_ms / trace_warm_ms.max(0.001)
    );

    // 3. The verdict pass: one node analysis per costed error.
    let mut node_total = 0.0;
    let mut node_each = Vec::new();
    for &node in &errors {
        let started = Instant::now();
        let analysis = node_alternatives(&input, node).expect("a costed node analyses");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        node_total += ms;
        node_each.push((node, analysis.alternatives.len(), ms));
    }
    println!("\nverdicts (per costed error, no cache is shared between them):");
    for (node, alts, ms) in &node_each {
        println!("  node {node:<3} {alts:>2} alternatives   {ms:>8.1} ms");
    }
    println!("  total               {node_total:>8.1} ms");

    let total = table_ms + trace_cold_ms + node_total;
    println!("\n=== what a worker pool could take off the wall clock ===");
    println!("  total sequential    {total:>8.1} ms");
    println!(
        "  dd_table            {table_ms:>8.1} ms  ({:>4.1}%)  splits 5 ways, caches are per-strain already",
        100.0 * table_ms / total
    );
    println!(
        "  running_trace       {trace_cold_ms:>8.1} ms  ({:>4.1}%)  sequential: each position builds on the last",
        100.0 * trace_cold_ms / total
    );
    println!(
        "  verdicts            {node_total:>8.1} ms  ({:>4.1}%)  splits {} ways, shares no cache at all",
        100.0 * node_total / total,
        errors.len()
    );

    // The honest ceiling, not work/N. Two things bound it:
    //
    //   * the verdict pass cannot start until the trace has said which cards
    //     were errors, so the two are in series, not in parallel;
    //   * verdict nodes are wildly uneven — search depth falls as the hand is
    //     played, so the opening lead costs two orders of magnitude more than a
    //     node at trick seven. With enough workers the pass still takes as long
    //     as its single dearest node.
    let dearest_node = node_each.iter().map(|(_, _, ms)| *ms).fold(0.0, f64::max);
    let ideal = table_ms.max(trace_cold_ms) + dearest_node;
    println!(
        "\n  dearest single verdict node {dearest_node:>8.1} ms  ({:.0}% of the whole verdict pass)",
        100.0 * dearest_node / node_total.max(0.001)
    );
    println!(
        "  ceiling with unlimited workers {:>6.1} ms, or {:.1}x",
        ideal,
        total / ideal.max(0.001)
    );
    println!(
        "\n  Splitting whole nodes across workers therefore buys about {:.1}x and no more,",
        total / ideal.max(0.001)
    );
    println!("  however many workers there are. Going past it means splitting the dearest");
    println!("  node itself — its alternatives are independent full-depth solves — rather");
    println!("  than handing one node to one worker.");
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "units".into());
    match mode.as_str() {
        "survey" => {
            let count = std::env::args()
                .nth(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(200);
            survey(count)
        }
        "units" => units(),
        other => {
            eprintln!("unknown mode {other:?}; expected \"survey\" or \"units\"");
            std::process::exit(2);
        }
    }
}
