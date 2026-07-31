//! Benchmark and reconcile the frozen board set.
//!
//! Lives in this crate rather than the engine's because the LIN parser and the
//! contract resolution it needs are here. It deliberately reuses
//! `lin_input::parse_file` rather than re-deriving declarer: getting that wrong
//! inverts the opening leader and invalidates every trick count downstream, and
//! there should be exactly one implementation of it.
//!
//! Two jobs, and the second matters more than the first:
//!
//! * **Timings** for each of the three work units — the double-dummy table, the
//!   running trace, and the verdict pass — so a regression shows up per stage
//!   rather than as one number that has moved for an unknown reason.
//! * **Reconciliation** against the per-player error counts recorded by
//!   EDGAR-Defense-Toolkit, a second and independent implementation. Two
//!   analysers that disagree about a player's mistakes is a bad thing to
//!   discover from a student.
//!
//! ```text
//! cargo run --release --example bench_boards
//! ```

use std::collections::HashMap;
use std::time::Instant;

use bridge_solver::analyse_play::{
    node_alternatives, parse_card, parse_seat, parse_trump, prefix_keys, running_trace, PlayInput,
};
use bridge_solver::{solve_dd_table, Hands};
use bridge_solver_wasm::lin_input;
use bridge_types::Deal;

/// Seat letters in the solver's index order (WEST=0, NORTH=1, EAST=2, SOUTH=3).
const SEATS: [char; 4] = ['W', 'N', 'E', 'S'];

struct Measured {
    board: usize,
    cards: usize,
    contract: String,
    table_ms: f64,
    trace_ms: f64,
    verdict_ms: f64,
    ddtricks: String,
    /// Errors per seat letter, before dummy is folded into declarer.
    by_seat: HashMap<String, u32>,
    /// The same, with dummy's errors credited to the declarer who chose them.
    by_player: HashMap<String, u32>,
}

/// The 20-cell table as BSOL's `ddtricks`: seat-major N,S,E,W over NT,S,H,D,C.
fn dd_tricks_string(tricks: &[[u8; 5]; 4]) -> String {
    let seat_row = [0usize, 2, 1, 3];
    let strain_col = [4usize, 3, 2, 1, 0];
    let mut out = String::new();
    for &row in &seat_row {
        for &col in &strain_col {
            let n = tricks[row][col];
            out.push(if n < 10 {
                (b'0' + n) as char
            } else {
                (b'a' + n - 10) as char
            });
        }
    }
    out
}

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/bench-boards.lin");
    let content = std::fs::read_to_string(path).expect("the fixture set is readable");

    let parsed = lin_input::parse_file(&content).expect("the fixture set parses");
    let mut rows: Vec<Measured> = Vec::new();

    for (i, board) in parsed.iter().enumerate() {
        let board_no = i + 1;
        let lin = match board {
            Ok(l) => l,
            Err(e) => {
                eprintln!("board {board_no}: {e}");
                continue;
            }
        };
        let req = &lin.request;

        // Field mapping only — the contract resolution already happened in the
        // parser, which is the part worth having exactly one copy of.
        let hands = Hands::from_pbn(&req.dealstr).expect("a parsed deal converts");
        let deal = Deal::from_pbn(&req.dealstr).expect("a parsed deal converts");
        let trump = parse_trump(&req.trump).expect("a parsed trump converts");
        let declarer = parse_seat(&req.declarer).expect("a parsed seat converts");
        let leader = parse_seat(&req.leader).expect("a parsed seat converts");
        let plays: Vec<usize> = req
            .plays
            .iter()
            .map(|c| parse_card(c).expect("a parsed card converts"))
            .collect();

        let input = PlayInput {
            hands,
            trump,
            declarer,
            leader,
            plays: plays.clone(),
        };

        let started = Instant::now();
        let table = solve_dd_table(&deal);
        let table_ms = started.elapsed().as_secs_f64() * 1000.0;

        let keys = prefix_keys(&req.dealstr, trump, leader, &plays);
        let cold = HashMap::new();
        let started = Instant::now();
        let trace = running_trace(&input, &keys, &cold).expect("the trace replays");
        let trace_ms = started.elapsed().as_secs_f64() * 1000.0;

        // The verdict pass runs only on costed cards, so a cleanly played board
        // does no work here at all — which is itself worth measuring.
        let errors: Vec<usize> = trace
            .trace
            .iter()
            .filter(|e| e.cost > 0)
            .map(|e| e.index)
            .collect();
        let started = Instant::now();
        for &node in &errors {
            node_alternatives(&input, node).expect("a costed node analyses");
        }
        let verdict_ms = started.elapsed().as_secs_f64() * 1000.0;

        // Dummy is declarer's partner; the seat opposite.
        let dummy = (declarer + 2) % 4;
        let mut by_seat: HashMap<String, u32> = HashMap::new();
        let mut by_player: HashMap<String, u32> = HashMap::new();
        for e in trace.trace.iter().filter(|e| e.cost > 0) {
            let seat_char = e.seat.chars().next().expect("the trace names a seat");
            let seat_idx = SEATS
                .iter()
                .position(|&c| c == seat_char)
                .expect("the trace names a known seat");
            *by_seat.entry(e.seat.clone()).or_default() += 1;

            // Dummy's cards were chosen by declarer, so they are charged there.
            // That is BBO's own BSOL convention and what the web app shows.
            let credited = if seat_idx == dummy { declarer } else { seat_idx };
            let name = match SEATS[credited] {
                'N' => &lin.player_names.north,
                'E' => &lin.player_names.east,
                'S' => &lin.player_names.south,
                _ => &lin.player_names.west,
            };
            *by_player.entry(name.clone()).or_default() += 1;
        }

        rows.push(Measured {
            board: board_no,
            cards: plays.len(),
            contract: lin.contract.description.clone(),
            table_ms,
            trace_ms,
            verdict_ms,
            ddtricks: dd_tricks_string(&table.tricks),
            by_seat,
            by_player,
        });
    }

    println!("=== per-board cost, native release ===\n");
    println!(
        "{:>5} {:>6} {:>9} {:>7} {:>7} {:>9} {:>7}  {}",
        "board", "cards", "contract", "table", "trace", "verdicts", "total", "errors by player"
    );
    for r in &rows {
        let total = r.table_ms + r.trace_ms + r.verdict_ms;
        let mut who: Vec<String> = r
            .by_player
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect();
        who.sort();
        println!(
            "{:>5} {:>6} {:>9} {:>6.1}ms {:>6.1}ms {:>7.1}ms {:>6.1}ms  {}",
            r.board,
            r.cards,
            r.contract,
            r.table_ms,
            r.trace_ms,
            r.verdict_ms,
            total,
            who.join(" ")
        );
    }

    let sum = |f: fn(&Measured) -> f64| rows.iter().map(f).sum::<f64>();
    let (t, tr, v) = (
        sum(|r| r.table_ms),
        sum(|r| r.trace_ms),
        sum(|r| r.verdict_ms),
    );
    // Raw seat counts alongside the credited ones, because the two conventions
    // are exactly where a comparison against another implementation goes wrong:
    // if EDGAR charges dummy's errors to dummy rather than to declarer, the
    // per-player totals differ without either analyser being incorrect.
    println!("\n=== errors by seat, before dummy is folded into declarer ===");
    for r in &rows {
        let mut who: Vec<String> = r.by_seat.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        who.sort();
        println!("  board {:>2}  {}", r.board, who.join(" "));
    }

    let all = t + tr + v;
    println!("\n=== totals over {} boards ===", rows.len());
    println!("  dd_table       {t:>8.1} ms  ({:>4.1}%)", 100.0 * t / all);
    println!("  running_trace  {tr:>8.1} ms  ({:>4.1}%)", 100.0 * tr / all);
    println!("  verdicts       {v:>8.1} ms  ({:>4.1}%)", 100.0 * v / all);
    println!("  total          {all:>8.1} ms");

    println!("\n=== fixture JSON (bench-v2) ===");
    println!("{{");
    println!("  \"version\": \"bench-v2\",");
    println!("  \"source\": \"fixtures/bench-boards.lin — 10 real boards, names anonymised\",");
    println!("  \"note\": \"ms figures are a native release build on the reference machine, NOT the browser; treat them as a ranking. ddtricks is seat-major N,S,E,W over strains NT,S,H,D,C.\",");
    println!("  \"boards\": [");
    for (i, r) in rows.iter().enumerate() {
        let comma = if i + 1 == rows.len() { "" } else { "," };
        let mut who: Vec<String> = r
            .by_player
            .iter()
            .map(|(k, v)| format!("\"{k}\": {v}"))
            .collect();
        who.sort();
        println!(
            "    {{ \"board\": {}, \"cards\": {}, \"contract\": \"{}\", \"ddtricks\": \"{}\", \"tableMs\": {:.1}, \"traceMs\": {:.1}, \"verdictMs\": {:.1}, \"errorsByPlayer\": {{{}}} }}{comma}",
            r.board,
            r.cards,
            r.contract,
            r.ddtricks,
            r.table_ms,
            r.trace_ms,
            r.verdict_ms,
            who.join(", ")
        );
    }
    println!("  ]");
    println!("}}");
}
